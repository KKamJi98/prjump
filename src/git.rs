//! Git queries. Git runs as a subprocess so that the user's git config
//! (insteadOf, includeIf, worktrees, safe.directory) applies exactly as it
//! does for their own git commands.

use std::io::{self, Read};
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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

    /// Runs a git command that talks to the remote. `Ok(None)` means it did
    /// not finish within the timeout and was stopped.
    fn run_remote(&self, args: &[&str]) -> Result<Option<GitOutput>, Error> {
        self.run(args).map(Some)
    }
}

/// Seconds a remote query may take before prj gives up on it.
/// `PRJ_GIT_TIMEOUT` overrides the default.
const DEFAULT_REMOTE_TIMEOUT_SECS: u64 = 10;

fn remote_timeout() -> Duration {
    let secs = std::env::var("PRJ_GIT_TIMEOUT")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|&s| s > 0)
        .unwrap_or(DEFAULT_REMOTE_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

fn spawn_error(e: io::Error) -> Error {
    if e.kind() == io::ErrorKind::NotFound {
        Error::user("git executable not found. Install git and make sure it is on PATH.")
    } else {
        Error::unexpected(format!("failed to run git: {e}"))
    }
}

fn decode(args: &[&str], output: Output) -> Result<GitOutput, Error> {
    let text = |bytes: Vec<u8>| {
        String::from_utf8(bytes).map_err(|_| {
            Error::user(format!(
                "git {} printed output that is not valid UTF-8.",
                args.first().unwrap_or(&"")
            ))
        })
    };
    Ok(GitOutput {
        success: output.status.success(),
        stdout: text(output.stdout)?,
        stderr: text(output.stderr)?,
    })
}

/// Stops a timed-out remote query together with the ssh or credential
/// helper it started (they share the child's process group).
fn stop(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &format!("-{}", child.id())])
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// Runs the `git` found on PATH in the current directory.
pub struct SystemGit;

impl Git for SystemGit {
    fn run(&self, args: &[&str]) -> Result<GitOutput, Error> {
        let output = Command::new("git")
            .args(args)
            .output()
            .map_err(spawn_error)?;
        decode(args, output)
    }

    fn run_remote(&self, args: &[&str]) -> Result<Option<GitOutput>, Error> {
        let mut cmd = Command::new("git");
        cmd.args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Own process group: an ssh that wants a passphrase cannot take over
        // the terminal (it is stopped instead), and the whole group can be
        // stopped on timeout.
        #[cfg(unix)]
        std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
        let mut child = cmd.spawn().map_err(spawn_error)?;

        let mut stdout = child.stdout.take().expect("piped");
        let mut stderr = child.stderr.take().expect("piped");
        let out_reader = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stdout.read_to_end(&mut buf);
            buf
        });
        let err_reader = thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = stderr.read_to_end(&mut buf);
            buf
        });

        let deadline = Instant::now() + remote_timeout();
        let status = loop {
            if let Some(status) = child.try_wait().map_err(spawn_error)? {
                break status;
            }
            if Instant::now() >= deadline {
                stop(&mut child);
                // The readers may still be blocked on a pipe a grandchild
                // holds open; leave them behind rather than wait.
                return Ok(None);
            }
            thread::sleep(Duration::from_millis(20));
        };
        let output = Output {
            status,
            stdout: out_reader.join().unwrap_or_default(),
            stderr: err_reader.join().unwrap_or_default(),
        };
        decode(args, output).map(Some)
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

/// `git branch --show-current` (git 2.22+) prints the full branch name even
/// when a tag shares it (`rev-parse --abbrev-ref` printed `heads/foo`) and
/// before the first commit (where `rev-parse` failed). It prints nothing on
/// a detached HEAD.
pub fn get_current_branch(git: &dyn Git) -> Result<String, Error> {
    let result = git.run(&["branch", "--show-current"])?;
    if !result.success {
        return Err(Error::user("Failed to detect current branch."));
    }
    let branch = pystr::strip(&result.stdout);
    if branch.is_empty() {
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

/// Finds `ref: refs/heads/<branch>\tHEAD` in `git ls-remote --symref` output.
fn parse_symref_head(stdout: &str) -> Option<String> {
    stdout.lines().find_map(|line| {
        let target = line.strip_prefix("ref: ")?.strip_suffix("\tHEAD")?;
        let branch = target.strip_prefix("refs/heads/")?;
        (!branch.is_empty()).then(|| branch.to_string())
    })
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

    // Ask the remote which branch its HEAD points to. `git remote show`
    // did this before, but its output is translated by the locale and it
    // had no time limit.
    if let Some(result) = git.run_remote(&["ls-remote", "--symref", remote, "HEAD"])? {
        if result.success {
            if let Some(branch) = parse_symref_head(&result.stdout) {
                return Ok(branch);
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
    fn default_branch_asks_the_remote_with_ls_remote() {
        let git = MockGit(|args| match args {
            ["symbolic-ref", ..] => fail(),
            ["ls-remote", "--symref", "origin", "HEAD"] => {
                ok("ref: refs/heads/release/2026\tHEAD\n0123abcd\tHEAD\n")
            }
            _ => fail(),
        });
        assert_eq!(get_default_branch(&git, "origin").unwrap(), "release/2026");
    }

    #[test]
    fn default_branch_never_parses_remote_show() {
        let git = MockGit(|args| match args {
            ["remote", "show", ..] => ok("  HEAD branch: develop\n"),
            ["branch", "-r", "--list", "origin/trunk"] => ok("  origin/trunk\n"),
            ["branch", ..] => ok(""),
            _ => fail(),
        });
        assert_eq!(get_default_branch(&git, "origin").unwrap(), "trunk");
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
