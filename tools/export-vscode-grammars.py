#!/usr/bin/env python3
"""Export JSON and plist TextMate grammars from a VS Code checkout as JSON."""
import json
import plistlib
import sys
from pathlib import Path

checkout = Path(sys.argv[1]).resolve()
grammars = {}
for path in sorted((checkout / "extensions").rglob("*")):
    if not path.is_file() or "syntaxes" not in path.parts:
        continue
    data = path.read_bytes()
    try:
        value = json.loads(data) if data.lstrip().startswith(b"{") else plistlib.loads(data)
    except (ValueError, plistlib.InvalidFileException):
        continue
    if isinstance(value, dict) and isinstance(value.get("scopeName"), str):
        grammars.setdefault(value["scopeName"], {
            "path": str(path.relative_to(checkout)),
            "grammar": value,
        })
json.dump(grammars, sys.stdout, ensure_ascii=False)
