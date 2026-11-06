#!/usr/bin/env bash
# Called by record.sh only, after replacing HOME and the inherited environment.
set -euo pipefail
if [[ $# -ne 1 || "${HOME}" != "$1/home" || "${PRJ_DEMO_ROOT:-}" != "$1" ]]; then
  printf 'Run this fixture through: bash docs/demo/record.sh\n' >&2
  exit 2
fi
root="$1"
mkdir -p "$HOME/.config/prjump" "$HOME/.aws" "$HOME/.ssh" "$root/repo" "$root/tmp"
cat > "$HOME/.bashrc" <<'BASHRC'
export PS1='$ '
export PS2='> '
unset PROMPT_COMMAND
export HISTFILE=/dev/null
BASHRC
cat > "$HOME/.config/prjump/config.toml" <<'CONFIG'
[hosts."demo-gh"]
web_host = "github.com"
CONFIG
# Deliberately synthetic, unused by prj. Never read the operator's config.
cat > "$HOME/.aws/config" <<'AWS'
[profile demo-sandbox]
region = us-east-1
AWS
cat > "$HOME/.ssh/config" <<'SSH'
Host demo-gh
    HostName github.com
    User git
SSH
# Block network and browser fallback even if a future tape accidentally asks.
cat > "$root/bin/ssh" <<'SSH'
#!/bin/sh
printf 'demo: network SSH is disabled\n' >&2
exit 1
SSH
cat > "$root/bin/demo-browser" <<'BROWSER'
#!/bin/sh
printf 'demo: browser opening is disabled; use --print\n' >&2
exit 0
BROWSER
chmod +x "$root/bin/ssh" "$root/bin/demo-browser"
git init -q --bare --initial-branch=main "$root/origin.git"
cd "$root/repo"
git init -q --initial-branch=main
git config user.name 'Example Developer'
git config user.email 'dev@example.invalid'
git config commit.gpgsign false
git config core.hooksPath /dev/null
git commit -q --allow-empty -m 'Start demo'
git remote add origin ../origin.git
git push -qu origin main
git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/main
git branch develop
git update-ref refs/remotes/alias/main HEAD
git symbolic-ref refs/remotes/alias/HEAD refs/remotes/alias/main
git remote add bitbucket git@bitbucket.org:octo-demo/shop-api.git
git remote add alias git@demo-gh:octo-demo/shop-api.git
