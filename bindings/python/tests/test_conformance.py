"""Runs every shared conformance case through the Python API."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

import syntaxmate

FIXTURES = Path(__file__).resolve().parents[2] / "conformance"
CASES = json.loads((FIXTURES / "cases.json").read_text(encoding="utf-8"))
EXPECTED = json.loads((FIXTURES / "expected.json").read_text(encoding="utf-8"))
NO_COLOR = 0xFFFFFFFF

HIGHLIGHTER = syntaxmate.Highlighter()
CUSTOM_THEME = syntaxmate.Theme.from_json(CASES["customTheme"])


def packed(style: syntaxmate.Style) -> list[int]:
    def color(value: int | None) -> int:
        return NO_COLOR if value is None else value

    return [color(style.foreground), color(style.background), style.modifiers]


def buffer(tokens: syntaxmate.Tokens) -> dict:
    return {
        "complete": tokens.complete,
        "lineStarts": tokens.line_starts.tolist(),
        "lineTokenRanges": tokens.line_token_ranges.tolist(),
        "tokenStarts": tokens.starts.tolist(),
        "tokenLengths": tokens.lengths.tolist(),
        "tokenStyles": tokens.style_ids.tolist(),
        "styles": [packed(style) for style in tokens.styles],
        "defaultStyle": packed(tokens.default_style),
    }


@pytest.fixture(params=CASES["cases"], ids=lambda case: case["name"])
def case(request):
    case = request.param
    theme = CUSTOM_THEME if case["theme"] is None else syntaxmate.Theme.bundled(case["theme"])
    return case, theme, EXPECTED[case["name"]]


def test_expected_covers_every_case():
    assert sorted(EXPECTED) == sorted(case["name"] for case in CASES["cases"])


def test_html(case):
    case, theme, expected = case
    assert HIGHLIGHTER.html(case["source"], case["language"], theme) == expected["html"]


def test_html_classes(case):
    case, theme, expected = case
    html = HIGHLIGHTER.html(case["source"], case["language"], theme, class_prefix="sm")
    assert html == expected["htmlClasses"]


def test_ansi(case):
    case, theme, expected = case
    assert HIGHLIGHTER.ansi(case["source"], case["language"], theme) == expected["ansi"]


@pytest.mark.parametrize(
    ("unit", "key"), [("codepoint", "codePoint"), ("utf8", "utf8"), ("utf16", "utf16")]
)
def test_tokens(case, unit, key):
    case, theme, expected = case
    tokens = HIGHLIGHTER.tokens(case["source"], case["language"], theme, unit=unit)
    assert tokens.unit == unit
    assert buffer(tokens) == expected["tokens"][key]


def test_scopes(case):
    case, theme, expected = case
    tokens = HIGHLIGHTER.tokens(
        case["source"], case["language"], theme, include_scopes=True, unit="utf8"
    )
    assert tokens.scope_ids.tolist() == expected["scopes"]["tokenScopes"]
    assert [list(stack) for stack in tokens.scope_stacks] == expected["scopes"]["scopeStacks"]


def test_session(case):
    case, theme, expected = case
    session = HIGHLIGHTER.session(case["language"], theme, unit="utf16")
    lines = [buffer(session.line(line)) for line in case["source"].split("\n")]
    assert lines == expected["session"]
