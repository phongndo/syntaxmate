#!/usr/bin/env python3
"""Shared hard-lock checks for the completed TextMate validation contract."""

import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path


POLICY = Path("benchmarks/textmate/validation-policy.json")
CASES = Path("tests/fixtures/textmate/cases.toml")
COVERAGE = Path("assets/grammars/coverage.toml")
DIVERGENCES = Path("tests/fixtures/textmate/divergences.toml")
EXPECTED_COUNT_KEYS = (
    "publicLanguages",
    "validatedLanguages",
    "oracleLanguages",
    "stressCorpusLanguages",
)
LOCKED_COMPLETED_COUNT = 264
# SHA-256 of the sorted, newline-joined public catalog IDs. Locks the exact
# 264-language identity: a coverage edit that swaps one language for another
# keeps every count at 264 and regenerates cleanly, so the count locks alone
# would not notice.
LOCKED_CATALOG_SHA256 = "5447bcb68e371a9c5152d2d2b0ff262f86742018da622437025a87cb0a91a7f1"


@dataclass(frozen=True)
class ContractSnapshot:
    public_ids: frozenset[str]
    validated_ids: frozenset[str]
    oracle_ids: frozenset[str]
    stress_ids: frozenset[str]


def _blocks(text, header):
    return re.split(rf"(?m)^\[\[{re.escape(header)}\]\]\s*$", text)[1:]


def _string(block, key):
    match = re.search(rf'(?m)^{re.escape(key)}\s*=\s*"([^"]*)"', block)
    return match.group(1) if match else None


def load_policy(root, path=POLICY, *, locked_count=LOCKED_COMPLETED_COUNT):
    policy_path = root / path
    try:
        policy = json.loads(policy_path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {path}: {error}") from error
    if policy.get("schemaVersion") != 1:
        raise ValueError(f"{path}: schemaVersion must be 1")
    expected = policy.get("expectedCounts")
    if not isinstance(expected, dict) or set(expected) != set(EXPECTED_COUNT_KEYS):
        raise ValueError(
            f"{path}: expectedCounts must contain exactly "
            + ", ".join(EXPECTED_COUNT_KEYS)
        )
    for key in EXPECTED_COUNT_KEYS:
        value = expected[key]
        if isinstance(value, bool) or not isinstance(value, int) or value < 0:
            raise ValueError(f"{path}: expectedCounts.{key} must be a nonnegative integer")
        if value != locked_count:
            raise ValueError(
                f"{path}: expectedCounts.{key} must remain locked at {locked_count}"
            )
    return policy


def assert_locked_counts(policy, *, public, validated, oracle, stress_corpus):
    actual = {
        "publicLanguages": public,
        "validatedLanguages": validated,
        "oracleLanguages": oracle,
        "stressCorpusLanguages": stress_corpus,
    }
    for key, value in actual.items():
        expected = policy["expectedCounts"][key]
        if value != expected:
            raise ValueError(
                f"validation policy requires {key}={expected}, found {value}"
            )


def public_languages(root):
    text = (root / COVERAGE).read_text()
    count = re.search(r"(?m)^public_language_count\s*=\s*(\d+)$", text)
    kept = re.search(r"(?ms)^kept\s*=\s*\[(.*?)^\]", text)
    if not count or not kept:
        raise ValueError(f"{COVERAGE} lacks public_language_count or kept")
    languages = frozenset(re.findall(r'"([^"]+)"', kept.group(1)))
    declared = int(count.group(1))
    if len(languages) != declared:
        raise ValueError(
            f"{COVERAGE} declares {declared} public IDs but lists {len(languages)}"
        )
    if declared == LOCKED_COMPLETED_COUNT:
        digest = hashlib.sha256("\n".join(sorted(languages)).encode()).hexdigest()
        if digest != LOCKED_CATALOG_SHA256:
            raise ValueError(
                f"{COVERAGE} lists {declared} public IDs whose identity does not "
                f"match the locked completed catalog "
                f"(sha256 {digest} != {LOCKED_CATALOG_SHA256})"
            )
    return languages


def contract_snapshot(root):
    """Derive exact basic/stress membership independently of generated docs."""
    public = public_languages(root)
    divergent = {
        fixture
        for block in _blocks((root / DIVERGENCES).read_text(), "divergence")
        if (fixture := _string(block, "fixture"))
    }
    by_language = {}
    for block in _blocks((root / CASES).read_text(), "case"):
        language = _string(block, "language")
        grammar = _string(block, "grammar")
        fixture = _string(block, "fixture")
        golden = _string(block, "golden")
        if not all((language, grammar, fixture, golden)):
            raise ValueError(f"{CASES} has a case missing language, grammar, fixture, or golden")
        grammar_id = Path(grammar).name.removesuffix(".tmLanguage.json")
        public_id = language if language in public else grammar_id
        if public_id not in public:
            raise ValueError(f"{CASES}: cannot map {language!r} to a public ID")
        kind = Path(fixture).name.split(".", 1)[0]
        clean = fixture not in divergent and _clean_golden(root / golden)
        line_count = len((root / fixture).read_text().splitlines())
        shaped = (
            10 <= line_count <= 30
            if kind == "basic"
            else 140 <= line_count <= 260
            if kind == "stress"
            else True
        )
        by_language.setdefault(public_id, []).append((kind, clean and shaped))

    oracle = frozenset(by_language)
    stress = frozenset(
        language
        for language, records in by_language.items()
        if any(kind == "stress" for kind, _ in records)
    )
    validated = frozenset(
        language
        for language, records in by_language.items()
        if {kind for kind, _ in records} >= {"basic", "stress"}
        and all(clean for kind, clean in records if kind in {"basic", "stress"})
    )
    return ContractSnapshot(public, validated, oracle, stress)


def _clean_golden(path):
    try:
        lines = [line for line in path.read_text().splitlines() if line.strip()]
    except OSError:
        return False
    if not lines:
        return False
    for line_number, line in enumerate(lines, 1):
        try:
            record = json.loads(line)
        except json.JSONDecodeError as error:
            raise ValueError(f"{path}:{line_number}: {error}") from error
        if record.get("stoppedEarly") is not False:
            return False
    return True
