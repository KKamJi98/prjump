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

