#!/usr/bin/env python3
"""Profile one real Python consumer operation; emit one JSON sample.

Run separate processes in alternating baseline/candidate order. Import, engine
construction and first call have separate timers. A parent can subtract its
pre-spawn time.time_ns() from firstFinishedUnixNs for time to first result,
including interpreter startup, argument parsing, and input loading. Warm
intervals exclude validation/digest construction. `changed`
cycles through more than 1024 unique line texts, including formerly blank lines,
to defeat the engine's line-result cache. Padding is part of the measured input,
not a claim that the padded source has identical grammar semantics.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import time
from pathlib import Path


def packed(style):
    return [style.foreground, style.background, style.modifiers]


def canonical(tokens):
    assert tokens.complete, "degraded tokenization"
    return {
        "unit": tokens.unit,
        "arrays": [getattr(tokens, field).tolist() for field in
                   ("line_starts", "line_token_ranges", "starts", "lengths",
                    "style_ids", "scope_ids")],
        "styles": [packed(style) for style in tokens.styles],
        "defaultStyle": packed(tokens.default_style),
        "scopes": tokens.scope_stacks,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("file", type=Path)
    parser.add_argument("--language", default="rust")
    parser.add_argument("--operation", choices=("html", "ansi", "tokens", "arrays", "iterate", "session"), default="tokens")
    parser.add_argument("--phase", choices=("replay", "changed"), default="replay")
    parser.add_argument("--unit", choices=("utf8", "utf16", "codepoint"), default="codepoint")
    parser.add_argument("--scopes", action="store_true")
    parser.add_argument("--minimum-ms", type=float, default=100)
    args = parser.parse_args()
    if args.minimum_ms <= 0:
        parser.error("--minimum-ms must be positive")
    source = args.file.read_bytes().decode("utf-8")
    import_started = time.perf_counter_ns()
    import syntaxmate
    imported = time.perf_counter_ns()
    highlighter = syntaxmate.Highlighter()
    constructed = time.perf_counter_ns()
    theme = syntaxmate.Theme.bundled("github-dark")
    themed = time.perf_counter_ns()
    options = dict(include_scopes=args.scopes, unit=args.unit)
    session = None

    def operation(text):
        nonlocal session
        if args.operation in ("html", "ansi"):
            return getattr(highlighter, args.operation)(text, args.language, theme)
        if args.operation == "session":
            if session is None:
                session = highlighter.session(args.language, theme, **options)
            session.reset()
            return [session.line(line) for line in text.split("\n")]
        tokens = highlighter.tokens(text, args.language, theme, **options)
        if args.operation == "arrays":
            return tokens, [tokens.starts, tokens.lengths, tokens.style_ids,
                            tokens.line_starts, tokens.line_token_ranges, tokens.scope_ids]
        if args.operation == "iterate":
            return tokens, list(tokens)
        return tokens

    # Validate every input outside measurement. Rendered APIs return only str;
    # a token pass verifies completion for the same source and grammar.
    def validate(text):
        reference = highlighter.tokens(text, args.language, theme, **options)
        data = canonical(reference)
        result = operation(text)
        if args.operation in ("html", "ansi"):
            return [data, result]
        if args.operation == "session":
            return [data, [canonical(line) for line in result]]
        if args.operation == "arrays":
            assert [view.tolist() for view in result[1]] == [
                getattr(reference, field).tolist() for field in
                ("starts", "lengths", "style_ids", "line_starts", "line_token_ranges", "scope_ids")]
            result = result[0]
        if args.operation == "iterate":
            assert len(result[1]) == len(reference)
            for index, token in enumerate(result[1]):
                assert token.start == reference.starts[index]
                assert token.end == token.start + reference.lengths[index]
                assert token.style == reference.styles[reference.style_ids[index]]
                expected_scopes = (reference.scope_stacks[reference.scope_ids[index]]
                                   if args.scopes else None)
                assert token.scopes == expected_scopes
            result = result[0]
        assert canonical(result) == data
        return data

    started_first = time.perf_counter_ns()
    first = operation(source)
    first_ns = time.perf_counter_ns() - started_first
    first_finished_unix_ns = time.time_ns()
    del first
    lines = source.split("\n")
    if args.phase == "changed":
        count = max(2, 1024 // len(lines) + 1)
        docs = ["\n".join(line + " " + format(i * len(lines) + j, "032b").translate(
                    str.maketrans("01", " \t")) for j, line in enumerate(lines))
                for i in range(count)]
        # Fixed-width unique suffixes prevent duplicates within/across documents.
        assert len({line for doc in docs for line in doc.split("\n")}) > 1024
    else:
        docs = [source]
    digest = hashlib.sha256()
    for doc in docs:
        digest.update(json.dumps(validate(doc), sort_keys=True, ensure_ascii=True).encode())
    count = 1
    next_document = 0
    while True:
        started = time.perf_counter_ns()
        for _ in range(count):
            result = operation(docs[next_document % len(docs)])
            next_document += 1
            del result
        elapsed = time.perf_counter_ns() - started
        if elapsed >= args.minimum_ms * 1e6:
            break
        count *= 2
    print(json.dumps({
        "python": platform.python_version(), "syntaxmate": syntaxmate.__version__,
        "file": str(args.file), "sourceSha256": hashlib.sha256(source.encode()).hexdigest(),
        "sourceBytes": len(source.encode()), "operation": args.operation,
        "phase": args.phase, "unit": args.unit, "scopes": args.scopes,
        "importNs": imported - import_started, "constructionNs": constructed - imported,
        "themeNs": themed - constructed, "firstNs": first_ns,
        "firstFinishedUnixNs": first_finished_unix_ns,
        "iterations": count, "elapsedNs": elapsed, "nsPerCall": elapsed / count,
        "inputVariants": len(docs), "complete": True, "digest": digest.hexdigest(),
    }))


if __name__ == "__main__":
    main()
