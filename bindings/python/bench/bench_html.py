#!/usr/bin/env python3
"""Compare HTML highlighting throughput of the syntaxmate binding and Pygments.

Sources are the stress fixtures used by the repository's competitive benchmarks
(tests/fixtures/textmate/<language>/stress.*). Each product uses its own
grammars and a GitHub Dark theme with inline styles, so this is a product
comparison, not an identical-output one.

Modes (median of --samples):
  cold     fresh interpreter: import, construct, first highlight (in-process timer)
  first    fresh highlighter in a warm interpreter: grammar setup + one document
  steady   warm highlighter, rotating variants to displace cached documents
  replay   warm highlighter, the same document again (syntaxmate's line cache hits)
"""

from __future__ import annotations

import argparse
import json
import platform
import statistics
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FIXTURES = ROOT / "tests/fixtures/textmate"
# syntaxmate language ID -> (fixture file, Pygments lexer name)
LANGUAGES = {
    "python": ("python/stress.py", "python"),
    "rust": ("rust/stress.rs", "rust"),
    "javascript": ("javascript/stress.js", "javascript"),
    "cpp": ("cpp/stress.cpp", "cpp"),
    "json": ("json/stress.json", "json"),
    "html": ("html/stress.html", "html"),
}
THEME = "github-dark"
# More variant lines than the cache holds for these stress fixtures; duplicate
# lines within a document may still hit. This is not cache-disabled matching.
VARIANTS = 8

COLD_CHILD = r"""
import sys, time
start = time.perf_counter()
engine, language, lexer_name, path = sys.argv[1:5]
source = open(path, encoding="utf-8").read()
if engine == "syntaxmate":
    import syntaxmate
    out = syntaxmate.Highlighter().html(source, language, "github-dark")
else:
    from pygments import highlight
    from pygments.formatters import HtmlFormatter
    from pygments.lexers import get_lexer_by_name
    out = highlight(source, get_lexer_by_name(lexer_name), HtmlFormatter(style="github-dark", noclasses=True))
assert out
print(time.perf_counter() - start)
"""


def variants(source: str) -> list[str]:
    # Trailing spaces change every line's text (and cache key) without
    # changing its meaning for the benchmarked grammars.
    lines = source.split("\n")
    return ["\n".join(line + " " * (i + 1) for line in lines) for i in range(VARIANTS)]


def timed(fn, min_time: float) -> float:
    """Seconds per call, calibrated to run for at least ``min_time``."""
    calls = 1
    while True:
        start = time.perf_counter()
        for _ in range(calls):
            fn()
        elapsed = time.perf_counter() - start
        if elapsed >= min_time:
            return elapsed / calls
        calls *= 2


def run(args) -> dict:
    import pygments
    from pygments import highlight
    from pygments.formatters import HtmlFormatter
    from pygments.lexers import get_lexer_by_name

    import syntaxmate

    results = {}
    for language in args.languages:
        fixture, lexer_name = LANGUAGES[language]
        path = FIXTURES / fixture
        source = path.read_text(encoding="utf-8")
        docs = variants(source)
        size = len(source.encode())
        variant_size = statistics.mean(len(doc.encode()) for doc in docs)

        formatter = HtmlFormatter(style=THEME, noclasses=True)
        lexer = get_lexer_by_name(lexer_name)
        hl = syntaxmate.Highlighter()
        hl.html(source, language, THEME)

        def pyg(doc):
            return highlight(doc, lexer, formatter)

        def rotate(render):
            index = 0

            def call():
                nonlocal index
                render(docs[index % VARIANTS])
                index += 1

            return call

        engines = {
            "syntaxmate": {
                "first": lambda: syntaxmate.Highlighter().html(source, language, THEME),
                "steady": rotate(lambda doc: hl.html(doc, language, THEME)),
                "replay": lambda: hl.html(source, language, THEME),
            },
            "pygments": {
                "first": lambda: highlight(
                    source,
                    get_lexer_by_name(lexer_name),
                    HtmlFormatter(style=THEME, noclasses=True),
                ),
                "steady": rotate(pyg),
                "replay": lambda: pyg(source),
            },
        }
        # Steady mode highlights the padded variants, so it has its own size.
        entry = {"fixture": fixture, "bytes": size, "steadyBytes": variant_size}
        for engine, modes in engines.items():
            row = {}
            cold = [
                float(
                    subprocess.run(
                        [sys.executable, "-c", COLD_CHILD, engine, language, lexer_name, str(path)],
                        check=True,
                        capture_output=True,
                        text=True,
                    ).stdout
                )
                for _ in range(args.samples)
            ]
            row["cold"] = statistics.median(cold)
            for mode, fn in modes.items():
                # "first" is one-shot by definition; the others are calibrated.
                samples = [
                    timed(fn, 0.0 if mode == "first" else args.min_time)
                    for _ in range(args.samples)
                ]
                row[mode] = statistics.median(samples)
            row["steadyMBps"] = entry["steadyBytes"] / row["steady"] / 1e6
            row["replayMBps"] = entry["bytes"] / row["replay"] / 1e6
            entry[engine] = row
        results[language] = entry
    return {
        "environment": {
            "python": platform.python_version(),
            "machine": platform.machine(),
            "platform": platform.platform(),
            "processor": platform.processor(),
            "pygments": pygments.__version__,
            "syntaxmate": syntaxmate.__version__,
        },
        "samples": args.samples,
        "minTimeSeconds": args.min_time,
        "results": results,
    }


def report(data: dict) -> None:
    print(
        f"Python {data['environment']['python']}, syntaxmate "
        f"{data['environment']['syntaxmate']}, Pygments {data['environment']['pygments']}"
    )
    header = f"{'language':<11}{'bytes':>7}  {'mode':<7}{'syntaxmate':>12}{'pygments':>12}{'speedup':>9}"
    print(header)
    print("-" * len(header))
    for language, entry in data["results"].items():
        for mode in ("cold", "first", "steady", "replay"):
            ours, theirs = entry["syntaxmate"][mode], entry["pygments"][mode]
            print(
                f"{language:<11}{entry['bytes']:>7}  {mode:<7}"
                f"{ours * 1e3:>10.2f}ms{theirs * 1e3:>10.2f}ms{theirs / ours:>8.1f}x"
            )
    for mode, size in (("steady", "steadyBytes"), ("replay", "bytes")):
        total = sum(entry[size] for entry in data["results"].values())
        ours = sum(entry["syntaxmate"][mode] for entry in data["results"].values())
        theirs = sum(entry["pygments"][mode] for entry in data["results"].values())
        print(
            f"aggregate {mode}: syntaxmate {total / ours / 1e6:.2f} MB/s, "
            f"pygments {total / theirs / 1e6:.2f} MB/s"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--languages", nargs="+", default=list(LANGUAGES), choices=LANGUAGES)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--min-time", type=float, default=0.2, help="seconds per warm sample")
    parser.add_argument("--json", type=Path, help="also write raw results here")
    args = parser.parse_args()
    data = run(args)
    report(data)
    if args.json:
        args.json.write_text(json.dumps(data, indent=2) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
