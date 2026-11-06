//! Git queries. Git runs as a subprocess so that the user's git config
//! (insteadOf, includeIf, worktrees, safe.directory) applies exactly as it
//! does for their own git commands.

use std::io;
use std::process::Command;

use crate::error::Error;
use crate::pystr;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub trait Git {
    fn run(&self, args: &[&str]) -> Result<GitOutput, Error>;
}

/// Runs the `git` found on PATH in the current directory.
pub struct SystemGit;

impl Git for SystemGit {
    fn run(&self, args: &[&str]) -> Result<GitOutput, Error> {
        let output = Command::new("git").args(args).output().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                Error::user("git executable not found. Install git and make sure it is on PATH.")
            } else {
                Error::unexpected(format!("failed to run git: {e}"))
            }
        })?;
        let decode = |bytes: Vec<u8>| {
            String::from_utf8(bytes).map_err(|_| {
                Error::user(format!(
                    "git {} printed output that is not valid UTF-8.",
                    args.first().unwrap_or(&"")
                ))
            })
        };
        Ok(GitOutput {
            success: output.status.success(),
            stdout: decode(output.stdout)?,
            stderr: decode(output.stderr)?,
        })
    }
}

pub fn get_remote_url(git: &dyn Git, remote: &str) -> Result<String, Error> {
    let result = git.run(&["remote", "get-url", remote])?;
    if !result.success {
        let stderr = pystr::strip(&result.stderr);
        let detail = if stderr.is_empty() {
            String::new()
        } else {
            format!(": {stderr}")
        };
        return Err(Error::user(format!(
            "Failed to read remote URL for '{remote}'{detail}."
        )));
    }
    let url = pystr::strip(&result.stdout);
    if url.is_empty() {
        return Err(Error::user(format!(
            "Remote '{remote}' has no URL configured."
        )));
    }
    Ok(url.to_string())
}

pub fn get_current_branch(git: &dyn Git) -> Result<String, Error> {
    let result = git.run(&["rev-parse", "--abbrev-ref", "HEAD"])?;
    if !result.success {
        return Err(Error::user("Failed to detect current branch."));
    }
    let branch = pystr::strip(&result.stdout);
    if branch == "HEAD" {
        return Err(Error::user(
            "Detached HEAD. Use --source to specify a branch.",
        ));
    }
    Ok(branch.to_string())
}

fn remote_branch_exists(git: &dyn Git, remote: &str, branch: &str) -> Result<bool, Error> {
    let pattern = format!("{remote}/{branch}");
    let result = git.run(&["branch", "-r", "--list", &pattern])?;
    Ok(!pystr::strip(&result.stdout).is_empty())
}

pub fn get_default_branch(git: &dyn Git, remote: &str) -> Result<String, Error> {
    let head_ref = format!("refs/remotes/{remote}/HEAD");
    let result = git.run(&["symbolic-ref", &head_ref])?;
    if result.success {
        // Strip the whole `refs/remotes/<remote>/` prefix: the branch name
        // itself may contain slashes (release/2026).
        let full = pystr::strip(&result.stdout);
        let prefix = format!("refs/remotes/{remote}/");
        let branch = full
            .strip_prefix(&prefix)
            .unwrap_or_else(|| full.rsplit('/').next().unwrap_or(full));
        return Ok(branch.to_string());
    }

    let result = git.run(&["remote", "show", remote])?;
    if result.success {
        for line in result.stdout.lines() {
            if let Some(rest) = pystr::strip(line).strip_prefix("HEAD branch:") {
                return Ok(pystr::strip(rest).to_string());
            }
        }
    }

    for branch in ["main", "master", "develop", "trunk"] {
        if remote_branch_exists(git, remote, branch)? {
            return Ok(branch.to_string());
        }
    }

    Err(Error::user(
        "Unable to determine default branch. Use --target to specify one.",
    ))
}

#[cfg(test)]
pub mod mock {
    use super::*;

    /// A `Git` whose answers come from a closure, standing in for
    /// `patch("prjump.cli.run_git")` in the Python tests.
    pub struct MockGit<F: Fn(&[&str]) -> GitOutput>(pub F);

    impl<F: Fn(&[&str]) -> GitOutput> Git for MockGit<F> {
        fn run(&self, args: &[&str]) -> Result<GitOutput, Error> {
            Ok((self.0)(args))
        }
    }

    pub fn ok(stdout: &str) -> GitOutput {
        GitOutput {
            success: true,
            stdout: stdout.to_string(),
            stderr: String::new(),
        }
    }

    pub fn fail() -> GitOutput {
        GitOutput {
            success: false,
            stdout: String::new(),
            stderr: "error".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::{MockGit, fail, ok};
    use super::*;

    // Ported from tests/test_cli.py (prjump 0.2.2). The Python test that
    // expected "HEAD" from `symbolic-ref` printing refs/remotes/origin/HEAD
    // is dropped: git never prints that for a symbolic ref.

    #[test]
    fn default_branch_symbolic_ref_returns_main() {
        let git = MockGit(|_| ok("refs/remotes/origin/main\n"));
        assert_eq!(get_default_branch(&git, "origin").unwrap(), "main");
    }

    #[test]
    fn default_branch_keeps_slashes_in_branch_name() {
        let git = MockGit(|_| ok("refs/remotes/origin/release/2026\n"));
        assert_eq!(get_default_branch(&git, "origin").unwrap(), "release/2026");
    }

    #[test]
    fn default_branch_handles_remote_names_with_slashes() {
        let git = MockGit(|_| ok("refs/remotes/team/fork/main\n"));
        assert_eq!(get_default_branch(&git, "team/fork").unwrap(), "main");
    }

    #[test]
    fn default_branch_fallback_to_remote_show() {
        let git = MockGit(|args| match args {
            ["symbolic-ref", ..] => fail(),
            ["remote", "show", ..] => ok("* remote origin\n  HEAD branch: develop\n"),
            _ => fail(),
        });
        assert_eq!(get_default_branch(&git, "origin").unwrap(), "develop");
    }

    #[test]
    fn default_branch_fallback_to_known_branches() {
        let git = MockGit(|args| match args {
            ["branch", "-r", "--list", "origin/main"] => ok("  origin/main\n"),
            ["branch", ..] => ok(""),
            _ => fail(),
        });
        assert_eq!(get_default_branch(&git, "origin").unwrap(), "main");
    }

    #[test]
    fn default_branch_raises_when_all_fallbacks_fail() {
        let git = MockGit(|_| fail());
        let err = get_default_branch(&git, "origin").unwrap_err();
        assert!(err.message().contains("Unable to determine default branch"));
    }
}
