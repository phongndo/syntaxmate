#!/usr/bin/env python3
"""Regenerate Mark's TextMate adaptations from pinned Zenbones Vim themes."""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "assets" / "themes"
COMMIT = "8304d8df9b823ff11e103afa62f38c39f534abe6"
BASE_URL = f"https://raw.githubusercontent.com/zenbones-theme/zenbones.nvim/{COMMIT}/colors"
VARIANTS = {
    "zenbones-dark": ("zenbones", "dark"),
    "zenbones-light": ("zenbones", "light"),
    "duckbones": ("duckbones", "dark"),
    "forestbones-dark": ("forestbones", "dark"),
    "forestbones-light": ("forestbones", "light"),
    "kanagawabones": ("kanagawabones", "dark"),
    "neobones-dark": ("neobones", "dark"),
    "neobones-light": ("neobones", "light"),
    "nordbones": ("nordbones", "dark"),
    "rosebones-dark": ("rosebones", "dark"),
    "rosebones-light": ("rosebones", "light"),
    "seoulbones-dark": ("seoulbones", "dark"),
    "seoulbones-light": ("seoulbones", "light"),
    "tokyobones-dark": ("tokyobones", "dark"),
    "tokyobones-light": ("tokyobones", "light"),
    "vimbones": ("vimbones", "light"),
    "zenburned": ("zenburned", "dark"),
    "zenwritten-dark": ("zenwritten", "dark"),
    "zenwritten-light": ("zenwritten", "light"),
}
HIGHLIGHT = re.compile(
    r"^\s*highlight\s+(?P<group>\S+)\s+"
    r"guifg=(?P<fg>\S+)\s+guibg=(?P<bg>\S+)\s+guisp=\S+\s+gui=(?P<style>\S+)"
)

# TextMate scopes corresponding to Vim's canonical highlight groups. The
# generated Vim files already contain the fully resolved upstream palette.
SCOPES = {
    "Comment": ["comment"],
    "Constant": ["constant", "variable.other.constant"],
    "Boolean": ["constant.language.boolean"],
    "String": ["string"],
    "Number": ["constant.numeric"],
    "Identifier": ["variable", "support.variable"],
    "Function": ["entity.name.function", "support.function"],
    "Statement": ["keyword", "storage", "keyword.operator"],
    "PreProc": ["meta.preprocessor", "keyword.control.import"],
    "Type": ["entity.name.type", "entity.name.class", "support.type", "support.class"],
    "Special": ["constant.character.escape", "support.constant", "support.type.property-name"],
    "Delimiter": ["punctuation"],
    "Added": ["markup.inserted"],
    "Removed": ["markup.deleted"],
    "Changed": ["markup.changed"],
}
UI_GROUPS = {
    "Cursor": "editorCursor.foreground",
    "CursorLine": "editor.lineHighlightBackground",
    "Search": "editor.findMatchBackground",
    "StatusLine": "statusBar.background",
    "DiffAdd": "diffEditor.insertedLineBackground",
    "DiffDelete": "diffEditor.removedLineBackground",
    "Added": "gitDecoration.addedResourceForeground",
    "Removed": "gitDecoration.deletedResourceForeground",
    "Changed": "gitDecoration.modifiedResourceForeground",
}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    sources: dict[str, str] = {}
    changed: list[str] = []
    for theme_id, (family, mode) in VARIANTS.items():
        source = sources.get(family)
        if source is None:
            with urllib.request.urlopen(f"{BASE_URL}/{family}.vim") as response:
                source = response.read().decode()
            sources[family] = source
        generated = render_theme(theme_id, family, mode, source)
        destination = OUTPUT / f"{theme_id}.json"
        if not destination.exists() or destination.read_text() != generated:
            changed.append(str(destination.relative_to(ROOT)))
            if not args.check:
                destination.write_text(generated)
    if args.check and changed:
        raise SystemExit("stale Zenbones themes:\n  " + "\n  ".join(changed))
    action = "checked" if args.check else "generated"
    print(f"{action} {len(VARIANTS)} Zenbones themes from {COMMIT}")


def render_theme(theme_id: str, family: str, mode: str, source: str) -> str:
    section = extract_section(source, mode)
    groups = parse_highlights(section)
    normal = require(groups, "Normal", theme_id)
    colors = {
        "editor.background": require_color(normal[1], "Normal background", theme_id),
        "editor.foreground": require_color(normal[0], "Normal foreground", theme_id),
    }
    for group, key in UI_GROUPS.items():
        style = groups.get(group)
        if not style:
            continue
        color = style[1] if group in {"CursorLine", "Search", "StatusLine", "DiffAdd", "DiffDelete"} else style[0]
        if group == "Cursor":
            color = style[1]
        if color != "NONE":
            colors[key] = color

    token_colors = []
    for group, scopes in SCOPES.items():
        style = groups.get(group)
        if not style:
            # Vim's standard fallback links these groups to their canonical
            # parents after :highlight clear.
            fallback = {"Boolean": "Constant", "PreProc": "Statement"}.get(group)
            style = groups.get(fallback) if fallback else None
        if not style:
            continue
        fg, _, modifiers = style
        settings: dict[str, str] = {"fontStyle": modifiers}
        if fg != "NONE":
            settings["foreground"] = fg
        token_colors.append({"name": f"Vim {group}", "scope": scopes, "settings": settings})

    document = {
        "name": theme_id.replace("-", " ").title(),
        "type": mode,
        "colors": colors,
        "tokenColors": token_colors,
        "markSource": {
            "repository": "https://github.com/zenbones-theme/zenbones.nvim",
            "commit": COMMIT,
            "path": f"colors/{family}.vim",
            "background": mode,
        },
    }
    return json.dumps(document, indent=2) + "\n"


def extract_section(source: str, mode: str) -> str:
    match = re.search(
        rf'^\s*" {mode} start\s*$\n(?P<body>.*?)^\s*" {mode} end\s*$',
        source,
        re.MULTILINE | re.DOTALL,
    )
    if not match:
        raise ValueError(f"missing {mode} section")
    return match.group("body")


def parse_highlights(section: str) -> dict[str, tuple[str, str, str]]:
    groups: dict[str, tuple[str, str, str]] = {}
    for line in section.splitlines():
        match = HIGHLIGHT.match(line)
        if not match:
            continue
        style = match.group("style")
        modifiers = " ".join(
            item
            for item in style.split(",")
            if item in {"bold", "italic", "underline", "strikethrough"}
        )
        groups[match.group("group")] = (
            match.group("fg").upper(),
            match.group("bg").upper(),
            modifiers,
        )
    return groups


def require(groups: dict[str, tuple[str, str, str]], group: str, theme: str) -> tuple[str, str, str]:
    if group not in groups:
        raise ValueError(f"{theme}: missing {group}")
    return groups[group]


def require_color(color: str, label: str, theme: str) -> str:
    if color == "NONE":
        raise ValueError(f"{theme}: missing {label}")
    return color


if __name__ == "__main__":
    main()
