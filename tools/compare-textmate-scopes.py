#!/usr/bin/env python3
"""Compare vscode-textmate oracle JSONL with Mark's native tokenize JSONL.

The oracle writes UTF-16 ``startIndex``/``endIndex`` offsets, while Mark writes
UTF-8 byte ``start``/``end`` offsets.  This tool normalizes both streams before
comparing their complete scope stacks.
"""

from __future__ import annotations

import argparse
import json
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Sequence


class ComparisonInputError(ValueError):
    """Raised when an input is not one of the supported JSONL formats."""


@dataclass(frozen=True)
class ScopeToken:
    start: int
    end: int
    scopes: tuple[str, ...]

    def as_dict(self) -> dict:
        return {"start": self.start, "end": self.end, "scopes": list(self.scopes)}


@dataclass(frozen=True)
class ScopeLine:
    line_number: int
    line: str
    tokens: tuple[ScopeToken, ...]


@dataclass(frozen=True)
class Divergence:
    line_number: int
    oracle: ScopeLine
    native: ScopeLine


@dataclass(frozen=True)
class ReorderedLine:
    line_number: int
    oracle_position: int
    native_position: int


@dataclass(frozen=True)
class Comparison:
    oracle_count: int
    native_count: int
    matching_count: int
    divergent: tuple[Divergence, ...]
    missing: tuple[int, ...]
    extra: tuple[int, ...]
    reordered: tuple[ReorderedLine, ...]

    @property
    def equal(self) -> bool:
        return not (self.divergent or self.missing or self.extra or self.reordered)

    def as_dict(self) -> dict:
        return {
            "oracleLines": self.oracle_count,
            "nativeLines": self.native_count,
            "matchingLines": self.matching_count,
            "divergentLines": [item.line_number for item in self.divergent],
            "missingLines": list(self.missing),
            "extraLines": list(self.extra),
            "reorderedLines": [
                {
                    "lineNumber": item.line_number,
                    "oraclePosition": item.oracle_position,
                    "nativePosition": item.native_position,
                }
                for item in self.reordered
            ],
            "equal": self.equal,
        }


