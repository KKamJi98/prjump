#!/usr/bin/env bash
# Rebuild and record without loading personal shell, Git, SSH or cloud config.
set -euo pipefail
repo="$(cd "$(dirname "$0")/../.." && pwd)"
tool_paths=()
for tool in git vhs ttyd ffmpeg; do
  resolved="$(command -v "$tool")" || { printf 'Missing tool: %s\n' "$tool" >&2; exit 1; }
  [[ "$resolved" == /* && -x "$resolved" ]] || { printf 'Expected an executable path: %s\n' "$tool" >&2; exit 1; }
  tool_paths+=("$resolved")
done
if [[ -z "${PRJ_BIN:-}" ]]; then
  command -v cargo >/dev/null || { printf 'Missing tool: cargo\n' >&2; exit 1; }
  (cd "$repo" && cargo build --locked)
  binary="$repo/target/debug/prj"
else
  binary="$PRJ_BIN"
fi
[[ -x "$binary" ]] || { printf 'PRJ_BIN must name an executable\n' >&2; exit 2; }
# mktemp owns this directory; cleanup never targets an existing user directory.
stage="$(mktemp -d "${TMPDIR:-/tmp}/prjump-demo.XXXXXX")"
trap 'rm -rf "$stage"' EXIT
mkdir -p "$stage/home" "$stage/bin" "$stage/tmp"
cp "$binary" "$stage/bin/prj"
for resolved in "${tool_paths[@]}"; do
  ln -s "$resolved" "$stage/bin/$(basename "$resolved")"
done
# Deliberate allowlist: no credentials, inherited Git settings, or startup hooks.
env -i HOME="$stage/home" XDG_CONFIG_HOME="$stage/home/.config" \
  PRJ_CONFIG="$stage/home/.config/prjump/config.toml" PRJ_DEMO_ROOT="$stage" \
  PATH="$stage/bin:/usr/bin:/bin" \
  TMPDIR="$stage/tmp" TERM=xterm-256color LANG=en_US.UTF-8 \
  GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_TERMINAL_PROMPT=0 \
  GIT_ALLOW_PROTOCOL=file BROWSER=demo-browser \
  /bin/bash --noprofile --norc -c '
    set -euo pipefail
    cd "$1"
    bash docs/demo/setup.sh "$PRJ_DEMO_ROOT"
    vhs docs/demo/prjump.tape
  ' demo "$repo"
