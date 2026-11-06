# prjump

[![crates.io](https://img.shields.io/crates/v/prjump)](https://crates.io/crates/prjump)
[![CI](https://github.com/KKamJi98/prjump/actions/workflows/ci.yml/badge.svg)](https://github.com/KKamJi98/prjump/actions/workflows/ci.yml)

Open the pull request page for the branch you are on, without leaving the shell.

`prj open` reads the remote, works out whether it is GitHub or Bitbucket Cloud,
and opens the compare page with the right source and target already filled in.
No copying branch names into a URL.

The crate is `prjump`; the command is `prj`.

## Install

```bash
# Homebrew (macOS, Linux)
brew install KKamJi98/tap/prj

# Cargo (any platform with a Rust toolchain)
cargo install --locked prjump
```

Prebuilt archives for macOS (arm64, x86_64), Linux (arm64, x86_64, static
musl) and Windows (x86_64) are attached to every
[GitHub release](https://github.com/KKamJi98/prjump/releases), with
`SHA256SUMS` and build provenance attestations
(`gh attestation verify <archive> --repo KKamJi98/prjump`).

### Coming from the Python version (0.2.x)

prjump 0.3.0 is a Rust rewrite. The PyPI package stays at 0.2.2 and gets no
new releases. Install the new one first, check that `prj` resolves to it,
then remove the old one:

```bash
brew install KKamJi98/tap/prj      # or: cargo install --locked prjump
hash -r && command -v prj          # should not point into ~/.local/bin
uv tool uninstall prjump
```

The commands and flags are the same. See
[docs/rust-migration.md](docs/rust-migration.md) for the few edge cases that
changed.

## Usage

```bash
# Open PR page for current branch (auto-detect provider)
prj open

# Use a specific remote
prj open --remote origin

# Override target (merge into) or source (merge from) branch
# short flags: -t/--target, -s/--source
prj open --target main --source feature/my-branch
prj open -t main -s feature/my-branch

# Force provider if auto-detection fails
prj open --provider github
prj open --provider bitbucket

# Print URL without opening the browser
prj open --print
```

Notes:
- Run inside a git repository with the target remote configured.
- For Bitbucket Cloud, `--target` adds a `dest` query parameter as a best-effort
  preselection. If the UI ignores it, choose the destination branch manually.
- `$BROWSER` is honoured on every platform, like Python's `webbrowser` did.

## Development

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

`tests/parity.rs` replays a corpus of cases recorded from the Python 0.2.2
release; see [scripts/parity/README.md](scripts/parity/README.md).
