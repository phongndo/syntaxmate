import json
import tempfile
import unittest
from pathlib import Path

from tools import textmate_validation as validation


class TextMateContractSnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.write(
            validation.COVERAGE,
            'public_language_count = 2\nkept = [\n  "alpha",\n  "beta",\n]\n',
        )
        self.write(validation.DIVERGENCES, "# exact: no divergences\n")
        self.write_case_files("alpha", "alpha", "alpha")
        # Exercise fixture-name to public grammar-ID mapping (as bash does for
        # shellscript in the real manifest).
        self.write_case_files("alias", "beta", "beta")
        self.write_manifest()

    def tearDown(self):
        self.temporary.cleanup()

    def write(self, relative, text):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)

    def write_case_files(self, fixture_id, grammar_id, public_id):
        del grammar_id, public_id
        base = Path("case-data") / fixture_id
        self.write(base / "basic.txt", "basic\n" * 10)
        self.write(base / "stress.txt", "stress\n" * 140)
        golden = json.dumps({"stoppedEarly": False}) + "\n"
        self.write(base / "basic.golden.jsonl", golden)
        self.write(base / "stress.golden.jsonl", golden)

    def write_manifest(self):
        blocks = []
        for fixture_id, grammar_id in (("alpha", "alpha"), ("alias", "beta")):
            for kind in ("basic", "stress"):
                blocks.append(
                    "[[case]]\n"
                    f'language = "{fixture_id}"\n'
                    f'grammar = "assets/{grammar_id}.tmLanguage.json"\n'
                    f'fixture = "case-data/{fixture_id}/{kind}.txt"\n'
                    f'golden = "case-data/{fixture_id}/{kind}.golden.jsonl"\n'
                )
        self.write(validation.CASES, "\n".join(blocks))

    def test_complete_contract_and_alias_mapping(self):
        snapshot = validation.contract_snapshot(self.root)
        self.assertEqual(snapshot.public_ids, {"alpha", "beta"})
        self.assertEqual(snapshot.validated_ids, snapshot.public_ids)
        self.assertEqual(snapshot.stress_ids, snapshot.public_ids)

    def test_missing_stress_cannot_be_hidden_by_manifest_regeneration(self):
        text = (self.root / validation.CASES).read_text()
        blocks = text.split("[[case]]")
        self.write(
            validation.CASES,
            "[[case]]".join(block for block in blocks if 'case-data/alpha/stress.txt' not in block),
        )
        snapshot = validation.contract_snapshot(self.root)
        self.assertNotIn("alpha", snapshot.validated_ids)
        self.assertNotIn("alpha", snapshot.stress_ids)

    def test_completed_count_with_swapped_language_fails_identity_lock(self):
        ids = [f"lang{index:03}" for index in range(validation.LOCKED_COMPLETED_COUNT)]
        kept = "".join(f'  "{language}",\n' for language in ids)
        self.write(
            validation.COVERAGE,
            f"public_language_count = {validation.LOCKED_COMPLETED_COUNT}\n"
            f"kept = [\n{kept}]\n",
        )
        with self.assertRaisesRegex(ValueError, "identity"):
            validation.public_languages(self.root)

    def test_stopped_early_divergence_and_invalid_shape_break_exact_membership(self):
        golden = Path("case-data/alpha/basic.golden.jsonl")
        self.write(golden, json.dumps({"stoppedEarly": True}) + "\n")
        self.assertNotIn("alpha", validation.contract_snapshot(self.root).validated_ids)

        self.write(golden, json.dumps({"stoppedEarly": False}) + "\n")
        self.write(
            validation.DIVERGENCES,
            '[[divergence]]\nfixture = "case-data/alpha/basic.txt"\n',
        )
        self.assertNotIn("alpha", validation.contract_snapshot(self.root).validated_ids)

        self.write(validation.DIVERGENCES, "# exact: no divergences\n")
        self.write(Path("case-data/alpha/basic.txt"), "short\n" * 9)
        self.assertNotIn("alpha", validation.contract_snapshot(self.root).validated_ids)


if __name__ == "__main__":
    unittest.main()
