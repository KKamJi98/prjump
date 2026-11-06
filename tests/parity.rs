//! Replays the parity corpus against the Rust `prj` and compares the result
//! with what prjump 0.2.2 (Python) recorded in parity-expected.json.
//!
//! The harness mirrors scripts/parity/generate.py: same environment, same
//! setup commands, same normalisation. Intentional differences live in
//! parity-deviations.json, each with a reason.
//!
//! Unix only: the cases use shell scripts as the browser and as ssh.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;

const BROWSER_SH: &str = "#!/bin/sh\nprintf '%s\\n' \"$1\" >> \"$(dirname \"$0\")/browser.log\"\n";
const FAKE_SSH: &str = "#!/bin/sh\nexec git upload-pack \"$(dirname \"$0\")/upstream\"\n";

#[derive(Deserialize)]
struct Corpus {
    defaults: Defaults,
    cases: Vec<Value>,
}

#[derive(Deserialize)]
struct Defaults {
    args: Vec<String>,
    remote: String,
}

#[derive(Deserialize)]
struct Expected {
    results: Vec<Outcome>,
}

#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
struct Outcome {
    name: String,
    exit: i32,
    stdout: String,
    stderr: String,
    browser_log: String,
}

#[derive(Deserialize, Default)]
struct Deviations {
    /// Text replacements applied to every expected stdout and browser log.
    #[serde(default)]
    rules: Vec<Rule>,
    /// Per-case replacement of expected fields.
    #[serde(default)]
    cases: BTreeMap<String, Override>,
}

#[derive(Deserialize)]
struct Rule {
    /// Only applies to expected text that contains this marker.
    when_contains: String,
    /// Appended to every line that contains `when_contains`.
    append: String,
    #[allow(dead_code)]
    reason: String,
}

#[derive(Deserialize)]
struct Override {
    exit: Option<i32>,
    stdout: Option<String>,
    stderr: Option<String>,
    browser_log: Option<String>,
    #[serde(default)]
    skip_rules: bool,
    #[allow(dead_code)]
    reason: String,
}

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn write_exec(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn base_env(tmp: &Path) -> BTreeMap<String, String> {
    let home = tmp.join("home");
    let t = |p: &Path| p.to_string_lossy().into_owned();
    BTreeMap::from([
        ("PATH".into(), std::env::var("PATH").unwrap_or_default()),
        ("HOME".into(), t(&home)),
        ("GIT_CONFIG_GLOBAL".into(), t(&home.join(".gitconfig"))),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
        ("LC_ALL".into(), "C".into()),
        ("LANG".into(), "C".into()),
        ("GIT_AUTHOR_NAME".into(), "t".into()),
        ("GIT_AUTHOR_EMAIL".into(), "t@example.com".into()),
        ("GIT_COMMITTER_NAME".into(), "t".into()),
        ("GIT_COMMITTER_EMAIL".into(), "t@example.com".into()),
        ("GIT_AUTHOR_DATE".into(), "2026-01-01T00:00:00Z".into()),
        ("GIT_COMMITTER_DATE".into(), "2026-01-01T00:00:00Z".into()),
        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
        ("GIT_SSH_COMMAND".into(), "false".into()),
        ("GIT_SSH_VARIANT".into(), "simple".into()),
        ("BROWSER".into(), t(&tmp.join("browser.sh"))),
    ])
}

fn git(args: &[String], cwd: &Path, env: &BTreeMap<String, String>) {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .envs(env)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn str_list(v: &Value) -> Vec<String> {
    v.as_array()
        .expect("array")
        .iter()
        .map(|s| s.as_str().expect("string").to_string())
        .collect()
}

fn commands(v: Option<&Value>) -> Vec<Vec<String>> {
    v.map(|v| v.as_array().unwrap().iter().map(str_list).collect())
        .unwrap_or_default()
}

fn normalise_version(text: &str) -> String {
    let own = format!("prj {}\n", env!("CARGO_PKG_VERSION"));
    text.replace(&own, "prj {version}\n")
}

fn run_case(case: &Value, defaults: &Defaults) -> Outcome {
    let dir = tempfile::tempdir().unwrap();
    let tmp = dir.path().canonicalize().unwrap();
    fs::create_dir(tmp.join("home")).unwrap();
    fs::write(tmp.join("home/.gitconfig"), "").unwrap();
    write_exec(&tmp.join("browser.sh"), BROWSER_SH);
    write_exec(&tmp.join("fake-ssh"), FAKE_SSH);
    let mut env = base_env(&tmp);

    if let Some(up_cmds) = case.get("upstream") {
        let up = tmp.join("upstream");
        fs::create_dir(&up).unwrap();
        git(
            &str_list(&serde_json::json!(["init", "-q", "-b", "main"])),
            &up,
            &env,
        );
        for cmd in commands(Some(up_cmds)) {
            git(&cmd, &up, &env);
        }
    }

    let repo = tmp.join("repo");
    fs::create_dir(&repo).unwrap();
    if case.get("repo").and_then(Value::as_bool).unwrap_or(true) {
        git(
            &str_list(&serde_json::json!(["init", "-q", "-b", "main"])),
            &repo,
            &env,
        );
        if case.get("commit").and_then(Value::as_bool).unwrap_or(false) {
            let commit = serde_json::json!(["commit", "-q", "--allow-empty", "-m", "init"]);
            git(&str_list(&commit), &repo, &env);
        }
        let remote = match case.get("remote") {
            None => Some(defaults.remote.clone()),
            Some(Value::Null) => None,
            Some(v) => Some(v.as_str().unwrap().to_string()),
        };
        if let Some(remote) = remote {
            let add = vec!["remote".into(), "add".into(), "origin".into(), remote];
            git(&add, &repo, &env);
        }
        for cmd in commands(case.get("setup")) {
            git(&cmd, &repo, &env);
        }
    }

    if let Some(overrides) = case.get("env").and_then(Value::as_object) {
        for (key, value) in overrides {
            match value.as_str() {
                Some(v) => {
                    env.insert(key.clone(), v.replace("{tmp}", &tmp.to_string_lossy()));
                }
                None => {
                    env.remove(key);
                }
            }
        }
    }

    let args = case
        .get("args")
        .map(str_list)
        .unwrap_or_else(|| defaults.args.clone());
    let out = Command::new(env!("CARGO_BIN_EXE_prj"))
        .args(&args)
        .current_dir(&repo)
        .env_clear()
        .envs(&env)
        .output()
        .unwrap();
    let log = fs::read_to_string(tmp.join("browser.log")).unwrap_or_default();
    let tmp_str = tmp.to_string_lossy().into_owned();
    let norm = |b: &[u8]| normalise_version(&String::from_utf8_lossy(b).replace(&tmp_str, "{tmp}"));
    Outcome {
        name: case["name"].as_str().unwrap().to_string(),
        exit: out.status.code().unwrap_or(-1),
        stdout: norm(&out.stdout),
        stderr: norm(&out.stderr),
        browser_log: norm(log.as_bytes()),
    }
}

fn apply_rules(text: &str, rules: &[Rule]) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let (body, newline) = match line.strip_suffix('\n') {
                Some(body) => (body, "\n"),
                None => (line, ""),
            };
            let mut body = body.to_string();
            for rule in rules {
                if body.contains(&rule.when_contains) {
                    body.push_str(&rule.append);
                }
            }
            format!("{body}{newline}")
        })
        .collect()
}

