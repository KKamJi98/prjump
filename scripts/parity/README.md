# Parity corpus

Checks that the Rust `prj` behaves like prjump 0.2.2 (Python), case by case.

| File | Role |
|---|---|
| `tests/fixtures/parity-cases.json` | The cases. Edit this to add coverage. |
| `scripts/parity/generate.py` | Runs prjump 0.2.2 from PyPI on every case and writes the expected results. |
| `tests/fixtures/parity-expected.json` | Generated. Do not edit by hand. |
| `tests/fixtures/parity-deviations.json` | Intentional differences, each with a reason. |
| `scripts/parity/deviate.py` | Adds one entry to the deviations file. |
| `tests/parity.rs` | Runs the Rust binary on every case and compares (`cargo test --test parity`, Unix only). |

Regenerate after changing the cases (needs `uv` and network access to PyPI):

```bash
uv run --no-project python -I scripts/parity/generate.py
```

## Case schema

```jsonc
{
  "name": "group/what",            // unique
  "args": ["open", "--print"],     // default: defaults.args
  "repo": true,                    // false: run outside any git repository
  "commit": false,                 // true: one empty commit on main
  "remote": "git@github.com:o/r.git", // default: defaults.remote; null: no origin
  "setup": [["switch", "-c", "x"]],   // extra git commands in the repository
  "upstream": [["commit", "..."]],    // build {tmp}/upstream, reachable through
                                      // GIT_SSH_COMMAND={tmp}/fake-ssh
  "env": {"KEY": "value or null"},    // {tmp} is replaced; null removes the key
  "compare": "exact"               // "status": exit code and which streams are empty
}
```

Every case runs with a clean environment: an empty global git config, `LC_ALL=C`,
`GIT_SSH_COMMAND=false` (no network), and `BROWSER` pointing at a script that
appends the URL to `{tmp}/browser.log` instead of opening anything.

## Deviations

- `rules`: `{"when_contains", "append", "reason"}` appends text to every
  expected stdout or browser-log line that contains the marker. For changes
  that touch many cases the same way.
- `cases.<name>`: replaces `exit`, `stdout`, `stderr` or `browser_log` for one
  case. Rules apply after the override; `skip_rules: true` keeps them away
  from that case.

Add or replace a per-case entry with:

```bash
uv run --no-project python -I scripts/parity/deviate.py <case> "<reason>" \
  [--exit N] [--stdout TEXT] [--stderr TEXT] [--browser-log TEXT] [--skip-rules]
```
