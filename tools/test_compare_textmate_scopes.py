#!/usr/bin/env python3
"""Focused tests for compare-textmate-scopes.py."""

import importlib.util
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "compare_textmate_scopes", ROOT / "tools" / "compare-textmate-scopes.py"
)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def oracle_line(number, line, tokens):
    return MODULE.normalize_record(
        {"lineNumber": number, "line": line, "tokens": tokens}, oracle=True
    )


def native_line(number, line, tokens):
    return MODULE.normalize_record(
        {"lineNumber": number, "line": line, "tokens": tokens}, oracle=False
    )


class CompareTextmateScopesTests(unittest.TestCase):
    def test_normalizes_utf16_clamps_drops_and_coalesces(self):
        line = "aπ𝌆z"
        oracle = oracle_line(
            7,
            line,
            [
                {"startIndex": 0, "endIndex": 1, "scopes": ["source.test"]},
                {"startIndex": 1, "endIndex": 2, "scopes": ["source.test"]},
                {"startIndex": 2, "endIndex": 4, "scopes": ["source.test", "emoji"]},
                {"startIndex": 4, "endIndex": 6, "scopes": ["source.test"]},
                {"startIndex": 6, "endIndex": 99, "scopes": ["ignored.empty"]},
            ],
        )
        self.assertEqual(
            oracle.tokens,
            (
                MODULE.ScopeToken(0, 3, ("source.test",)),
                MODULE.ScopeToken(3, 7, ("source.test", "emoji")),
                MODULE.ScopeToken(7, 8, ("source.test",)),
            ),
        )

        native = native_line(
            7,
            line,
            [
                {"start": 0, "end": 3, "scopes": ["source.test"]},
                {"start": 3, "end": 7, "scopes": ["source.test", "emoji"]},
                {"start": 7, "end": 20, "scopes": ["source.test"]},
                {"start": 20, "end": 30, "scopes": ["ignored.empty"]},
            ],
        )
        self.assertEqual(native, oracle)

    def test_rejects_utf16_boundary_inside_surrogate_pair(self):
        with self.assertRaisesRegex(MODULE.ComparisonInputError, "surrogate pair"):
            MODULE.utf16_offset_to_utf8("𝌆", 1)

    def test_reports_divergent_missing_extra_and_reordered_lines(self):
        root = [{"start": 0, "end": 1, "scopes": ["source.test"]}]
        changed = [{"start": 0, "end": 1, "scopes": ["source.test", "changed"]}]
        oracle = [native_line(number, "x", root) for number in (0, 1, 2, 3)]
        native = [
            native_line(2, "x", root),
            native_line(0, "x", root),
            native_line(3, "x", changed),
            native_line(4, "x", root),
        ]

        comparison = MODULE.compare_lines(oracle, native)

        self.assertFalse(comparison.equal)
        self.assertEqual(comparison.matching_count, 2)
        self.assertEqual([item.line_number for item in comparison.divergent], [3])
        self.assertEqual(comparison.missing, (1,))
        self.assertEqual(comparison.extra, (4,))
        self.assertEqual(
            [item.line_number for item in comparison.reordered],
            [2, 0],
        )
        report = MODULE.format_report(comparison)
        self.assertIn("divergent=1 missing=1 extra=1 reordered=2", report)
        self.assertIn("line 3 (scopes differ)", report)

    def test_missing_and_extra_lines_do_not_create_false_reordering(self):
        token = [{"start": 0, "end": 1, "scopes": ["source.test"]}]
        oracle = [native_line(number, "x", token) for number in (0, 1, 2)]
        native = [native_line(number, "x", token) for number in (9, 0, 2)]

        comparison = MODULE.compare_lines(oracle, native)

        self.assertEqual(comparison.missing, (1,))
        self.assertEqual(comparison.extra, (9,))
        self.assertEqual(comparison.reordered, ())

    def test_equivalent_normalized_scope_streams_match(self):
        oracle = oracle_line(
            0,
            "éx",
            [
                {"startIndex": 0, "endIndex": 1, "scopes": ["source.test"]},
                {"startIndex": 1, "endIndex": 3, "scopes": ["source.test"]},
            ],
        )
        native = native_line(
            0,
            "éx",
            [{"start": 0, "end": 3, "scopes": ["source.test"]}],
        )

        comparison = MODULE.compare_lines([oracle], [native])

        self.assertTrue(comparison.equal)
        self.assertEqual(comparison.matching_count, 1)


if __name__ == "__main__":
    unittest.main()
