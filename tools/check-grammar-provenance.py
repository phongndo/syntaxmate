#!/usr/bin/env python3
"""Audit every grammar target and optionally compare pinned VS Code sources."""
from __future__ import annotations

import argparse
import hashlib
import json
import plistlib
import urllib.parse
import urllib.request
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets/grammars/languages"
REPORT = ROOT / "benchmarks/textmate/grammar-provenance-audit.json"
VSCODE_COMMIT = "fc3def6774c76082adf699d366f31a557ce5573f"
VSCODE_ASSETS = {
    "dart": "extensions/dart/syntaxes/dart.tmLanguage.json",
    "ignore": "extensions/git-base/syntaxes/ignore.tmLanguage.json",
    "js-regexp": "extensions/javascript/syntaxes/Regular Expressions (JavaScript).tmLanguage",
    "r": "extensions/r/syntaxes/r.tmLanguage.json",
    "handlebars": "extensions/handlebars/syntaxes/Handlebars.tmLanguage.json",
    "pug": "extensions/pug/syntaxes/pug.tmLanguage.json",
    "php": "extensions/php/syntaxes/php.tmLanguage.json",
    "rst": "extensions/restructuredtext/syntaxes/rst.tmLanguage.json",
    "yaml": "extensions/yaml/syntaxes/yaml.tmLanguage.json",
    "yaml-1.2": "extensions/yaml/syntaxes/yaml-1.2.tmLanguage.json",
    "yaml-embedded": "extensions/yaml/syntaxes/yaml-embedded.tmLanguage.json",
}


def canonical(value: object) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def dependencies(value: object) -> list[str]:
    found: set[str] = set()

    def visit(node: object) -> None:
        if isinstance(node, dict):
            include = node.get("include")
            if isinstance(include, str) and include not in {"$self", "$base"} and not include.startswith("#"):
                found.add(include.split("#", 1)[0])
            for child in node.values():
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)

    visit(value)
    return sorted(found)


def upstream_vscode(path: str) -> object:
    encoded = "/".join(urllib.parse.quote(part) for part in path.split("/"))
    url = f"https://raw.githubusercontent.com/microsoft/vscode/{VSCODE_COMMIT}/{encoded}"
    with urllib.request.urlopen(url) as response:  # noqa: S310 - pinned HTTPS source
        data = response.read()
    if path.endswith(".json"):
        return json.loads(data)
    return plistlib.loads(data)


def apply_recorded_transformation(language: str, grammar: object) -> object:
    unavailable_includes = {
        "pug": {"source.sass", "source.stylus"},
        "rst": {"source.cmake"},
    }.get(language)
    if unavailable_includes is None:
        return grammar
    def visit(node: object) -> None:
        if isinstance(node, dict):
            include = node.get("include")
            if include in unavailable_includes:
                node["include"] = f"{include}.vscode-unavailable"
            for child in node.values():
                visit(child)
        elif isinstance(node, list):
            for child in node:
                visit(child)
    visit(grammar)
    return grammar


def build_report(verify_vscode: bool) -> dict[str, object]:
    source_text = (ROOT / "assets/grammars/SOURCE.toml").read_text()
    license_data = json.loads((ROOT / "assets/grammars/licenses.json").read_text())
    metadata = json.loads((ROOT / "assets/grammars/language-metadata.json").read_text())
    records = {record["language"]: record for record in license_data["assets"]}
    files = {path.name.removesuffix(".tmLanguage.json"): path for path in ASSETS.glob("*.tmLanguage.json")}
    if set(records) != set(files):
        raise SystemExit(f"license/assets differ: records-only={set(records)-set(files)}, files-only={set(files)-set(records)}")

    entries = []
    vscode_mismatches = []
    for language, path in sorted(files.items()):
        record = records[language]
        grammar = json.loads(path.read_text())
        if grammar.get("scopeName") != record.get("scopeName"):
            raise SystemExit(f"{language}: scopeName differs from licenses.json")
        for required in ("source", "version", "license", "module"):
            if not record.get(required):
                raise SystemExit(f"{language}: missing provenance field {required}")
        if notice := record.get("licenseTextPath"):
            notice_path = ROOT / "assets/grammars" / notice
            if not notice_path.is_file():
                raise SystemExit(f"{language}: missing license notice {notice}")
        target = "pinned-package"
        reference_name = record.get("package") or record.get("repository") or record["source"]
        reference = f"{reference_name}@{record['version']}:{record['module']}"
        upstream_equal = None
        if language in VSCODE_ASSETS:
            target = "vscode-built-in"
            reference = f"microsoft/vscode@{VSCODE_COMMIT}:{VSCODE_ASSETS[language]}"
            if verify_vscode:
                upstream = apply_recorded_transformation(language, upstream_vscode(VSCODE_ASSETS[language]))
                upstream_equal = canonical(grammar) == canonical(upstream)
                if not upstream_equal:
                    vscode_mismatches.append(language)
        entries.append({
            "language": language,
            "scopeName": grammar.get("scopeName"),
            "canonicalSha256": hashlib.sha256(canonical(grammar)).hexdigest(),
            "target": target,
            "reference": reference,
            "dependencies": dependencies(grammar),
            **({"upstreamEqual": upstream_equal} if upstream_equal is not None else {}),
        })

    public = {language["id"] for language in metadata["languages"]}
    missing_public = public - set(records)
    if missing_public:
        raise SystemExit(f"public languages without provenance: {sorted(missing_public)}")
    if vscode_mismatches:
        raise SystemExit(f"pinned VS Code grammar mismatch: {', '.join(vscode_mismatches)}")
    return {
        "schemaVersion": 1,
        "publicLanguages": len(public),
        "grammarAssets": len(entries),
        "shiki": {
            "version": re.search(r'^package_version = "([^"]+)"', source_text, re.M).group(1),
            "commit": re.search(r'^source_commit = "([0-9a-f]+)"', source_text, re.M).group(1),
        },
        "vscode": {"version": "1.128.0", "commit": VSCODE_COMMIT},
        "vscodeUpstreamVerified": verify_vscode,
        "assets": entries,
    }


parser = argparse.ArgumentParser()
parser.add_argument("--write", action="store_true", help="regenerate the committed audit report")
parser.add_argument("--verify-vscode", action="store_true", help="download and compare pinned VS Code built-ins")
args = parser.parse_args()
report = build_report(args.verify_vscode)
serialized = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
if args.write:
    REPORT.write_text(serialized)
elif not args.verify_vscode and REPORT.read_text() != serialized:
    raise SystemExit(f"{REPORT.relative_to(ROOT)} is stale; run {Path(__file__).name} --write")
print(f"ok: {report['grammarAssets']} grammar targets, {report['publicLanguages']} public languages")
if args.verify_vscode:
    print("ok: pinned VS Code built-in grammar assets are canonically identical")
