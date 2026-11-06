#!/usr/bin/env python3
"""Add or replace one per-case entry in tests/fixtures/parity-deviations.json.

usage: deviate.py <case> <reason> [--exit N] [--stdout S] [--stderr S]
                  [--browser-log S] [--skip-rules]
Values are taken literally; a trailing newline is added to text fields.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

PATH = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "parity-deviations.json"


def main() -> int:
    p = argparse.ArgumentParser(description="Record an intentional parity deviation.")
    p.add_argument("case")
    p.add_argument("reason")
    p.add_argument("--exit", type=int)
    p.add_argument("--stdout")
    p.add_argument("--stderr")
    p.add_argument("--browser-log")
    p.add_argument("--skip-rules", action="store_true")
    a = p.parse_args()

    data = json.loads(PATH.read_text())
    entry: dict = {"reason": a.reason}
    if a.exit is not None:
        entry["exit"] = a.exit
    for key in ("stdout", "stderr", "browser_log"):
        value = getattr(a, key)
        if value is not None:
            entry[key] = value + "\n" if value else ""
    if a.skip_rules:
        entry["skip_rules"] = True
    data["cases"][a.case] = entry
    data["cases"] = dict(sorted(data["cases"].items()))
    PATH.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
