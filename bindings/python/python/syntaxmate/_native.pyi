import os
from collections.abc import Iterator, Mapping
from typing import Any, Literal, Optional, Tuple, Union, final

__all__ = [
    "Highlighter",
    "Session",
    "Style",
    "Theme",
    "Token",
    "Tokens",
    "BOLD",
    "ITALIC",
    "UNDERLINE",
    "STRIKETHROUGH",
    "DEFAULT_THEME",
    "VERSION",
]

_OffsetUnit = Literal["codepoint", "utf8", "utf16"]
_ThemeLike = Union[str, "Theme"]

BOLD: int
ITALIC: int
UNDERLINE: int
STRIKETHROUGH: int
DEFAULT_THEME: str
VERSION: str

@final
class Style:
    """A resolved style; colors are ``0xRRGGBB`` integers or ``None``."""

    @property
    def foreground(self) -> Optional[int]: ...
    @property
    def background(self) -> Optional[int]: ...
    @property
    def modifiers(self) -> int:
        """Bitset of ``BOLD``, ``ITALIC``, ``UNDERLINE``, and ``STRIKETHROUGH``."""
    @property
    def bold(self) -> bool: ...
    @property
    def italic(self) -> bool: ...
    @property
    def underline(self) -> bool: ...
    @property
    def strikethrough(self) -> bool: ...
    def __eq__(self, other: object, /) -> bool: ...
    def __hash__(self) -> int: ...

@final
class Theme:
    """A bundled or custom TextMate theme."""

    @staticmethod
    def bundled(name: str) -> Theme: ...
    @staticmethod
    def from_json(data: Union[str, bytes, bytearray, Mapping[str, Any]]) -> Theme:
        """Parses a TextMate JSON theme from text, UTF-8 bytes, or a mapping."""
    @property
    def name(self) -> str: ...
    @property
    def default_style(self) -> Style: ...
    def stylesheet(self, class_prefix: str) -> str:
        """CSS for HTML rendered with the same ``class_prefix``."""

@final
class Token:
    """One token; ``start``/``end`` are offsets in the buffer's unit."""

    @property
    def start(self) -> int: ...
    @property
    def end(self) -> int: ...
    @property
    def style(self) -> Style: ...
    @property
    def scopes(self) -> Optional[Tuple[str, ...]]:
        """Scope stack, outermost first; ``None`` unless scopes were requested."""

@final
class Tokens:
    """Highlighted tokens as flat arrays.

    Array properties are read-only ``memoryview``s of unsigned 32-bit integers
    (format ``"I"``). Token ``i`` spans ``starts[i]:starts[i] + lengths[i]``;
    tokens of line ``l`` are ``line_token_ranges[l]:line_token_ranges[l + 1]``.
    With the default ``"codepoint"`` unit, offsets index the source ``str``.
    """

    @property
    def unit(self) -> _OffsetUnit: ...
    @property
    def complete(self) -> bool:
        """Whether tokenization finished within resource limits."""
    @property
    def default_style(self) -> Style: ...
    @property
    def line_starts(self) -> memoryview: ...
    @property
    def line_token_ranges(self) -> memoryview: ...
    @property
    def starts(self) -> memoryview: ...
    @property
    def lengths(self) -> memoryview: ...
    @property
    def style_ids(self) -> memoryview: ...
    @property
    def scope_ids(self) -> memoryview:
        """Index into ``scope_stacks`` per token; empty unless scopes were requested."""
    @property
    def styles(self) -> Tuple[Style, ...]: ...
    @property
    def scope_stacks(self) -> Tuple[Tuple[str, ...], ...]: ...
    def __len__(self) -> int: ...
    def __getitem__(self, index: int, /) -> Token: ...
    def __iter__(self) -> Iterator[Token]: ...

@final
class Session:
    """Incremental highlighting: feed logical lines in order, without ``\\n``."""

    def line(self, text: str) -> Tokens:
        """Highlights the next line; offsets are line-relative."""
    def reset(self) -> None:
        """Returns to the start-of-document state, keeping caches."""

@final
class Highlighter:
    """Thread-safe highlighter; calls release the GIL while highlighting."""

    def __init__(self) -> None:
        """Creates a highlighter over the embedded grammar catalog."""
    @staticmethod
    def from_bundle(data: Union[bytes, bytearray]) -> Highlighter:
        """Creates a highlighter from grammar-bundle bytes."""
    def languages(self) -> list[str]: ...
    def themes(self) -> list[str]: ...
    def canonical_language(self, language: str) -> Optional[str]: ...
    def detect(
        self, source: str = "", *, path: Union[str, os.PathLike[str], None] = None
    ) -> Optional[str]: ...
    def html(
        self,
        code: str,
        language: str,
        theme: Optional[_ThemeLike] = None,
        *,
        include_wrapper: bool = True,
        wrapper_class: Optional[str] = "syntaxmate",
        include_scopes: bool = False,
        class_prefix: Optional[str] = None,
    ) -> str: ...
    def ansi(
        self,
        code: str,
        language: str,
        theme: Optional[_ThemeLike] = None,
        *,
        colors: bool = True,
        sanitize_control_characters: bool = True,
        include_default_background: bool = False,
    ) -> str: ...
    def tokens(
        self,
        code: str,
        language: str,
        theme: Optional[_ThemeLike] = None,
        *,
        include_scopes: bool = False,
        unit: _OffsetUnit = "codepoint",
    ) -> Tokens: ...
    def session(
        self,
        language: str,
        theme: Optional[_ThemeLike] = None,
        *,
        include_scopes: bool = False,
        unit: _OffsetUnit = "codepoint",
    ) -> Session: ...
