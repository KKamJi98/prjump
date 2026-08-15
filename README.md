# prjump

[![PyPI](https://img.shields.io/pypi/v/prjump)](https://pypi.org/project/prjump/)
[![CI](https://github.com/KKamJi98/prjump/actions/workflows/ci.yml/badge.svg)](https://github.com/KKamJi98/prjump/actions/workflows/ci.yml)

Open the pull request page for the branch you are on, without leaving the shell.

`prj open` reads the remote, works out whether it is GitHub or Bitbucket Cloud,
and opens the compare page with the right source and target already filled in.
No copying branch names into a URL.

## Install

```bash
# PyPI
uv tool install prjump

# Straight from the repository
uv tool install git+https://github.com/KKamJi98/prjump

# Local development
uv tool install --editable .
```

The package is `prjump`; the command is `prj`.

Uninstall:

```bash
uv tool uninstall prjump
```

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
