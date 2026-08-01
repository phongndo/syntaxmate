#!/usr/bin/env python3
"""Generate the truthful public-language validation ledger."""

import argparse
import datetime
import json
import math
import re
import sys
from collections import defaultdict
from pathlib import Path

try:
    from textmate_validation import assert_locked_counts, load_policy
except ModuleNotFoundError:  # Imported as tools.generate_language_status in tests.
    from tools.textmate_validation import assert_locked_counts, load_policy


ROOT = Path(__file__).resolve().parents[1]
CASES = Path("tests/fixtures/textmate/cases.toml")
DIVERGENCES = Path("tests/fixtures/textmate/divergences.toml")
COVERAGE = Path("assets/grammars/coverage.toml")
CORPORA = Path("benchmarks/textmate/corpora.toml")
VALIDATION_POLICY = Path("benchmarks/textmate/validation-policy.json")
PERFORMANCE_REPORT = Path("benchmarks/textmate/catalog-performance.json")
PROMOTIONS = Path("benchmarks/textmate/language-promotions.json")
OUTPUT = Path("docs/language-status.md")
TIERS = Path("benchmarks/textmate/promotion-tiers.json")
FIXTURE_ORDER = {"basic": 0, "stress": 1, "smoke": 2}


def scalar(block, key):
    match = re.search(rf'(?m)^{re.escape(key)}\s*=\s*"([^"]*)"', block)
    return match.group(1) if match else None


def integer(block, key):
    match = re.search(rf"(?m)^{re.escape(key)}\s*=\s*(\d+)", block)
    return int(match.group(1)) if match else None


def blocks(text, header):
    return re.split(rf"(?m)^\[\[{re.escape(header)}\]\]\s*$", text)[1:]


def catalog(root):
    text = (root / COVERAGE).read_text()
    count_match = re.search(r"(?m)^public_language_count\s*=\s*(\d+)$", text)
    kept_match = re.search(r"(?ms)^kept\s*=\s*\[(.*?)^\]", text)
    if not count_match or not kept_match:
        raise ValueError("coverage.toml lacks public_language_count or kept")
    languages = sorted(re.findall(r'"([^"]+)"', kept_match.group(1)))
    expected = int(count_match.group(1))
    if len(languages) != expected or len(set(languages)) != expected:
        raise ValueError(
            f"coverage catalog says {expected} languages but kept contains {len(languages)}"
        )
    return languages


def case_records(root, public):
    records = []
    for block in blocks((root / CASES).read_text(), "case"):
        values = {key: scalar(block, key) for key in ("language", "grammar", "fixture", "golden")}
        if not all(values.values()):
            continue
        grammar_id = Path(values["grammar"]).name.removesuffix(".tmLanguage.json")
        if values["language"] in public:
            language = values["language"]
        elif grammar_id in public:
            language = grammar_id
        else:
            raise ValueError(f"cannot map case language {values['language']} to the public catalog")
        fixture_path = root / values["fixture"]
        golden_path = root / values["golden"]
        kind = fixture_path.name.split(".", 1)[0]
        source_lines = len(fixture_path.read_text().splitlines()) if fixture_path.is_file() else 0
        fixture_shape = (
            10 <= source_lines <= 30
            if kind == "basic"
            else 140 <= source_lines <= 260
            if kind == "stress"
            else True
        )
        clean = fixture_path.is_file() and golden_path.is_file()
        records_seen = 0
        if clean:
            for line_number, line in enumerate(golden_path.read_text().splitlines(), 1):
                if not line.strip():
                    continue
                records_seen += 1
                try:
                    record = json.loads(line)
                except json.JSONDecodeError as error:
                    raise ValueError(f"{golden_path}:{line_number}: {error}") from error
                if record.get("stoppedEarly") is not False:
                    clean = False
        clean = clean and records_seen > 0
        records.append(
            {
                **values,
                "catalog_language": language,
                "kind": kind,
                "clean": clean,
                "fixture_shape": fixture_shape,
            }
        )
    return records


def divergence_fixtures(root):
    result = set()
    for block in blocks((root / DIVERGENCES).read_text(), "divergence"):
        fixture = scalar(block, "fixture")
        if fixture:
            result.add(fixture)
    return result


def catalog_corpus(root):
    for block in blocks((root / CORPORA).read_text(), "corpus"):
        if scalar(block, "id") == "catalog-repeated":
            result = {
                "id": "catalog-repeated",
                "languages": integer(block, "languages"),
                "bytes": integer(block, "bytes"),
                "sha256": scalar(block, "sha256"),
            }
            if any(value is None for value in result.values()):
                raise ValueError("catalog-repeated corpus lacks languages, bytes, or sha256")
            return result
    raise ValueError("corpora.toml lacks catalog-repeated")


