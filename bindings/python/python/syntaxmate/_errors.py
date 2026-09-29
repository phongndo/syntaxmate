"""Exception hierarchy; each class maps to one stable boundary error kind."""

from __future__ import annotations

__all__ = [
    "SyntaxmateError",
    "UnknownLanguageError",
    "UnknownThemeError",
    "InvalidGrammarError",
    "InvalidThemeError",
    "InvalidBundleError",
    "InvalidInputError",
    "RenderError",
    "InternalError",
]


class SyntaxmateError(Exception):
    """Base class for Syntaxmate errors. ``kind`` names the stable category."""

    kind: str = "internal"


class UnknownLanguageError(SyntaxmateError, LookupError):
    """The language ID or alias is not in the catalog."""

    kind = "unknown_language"


class UnknownThemeError(SyntaxmateError, LookupError):
    """The bundled theme name is unknown."""

    kind = "unknown_theme"


class InvalidGrammarError(SyntaxmateError, ValueError):
    """A grammar could not be parsed or prepared."""

    kind = "invalid_grammar"


class InvalidThemeError(SyntaxmateError, ValueError):
    """A custom theme could not be parsed."""

    kind = "invalid_theme"


class InvalidBundleError(SyntaxmateError, ValueError):
    """A grammar bundle could not be decoded."""

    kind = "invalid_bundle"


class InvalidInputError(SyntaxmateError, ValueError):
    """Input was rejected, such as a session line containing a newline."""

    kind = "invalid_input"


class RenderError(SyntaxmateError):
    """Output rendering failed."""

    kind = "render"


class InternalError(SyntaxmateError):
    """An unexpected internal failure, including a caught Rust panic."""

    kind = "internal"
