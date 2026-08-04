#!/usr/bin/env python3
"""Enforce deterministic allocation/peak-live-memory ceilings and report corpus percentiles."""

import argparse
import hashlib
import json
import math
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "benchmarks/textmate/allocation-policy.json"
BINARY = ROOT / "target/release/examples/profile-alloc"
REPORT_SCHEMA_VERSION = 1
REQUIRED_PHASES = (
    "construct",
    "tokenize-first",
    "tokenize-warm",
    "incremental-first",
    "incremental-warm",
    "highlight-lines-first",
    "highlight-lines-warm",
    "prepare-language",
    "prepared-new",
    "prepared-first",
    "prepared-new-warm",
    "prepared-reuse",
)
OUTPUT_GROUPS = (
    ("tokenize-first", "tokenize-warm", "prepared-first", "prepared-reuse"),
    (
        "incremental-first",
        "incremental-warm",
        "highlight-lines-first",
        "highlight-lines-warm",
    ),
)
OUTPUT_PHASES = frozenset(phase for group in OUTPUT_GROUPS for phase in group)
REPORTED_METRICS = (
    "allocationCallsPerKib",
    "allocatedBytesPerKib",
    "peakRetainedBytesPerKib",
)
HEX_DIGEST = re.compile(r"^[0-9a-f]{16}$")
SHA256 = re.compile(r"^[0-9a-f]{64}$")


def positive_integer(value, label):
    if isinstance(value, bool) or not isinstance(value, int) or value < 1:
        raise ValueError(f"{label} must be a positive integer")
    return value


def load_policy(path=POLICY):
    try:
        policy = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read allocation policy from {path}: {error}") from error
    if policy.get("schemaVersion") != 1:
        raise ValueError(f"{path}: schemaVersion must be 1")
    if policy.get("percentileMethod") != "nearest-rank":
        raise ValueError(f"{path}: percentileMethod must be nearest-rank")
    if policy.get("reportedPercentiles") != [50, 95]:
        raise ValueError(f"{path}: reportedPercentiles must be [50, 95]")
    if policy.get("metrics") != list(REPORTED_METRICS):
        raise ValueError(f"{path}: metrics must remain {list(REPORTED_METRICS)!r}")
    corpora = policy.get("corpora")
    if not isinstance(corpora, list) or len(corpora) < 4:
        raise ValueError(f"{path}: at least four allocation corpora are required")

    languages = set()
    for index, corpus in enumerate(corpora):
        label = f"{path}: corpora[{index}]"
        if not isinstance(corpus, dict):
            raise ValueError(f"{label} must be an object")
        language = corpus.get("language")
        if not isinstance(language, str) or not language or language in languages:
            raise ValueError(f"{label}.language must be a unique nonempty string")
        languages.add(language)
        source = corpus.get("path")
        if (
            not isinstance(source, str)
            or not source
            or Path(source).is_absolute()
            or ".." in Path(source).parts
        ):
            raise ValueError(f"{label}.path must be a nonempty repository-relative path")
        positive_integer(corpus.get("bytes"), f"{label}.bytes")
        if not isinstance(corpus.get("sha256"), str) or not SHA256.fullmatch(corpus["sha256"]):
            raise ValueError(f"{label}.sha256 must be lowercase SHA-256")
        ceilings = corpus.get("ceilings")
        if not isinstance(ceilings, dict) or set(ceilings) != set(REQUIRED_PHASES):
            raise ValueError(f"{label}.ceilings must cover every required phase")
        for phase in REQUIRED_PHASES:
            ceiling = ceilings[phase]
            if not isinstance(ceiling, dict) or set(ceiling) != {
                "maxAllocationCalls",
                "maxAllocatedBytes",
                "maxPeakRetainedBytes",
            }:
                raise ValueError(f"{label}.ceilings.{phase} has invalid fields")
            positive_integer(
                ceiling["maxAllocationCalls"],
                f"{label}.ceilings.{phase}.maxAllocationCalls",
            )
            positive_integer(
                ceiling["maxAllocatedBytes"],
                f"{label}.ceilings.{phase}.maxAllocatedBytes",
            )
            positive_integer(
                ceiling["maxPeakRetainedBytes"],
                f"{label}.ceilings.{phase}.maxPeakRetainedBytes",
            )
    return policy