def _integer(value: object, field: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise ComparisonInputError(f"{field} must be a non-negative integer")
    if value < 0:
        raise ComparisonInputError(f"{field} must be a non-negative integer")
    return value


def utf16_offset_to_utf8(line: str, offset: int) -> int:
    """Convert a UTF-16 code-unit offset to a UTF-8 byte offset.

    vscode-textmate commonly extends its final token through a synthetic line
    terminator.  Any offset beyond the real line is therefore clamped to the
    real UTF-8 line end.  An offset within a surrogate pair has no UTF-8
    boundary and is rejected rather than silently moving a scope boundary.
    """

    offset = _integer(offset, "UTF-16 offset")
    utf16 = 0
    utf8 = 0
    for character in line:
        if utf16 == offset:
            return utf8
        width16 = 2 if ord(character) > 0xFFFF else 1
        if utf16 < offset < utf16 + width16:
            raise ComparisonInputError(
                f"UTF-16 offset {offset} falls inside a surrogate pair"
            )
        utf16 += width16
        utf8 += len(character.encode("utf-8"))
    if offset >= utf16:
        return utf8
    # The loop handles every other case; retain a defensive error in case its
    # boundary logic changes.
    raise ComparisonInputError(f"invalid UTF-16 offset {offset}")


def _scopes(value: object, field: str) -> tuple[str, ...]:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        raise ComparisonInputError(f"{field} must be an array of strings")
    return tuple(value)


def normalize_record(record: object, *, oracle: bool) -> ScopeLine:
    """Normalize one oracle or native record into UTF-8 scope tokens."""

    if not isinstance(record, dict):
        raise ComparisonInputError("record must be a JSON object")
    line_number = _integer(record.get("lineNumber"), "lineNumber")
    line = record.get("line")
    if not isinstance(line, str):
        raise ComparisonInputError("line must be a string")
    raw_tokens = record.get("tokens")
    if not isinstance(raw_tokens, list):
        raise ComparisonInputError("tokens must be an array")

    line_end = len(line.encode("utf-8"))
    normalized: list[ScopeToken] = []
    for index, token in enumerate(raw_tokens):
        field = f"tokens[{index}]"
        if not isinstance(token, dict):
            raise ComparisonInputError(f"{field} must be an object")
        if oracle:
            start16 = _integer(token.get("startIndex"), f"{field}.startIndex")
            end16 = _integer(token.get("endIndex"), f"{field}.endIndex")
            start = utf16_offset_to_utf8(line, start16)
            end = utf16_offset_to_utf8(line, end16)
        else:
            start = min(_integer(token.get("start"), f"{field}.start"), line_end)
            end = min(_integer(token.get("end"), f"{field}.end"), line_end)
        if start > end:
            raise ComparisonInputError(
                f"{field} has reversed range {start}..{end} on line {line_number}"
            )
        if start == end:
            continue
        scopes = _scopes(token.get("scopes"), f"{field}.scopes")
        current = ScopeToken(start, end, scopes)
        if (
            normalized
            and normalized[-1].end == current.start
            and normalized[-1].scopes == current.scopes
        ):
            previous = normalized[-1]
            normalized[-1] = ScopeToken(previous.start, current.end, previous.scopes)
        else:
            normalized.append(current)
    return ScopeLine(line_number, line, tuple(normalized))


def read_jsonl(path: Path, *, oracle: bool) -> list[ScopeLine]:
    """Read and normalize a TextMate JSONL stream, rejecting duplicate lines."""

    records: list[ScopeLine] = []
    seen: set[int] = set()
    try:
        source = path.open(encoding="utf-8")
    except OSError as error:
        raise ComparisonInputError(f"{path}: {error}") from error
    with source:
        for jsonl_line, raw in enumerate(source, 1):
            if not raw.strip():
                continue
            try:
                value = json.loads(raw)
                record = normalize_record(value, oracle=oracle)
            except (json.JSONDecodeError, ComparisonInputError) as error:
                raise ComparisonInputError(f"{path}:{jsonl_line}: {error}") from error
            if record.line_number in seen:
                raise ComparisonInputError(
                    f"{path}:{jsonl_line}: duplicate lineNumber {record.line_number}"
                )
            seen.add(record.line_number)
            records.append(record)
    return records


def compare_lines(
    oracle_lines: Sequence[ScopeLine], native_lines: Sequence[ScopeLine]
) -> Comparison:
    """Compare normalized lines by lineNumber while also checking stream order."""

    oracle_by_number = {line.line_number: line for line in oracle_lines}
    native_by_number = {line.line_number: line for line in native_lines}
    if len(oracle_by_number) != len(oracle_lines):
        raise ComparisonInputError("oracle lines contain duplicate lineNumber values")
    if len(native_by_number) != len(native_lines):
        raise ComparisonInputError("native lines contain duplicate lineNumber values")

    oracle_numbers = set(oracle_by_number)
    native_numbers = set(native_by_number)
    common = oracle_numbers & native_numbers
    missing = tuple(line.line_number for line in oracle_lines if line.line_number not in common)
    extra = tuple(line.line_number for line in native_lines if line.line_number not in common)

    divergent: list[Divergence] = []
    matching = 0
    for oracle in oracle_lines:
        if oracle.line_number not in common:
            continue
        native = native_by_number[oracle.line_number]
        if oracle.line == native.line and oracle.tokens == native.tokens:
            matching += 1
        else:
            divergent.append(Divergence(oracle.line_number, oracle, native))

    # Remove missing/extra lines before order comparison so one omission does
    # not make every subsequent line look reordered.
    oracle_common = [line.line_number for line in oracle_lines if line.line_number in common]
    native_common = [line.line_number for line in native_lines if line.line_number in common]
    oracle_rank = {number: index for index, number in enumerate(oracle_common)}
    native_rank = {number: index for index, number in enumerate(native_common)}
    oracle_position = {line.line_number: index for index, line in enumerate(oracle_lines)}
    native_position = {line.line_number: index for index, line in enumerate(native_lines)}
    reordered = tuple(
        ReorderedLine(number, oracle_position[number], native_position[number])
        for number in native_common
        if oracle_rank[number] != native_rank[number]
    )

    return Comparison(
        oracle_count=len(oracle_lines),
        native_count=len(native_lines),
        matching_count=matching,
        divergent=tuple(divergent),
        missing=missing,
        extra=extra,
        reordered=reordered,
    )


def compare_files(oracle_path: Path, native_path: Path) -> Comparison:
    return compare_lines(
        read_jsonl(oracle_path, oracle=True),
        read_jsonl(native_path, oracle=False),
    )


def _numbers(values: Iterable[int], limit: int) -> str:
    values = list(values)
    shown = ", ".join(str(value) for value in values[:limit])
    if len(values) > limit:
        shown += f", ... (+{len(values) - limit})"
    return shown or "none"


def _tokens(tokens: tuple[ScopeToken, ...], width: int = 1000) -> str:
    rendered = json.dumps([token.as_dict() for token in tokens], ensure_ascii=False)
    return rendered if len(rendered) <= width else rendered[: width - 3] + "..."


def format_report(comparison: Comparison, *, max_details: int = 10) -> str:
    lines = [
        f"Compared oracle={comparison.oracle_count} native={comparison.native_count} lines",
        (
            f"Summary: matching={comparison.matching_count} "
            f"divergent={len(comparison.divergent)} missing={len(comparison.missing)} "
            f"extra={len(comparison.extra)} reordered={len(comparison.reordered)}"
        ),
    ]
    if comparison.missing:
        lines.append(f"Missing native lines: {_numbers(comparison.missing, max_details)}")
    if comparison.extra:
        lines.append(f"Extra native lines: {_numbers(comparison.extra, max_details)}")
    if comparison.reordered:
        details = [
            f"{item.line_number} ({item.oracle_position}->{item.native_position})"
            for item in comparison.reordered
        ]
        lines.append(f"Reordered native lines: {_numbers(details, max_details)}")
    if comparison.divergent:
        lines.append("Divergent line details (zero-based lineNumber):")
        for item in comparison.divergent[:max_details]:
            reasons = []
            if item.oracle.line != item.native.line:
                reasons.append("text")
            if item.oracle.tokens != item.native.tokens:
                reasons.append("scopes")
            lines.append(f"  line {item.line_number} ({' and '.join(reasons)} differ)")
            if item.oracle.line != item.native.line:
                lines.append(f"    oracle text: {item.oracle.line!r}")
                lines.append(f"    native text: {item.native.line!r}")
            if item.oracle.tokens != item.native.tokens:
                lines.append(f"    oracle: {_tokens(item.oracle.tokens)}")
                lines.append(f"    native: {_tokens(item.native.tokens)}")
        remaining = len(comparison.divergent) - max_details
        if remaining > 0:
            lines.append(f"  ... {remaining} more divergent lines")
    return "\n".join(lines)


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("oracle", type=Path, help="vscode-textmate oracle JSONL")
    parser.add_argument("native", type=Path, help="Mark tokenize JSONL")
    parser.add_argument(
        "--max-details",
        type=int,
        default=10,
        help="maximum entries shown per human-readable detail section (default: 10)",
    )
    parser.add_argument("--json", action="store_true", help="write a JSON summary")
    args = parser.parse_args(argv)
    if args.max_details < 1:
        parser.error("--max-details must be at least 1")
    return args


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        comparison = compare_files(args.oracle, args.native)
    except ComparisonInputError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if args.json:
        print(json.dumps(comparison.as_dict(), indent=2))
    else:
        print(format_report(comparison, max_details=args.max_details))
    return 0 if comparison.equal else 1


if __name__ == "__main__":
    raise SystemExit(main())
