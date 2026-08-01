#!/usr/bin/env python3
"""Validate vendored TextMate theme manifests, hashes, and JSON shape."""
from __future__ import annotations

import hashlib
import json
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parent.parent
ASSETS = ROOT / "assets" / "themes"
LICENSE_FILES = {
    "github-vscode-themes": "github-vscode-themes.license",
    "@shikijs/themes": "shiki-themes.license",
    "nordic.nvim": "nordic.nvim.license",
    "vscode-dark-molokai-theme": "vscode-dark-molokai-theme.license",
    "zenbones.nvim": "zenbones.nvim.license",
    "token": "token.license",
    "gruvbox-material-vscode": "gruvbox-material-vscode.license",
    "mfd.nvim": "mfd.nvim.license",
}


def main() -> None:
    manifest = (ASSETS / "SOURCE.toml").read_text()
    declared_packages = set(re.findall(r'^package\s*=\s*"([^"]+)"', manifest, re.MULTILINE))
    if declared_packages != set(LICENSE_FILES):
        raise SystemExit(
            "theme license inventory mismatch: "
            f"declared={sorted(declared_packages)} licensed={sorted(LICENSE_FILES)}"
        )
    for package, filename in LICENSE_FILES.items():
        path = ASSETS / "licenses" / filename
        if not path.is_file() or not path.read_text().strip():
            raise SystemExit(f"{package}: missing license text at {path}")
    attribution = ASSETS / "licenses" / "theme-attributions.notice"
    if not attribution.is_file() or not attribution.read_text().strip():
        raise SystemExit(f"missing theme attribution notice at {attribution}")

    declared: dict[str, str] = {}
    for block in manifest.split("[[source.theme]]")[1:]:
        theme_id = re.search(r'^id\s*=\s*"([^"]+)"', block, re.MULTILINE)
        sha256 = re.search(r'^sha256\s*=\s*"([0-9a-f]{64})"', block, re.MULTILINE)
        if not theme_id or not sha256:
            raise SystemExit("SOURCE.toml has an incomplete [[source.theme]] block")
        declared[theme_id.group(1)] = sha256.group(1)
    files = {path.stem: path for path in ASSETS.glob("*.json") if path.name != "licenses.json"}
    if set(declared) != set(files):
        raise SystemExit(f"theme manifest mismatch: declared={sorted(declared)} files={sorted(files)}")
    for theme_id, path in sorted(files.items()):
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        if digest != declared[theme_id]:
            raise SystemExit(f"{path}: sha256 {digest} != {declared[theme_id]}")
        parsed = json.loads(data)
        if not isinstance(parsed.get("tokenColors"), list) or not parsed["tokenColors"]:
            raise SystemExit(f"{path}: missing tokenColors")
        if not isinstance(parsed.get("colors", {}).get("editor.foreground"), str):
            raise SystemExit(f"{path}: missing editor.foreground")
        if not isinstance(parsed.get("colors", {}).get("editor.background"), str):
            raise SystemExit(f"{path}: missing editor.background")
    print(f"ok: {len(files)} vendored TextMate themes")


if __name__ == "__main__":
    main()