def nonnegative_integer(value, label):
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise ValueError(f"{label} must be a nonnegative integer")
    return value


def validate_protocol(protocol, corpus):
    language = corpus["language"]
    if not isinstance(protocol, dict) or protocol.get("schemaVersion") != 1:
        raise ValueError(f"{language}: profiler schemaVersion must be 1")
    if protocol.get("language") != language:
        raise ValueError(f"{language}: profiler language mismatch")
    if protocol.get("sourceBytes") != corpus["bytes"]:
        raise ValueError(f"{language}: profiler source-byte mismatch")
    phases = protocol.get("phases")
    if not isinstance(phases, dict) or set(phases) != set(REQUIRED_PHASES):
        raise ValueError(f"{language}: profiler phases do not match the policy")

    for phase in REQUIRED_PHASES:
        record = phases[phase]
        label = f"{language}.{phase}"
        if not isinstance(record, dict):
            raise ValueError(f"{label} must be an object")
        for field in (
            "allocations",
            "deallocations",
            "reallocations",
            "allocationCalls",
            "allocatedBytes",
            "deallocatedBytes",
            "peakRetainedBytes",
            "elapsedNanoseconds",
            "items",
        ):
            nonnegative_integer(record.get(field), f"{label}.{field}")
        retained = record.get("retainedBytes")
        if isinstance(retained, bool) or not isinstance(retained, int):
            raise ValueError(f"{label}.retainedBytes must be an integer")
        if record["allocationCalls"] != record["allocations"] + record["reallocations"]:
            raise ValueError(f"{label}: allocationCalls accounting mismatch")
        if record["retainedBytes"] != record["allocatedBytes"] - record["deallocatedBytes"]:
            raise ValueError(f"{label}: retainedBytes accounting mismatch")
        if record["peakRetainedBytes"] < max(record["retainedBytes"], 0):
            raise ValueError(f"{label}: peakRetainedBytes is below boundary retention")

        if phase in OUTPUT_PHASES:
            if record.get("complete") is not True:
                raise ValueError(f"{label}: benchmark output must be complete")
            token_digest = record.get("tokenDigest")
            scope_digest = record.get("scopeDigest")
            if not isinstance(token_digest, str) or not HEX_DIGEST.fullmatch(token_digest):
                raise ValueError(f"{label}.tokenDigest must be lowercase 64-bit hex")
            if not isinstance(scope_digest, str) or not HEX_DIGEST.fullmatch(scope_digest):
                raise ValueError(f"{label}.scopeDigest must be lowercase 64-bit hex")
        elif any(
            record.get(field) is not None
            for field in ("complete", "tokenDigest", "scopeDigest")
        ):
            raise ValueError(f"{label}: non-output phase unexpectedly has output metadata")

    for group in OUTPUT_GROUPS:
        first = phases[group[0]]
        identity = (first["items"], first["tokenDigest"], first["scopeDigest"])
        for phase in group[1:]:
            record = phases[phase]
            candidate = (record["items"], record["tokenDigest"], record["scopeDigest"])
            if candidate != identity:
                raise ValueError(
                    f"{language}: token/scope stream differs in {phase}: "
                    f"{candidate!r} != {identity!r}"
                )
    return phases


def check_source(corpus, root=ROOT):
    path = root / corpus["path"]
    try:
        contents = path.read_bytes()
    except OSError as error:
        raise ValueError(f"{corpus['language']}: cannot read {path}: {error}") from error
    if len(contents) != corpus["bytes"]:
        raise ValueError(f"{corpus['language']}: stale source byte count")
    if hashlib.sha256(contents).hexdigest() != corpus["sha256"]:
        raise ValueError(f"{corpus['language']}: stale source SHA-256")
    return path


def evaluate_ceilings(phases, corpus):
    failures = []
    for phase in REQUIRED_PHASES:
        measured = phases[phase]
        ceiling = corpus["ceilings"][phase]
        for field, ceiling_field in (
            ("allocationCalls", "maxAllocationCalls"),
            ("allocatedBytes", "maxAllocatedBytes"),
            ("peakRetainedBytes", "maxPeakRetainedBytes"),
        ):
            if measured[field] > ceiling[ceiling_field]:
                failures.append(
                    f"{corpus['language']}.{phase}.{field}={measured[field]} exceeds "
                    f"{ceiling[ceiling_field]}"
                )
    return failures


