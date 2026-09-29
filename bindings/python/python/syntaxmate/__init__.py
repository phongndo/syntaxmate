"""Fast TextMate syntax highlighting to HTML, ANSI, or flat token arrays."""

from __future__ import annotations

from ._errors import (
    InternalError,
    InvalidBundleError,
    InvalidGrammarError,
    InvalidInputError,
    InvalidThemeError,
    RenderError,
    SyntaxmateError,
    UnknownLanguageError,
    UnknownThemeError,
)
from ._native import (
    BOLD,
    DEFAULT_THEME,
    ITALIC,
    STRIKETHROUGH,
    UNDERLINE,
    Highlighter,
    Session,
    Style,
    Theme,
    Token,
    Tokens,
)
from ._native import VERSION as __version__

__all__ = [
    "BOLD",
    "DEFAULT_THEME",
    "ITALIC",
    "STRIKETHROUGH",
    "UNDERLINE",
    "Highlighter",
    "InternalError",
    "InvalidBundleError",
    "InvalidGrammarError",
    "InvalidInputError",
    "InvalidThemeError",
    "RenderError",
    "Session",
    "Style",
    "SyntaxmateError",
    "Theme",
    "Token",
    "Tokens",
    "UnknownLanguageError",
    "UnknownThemeError",
    "__version__",
]
