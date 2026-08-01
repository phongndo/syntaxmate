# Python stress fixture with non-ASCII text: café λ🚀.

from __future__ import annotations

import asyncio
import contextlib
import json
import re
from collections.abc import AsyncIterator, Callable, Iterable, Iterator, Mapping
from dataclasses import dataclass, field
from enum import Enum, auto
from functools import wraps
from pathlib import Path
from typing import ClassVar, Generic, Literal, ParamSpec, Protocol, TypeVar

ROUTE = re.compile(
    r"""^/api/(?P<name>[\w-]+)
    # verbose-regex comment that spans the raw triple-quoted string
    $""",
    re.VERBOSE,
)

DOC = """multi-line string
with café and "quotes"
and emoji 🚀
"""


def ratio(total: float, count: float) -> float:
    # regular comment before a division expression
    return total / count


class Greeter:
    def __init__(self, name: str = "λ") -> None:
        self.name = name

    def __str__(self) -> str:
        return f"{self.name}: {DOC.strip()} {ROUTE.match('/api/café') is not None}"


JSON_SAMPLE = r'{"city": "Zürich", "escaped": "\\u03bb"}'
BYTE_HEADER = b"MARK\x00\xff"
WINDOWS_PATH = r"C:\Users\reader\Documents\notes.txt"
QUERY = "SELECT name FROM entries WHERE symbol = '雪'"
FORMATTED = f"{Greeter('Ada')!s:^24} | {len(BYTE_HEADER)=}"


class Status(Enum):
    NEW = auto()
    RUNNING = auto()
    DONE = auto()
    FAILED = "failed"


@dataclass(frozen=True, slots=True)
class Event:
    kind: str
    payload: Mapping[str, object] = field(default_factory=dict)
    status: Status = Status.NEW
    tags: tuple[str, ...] = ()

    namespace: ClassVar[str] = "mark.syntax"

    @property
    def identifier(self) -> str:
        suffix = "-".join(self.tags) or "untagged"
        return f"{self.namespace}:{self.kind}:{suffix}"


T = TypeVar("T")
P = ParamSpec("P")


class Store(Protocol[T]):
    def get(self, key: str, default: T | None = None) -> T | None: ...

    def put(self, key: str, value: T, /) -> None: ...


class MemoryStore(Generic[T]):
    def __init__(self, initial: Mapping[str, T] | None = None) -> None:
        self._items = dict(initial or {})

    def get(self, key: str, default: T | None = None) -> T | None:
        return self._items.get(key, default)

    def put(self, key: str, value: T, /) -> None:
        self._items[key] = value

    def __iter__(self) -> Iterator[tuple[str, T]]:
        yield from self._items.items()


def traced(func: Callable[P, T]) -> Callable[P, T]:
    @wraps(func)
    def wrapper(*args: P.args, **kwargs: P.kwargs) -> T:
        print(f"calling {func.__qualname__} with {args=!r}, {kwargs=!r}")
        return func(*args, **kwargs)

    return wrapper


@traced
def normalize(words: Iterable[str], *, limit: int = 8) -> list[str]:
    cleaned = [word.casefold().strip() for word in words if word.strip()]
    return sorted({word for word in cleaned if len(word) <= limit})


def describe(subject: object) -> str:
    match subject:
        case Event(kind="error", payload={"code": int(code)}) if code >= 500:
            return f"server error {code}"
        case Event(kind=kind, tags=[first, *rest]):
            return f"{kind}: {first} (+{len(rest)})"
        case {"x": x, "y": y, **extra}:
            return f"point ({x}, {y}), metadata={extra!r}"
        case [single] | (single,):
            return f"singleton {single!r}"
        case str() as text if (length := len(text)) > 12:
            return f"long text ({length})"
        case None:
            return "nothing"
        case _:
            return "unknown"


class ParseError(ValueError):
    """Raised when an input document cannot be decoded."""


@contextlib.contextmanager
def translated_errors(source: Path) -> Iterator[None]:
    try:
        yield
    except (UnicodeError, json.JSONDecodeError) as exc:
        exc.add_note(f"while reading {source}")
        raise ParseError(source.name) from exc
    finally:
        source.exists()  # Exercise an attribute call in a finally suite.


@contextlib.asynccontextmanager
async def timer(label: str) -> AsyncIterator[Callable[[], float]]:
    loop = asyncio.get_running_loop()
    started = loop.time()
    try:
        yield lambda: loop.time() - started
    finally:
        await asyncio.sleep(0)
        print(f"{label} finished after {loop.time() - started:.3f}s")


async def event_stream(names: Iterable[str]) -> AsyncIterator[Event]:
    for index, name in enumerate(names):
        await asyncio.sleep(0)
        yield Event("message", {"index": index, "name": name}, tags=("α", "🌌"))


async def collect_names(names: Iterable[str]) -> dict[str, int]:
    async with timer("collection") as elapsed:
        events = [event async for event in event_stream(names)]
        lengths = {
            str(event.payload["name"]): len(str(event.payload["name"]))
            for event in events
        }
    print(f"collected {len(lengths)} names in {elapsed():.6f}s")
    return lengths


def parse_integer(text: str, base: Literal[2, 8, 10, 16] = 10) -> int:
    try:
        return int(text.replace("_", ""), base)
    except ValueError as exc:
        raise ParseError(f"invalid base-{base} integer: {text!r}") from exc


if __name__ == "__main__":
    sample = normalize([" Café ", "PYTHON", "λ", "café", "東京"])
    matrix = {(row, column): row * column for row in range(3) for column in range(3)}
    print(sample, matrix, describe(Event("ready", tags=("β", "🛰️"))))
