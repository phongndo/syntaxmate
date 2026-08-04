#!/usr/bin/env python3
"""Unit and committed-report checks for competitive benchmark tooling."""

import importlib.util
import json
import sys
import tomllib
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RUNNER = ROOT / "tools/run-competitive-benchmarks.py"
RESULT = ROOT / "benchmarks/competitors/results-2026-08-04.json"
NODE_PACKAGE = ROOT / "benchmarks/competitors/package.json"
SYNTECT_LOCK = ROOT / "benchmarks/competitors/syntect-driver/Cargo.lock"

spec = importlib.util.spec_from_file_location("competitive_benchmarks", RUNNER)
benchmarks = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = benchmarks
spec.loader.exec_module(benchmarks)


class CompetitiveBenchmarkTests(unittest.TestCase):
    def test_two_engine_order_alternates(self):
        engines = ("a", "b")
        self.assertEqual(benchmarks.balanced_order(engines, 0), ("a", "b"))
        self.assertEqual(benchmarks.balanced_order(engines, 1), ("b", "a"))

    def test_three_engine_cycle_covers_every_order(self):
        engines = ("a", "b", "c")
        orders = {benchmarks.balanced_order(engines, index) for index in range(6)}
        self.assertEqual(len(orders), 6)

    def test_nearest_rank(self):
        values = [7, 1, 5, 3, 2, 6, 4]
        self.assertEqual(benchmarks.nearest_rank(values, 0.50), 4)
        self.assertEqual(benchmarks.nearest_rank(values, 0.95), 7)

    def test_cold_uses_process_wall_and_warm_uses_per_iteration(self):
        cold = {
            "track": "end-to-end",
            "phase": "cold",
            "wallNanoseconds": 90,
            "elapsedNanoseconds": 20,
            "iterations": 1,
        }
        warm = {
            "track": "end-to-end",
            "phase": "steady",
            "wallNanoseconds": 90,
            "elapsedNanoseconds": 20,
            "iterations": 4,
        }
        self.assertEqual(benchmarks.metric_nanos(cold), 90)
        self.assertEqual(benchmarks.metric_nanos(warm), 5)

    def test_committed_report_is_complete_and_pinned(self):
        report = json.loads(RESULT.read_text())
        self.assertEqual(report["schemaVersion"], 1)
        self.assertEqual(report["samplesPerSet"], 7)
        self.assertEqual(report["sampleCount"], 987)
        self.assertNotIn("samples", report)
        self.assertEqual(len(report["languages"]["engine"]), 10)
        self.assertEqual(len(report["languages"]["end-to-end"]), 9)

        node = json.loads(NODE_PACKAGE.read_text())["dependencies"]
        self.assertEqual(report["versions"]["shiki"], node["shiki"])
        self.assertEqual(
            report["versions"]["vscode-textmate"], node["vscode-textmate"]
        )
        self.assertIn(
            ("syntect", report["versions"]["syntect"]),
            {
                (package["name"], package["version"])
                for package in tomllib.loads(SYNTECT_LOCK.read_text())["package"]
            },
        )

        for phases in report["summaries"].values():
            for languages in phases.values():
                for engines in languages.values():
                    for summary in engines.values():
                        self.assertEqual(summary["samples"], 7)
                        self.assertGreater(summary["latencyNanosecondsP50"], 0)
                        self.assertGreaterEqual(
                            summary["latencyNanosecondsP95"],
                            summary["latencyNanosecondsP50"],
                        )
                        self.assertGreater(summary["megabytesPerSecondP50"], 0)

        engine = report["summaries"]["engine"]
        for language in report["languages"]["engine"]:
            digests = {
                engines[language][name]["scopeDigest"]
                for engines in engine.values()
                for name in ("syntaxmate", "vscode-textmate")
            }
            self.assertEqual(len(digests), 1)

        product = report["summaries"]["end-to-end"]
        for language in report["languages"]["end-to-end"]:
            for name in ("syntaxmate", "shiki", "syntect"):
                digests = {
                    languages[language][name]["outputDigest"]
                    for languages in product.values()
                }
                self.assertEqual(len(digests), 1)


if __name__ == "__main__":
    unittest.main()
