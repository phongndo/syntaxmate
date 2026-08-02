#!/usr/bin/env python3
"""Compare Mark grammar JSON with a pinned VS Code extensions checkout."""
from __future__ import annotations

import argparse
import hashlib
import json
import plistlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "benchmarks/textmate/vscode-grammar-differences.json"
COMMIT = "fc3def6774c76082adf699d366f31a557ce5573f"


def load_grammar(path: Path) -> dict | None:
    data = path.read_bytes()
    try:
        value = json.loads(data) if data.lstrip().startswith(b"{") else plistlib.loads(data)
    except (ValueError, plistlib.InvalidFileException):
        return None
    return value if isinstance(value, dict) and isinstance(value.get("scopeName"), str) else None


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


parser = argparse.ArgumentParser()
parser.add_argument("--vscode-checkout", type=Path, required=True)
parser.add_argument("--check", action="store_true")
args = parser.parse_args()
checkout = args.vscode_checkout.resolve()
actual_commit = __import__("subprocess").check_output(
    ["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True
).strip()
if actual_commit != COMMIT:
    raise SystemExit(f"expected VS Code {COMMIT}, got {actual_commit}")

vscode: dict[str, tuple[Path, dict]] = {}
for path in sorted((checkout / "extensions").rglob("*")):
    if not path.is_file() or "syntaxes" not in path.parts:
        continue
    grammar = load_grammar(path)
    if grammar is not None:
        vscode.setdefault(grammar["scopeName"], (path, grammar))

syntaxmate: dict[str, tuple[Path, dict]] = {}
for path in sorted((ROOT / "assets/grammars/languages").glob("*.tmLanguage.json")):
    grammar = load_grammar(path)
    if grammar is not None:
        syntaxmate[grammar["scopeName"]] = (path, grammar)

entries = []
for scope in sorted(set(syntaxmate) & set(vscode)):
    syntaxmate_path, syntaxmate_grammar = syntaxmate[scope]
    vscode_path, vscode_grammar = vscode[scope]
    syntaxmate_bytes = canonical(syntaxmate_grammar)
    vscode_bytes = canonical(vscode_grammar)
    equal = syntaxmate_bytes == vscode_bytes
    entries.append({
        "language": syntaxmate_path.name.removesuffix(".tmLanguage.json"),
        "scopeName": scope,
        "syntaxmatePath": str(syntaxmate_path.relative_to(ROOT)),
        "vscodePath": str(vscode_path.relative_to(checkout)),
        "syntaxmateCanonicalSha256": hashlib.sha256(syntaxmate_bytes).hexdigest(),
        "vscodeCanonicalSha256": hashlib.sha256(vscode_bytes).hexdigest(),
        "canonicalEqual": equal,
        "referencePolicy": "vscode-source-identical" if equal else "source-differs-behavior-audit-required",
    })

report = {
    "schemaVersion": 2,
    "vscodeCommit": COMMIT,
    "vscodeGrammarScopes": len(vscode),
    "matchingRootScopes": len(entries),
    "canonicalEqual": sum(entry["canonicalEqual"] for entry in entries),
    "canonicalDifferent": sum(not entry["canonicalEqual"] for entry in entries),
    "entries": entries,
}
serialized = json.dumps(report, indent=2) + "\n"
if args.check:
    if OUTPUT.read_text() != serialized:
        raise SystemExit(f"{OUTPUT.relative_to(ROOT)} is stale")
else:
    OUTPUT.write_text(serialized)
print(
    f"ok: {report['matchingRootScopes']} shared scopes; "
    f"{report['canonicalEqual']} equal, {report['canonicalDifferent']} explicitly different"
)
