#!/usr/bin/env python3
"""Summarize macOS /usr/bin/sample top-of-stack output by TextMate cost area."""

import argparse
import json
import re
from pathlib import Path


CATEGORIES = {
    "allocation": re.compile(
        r"malloc|free|alloc::|dealloc|drop_in_place|memset|bzero|Vec<|RawVec",
        re.IGNORECASE,
    ),
    "vm": re.compile(
        r"match_(?:position_)?node|match_repeat|match_look|FallbackMatcher|VmState|bytecode::Program",
        re.IGNORECASE,
    ),
    "substring": re.compile(
        r"Prefilter|LiteralSet|find_byte|memchr|memmem|Searcher|str::.*find|contains",
        re.IGNORECASE,
    ),
    "hashing": re.compile(
        r"(?:^|::)(?:hash|Hasher|HashMap|RawTable)|fnv_mix",
        re.IGNORECASE,
    ),
}

SAMPLE_LINE = re.compile(r"^\s*(\d+)\s+(.+?)\s*$")
SAMPLE_LINE_COUNT_LAST = re.compile(r"^\s*(.+?\S)\s{2,}(\d+)\s*$")


def summarize(text: str) -> dict:
    marker = "Sort by top of stack"
    if marker not in text:
        raise ValueError(f"sample report does not contain {marker!r}")
    rows = []
    section = text.split(marker, 1)[1].split("Binary Images:", 1)[0]
    for line in section.splitlines():
        match = SAMPLE_LINE.match(line)
        if match:
            rows.append((int(match.group(1)), match.group(2)))
            continue
        match = SAMPLE_LINE_COUNT_LAST.match(line)
        if match:
            rows.append((int(match.group(2)), match.group(1)))

    counts = {category: 0 for category in CATEGORIES}
    counts["other"] = 0
    for samples, symbol in rows:
        category = next(
            (name for name, pattern in CATEGORIES.items() if pattern.search(symbol)),
            "other",
        )
        counts[category] += samples

    total = sum(counts.values())
    return {
        "total_samples": total,
        "categories": {
            category: {
                "samples": samples,
                "percent": round(samples * 100 / total, 2) if total else 0.0,
            }
            for category, samples in counts.items()
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("report", type=Path, help="text output from /usr/bin/sample")
    args = parser.parse_args()
    print(json.dumps(summarize(args.report.read_text()), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
