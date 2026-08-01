#!/usr/bin/env python3
"""Fixture tests for summarize-textmate-sample.py."""

import importlib.util
import json
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "benchmarks" / "textmate" / "sample-fixtures"
SPEC = importlib.util.spec_from_file_location(
    "summarize_textmate_sample", ROOT / "tools" / "summarize-textmate-sample.py"
)
assert SPEC and SPEC.loader
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class SummarizeTextmateSampleTests(unittest.TestCase):
    def test_basic_fixture(self) -> None:
        actual = MODULE.summarize((FIXTURES / "basic.txt").read_text())
        expected = json.loads((FIXTURES / "basic.expected.json").read_text())
        self.assertEqual(actual, expected)

    def test_requires_top_of_stack_section(self) -> None:
        with self.assertRaisesRegex(ValueError, "Sort by top of stack"):
            MODULE.summarize("Call graph only")

    def test_accepts_macos_count_last_format(self) -> None:
        report = """Sort by top of stack, same collapsed (when >= 5):
syntaxmate::engine::regex::bytecode::Program::execute_inner  (in mark)        12
syntaxmate::engine::regex::prefilter::Prefilter::next_occurrence  (in mark)         8
_malloc  (in libsystem_malloc.dylib)         5
Binary Images:
0x1000 - 0x2000 mark
"""
        actual = MODULE.summarize(report)
        self.assertEqual(actual["total_samples"], 25)
        self.assertEqual(actual["categories"]["vm"]["samples"], 12)
        self.assertEqual(actual["categories"]["substring"]["samples"], 8)
        self.assertEqual(actual["categories"]["allocation"]["samples"], 5)


if __name__ == "__main__":
    unittest.main()