def finite_number(value, label, *, positive=False):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ValueError(f"{label} must be a number")
    value = float(value)
    if not math.isfinite(value) or (value <= 0 if positive else value < 0):
        qualifier = "positive" if positive else "nonnegative"
        raise ValueError(f"{label} must be finite and {qualifier}")
    return value


def performance_measurements(root, expected_languages, floor, corpus):
    path = root / PERFORMANCE_REPORT
    try:
        report = json.loads(path.read_text())
    except FileNotFoundError as error:
        raise ValueError(
            f"{PERFORMANCE_REPORT} is missing; run "
            "check-textmate-catalog-performance.py --write-report"
        ) from error
    except json.JSONDecodeError as error:
        raise ValueError(f"{PERFORMANCE_REPORT}: {error}") from error

    if report.get("schemaVersion") != 1:
        raise ValueError(f"{PERFORMANCE_REPORT}: unsupported schemaVersion")
    measured_corpus = report.get("corpus")
    measurement = report.get("measurement")
    results = report.get("results")
    if not isinstance(measured_corpus, dict) or not isinstance(measurement, dict):
        raise ValueError(f"{PERFORMANCE_REPORT}: corpus and measurement must be objects")
    if not isinstance(results, list):
        raise ValueError(f"{PERFORMANCE_REPORT}: results must be an array")

    expected_languages = sorted(expected_languages)
    measured_languages = measured_corpus.get("languages")
    if measured_languages != expected_languages:
        raise ValueError(
            f"{PERFORMANCE_REPORT}: stale language membership: "
            f"expected {expected_languages!r}, got {measured_languages!r}"
        )
    for key in ("id", "bytes", "sha256"):
        if measured_corpus.get(key) != corpus[key]:
            raise ValueError(
                f"{PERFORMANCE_REPORT}: stale corpus {key}: "
                f"expected {corpus[key]!r}, got {measured_corpus.get(key)!r}"
            )
    measured_floor = finite_number(
        measurement.get("floorMbPerSecond"),
        f"{PERFORMANCE_REPORT}: measurement.floorMbPerSecond",
    )
    if measured_floor != floor:
        raise ValueError(
            f"{PERFORMANCE_REPORT}: stale floor: expected {floor:g}, got {measured_floor:g}"
        )
    if measurement.get("mode") != "process-cold":
        raise ValueError(f"{PERFORMANCE_REPORT}: measurement.mode must be process-cold")
    iterations = measurement.get("iterations")
    if isinstance(iterations, bool) or not isinstance(iterations, int) or iterations < 1:
        raise ValueError(f"{PERFORMANCE_REPORT}: measurement.iterations must be positive")

    rates = {}
    total_bytes = 0
    total_elapsed_nanoseconds = 0
    passed = 0
    for index, result in enumerate(results):
        label = f"{PERFORMANCE_REPORT}: results[{index}]"
        if not isinstance(result, dict):
            raise ValueError(f"{label} must be an object")
        language = result.get("language")
        if not isinstance(language, str) or language in rates:
            raise ValueError(f"{label}.language is missing or duplicated")
        rate = finite_number(result.get("mbPerSecond"), f"{label}.mbPerSecond", positive=True)
        byte_count = result.get("bytes")
        if isinstance(byte_count, bool) or not isinstance(byte_count, int) or byte_count < 1:
            raise ValueError(f"{label}.bytes must be positive")
        result_passed = result.get("passed")
        if not isinstance(result_passed, bool):
            raise ValueError(f"{label}.passed must be boolean")
        measured_bytes = byte_count * iterations
        processed_bytes = result.get("processedBytes")
        elapsed_nanoseconds = result.get("elapsedNanoseconds")
        if processed_bytes != measured_bytes:
            raise ValueError(f"{label}.processedBytes does not match bytes and iterations")
        if (
            isinstance(elapsed_nanoseconds, bool)
            or not isinstance(elapsed_nanoseconds, int)
            or elapsed_nanoseconds < 1
        ):
            raise ValueError(f"{label}.elapsedNanoseconds must be positive")
        computed_rate = measured_bytes * 1_000 / elapsed_nanoseconds
        if not math.isclose(rate, computed_rate, abs_tol=0.0005):
            raise ValueError(f"{label}.mbPerSecond does not match exact timing")
        if result_passed != (computed_rate >= floor):
            raise ValueError(f"{label}.passed does not match the policy floor")
        rates[language] = {"mbPerSecond": rate, "passed": result_passed}
        passed += int(result_passed)
        total_bytes += measured_bytes
        total_elapsed_nanoseconds += elapsed_nanoseconds

    if sorted(rates) != expected_languages:
        raise ValueError(f"{PERFORMANCE_REPORT}: results have stale language membership")
    if total_bytes != corpus["bytes"] * iterations:
        raise ValueError(f"{PERFORMANCE_REPORT}: result bytes do not match the catalog corpus")
    aggregate = finite_number(
        measurement.get("aggregateMbPerSecond"),
        f"{PERFORMANCE_REPORT}: measurement.aggregateMbPerSecond",
        positive=True,
    )
    computed_aggregate = round(total_bytes * 1_000 / total_elapsed_nanoseconds, 3)
    if not math.isclose(aggregate, computed_aggregate, abs_tol=0.0005):
        raise ValueError(
            f"{PERFORMANCE_REPORT}: aggregate does not match results: "
            f"expected {computed_aggregate:.2f}, got {aggregate:.2f}"
        )
    if report.get("passed") != passed or report.get("failed") != len(results) - passed:
        raise ValueError(f"{PERFORMANCE_REPORT}: pass totals do not match results")
    return rates, aggregate


