#!/usr/bin/env python3
"""Check local Markdown link targets and explicitly packaged documentation."""

import re
import subprocess
import sys
import tomllib
from pathlib import Path
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]


def is_documentation(path):
    # Markdown syntax samples are program input, not repository guidance.
    fixture = path.parts[:2] == ("tests", "fixtures")
    corpus = path.parts[:3] == ("benchmarks", "textmate", "corpora")
    return path.suffix == ".md" and (not (fixture or corpus) or path.name == "README.md")


def documentation_paths(root):
    result = subprocess.run(
        ["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "*.md"],
        cwd=root, check=True, capture_output=True,
    )
    return sorted({
        Path(name) for name in result.stdout.decode().split("\0")
        if name and is_documentation(Path(name)) and (root / name).is_file()
    })


def link_targets(text):
    # Ignore examples of Markdown links inside fenced code blocks.
    prose = []
    fence = None
    for line in text.splitlines():
        marker = re.match(r"^\s*(`{3,}|~{3,})(.*)$", line)
        if marker:
            run, rest = marker.groups()
            if fence is None:
                fence = run
            elif run[0] == fence[0] and len(run) >= len(fence) and not rest.strip():
                fence = None
            continue
        if fence is None:
            prose.append(line)
    text = "\n".join(prose)
    for match in re.finditer(r"\]\(\s*(?:<([^>]+)>|([^\s)]+))", text):
        yield match.group(1) or match.group(2)
    for match in re.finditer(r"(?m)^\s{0,3}\[[^\]]+\]:\s*(?:<([^>]+)>|([^\s]+))", text):
        yield match.group(1) or match.group(2)


def check_links(root, paths):
    errors = []
    root = root.resolve()
    for path in paths:
        for target in link_targets((root / path).read_text()):
            url = urlsplit(target)
            if url.scheme or url.netloc or not url.path:
                continue
            destination = ((root / path).parent / unquote(url.path)).resolve()
            if not destination.is_relative_to(root) or not destination.exists():
                errors.append(f"{path}: missing local link target {target}")
    return errors


def check_package_docs(root):
    manifest = tomllib.loads((root / "Cargo.toml").read_text())
    errors = []
    for entry in manifest["package"]["include"]:
        if entry.endswith(".md") and not any(char in entry for char in "*?["):
            if not (root / entry.lstrip("/")).is_file():
                errors.append(f"Cargo.toml: missing packaged document {entry}")
    return errors


def main():
    paths = documentation_paths(ROOT)
    errors = check_links(ROOT, paths) + check_package_docs(ROOT)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"Local link targets in {len(paths)} documents and packaged documentation are valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
