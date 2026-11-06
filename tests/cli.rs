//! End-to-end checks of the `prj` binary against real git repositories.

use std::path::Path;
use std::process::Command as StdCommand;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// A git repository with one commit on `main` and the given origin URL.
fn repo_with_origin(url: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.com",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ],
        vec!["remote", "add", "origin", url],
    ] {
        let status = StdCommand::new("git")
            .args(&args)
            .current_dir(dir.path())
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }
    dir
}

fn prj(cwd: &Path) -> Command {
    let mut cmd = Command::cargo_bin("prj").unwrap();
    cmd.current_dir(cwd)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", "false")
        .env_remove("SSH_CONNECTION")
        .env_remove("SSH_CLIENT")
        .env_remove("SSH_TTY");
    cmd
}

#[test]
fn prints_github_compare_url() {
    let repo = repo_with_origin("git@github.com:o/r.git");
    prj(repo.path())
        .args(["open", "--print", "-t", "main", "-s", "feature/x"])
        .assert()
        .success()
        .stdout("https://github.com/o/r/compare/main...feature%2Fx\n")
        .stderr("");
}

#[test]
fn credentials_in_remote_url_never_reach_output() {
    for url in [
        "https://user:s3cr3t@github.com/o/r.git",
        "https://user:s3cr3t@unsupported.example/o/r.git",
        "https://user:s3cr3t@github.com/single-segment",
        "https://user:s3cr3t@[::1/o/r",
    ] {
        let repo = repo_with_origin(url);
        prj(repo.path())
            .args(["open", "--print", "-t", "main", "-s", "x"])
            .assert()
            .stdout(predicate::str::contains("s3cr3t").not())
            .stderr(predicate::str::contains("s3cr3t").not());
    }
}

#[cfg(unix)]
#[test]
fn closed_stdout_pipe_is_not_an_error() {
    let repo = repo_with_origin("git@github.com:o/r.git");
    let prj = assert_cmd::cargo::cargo_bin("prj");
    // `head -c0` exits without reading, so prj writes into a closed pipe.
    let script = format!(
        "'{}' open --print -t main -s x | head -c0; echo \"status=${{PIPESTATUS[0]}}\"",
        prj.display()
    );
    let out = StdCommand::new("bash")
        .args(["-c", &script])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stdout.contains("status=0"),
        "stdout={stdout} stderr={stderr}"
    );
    assert!(!stderr.contains("panicked"), "stderr={stderr}");
}

#[test]
fn missing_git_is_reported_clearly() {
    let empty = tempfile::tempdir().unwrap();
    prj(empty.path())
        .env("PATH", empty.path())
        .args(["open", "--print", "-t", "main", "-s", "x"])
        .assert()
        .code(1)
        .stderr(predicate::str::starts_with(
            "Error: git executable not found",
        ));
}

#[test]
fn no_subcommand_prints_help_to_stdout_and_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    prj(dir.path())
        .assert()
        .code(1)
        .stdout(predicate::str::contains("Usage: prj"))
        .stderr("");
}

#[test]
fn version_flag_prints_crate_version() {
    let dir = tempfile::tempdir().unwrap();
    prj(dir.path())
        .arg("--version")
        .assert()
        .success()
        .stdout(format!("prj {}\n", env!("CARGO_PKG_VERSION")));
}

fn git_in(dir: &Path, args: &[&str]) {
    let status = StdCommand::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

#[test]
fn current_branch_ignores_a_tag_with_the_same_name() {
    let repo = repo_with_origin("git@github.com:o/r.git");
    git_in(repo.path(), &["switch", "-q", "-c", "foo"]);
    git_in(repo.path(), &["tag", "foo"]);
    prj(repo.path())
        .args(["open", "--print", "-t", "main"])
        .assert()
        .success()
        .stdout("https://github.com/o/r/compare/main...foo\n");
}

#[test]
fn current_branch_works_before_the_first_commit() {
    let dir = tempfile::tempdir().unwrap();
    git_in(dir.path(), &["init", "-q", "-b", "fresh"]);
    git_in(
        dir.path(),
        &["remote", "add", "origin", "git@github.com:o/r.git"],
    );
    prj(dir.path())
        .args(["open", "--print", "-t", "main"])
        .assert()
        .success()
        .stdout("https://github.com/o/r/compare/main...fresh\n");
}

#[test]
fn detached_head_still_asks_for_source() {
    let repo = repo_with_origin("git@github.com:o/r.git");
    git_in(repo.path(), &["switch", "-q", "--detach"]);
    prj(repo.path())
        .args(["open", "--print", "-t", "main"])
        .assert()
        .code(1)
        .stderr("Error: Detached HEAD. Use --source to specify a branch.\n");
}

/// A remote whose ssh never answers must not hang prj: the remote lookup
/// gives up after PRJ_GIT_TIMEOUT seconds and the local fallbacks decide.
#[cfg(unix)]
#[test]
fn hanging_remote_lookup_times_out() {
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    let repo = repo_with_origin("git@github.com:o/r.git");
    git_in(
        repo.path(),
        &["update-ref", "refs/remotes/origin/develop", "HEAD"],
    );
    let ssh = repo.path().join("hanging-ssh");
    std::fs::write(&ssh, "#!/bin/sh\nexec sleep 30\n").unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();

    let started = Instant::now();
    prj(repo.path())
        .env("GIT_SSH_COMMAND", &ssh)
        .env("GIT_SSH_VARIANT", "simple")
        .env("PRJ_GIT_TIMEOUT", "1")
        .args(["open", "--print", "-s", "x"])
        .timeout(Duration::from_secs(10))
        .assert()
        .success()
        .stdout("https://github.com/o/r/compare/develop...x\n");
    assert!(
        started.elapsed() < Duration::from_secs(8),
        "{:?}",
        started.elapsed()
    );
}