def promotion_dates(root, public):
    path = root / PROMOTIONS
    try:
        source = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read {PROMOTIONS}: {error}") from error
    if source.get("schemaVersion") != 1 or not isinstance(source.get("promotions"), dict):
        raise ValueError(f"{PROMOTIONS}: expected schemaVersion 1 and a promotions object")
    result = {}
    for language, value in source["promotions"].items():
        if language not in public:
            raise ValueError(f"{PROMOTIONS}: unknown public language {language}")
        if not isinstance(value, str):
            raise ValueError(f"{PROMOTIONS}: promotion date for {language} must be a string")
        try:
            parsed = datetime.date.fromisoformat(value)
        except ValueError as error:
            raise ValueError(f"{PROMOTIONS}: invalid promotion date for {language}: {value}") from error
        if parsed.isoformat() != value:
            raise ValueError(f"{PROMOTIONS}: promotion date for {language} must be YYYY-MM-DD")
        result[language] = value
    return result


def validate_final_promotion_batch(promotions, public, final_date):
    if set(promotions) != set(public):
        missing = sorted(set(public) - set(promotions))
        raise ValueError(f"{PROMOTIONS}: missing public promotion dates: {missing}")
    if set(promotions.values()) != {final_date}:
        raise ValueError(
            f"{PROMOTIONS}: every recorded promotion must be the genuine final batch "
            f"date {final_date}"
        )


def fixture_label(kinds):
    def key(kind):
        return (FIXTURE_ORDER.get(kind, 99), kind)

    return ", ".join(f"`{kind}`" for kind in sorted(kinds, key=key)) or "—"


def promotion_tiers(root):
    data = json.loads((root / TIERS).read_text())
    if data.get("schemaVersion") != 1:
        raise ValueError(f"{TIERS}: schemaVersion must be 1")
    result = {}
    for tier, languages in data["tiers"].items():
        for language in languages:
            if language in result:
                raise ValueError(f"language {language} appears in multiple tiers")
            result[language] = tier
    return result


