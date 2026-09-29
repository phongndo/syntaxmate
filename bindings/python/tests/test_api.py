from __future__ import annotations

from pathlib import Path
from types import MappingProxyType

import pytest

import syntaxmate
from syntaxmate import Highlighter, Theme

ROOT = Path(__file__).resolve().parents[3]
SOURCE = "// café ☕\nconst s = \"😀 𝒳\";\r\nlet t = 1;\n"


@pytest.fixture(scope="module")
def hl() -> Highlighter:
    return Highlighter()


def test_catalog(hl):
    assert "rust" in hl.languages()
    assert "github-dark" in hl.themes()
    assert syntaxmate.DEFAULT_THEME in hl.themes()
    assert hl.canonical_language("py") == "python"
    assert hl.canonical_language("no-such-language") is None
    assert syntaxmate.__version__ == "0.2.0"


def test_detect(hl):
    assert hl.detect(path="src/main.rs") == "rust"
    assert hl.detect(path=Path("setup.py")) == "python"
    assert hl.detect("#!/usr/bin/env python3\nprint(1)\n") == "python"
    assert hl.detect("plain words") is None


def test_theme_argument_forms(hl):
    by_name = hl.html("let x = 1;", "rust", "github-light")
    assert by_name == hl.html("let x = 1;", "rust", Theme.bundled("github-light"))
    assert hl.html("let x = 1;", "rust") == hl.html("let x = 1;", "rust", "github-dark")
    with pytest.raises(TypeError):
        hl.html("x", "rust", 42)


def test_theme_from_json_forms():
    data = {"name": "T", "tokenColors": [{"scope": "keyword", "settings": {"foreground": "#ff0000"}}]}
    import json

    for form in (data, json.dumps(data), json.dumps(data).encode()):
        theme = Theme.from_json(form)
        assert theme.name == "T"
    # Non-dict mappings are accepted at any depth.
    settings = MappingProxyType({"foreground": "#ff0000"})
    rule = MappingProxyType({"scope": "keyword", "settings": settings})
    frozen = MappingProxyType({"name": "T", "tokenColors": (rule,)})
    assert Theme.from_json(frozen).name == "T"
    assert Theme.from_json(frozen).stylesheet("sm") == Theme.from_json(data).stylesheet("sm")
    with pytest.raises(TypeError):
        Theme.from_json({"name": "T", "tokenColors": [object()]})
    assert "sm-" in Theme.bundled("github-dark").stylesheet("sm")
    assert Theme.bundled("github-dark").default_style.background == 0x0D1117


def test_html_options(hl):
    bare = hl.html("<a>", "html", include_wrapper=False)
    assert not bare.startswith("<pre")
    assert "&lt;" in bare
    assert 'class="x"' in hl.html("1", "rust", wrapper_class="x")
    assert "data-scopes" in hl.html("fn", "rust", include_scopes=True)


def test_ansi_options(hl):
    assert "\x1b[" not in hl.ansi("let x = 1;", "rust", colors=False)
    assert "␛" in hl.ansi("a\x1bb", "rust", colors=False)
    assert "\x1b" in hl.ansi("a\x1bb", "rust", colors=False, sanitize_control_characters=False)


def test_tokens_index_python_strings(hl):
    tokens = hl.tokens(SOURCE, "javascript", include_scopes=True)
    assert tokens.unit == "codepoint"
    assert len(tokens) == len(tokens.starts) == len(tokens.lengths) == len(tokens.scope_ids)
    assert tokens.starts.format == "I" and tokens.starts.readonly
    text = "".join(SOURCE[t.start : t.end] for t in tokens)
    assert text == SOURCE.replace("\n", "")
    assert all(t.scopes and t.scopes[0] == "source.js" for t in tokens)
    assert tokens[-1].end == tokens.starts[-1] + tokens.lengths[-1]
    with pytest.raises(IndexError):
        tokens[len(tokens)]
    lines = SOURCE.split("\n")
    for line, start in enumerate(tokens.line_starts):
        assert SOURCE[start:].startswith(lines[line])
    assert tokens.styles[tokens.style_ids[0]] == tokens[0].style
    assert tokens.starts.obj is tokens.starts.obj  # backing bytes cached, not rebuilt


