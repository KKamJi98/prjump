# Rust migration notes

prjump moved from Python (0.2.x on PyPI) to Rust (0.3.0). This file records
what was checked before and during the port, the decisions that came out of
it, and every place where the Rust `prj` intentionally behaves differently
from prjump 0.2.2.

Reference for "Python behaviour" below: prjump 0.2.2 from PyPI running on
CPython 3.13.5, the interpreter `uv tool install prjump` picked on macOS.

## Phase 0: spike results (2026-11-06)

### Opening the browser: `webbrowser` crate 1.2.6

Read from the crate source and checked with a dry-run build on macOS.

| Environment | Python `webbrowser.open` | `webbrowser` crate 1.2.6 |
|---|---|---|
| `$BROWSER` set, macOS | Tried first (`_tryorder` starts with the `$BROWSER` entries) | **Ignored**. `src/macos.rs` asks NSWorkspace for the default https handler and never reads `$BROWSER`. Confirmed: `BROWSER=/nonexistent` still succeeds in dry-run. |
| `$BROWSER` set, Linux/BSD | Tried first, then the platform list | Tried first (`try_with_browser_env`), then `xdg-open`, desktop-specific openers, WSL, `x-www-browser` |
| WSL | Platform list (`xdg-open`, `wslview` if installed, text browsers) | Detects WSL through `/proc/sys/fs/binfmt_misc/WSLInterop` and starts the Windows default browser through PowerShell or `wsl-open`. **Unverified**: running the spike over `ssh wsl` was blocked by the local permission policy, so this row comes from the source only. |
| SSH session without `DISPLAY` | Falls through to text browsers (`www-browser`, `lynx`, `w3m`) that take over the terminal | Does not look at `DISPLAY` or `SSH_*`. On plain Linux `xdg-open` fails and the call returns `NotFound`; on WSL it still opens the Windows browser. |
| Text browser in the list | Runs in the foreground | Runs in the foreground (`TEXT_BROWSERS` list, `background = false`) |

Decision: prj handles `$BROWSER` itself on every platform with Python's
`GenericBrowser` rules (each `:`-separated entry, `;` on Windows, is a whole
command run with the URL as its only argument; exit status 0 wins). Only when
every entry fails does it call the crate, after removing `$BROWSER` so the
crate does not run the same entries a second time. This keeps macOS users who
set `$BROWSER` working.

### Release tooling: dist 0.34.0 vs a hand-written workflow

dist (axodotdev, latest release v0.34.0, 2026-11-02) does not fit the
authentication the plan fixed:

- Its Homebrew job (`templates/ci/github/partials/publish_homebrew.yml.j2`)
  checks out the tap with `secrets.HOMEBREW_TAP_TOKEN`, a personal access
  token. prjump pushes with an SSH deploy key (`HOMEBREW_TAP_DEPLOY_KEY`)
  that can only write to the tap.
- The same job ends in a plain `git push` with no retry. Four repositories
  share `KKamJi98/homebrew-tap` and can release at the same time, so the push
  has to rebase and retry on a non-fast-forward.
- dist does not publish to crates.io; that job would be custom anyway.

Decision: hand-written `.github/workflows/release.yml`.

### crates.io

- `prjump` is free on crates.io (404 on 2026-11-06). `prj` is taken by an
  unrelated crate, so the crate is `prjump` and the binary is `prj`.
- Trusted Publishing can only be configured after a first manual publish
  (crates.io docs: "initial publish requires an API token"; RFC 3691).
- Browser checks for crates.io use the personal Aside profile
  (`aside --account u0`). An earlier check on 2026-11-06 ran on the default
  (company) profile and does not say anything about the personal login.

## Phase 1: parity port

`tests/parity.rs` replays `tests/fixtures/parity-cases.json` against the Rust
binary and compares exit status, stdout, stderr and the URL handed to the
browser with what prjump 0.2.2 recorded (`parity-expected.json`, produced by
`scripts/parity/generate.py`). At the end of Phase 1 all 95 cases matched
with an empty deviations file.

Choices made to get there:

- **No `url` crate.** It rejects scp-style remotes, normalises `..`,
  percent-encodes paths, keeps IPv6 brackets and keeps the case of `ssh://`
  hosts. `src/remote.rs` ports CPython 3.13's `urlsplit`/`urlparse` instead,
  including `;params`, IPv6/IPvFuture validation and the error texts that
  surfaced as `Unexpected error: ...`.
- **`quote(safe="")` set.** `percent_encoding::NON_ALPHANUMERIC` minus
  `-._~`.
- **argparse compatibility.** `args_override_self` (last value wins),
  `infer_long_args` (`--tar`, `--pri`), `allow_negative_numbers` on the value
  options (argparse takes `-t -1` as the value `-1`; the corpus caught this),
  no `-V` short flag, and `prj` with no subcommand prints help to stdout and
  exits 1.

### Known differences from prjump 0.2.2