def render(root=ROOT):
    languages = catalog(root)
    public = set(languages)
    records = case_records(root, public)
    policy = load_policy(root, VALIDATION_POLICY)
    tiers = promotion_tiers(root)
    performance_floor = finite_number(
        policy["minimumStressMbPerSecond"], "minimumStressMbPerSecond"
    )
    ci_performance = policy["ciPerformance"]
    ci_iterations = ci_performance["iterations"]
    if isinstance(ci_iterations, bool) or not isinstance(ci_iterations, int) or ci_iterations < 1:
        raise ValueError("ciPerformance.iterations must be a positive integer")
    ci_language_floor = finite_number(
        ci_performance["minimumLanguageMbPerSecond"],
        "ciPerformance.minimumLanguageMbPerSecond",
    )
    ci_aggregate_floor = finite_number(
        ci_performance["minimumAggregateMbPerSecond"],
        "ciPerformance.minimumAggregateMbPerSecond",
    )
    strict_contract_date = policy["strictContractIntroduced"]
    final_promotion_batch_date = policy["finalPromotionBatchDate"]
    promotions = promotion_dates(root, public)
    validate_final_promotion_batch(promotions, public, final_promotion_batch_date)
    divergent = divergence_fixtures(root)
    by_language = defaultdict(list)
    for record in records:
        by_language[record["catalog_language"]].append(record)

    stress_languages = {
        language
        for language, language_records in by_language.items()
        if any(record["kind"] == "stress" for record in language_records)
    }
    corpus = catalog_corpus(root)
    if corpus["languages"] != len(stress_languages):
        raise ValueError(
            "catalog-repeated languages does not match stress fixtures: "
            f"{corpus['languages']} != {len(stress_languages)}"
        )
    performance, aggregate_rate = performance_measurements(
        root, stress_languages, performance_floor, corpus
    )

    rows = []
    validated = 0
    oracle_covered = 0
    for language in languages:
        language_records = by_language.get(language, [])
        kinds = {record["kind"] for record in language_records}
        exact = bool(language_records) and all(
            record["clean"] and record["fixture"] not in divergent
            for record in language_records
        )
        fixture_contract = all(record["fixture_shape"] for record in language_records)
        fixture_validated = {"basic", "stress"}.issubset(kinds) and exact and fixture_contract
        if fixture_validated and language not in promotions:
            raise ValueError(
                f"{PROMOTIONS}: validated language {language} needs an explicit promotion date"
            )
        measurement = performance.get(language)
        is_validated = fixture_validated and measurement is not None and measurement["passed"]
        validated += int(is_validated)
        oracle_covered += int(bool(language_records))
        status = "validated" if is_validated else "supported"
        tier = tiers.get(language, "core")
        parity = "exact + coarse" if exact else ("not exact" if language_records else "—")
        sweep = "member" if language in stress_languages else "—"
        sweep_rate = f"{measurement['mbPerSecond']:.2f}" if measurement else "—"
        promoted = promotions.get(language, "—")
        rows.append(
            f"| `{language}` | {tier} | {status} | {fixture_label(kinds)} | {parity} | "
            f"{sweep} | {sweep_rate} | {promoted} |"
        )

    assert_locked_counts(
        policy,
        public=len(languages),
        validated=validated,
        oracle=oracle_covered,
        stress_corpus=len(stress_languages),
    )
    more_supported = len(languages) - validated
    return "\n".join(
        [
            "# Language validation status",
            "",
            "<!-- Generated by tools/generate-language-status.py; do not edit. -->",
            "",
            f"Syntaxmate bundles **{len(languages)} supported public language IDs**. "
            f"**{validated} are validated** by the currently machine-readable contract below; "
            f"**{more_supported} more are supported** by real bundled grammars and the "
            "catalog-wide smoke/budget gate.",
            "",
            "For this ledger, *validated* means `cases.toml` contains both `basic` and "
            "`stress` source/golden pairs (10–30 and 140–260 source lines respectively), "
            "every golden record has `stoppedEarly: false`, "
            "and neither fixture uses a `divergences.toml` exception. The Rust harness "
            "checks both exact scope stacks and coarse syntax classes, and the persisted "
            "process-cold measurement must pass the policy floor. A `smoke` fixture alone "
            "is oracle coverage, not validation to this contract.",
            "",
            "The independent `benchmarks/textmate/validation-policy.json` hard-locks "
            f"expected public, validated, oracle, and stress-corpus counts at "
            f"{policy['expectedCounts']['publicLanguages']} each. Generation fails before "
            "writing this ledger if any count falls below that completed state.",
            "",
            f"The oracle manifest currently covers **{oracle_covered}** public IDs. "
            f"`catalog-repeated` contains the **{len(stress_languages)}** IDs with stress "
            f"fixtures. Every persisted reference sweep member is gated at "
            f"**≥ {performance_floor:g} MB/s**. Shared-runner CI uses {ci_iterations} "
            f"iterations with floors of **≥ {ci_language_floor:g} MB/s per language** "
            f"and **≥ {ci_aggregate_floor:g} MB/s aggregate**. The persisted sweep "
            f"measures **{aggregate_rate:.2f} MB/s aggregate**. "
            f"All {len(promotions)} promotion dates are the actual final-batch date, "
            f"{final_promotion_batch_date}; they are explicitly recorded per language in "
            f"`{PROMOTIONS}`, not inferred during generation. The strict contract was "
            f"introduced on {strict_contract_date}.",
            "",
            "Regenerate with `python3 tools/generate-language-status.py`; verify with "
            "`python3 tools/generate-language-status.py --check`. Performance numbers only "
            "change when `python3 tools/check-textmate-catalog-performance.py "
            "--iterations 3 --write-report` is explicitly run.",
            "",
            "| Language ID | Tier | Status | Oracle fixtures | Parity | Catalog sweep | Sweep MB/s | Promoted |",
            "| --- | --- | --- | --- | --- | --- | ---: | --- |",
            *rows,
            "",
        ]
    )


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true", help="fail if the checked-in ledger is stale")
    args = parser.parse_args()
    try:
        expected = render()
    except (OSError, KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
        print(f"cannot generate {OUTPUT}: {error}", file=sys.stderr)
        return 1
    output = ROOT / OUTPUT
    if args.check:
        actual = output.read_text() if output.exists() else ""
        if actual != expected:
            print(f"{OUTPUT} is stale; run {Path(__file__).name}", file=sys.stderr)
            return 1
        print(f"{OUTPUT} is current")
        return 0
    output.write_text(expected)
    print(f"wrote {OUTPUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
