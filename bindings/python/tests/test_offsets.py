"""Token offsets agree with Python string slicing on random mixed-script text."""

from __future__ import annotations

import random

import pytest

import syntaxmate

HIGHLIGHTER = syntaxmate.Highlighter()
# ASCII code, Latin-1, CJK, combining marks, astral emoji and math letters,
# tabs, CR, and empty lines: every UTF-8 width and both UTF-16 widths.
PIECES = [
    "let", "x", "=", "1", ";", "(", ")", "{", "}", '"', "'", "//", "#", "<a>", "</a>",
    " ", "  ", "\t", "\n", "\n\n", "\r\n", "\r",
    "é", "ß", "ü", "中文", "字", "é", "😀", "𝒳", "🇺🇳", " ", " ",
]
LANGUAGES = ["rust", "python", "javascript", "html", "json", "markdown"]


def random_source(rng: random.Random) -> str:
    return "".join(rng.choice(PIECES) for _ in range(rng.randrange(0, 120)))


def slices(text: str, tokens: syntaxmate.Tokens) -> list[str]:
    """The text of each token, decoded from the buffer's offset unit."""
    if tokens.unit == "codepoint":
        return [text[t.start : t.end] for t in tokens]
    if tokens.unit == "utf8":
        data, width, codec = text.encode("utf-8"), 1, "utf-8"
    else:
        data, width, codec = text.encode("utf-16-le"), 2, "utf-16-le"
    return [data[t.start * width : t.end * width].decode(codec) for t in tokens]


def check(text: str, tokens: syntaxmate.Tokens) -> None:
    lines = text.split("\n")
    size = {"codepoint": len, "utf8": lambda s: len(s.encode()), "utf16": lambda s: len(s.encode("utf-16-le")) // 2}[
        tokens.unit
    ]
    # Line starts sit just after each "\n".
    expected_starts, offset = [], 0
    for line in lines:
        expected_starts.append(offset)
        offset += size(line) + 1
    assert tokens.line_starts.tolist() == expected_starts

    # Each line's tokens are ordered, contiguous, and cover exactly its text
    # (a "\r" before "\n" belongs to the line).
    ranges = tokens.line_token_ranges.tolist()
    assert len(ranges) == len(lines) + 1 and ranges[0] == 0 and ranges[-1] == len(tokens)
    texts = slices(text, tokens)
    starts, lengths = tokens.starts.tolist(), tokens.lengths.tolist()
    for number, line in enumerate(lines):
        position = expected_starts[number]
        for i in range(ranges[number], ranges[number + 1]):
            assert starts[i] == position
            position += lengths[i]
        assert "".join(texts[ranges[number] : ranges[number + 1]]) == line


@pytest.mark.parametrize("seed", range(40))
def test_offsets_match_string_slices(seed):
    rng = random.Random(seed)
    text = random_source(rng)
    language = LANGUAGES[seed % len(LANGUAGES)]
    for unit in ("codepoint", "utf8", "utf16"):
        check(text, HIGHLIGHTER.tokens(text, language, unit=unit))


@pytest.mark.parametrize("seed", range(10))
def test_session_offsets_match_line_slices(seed):
    rng = random.Random(1000 + seed)
    text = random_source(rng)
    session = HIGHLIGHTER.session(LANGUAGES[seed % len(LANGUAGES)])
    for line in text.split("\n"):
        tokens = session.line(line)
        assert tokens.line_starts.tolist() == [0]
        assert "".join(line[t.start : t.end] for t in tokens) == line


def test_views_are_native_uint32():
    tokens = HIGHLIGHTER.tokens("a = 😀\n", "python")
    for view in (tokens.starts, tokens.lengths, tokens.line_starts, tokens.line_token_ranges):
        assert view.format == "I" and view.itemsize == 4 and view.readonly
    assert tokens.starts.nbytes == 4 * len(tokens)
