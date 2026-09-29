#!/usr/bin/env python3
"""GitHub Actions annotation helper.
  annotate.py tail <file> <title> [lines]   -> ::error with the last lines of a log
  annotate.py json <file> <title>           -> ::notice with the numeric fields of a JSON result
Annotations are readable through the public API even when the raw logs are not."""
import json, sys
from pathlib import Path


def esc(s):
    return s.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


mode, path, title = sys.argv[1], Path(sys.argv[2]), sys.argv[3]
t = esc(title).replace(",", "%2C").replace(":", "%3A")
if mode == "tail":
    n = int(sys.argv[4]) if len(sys.argv) > 4 else 40
    body = "\n".join(path.read_text(errors="replace").splitlines()[-n:]) if path.exists() else "(no log file)"
    print(f"::error title={t}::{esc(body)}")
else:
    d = json.loads(path.read_text()) if path.exists() else {}
    msg = " ".join(f"{k}={round(v, 2) if isinstance(v, float) else v}" for k, v in d.items())
    print(f"::notice title={t}::{esc(msg)}")
