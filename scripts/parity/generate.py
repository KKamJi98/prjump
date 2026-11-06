#!/usr/bin/env python3
"""Record what prjump 0.2.2 (Python) does for every case in the parity corpus.

Writes tests/fixtures/parity-expected.json, which tests/parity.rs replays
against the Rust `prj`. The Python package is gone from this repository, so
the reference comes from PyPI:

    uv run --no-project python -I scripts/parity/generate.py

The script installs prjump 0.2.2 into a throwaway uv environment once and
runs its `prj` directly, so uv's own output never reaches the recording.
Pass --reference to use another `prj` executable instead.

The harness here and the one in tests/parity.rs must stay in step: same
environment, same setup commands, same normalisation.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import stat
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "tests" / "fixtures" / "parity-cases.json"
EXPECTED = ROOT / "tests" / "fixtures" / "parity-expected.json"
REFERENCE_SPEC = "prjump==0.2.2"

BROWSER_SH = '#!/bin/sh\nprintf \'%s\\n\' "$1" >> "$(dirname "$0")/browser.log"\n'
FAKE_SSH = '#!/bin/sh\nexec git upload-pack "$(dirname "$0")/upstream"\n'
VERSION_RE = re.compile(r"^prj \d+\.\d+\.\d+\S*$", re.MULTILINE)


def write_exec(path: Path, body: str) -> None:
    path.write_text(body)
    path.chmod(path.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def base_env(tmp: Path) -> dict[str, str]:
    home = tmp / "home"
    return {
        "PATH": os.environ["PATH"],
        "HOME": str(home),
        "GIT_CONFIG_GLOBAL": str(home / ".gitconfig"),
        "GIT_CONFIG_NOSYSTEM": "1",
        "LC_ALL": "C",
        "LANG": "C",
        "GIT_AUTHOR_NAME": "t",
        "GIT_AUTHOR_EMAIL": "t@example.com",
        "GIT_COMMITTER_NAME": "t",
        "GIT_COMMITTER_EMAIL": "t@example.com",
        "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z",
        "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z",
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_SSH_COMMAND": "false",
        "GIT_SSH_VARIANT": "simple",
        "BROWSER": str(tmp / "browser.sh"),
    }


def git(args: list[str], cwd: Path, env: dict[str, str]) -> None:
    subprocess.run(["git", *args], cwd=cwd, env=env, check=True, capture_output=True)


def run_case(case: dict, defaults: dict, reference: list[str]) -> dict:
    with tempfile.TemporaryDirectory(prefix="prj-parity-") as raw:
        tmp = Path(raw).resolve()
        (tmp / "home").mkdir()
        (tmp / "home" / ".gitconfig").write_text("")
        write_exec(tmp / "browser.sh", BROWSER_SH)
        write_exec(tmp / "fake-ssh", FAKE_SSH)
        env = base_env(tmp)

        if "upstream" in case:
            up = tmp / "upstream"
            up.mkdir()
            git(["init", "-q", "-b", "main"], up, env)
            for cmd in case["upstream"]:
                git(cmd, up, env)

        repo = tmp / "repo"
        repo.mkdir()
        if case.get("repo", True):
            git(["init", "-q", "-b", "main"], repo, env)
            if case.get("commit", False):
                git(["commit", "-q", "--allow-empty", "-m", "init"], repo, env)
            remote = case.get("remote", defaults["remote"])
            if remote is not None:
                git(["remote", "add", "origin", remote], repo, env)
            for cmd in case.get("setup", []):
                git(cmd, repo, env)

        for key, value in case.get("env", {}).items():
            if value is None:
                env.pop(key, None)
            else:
                env[key] = value.replace("{tmp}", str(tmp))

        args = case.get("args", defaults["args"])
        proc = subprocess.run(
            [*reference, *args], cwd=repo, env=env, capture_output=True, text=True
        )
        log = tmp / "browser.log"
        browser_log = log.read_text() if log.exists() else ""

        def norm(text: str) -> str:
            return VERSION_RE.sub("prj {version}", text.replace(str(tmp), "{tmp}"))

        return {
            "name": case["name"],
            "exit": proc.returncode,
            "stdout": norm(proc.stdout),
            "stderr": norm(proc.stderr),
            "browser_log": norm(browser_log),
        }


def main() -> int:
    parser = argparse.ArgumentParser(description="Record prjump 0.2.2 parity results.")
    parser.add_argument("--reference", help="path to a prj executable")
    opts = parser.parse_args()

    corpus = json.loads(CASES.read_text())
    with tempfile.TemporaryDirectory(prefix="prj-reference-") as venv:
        if opts.reference:
            reference, label = [opts.reference], opts.reference
        else:
            subprocess.run(["uv", "venv", "-q", venv], check=True)
            subprocess.run(
                ["uv", "pip", "install", "-q", "--python", f"{venv}/bin/python", REFERENCE_SPEC],
                check=True,
            )
            reference, label = [f"{venv}/bin/prj"], REFERENCE_SPEC
        if shutil.which(reference[0]) is None:
            parser.error(f"{reference[0]} is not executable")
        results = [run_case(c, corpus["defaults"], reference) for c in corpus["cases"]]

    EXPECTED.write_text(
        json.dumps({"reference": label, "results": results}, ensure_ascii=False, indent=2) + "\n"
    )
    print(f"wrote {len(results)} results to {EXPECTED.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