fn expected_for(base: &Outcome, devs: &Deviations) -> Outcome {
    let mut exp = base.clone();
    let ov = devs.cases.get(&base.name);
    if !ov.is_some_and(|o| o.skip_rules) {
        exp.stdout = apply_rules(&exp.stdout, &devs.rules);
        exp.browser_log = apply_rules(&exp.browser_log, &devs.rules);
    }
    if let Some(ov) = ov {
        if let Some(v) = ov.exit {
            exp.exit = v;
        }
        if let Some(v) = &ov.stdout {
            exp.stdout = v.clone();
        }
        if let Some(v) = &ov.stderr {
            exp.stderr = v.clone();
        }
        if let Some(v) = &ov.browser_log {
            exp.browser_log = v.clone();
        }
    }
    exp
}

#[test]
fn parity_with_prjump_0_2_2() {
    let corpus: Corpus = serde_json::from_str(&fixture("parity-cases.json")).unwrap();
    let expected: Expected = serde_json::from_str(&fixture("parity-expected.json")).unwrap();
    let devs: Deviations = serde_json::from_str(&fixture("parity-deviations.json")).unwrap();
    let by_name: BTreeMap<_, _> = expected
        .results
        .iter()
        .map(|r| (r.name.clone(), r))
        .collect();
    assert_eq!(
        by_name.len(),
        corpus.cases.len(),
        "regenerate parity-expected.json"
    );
    for name in devs.cases.keys() {
        assert!(
            by_name.contains_key(name),
            "deviation for unknown case {name}"
        );
    }

    let mut failures = Vec::new();
    for case in &corpus.cases {
        let name = case["name"].as_str().unwrap();
        let base = by_name
            .get(name)
            .unwrap_or_else(|| panic!("no expected result for {name}"));
        let exp = expected_for(base, &devs);
        let got = run_case(case, &corpus.defaults);
        let status_only = case.get("compare").and_then(Value::as_str) == Some("status");
        let ok = if status_only {
            got.exit == exp.exit
                && got.stdout.is_empty() == exp.stdout.is_empty()
                && got.stderr.is_empty() == exp.stderr.is_empty()
        } else {
            got == exp
        };
        if !ok {
            failures.push(format!("{name}\n  expected: {exp:?}\n  got:      {got:?}"));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} parity cases differ:\n{}",
        failures.len(),
        corpus.cases.len(),
        failures.join("\n")
    );
}