def test_released_array_view_does_not_poison_later_access(hl):
    tokens = hl.tokens("let x = 1;", "rust")
    with tokens.starts as starts:
        expected = starts.tolist()
    assert tokens.starts.tolist() == expected
    assert tokens.starts[0] == 0
    view = tokens.lengths
    view.release()
    assert len(tokens.lengths) == len(tokens)


def test_tokens_without_scopes(hl):
    tokens = hl.tokens("fn main() {}", "rust")
    assert len(tokens.scope_ids) == 0 and tokens.scope_stacks == ()
    assert all(t.scopes is None for t in tokens)
    assert hl.tokens("", "rust").line_starts.tolist() == [0]


def test_session(hl):
    session = hl.session("python", "github-dark")
    first = session.line('s = """doc')
    inside = session.line("still doc")
    session.reset()
    fresh = session.line("still doc")
    assert first.line_starts.tolist() == [0]
    assert inside.styles != fresh.styles or inside.style_ids.tolist() != fresh.style_ids.tolist()
    with pytest.raises(syntaxmate.InvalidInputError):
        session.line("two\nlines")


def test_errors(hl):
    with pytest.raises(syntaxmate.UnknownLanguageError) as info:
        hl.html("x", "no-such-language")
    assert info.value.kind == "unknown_language"
    assert isinstance(info.value, (syntaxmate.SyntaxmateError, LookupError))
    with pytest.raises(syntaxmate.UnknownThemeError):
        hl.html("x", "rust", "no-such-theme")
    with pytest.raises(syntaxmate.UnknownThemeError):
        Theme.bundled("no-such-theme")
    with pytest.raises(syntaxmate.InvalidThemeError) as info:
        Theme.from_json("{not json")
    assert isinstance(info.value, ValueError)
    with pytest.raises(syntaxmate.InvalidBundleError):
        Highlighter.from_bundle(b"not a bundle")
    with pytest.raises(ValueError):
        hl.tokens("x", "rust", unit="bytes")


def test_from_bundle_subset():
    data = (ROOT / "tests/fixtures/bundles/json.bundle").read_bytes()
    subset = Highlighter.from_bundle(data)
    assert "json" in subset.languages()
    assert "rust" not in subset.languages()
    html = subset.html('{"a": 1}', "json")
    assert html == Highlighter().html('{"a": 1}', "json")
    with pytest.raises(syntaxmate.UnknownLanguageError):
        subset.html("fn x() {}", "rust")


def test_tokens_indexing_edge_cases(hl):
    tokens = hl.tokens("let x = 1;", "rust")
    assert tokens[-len(tokens)].start == 0
    for index in (len(tokens), -len(tokens) - 1, 2**70, -(2**70)):
        with pytest.raises(IndexError):
            tokens[index]
    with pytest.raises(TypeError):
        tokens[0:1]
    assert [t.start for t in reversed(tokens)] == tokens.starts.tolist()[::-1]
    iterator = iter(tokens)
    assert len(list(iterator)) == len(tokens) and next(iterator, None) is None
    empty = hl.tokens("", "rust")
    assert len(empty) == 0 and not empty and list(empty) == []
    assert empty.line_token_ranges.tolist() == [0, 0]
    assert repr(empty) == "<Tokens tokens=0 lines=1 unit='codepoint' complete=True>"


def test_strings_must_be_utf8_encodable(hl):
    # A lone surrogate cannot cross into Rust; it raises the standard
    # UnicodeEncodeError (a ValueError) wherever a str is accepted.
    lone = "a\ud800b"
    calls = [
        lambda: hl.html(lone, "rust"),
        lambda: hl.ansi(lone, "rust"),
        lambda: hl.tokens(lone, "rust"),
        lambda: hl.html("x", lone),
        lambda: hl.html("x", "rust", lone),
        lambda: hl.session("rust").line(lone),
        lambda: Theme.bundled(lone),
        lambda: Theme.from_json(lone),
    ]
    for call in calls:
        with pytest.raises(UnicodeEncodeError):
            call()
    with pytest.raises(TypeError):
        hl.html(b"fn main() {}", "rust")


def test_exceptions_pickle():
    import pickle

    try:
        Highlighter().html("x", "no-such-language")
    except syntaxmate.UnknownLanguageError as error:
        copy = pickle.loads(pickle.dumps(error))
        assert type(copy) is syntaxmate.UnknownLanguageError
        assert copy.args == error.args and copy.kind == "unknown_language"
    else:
        pytest.fail("expected UnknownLanguageError")
