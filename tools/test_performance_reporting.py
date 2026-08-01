#!/usr/bin/env python3
"""Focused tests for the persisted TextMate performance/status reporting contract."""

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def load_tool(name, filename):
    spec = importlib.util.spec_from_file_location(name, ROOT / "tools" / filename)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


PERFORMANCE = load_tool(
    "check_textmate_catalog_performance", "check-textmate-catalog-performance.py"
)
STATUS = load_tool("generate_language_status", "generate-language-status.py")
CORPORA = load_tool("build_textmate_corpora", "build-textmate-corpora.py")


class CatalogPerformanceTests(unittest.TestCase):
    def test_completed_count_policy_cannot_be_lowered_with_regenerated_outputs(self):
        policy = {
            "expectedCounts": {
                "publicLanguages": 264,
                "validatedLanguages": 264,
                "oracleLanguages": 264,
                "stressCorpusLanguages": 264,
            }
        }
        with self.assertRaisesRegex(ValueError, "validatedLanguages=264, found 263"):
            CORPORA.assert_locked_counts(
                policy, public=264, validated=263, oracle=264, stress_corpus=264
            )
        policy["schemaVersion"] = 1
        policy["expectedCounts"] = dict(policy["expectedCounts"])
        policy["expectedCounts"]["validatedLanguages"] = 263
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "policy.json").write_text(json.dumps(policy))
            with self.assertRaisesRegex(ValueError, "must remain locked at 264"):
                CORPORA.load_policy(root, Path("policy.json"))

    def test_corpus_check_rejects_regenerated_count_drift(self):
        manifest = {
            "corpora": [
                {
                    "id": "catalog-repeated",
                    "languages": 253,
                    "bytes": 10,
                    "tokenizer_lines": 2,
                    "sha256": "same",
                }
            ]
        }
        checked = {
            "catalog-repeated": {
                "id": "catalog-repeated",
                "languages": 254,
                "bytes": 10,
                "tokenizer_lines": 2,
                "sha256": "same",
            }
        }
        with self.assertRaisesRegex(ValueError, "stale catalog-repeated.languages"):
            CORPORA.check_committed_manifest(manifest, checked)

    def test_dynamic_corpus_does_not_lock_documentation_metadata(self):
        manifest = {
            "corpora": [
                {
                    "id": "representative-markdown",
                    "bytes": 99,
                    "tokenizer_lines": 10,
                    "sha256": "changes-with-documentation",
                }
            ]
        }
        checked = {
            "representative-markdown": {
                "id": "representative-markdown",
                "content_contract": "dynamic",
            }
        }
        CORPORA.check_committed_manifest(manifest, checked)

    def test_policy_floor_is_the_default_source_of_truth(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "policy.json"
            path.write_text('{"minimumStressMbPerSecond": 3.25}\n')
            self.assertEqual(PERFORMANCE.load_policy_floor(path), 3.25)

            path.write_text('{"minimumStressMbPerSecond": "3.25"}\n')
            with self.assertRaisesRegex(ValueError, "must be a number"):
                PERFORMANCE.load_policy_floor(path)

    def test_aggregate_is_total_bytes_over_inferred_total_time(self):
        results = [
            {"bytes": 1_000_000, "mbPerSecond": 2.0},
            {"bytes": 1_000_000, "mbPerSecond": 4.0},
        ]
        self.assertEqual(PERFORMANCE.aggregate_mb_per_second(results, 3), 2.667)

    def test_report_has_no_ambient_timestamp_or_machine_fields(self):
        corpus = {"id": "catalog-repeated", "bytes": 10, "sha256": "abc"}
        results = [
            {
                "language": "a",
                "scope": "source.a",
                "bytes": 10,
                "sha256": "def",
                "mbPerSecond": 2.5,
                "processedBytes": 10,
                "elapsedNanoseconds": 4_000,
                "passed": True,
            }
        ]
        first = PERFORMANCE.make_report(corpus, results, 2.0, 1)
        second = PERFORMANCE.make_report(corpus, results, 2.0, 1)
        self.assertEqual(first, second)
        self.assertEqual(first["measurement"]["aggregateMbPerSecond"], 2.5)
        self.assertNotIn("generatedAt", first)


class LanguageStatusPerformanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        (self.root / "benchmarks/textmate").mkdir(parents=True)
        self.corpus = {
            "id": "catalog-repeated",
            "bytes": 400,
            "sha256": "corpus-digest",
            "languages": 2,
        }
        self.results = [
            {
                "language": "alpha",
                "scope": "source.alpha",
                "bytes": 100,
                "sha256": "alpha-digest",
                "processedBytes": 200,
                "elapsedNanoseconds": 100_000,
                "mbPerSecond": 2.0,
                "passed": True,
            },
            {
                "language": "beta",
                "scope": "source.beta",
                "bytes": 300,
                "sha256": "beta-digest",
                "processedBytes": 600,
                "elapsedNanoseconds": 150_000,
                "mbPerSecond": 4.0,
                "passed": True,
            },
        ]

    def tearDown(self):
        self.temporary.cleanup()

    def write_report(self, *, floor=2.0, languages=None, corpus_sha="corpus-digest"):
        report_corpus = {
            "id": "catalog-repeated",
            "bytes": 400,
            "sha256": corpus_sha,
        }
        report = PERFORMANCE.make_report(report_corpus, self.results, floor, 2)
        if languages is not None:
            report["corpus"]["languages"] = languages
        path = self.root / STATUS.PERFORMANCE_REPORT
        path.write_text(json.dumps(report))

    def test_consumes_measured_rates_and_checked_aggregate(self):
        self.write_report()
        rates, aggregate = STATUS.performance_measurements(
            self.root, {"beta", "alpha"}, 2.0, self.corpus
        )
        self.assertEqual(rates["alpha"]["mbPerSecond"], 2.0)
        self.assertTrue(rates["beta"]["passed"])
        self.assertEqual(aggregate, 3.2)

    def test_refuses_stale_membership_floor_and_corpus(self):
        cases = (
            ({"languages": ["alpha"]}, "stale language membership"),
            ({"floor": 1.5}, "stale floor"),
            ({"corpus_sha": "old"}, "stale corpus sha256"),
        )
        for arguments, message in cases:
            with self.subTest(message=message):
                self.write_report(**arguments)
                with self.assertRaisesRegex(ValueError, message):
                    STATUS.performance_measurements(
                        self.root, {"alpha", "beta"}, 2.0, self.corpus
                    )

    def test_missing_report_explains_explicit_write(self):
        with self.assertRaisesRegex(ValueError, "--write-report"):
            STATUS.performance_measurements(
                self.root, {"alpha", "beta"}, 2.0, self.corpus
            )

    def test_promotion_dates_are_explicit_and_validated(self):
        path = self.root / STATUS.PROMOTIONS
        path.write_text(
            json.dumps({"schemaVersion": 1, "promotions": {"alpha": "2026-07-12"}})
        )
        self.assertEqual(
            STATUS.promotion_dates(self.root, {"alpha"}), {"alpha": "2026-07-12"}
        )
        path.write_text(
            json.dumps({"schemaVersion": 1, "promotions": {"alpha": "07/12/2026"}})
        )
        with self.assertRaisesRegex(ValueError, "invalid promotion date"):
            STATUS.promotion_dates(self.root, {"alpha"})

    def test_final_batch_date_is_complete_and_not_synthesized(self):
        promotions = {"alpha": "2026-07-12", "beta": "2026-07-12"}
        STATUS.validate_final_promotion_batch(
            promotions, {"alpha", "beta"}, "2026-07-12"
        )
        with self.assertRaisesRegex(ValueError, "genuine final batch date"):
            STATUS.validate_final_promotion_batch(
                {**promotions, "beta": "2026-07-11"},
                {"alpha", "beta"},
                "2026-07-12",
            )


if __name__ == "__main__":
    unittest.main()
