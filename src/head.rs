//! Working out the pull request's head when `--source` is not given: which
//! remote the current branch is pushed to, under which name, and whether the
//! remote has its latest commits.

use crate::error::Error;
use crate::git::Git;
use crate::pystr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// Branch name on the remote.
    pub branch: String,
    /// Remote `git push` sends the branch to, when one is configured.
    pub remote: Option<String>,
    /// Problems to show on stderr; they do not stop prj.
    pub warnings: Vec<String>,
}

/// `local` is the checked-out branch, `target_remote` the remote the pull
/// request goes to (`--remote`).
pub fn resolve_head(git: &dyn Git, local: &str, target_remote: &str) -> Result<Head, Error> {
    let refname = format!("refs/heads/{local}");
    let info = git.run(&[
        "for-each-ref",
        "--format=%(upstream:remotename)%00%(upstream:remoteref)%00%(push:remotename)",
        &refname,
    ])?;
    let line = if info.success {
        info.stdout.lines().next().unwrap_or("")
    } else {
        ""
    };
    let mut fields = line.split('\0').map(pystr::strip);
    let non_empty = |s: Option<&str>| s.filter(|v| !v.is_empty()).map(str::to_string);
    // Only configured remotes count: a branch that tracks a local branch
    // reports "." and branch.<name>.remote may hold a URL.
    let remotes = git.run(&["remote"])?;
    let is_remote = |name: &String| remotes.stdout.lines().any(|r| pystr::strip(r) == name);
    let upstream_remote = non_empty(fields.next()).filter(is_remote);
    let upstream_ref = non_empty(fields.next());
    let push_remote = non_empty(fields.next()).filter(is_remote);

    // %(push:remotename) already follows branch.<name>.pushRemote and
    // remote.pushDefault; it is empty when nothing is configured at all.
    let Some(remote) = push_remote.or_else(|| upstream_remote.clone()) else {
        return Ok(Head {
            branch: local.to_string(),
            remote: None,
            warnings: vec![format!(
                "warning: branch '{local}' has no upstream; push it first: git push -u {target_remote} {local}"
            )],
        });
    };

    // Only push.default=upstream pushes under the upstream branch's name;
    // every other mode pushes under the local name.
    let push_default = git.run(&["config", "--get", "push.default"])?;
    let pushes_to_upstream = matches!(pystr::strip(&push_default.stdout), "upstream" | "tracking");
    let branch = match (&upstream_remote, &upstream_ref) {
        (Some(up), Some(up_ref)) if pushes_to_upstream && *up == remote => up_ref
            .strip_prefix("refs/heads/")
            .unwrap_or(local)
            .to_string(),
        _ => local.to_string(),
    };

    let mut warnings = Vec::new();
    let tracking = format!("refs/remotes/{remote}/{branch}");
    let exists = git.run(&["rev-parse", "--verify", "--quiet", &tracking])?;
    if !exists.success {
        warnings.push(format!(
            "warning: branch '{local}' is not on '{remote}' yet; push it first: git push -u {remote} {local}"
        ));
    } else {
        let range = format!("{tracking}..{refname}");
        let ahead = git.run(&["rev-list", "--count", &range])?;
        let count: u64 = pystr::strip(&ahead.stdout).parse().unwrap_or(0);
        if ahead.success && count > 0 {
            let commits = if count == 1 { "commit" } else { "commits" };
            warnings.push(format!(
                "warning: {count} local {commits} on '{local}' not pushed to '{remote}' yet"
            ));
        }
    }

    Ok(Head {
        branch,
        remote: Some(remote),
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::GitOutput;
    use crate::git::mock::{MockGit, fail, ok};

    fn refs(up_remote: &str, up_ref: &str, push_remote: &str) -> GitOutput {
        ok(&format!("{up_remote}\0{up_ref}\0{push_remote}\n"))
    }

    #[test]
    fn no_upstream_warns_and_keeps_local_name() {
        let git = MockGit(|args| match args {
            ["for-each-ref", ..] => ok("\0\0\n"),
            _ => fail(),
        });
        let head = resolve_head(&git, "feat", "origin").unwrap();
        assert_eq!(head.branch, "feat");
        assert_eq!(head.remote, None);
        assert_eq!(
            head.warnings,
            ["warning: branch 'feat' has no upstream; push it first: git push -u origin feat"]
        );
    }

    #[test]
    fn pushed_and_up_to_date_has_no_warning() {
        let git = MockGit(|args| match args {
            ["for-each-ref", ..] => refs("origin", "refs/heads/feat", "origin"),
            ["remote"] => ok("origin\n"),
            ["config", ..] => fail(),
            [
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/remotes/origin/feat",
            ] => ok("abc\n"),
            [
                "rev-list",
                "--count",
                "refs/remotes/origin/feat..refs/heads/feat",
            ] => ok("0\n"),
            _ => fail(),
        });
        let head = resolve_head(&git, "feat", "origin").unwrap();
        assert_eq!(head.remote.as_deref(), Some("origin"));
        assert!(head.warnings.is_empty(), "{:?}", head.warnings);
    }

    #[test]
    fn unpushed_branch_warns() {
        let git = MockGit(|args| match args {
            ["for-each-ref", ..] => refs("upstream", "refs/heads/main", "origin"),
            ["remote"] => ok("origin\nupstream\n"),
            _ => fail(),
        });
        let head = resolve_head(&git, "feat", "upstream").unwrap();
        assert_eq!(head.remote.as_deref(), Some("origin"));
        assert_eq!(
            head.warnings,
            [
                "warning: branch 'feat' is not on 'origin' yet; push it first: git push -u origin feat"
            ]
        );
    }

    #[test]
    fn local_commits_ahead_warn() {
        let git = MockGit(|args| match args {
            ["for-each-ref", ..] => refs("origin", "refs/heads/feat", ""),
            ["remote"] => ok("origin\n"),
            ["rev-parse", ..] => ok("abc\n"),
            ["rev-list", ..] => ok("2\n"),
            _ => fail(),
        });
        let head = resolve_head(&git, "feat", "origin").unwrap();
        assert_eq!(
            head.warnings,
            ["warning: 2 local commits on 'feat' not pushed to 'origin' yet"]
        );
    }

    #[test]
    fn push_default_upstream_uses_the_upstream_branch_name() {
        let git = MockGit(|args| match args {
            ["for-each-ref", ..] => refs("origin", "refs/heads/remote-name", "origin"),
            ["remote"] => ok("origin\n"),
            ["config", "--get", "push.default"] => ok("upstream\n"),
            [
                "rev-parse",
                "--verify",
                "--quiet",
                "refs/remotes/origin/remote-name",
            ] => ok("abc\n"),
            ["rev-list", ..] => ok("0\n"),
            _ => fail(),
        });
        let head = resolve_head(&git, "local-name", "origin").unwrap();
        assert_eq!(head.branch, "remote-name");
        assert!(head.warnings.is_empty(), "{:?}", head.warnings);
    }

    #[test]
    fn other_push_modes_keep_the_local_name() {
        let git = MockGit(|args| match args {
            ["for-each-ref", ..] => refs("origin", "refs/heads/main", "origin"),
            ["remote"] => ok("origin\n"),
            ["config", "--get", "push.default"] => ok("simple\n"),
            ["rev-parse", ..] => ok("abc\n"),
            ["rev-list", ..] => ok("0\n"),
            _ => fail(),
        });
        assert_eq!(resolve_head(&git, "feat", "origin").unwrap().branch, "feat");
    }

    #[test]
    fn local_tracking_branch_is_treated_as_unpushed() {
        let git = MockGit(|args| match args {
            ["for-each-ref", ..] => refs(".", "refs/heads/main", "."),
            ["remote"] => ok("origin\n"),
            ["config", "--get", "push.default"] => ok("upstream\n"),
            _ => fail(),
        });
        let head = resolve_head(&git, "feat", "origin").unwrap();
        assert_eq!(head.branch, "feat");
        assert_eq!(head.remote, None);
        assert_eq!(
            head.warnings,
            ["warning: branch 'feat' has no upstream; push it first: git push -u origin feat"]
        );
    }
}