| Situation | prjump 0.2.2 | prj 0.3.0 | Why |
|---|---|---|---|
| Help, usage and argument-error text | argparse wording | clap wording | Not a contract. Exit status (0 for help, 2 for errors) and the stream (stdout/stderr) are the same, which is what the corpus checks for these cases. |
| stdout closed early (`prj open --print \| head -c0`) | `Exception ignored ... BrokenPipeError`, exit 120 | No output, exit 0 | Plan: a closed pipe is not an error. |
| `git` not on PATH | `Unexpected error: [Errno 2] No such file or directory: 'git'` | `Error: git executable not found. Install git and make sure it is on PATH.` | Clearer message, same exit status 1. |
| git prints non-UTF-8 bytes | `Unexpected error: 'utf-8' codec can't decode ...` | `Error: git <cmd> printed output that is not valid UTF-8.` | Same exit status 1. |
| Non-UTF-8 command-line argument | Accepted (surrogateescape) | clap argument error, exit 2 | Branch names on the command line are UTF-8 in practice. |
| `$BROWSER` names a browser Python knows (`firefox`, `chrome`, ...) | Python swaps in its own controller for that browser | Runs the entry as given with the URL | Same browser opens; flags may differ. |
| Non-ASCII host that NFKC-normalises into `/?#@:` | `ValueError` from `_checknetloc` | Not checked | Needs a Unicode normalisation dependency for a case that cannot name a real host. |
| Any panic | n/a | `Unexpected error: <msg>`, exit 1 | Matches Python's catch-all instead of Rust's exit 101. |

## Phase 3: bug fixes

Each fix is its own commit with a test that failed before it. Parity cases
whose expected result changed are listed with the reason in
`tests/fixtures/parity-deviations.json`.

| # | Was | Now | Parity cases |
|---|---|---|---|
| 1 | Default branch `release/2026` became `2026` | `refs/remotes/<remote>/` prefix stripped, slashes kept | `default/symbolic-ref-slash` |
| 2 | Branch `foo` with a tag `foo` became `heads/foo` | `git branch --show-current` | `current/ambiguous-tag` |
| 3 | No commit yet: "Failed to detect current branch." | Same command knows the branch | `current/unborn` |
| 4 | `https://host:8443/...` lost the port; IPv6 hosts lost their brackets | Port kept for http(s) remotes unless it is the default; ssh ports dropped; `[...]` around IPv6 | `url/https-port`, `url/ipv6`, `url/ipv6-scoped` |
| 5 | `ssh.github.com`, `altssh.bitbucket.org` used as web hosts | Mapped to `github.com`, `bitbucket.org` | `url/ssh-github-port-443`, `url/altssh-bitbucket` |
| 6 | `notgithub.com` detected as GitHub | Exact domain or subdomain only | `url/notgithub` |
| 7 | `git remote show`: locale-dependent text, no time limit | `git ls-remote --symref <remote> HEAD`, `GIT_TERMINAL_PROMPT=0`, own process group, stopped after `PRJ_GIT_TIMEOUT` seconds (default 10) | none (same answers through the fake ssh) |

The `HEAD branch:` line of `git remote show` being translated was not
confirmed (only other lines were seen translated); the fix removes the
dependency either way.

## Phase 4: P1 features

| Feature | Notes |
|---|---|
| Open the PR form directly | Planned as `?expand=1`. GitHub Docs ("Using query parameters to create a pull request", fpt/ghes/ghec) document `quick_pull=1` for this and do not mention `expand=1`, so prj uses `quick_pull=1`. Parity: one rule appends it to every compare URL. |
| Source from the push target | `git for-each-ref --format=%(upstream:remotename)%00%(upstream:remoteref)%00%(push:remotename)`. On git 2.56 `%(push:remoteref)` and `%(push)` stayed empty in triangular setups (checked 2026-11-06), so the branch name comes from `push.default` instead (`upstream`/`tracking` push under the upstream name, every other mode under the local name). Warnings: no upstream, not on the remote yet, N local commits not pushed. Parity: the five cases without `--source` gain the "no upstream" warning on stderr. |
| Forks | When the push remote differs from `--remote`, is on the same host and has another owner, the GitHub head is `owner:branch` (GitHub Docs, "Comparing across forks"). Bitbucket's create URL has no field for it, so prj warns instead. |
| Host aliases | `config.toml` `[hosts."<host>"]` with `web_host` and `provider`. Adds `serde` and `toml` as dependencies. |
| SSH sessions, `--copy` | `SSH_CONNECTION`/`SSH_CLIENT`/`SSH_TTY` set, no `DISPLAY`/`WAYLAND_DISPLAY`, no `$BROWSER`: print instead of open. `--copy` writes OSC 52 to `/dev/tty` (stderr on Windows), wrapped for tmux passthrough. Whether a terminal accepts OSC 52 is up to the terminal; failures are warnings. |
| Completions | `prj completions <shell>` via `clap_complete`; the Homebrew formula installs bash, zsh and fish completions. |

P2 and P3 ideas are in [ROADMAP.md](ROADMAP.md).
