# Recreate the prjump demo

From the repository root:

```bash
bash docs/demo/record.sh
```

Requirements: Git, VHS, ttyd, ffmpeg, and a font suitable for a terminal.
Cargo is needed only when building the checkout. Tools must be executables on `PATH`;
the script resolves their paths and links only those tools into the isolated environment.
The tape requests Menlo; on Linux the terminal renderer can use its monospace fallback.
The recording was made with VHS 0.12.1 on macOS. To use a released binary instead
of building the checkout:

```bash
PRJ_BIN="$(command -v prj)" bash docs/demo/record.sh
```

The script writes `docs/demo/prjump.gif` and `docs/demo/prjump.mp4` using
[prjump.tape](prjump.tape). The tape must not be run directly in a real HOME.

## Safety boundary

- `record.sh` starts an allowlisted, empty environment with a temporary HOME,
  XDG config directory, and neutral `$ ` prompt. Personal shell initialization,
  Git configuration, credentials, SSH agent variables, and cloud configuration
  are not inherited.
- `setup.sh` creates only synthetic AWS, SSH, and prjump config, a new Git
  repository, fake commit identity, and a local bare remote. It refuses an
  ordinary HOME. Only local file transport is allowed; a fake `ssh` blocks
  network SSH, and a fake browser prevents accidental browser opening.
- The branch is committed and pushed to the local bare repository before the
  visible remote is replaced with `git@github.com:octo-demo/shop-api.git`.
  Nothing is pushed to GitHub or Bitbucket. All visible URLs are synthetic
  examples, not links to a repository required to exist.
- Every `prj` result comes directly from the actual built or supplied binary.
  No command output is rewritten or mocked. `Hide` only skips fixture setup;
  the visible commands and results are real.
- GitHub, Bitbucket Cloud, and their public provider domains are intentional.
  No personal account names, real project names, private hosts, or secrets
  belong in the recording.
- The temporary fixture is deleted when the script exits. It never reads the
  operator's real `~/.aws/config`, `~/.ssh/config`, or Git configuration.

## Inspect before publishing

Check duration, dimensions, and size:

```bash
ffprobe -v error -show_entries format=duration,size \
  -show_entries stream=width,height -of json docs/demo/prjump.gif
```

Extract representative frames into a fresh directory under your temporary
workspace (not the repository), then open and inspect the PNGs:

```bash
frames="$(mktemp -d "${TMPDIR:-/tmp}/prjump-frames.XXXXXX")"
ffmpeg -i docs/demo/prjump.gif -vf fps=1/2 "$frames/frame-%02d.png"
```

Verify all four scenes, the first and final frame, complete URLs, a neutral
prompt, no errors, no clipped text, and no personal or company data. Keep the
last result visible. Target 20-30 seconds and at most 3 MB. If needed, optimize
with `gifsicle -O3 --lossy` and inspect the result again; do not alter CLI output.