def nearest_rank(values, percentile):
    if not values:
        raise ValueError("cannot take a percentile of an empty sequence")
    if not 0 < percentile <= 100:
        raise ValueError("percentile must be in (0, 100]")
    ordered = sorted(values)
    return ordered[math.ceil(percentile / 100 * len(ordered)) - 1]


def per_kib(value, source_bytes):
    return value / (source_bytes / 1024)


def corpus_percentiles(results, percentiles=(50, 95)):
    report = {}
    for phase in REQUIRED_PHASES:
        values = {
            "allocationCallsPerKib": [
                per_kib(result["phases"][phase]["allocationCalls"], result["bytes"])
                for result in results
            ],
            "allocatedBytesPerKib": [
                per_kib(result["phases"][phase]["allocatedBytes"], result["bytes"])
                for result in results
            ],
            "peakRetainedBytesPerKib": [
                per_kib(result["phases"][phase]["peakRetainedBytes"], result["bytes"])
                for result in results
            ],
        }
        report[phase] = {
            metric: {
                f"p{percentile}": round(nearest_rank(samples, percentile), 3)
                for percentile in percentiles
            }
            for metric, samples in values.items()
        }
    return report


def run(command):
    return subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )


def write_report(path, report):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp")
    try:
        temporary.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--policy", type=Path, default=POLICY)
    parser.add_argument("--binary", type=Path, default=BINARY)
    parser.add_argument("--write-report", type=Path)
    args = parser.parse_args()
    policy_path = args.policy if args.policy.is_absolute() else ROOT / args.policy
    binary = args.binary if args.binary.is_absolute() else ROOT / args.binary
    try:
        policy = load_policy(policy_path)
    except ValueError as error:
        parser.error(str(error))

    if binary == BINARY:
        built = subprocess.run(
            [
                "cargo",
                "build",
                "--release",
                "--locked",
                "--example",
                "profile-alloc",
            ],
            cwd=ROOT,
            check=False,
        )
        if built.returncode:
            return built.returncode
    elif not binary.is_file():
        parser.error(f"custom profiler binary does not exist: {binary}")

    results = []
    failures = []
    for corpus in policy["corpora"]:
        try:
            source = check_source(corpus)
        except ValueError as error:
            failures.append(str(error))
            continue
        measured = run([str(binary), "--json", corpus["language"], str(source)])
        if measured.returncode:
            detail = measured.stderr.strip() or measured.stdout.strip() or "no profiler output"
            failures.append(f"{corpus['language']}: profiler failed: {detail}")
            continue
        try:
            protocol = json.loads(measured.stdout)
            phases = validate_protocol(protocol, corpus)
        except (json.JSONDecodeError, ValueError) as error:
            failures.append(str(error))
            continue
        ceiling_failures = evaluate_ceilings(phases, corpus)
        failures.extend(ceiling_failures)
        results.append(
            {
                "language": corpus["language"],
                "path": corpus["path"],
                "bytes": corpus["bytes"],
                "sha256": corpus["sha256"],
                "passed": not ceiling_failures,
                "phases": phases,
            }
        )

    report = {
        "schemaVersion": REPORT_SCHEMA_VERSION,
        "percentileMethod": "nearest-rank",
        "reportedPercentiles": policy["reportedPercentiles"],
        "passed": not failures and len(results) == len(policy["corpora"]),
        "corpusCount": len(results),
        "percentiles": corpus_percentiles(results, policy["reportedPercentiles"])
        if results
        else {},
        "results": results,
        "failures": failures,
    }
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    print(rendered, end="")
    if args.write_report is not None:
        path = args.write_report if args.write_report.is_absolute() else ROOT / args.write_report
        write_report(path, report)
    for failure in failures:
        print(failure, file=sys.stderr)
    return int(not report["passed"])


if __name__ == "__main__":
    raise SystemExit(main())
