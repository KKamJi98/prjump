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
[docs/rust-migration.md](docs/rust-migration.md) for the fixes and the few
edge cases that behave differently.

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

# Also copy the URL to your terminal's clipboard (works over SSH)
prj open --copy
```

What `prj open` works out for you:

- **Source branch.** Without `--source`, prj uses the branch you are on and
  the name and remote `git push` would use for it. It warns (and carries on)
  when the branch has no upstream, is not on the remote yet, or has local
  commits the remote does not have.
- **Forks.** When the branch is pushed to your fork (say `origin`) and the PR
  goes to another remote (`--remote upstream`), the GitHub page compares
  across forks (`main...you:branch`).
- **GitHub.** The page opens straight on the "Open a pull request" form
  (`?quick_pull=1`).
- **Target branch.** Without `--target`, GitHub PRs use the remote's default
  branch: `refs/remotes/<remote>/HEAD`, then `git ls-remote` (given up after
  `PRJ_GIT_TIMEOUT` seconds, default 10), then the first of `main`, `master`,
  `develop`, `trunk` that exists.
- **Bitbucket Cloud.** `--target` adds a `dest` query parameter as a
  best-effort preselection. If the UI ignores it, choose the destination
  branch manually.
- **Browser.** `$BROWSER` is honoured on every platform. In an SSH session
  without a forwarded display (and no `$BROWSER`), prj prints the URL
  instead of opening a browser on the remote machine.

### Host aliases and self-hosted servers

prj recognises `github.com`, `bitbucket.org` and their subdomains. For an
`~/.ssh/config` alias or a server with another name, add it to
`~/.config/prjump/config.toml` (or `$XDG_CONFIG_HOME/prjump/config.toml`, or
the file named by `$PRJ_CONFIG`):

```toml
# git@github-work:org/repo.git, where github-work is an ~/.ssh/config alias
[hosts."github-work"]
web_host = "github.com"

# A GitHub Enterprise server whose name says nothing about GitHub
[hosts."git.corp.example"]
provider = "github"
web_host = "git.corp.example:8443"   # optional, host[:port]
```

`--provider` still wins over the file.

### Shell completions

Homebrew installs them. Otherwise:

```bash
prj completions bash > ~/.local/share/bash-completion/completions/prj
prj completions zsh  > "${fpath[1]}/_prj"
prj completions fish > ~/.config/fish/completions/prj.fish
```

## Development

```bash
cargo fmt --all --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
```

`tests/parity.rs` replays a corpus of cases recorded from the Python 0.2.2
release; see [scripts/parity/README.md](scripts/parity/README.md).
