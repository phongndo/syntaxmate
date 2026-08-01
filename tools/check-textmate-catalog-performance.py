#!/usr/bin/env python3
"""Assert and optionally persist catalog process-cold throughput measurements."""

import argparse
import json
import math
import os
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "target/textmate-performance/corpora/manifest.json"
CHECKED_MANIFEST = ROOT / "benchmarks/textmate/corpora.toml"
VALIDATION_POLICY = ROOT / "benchmarks/textmate/validation-policy.json"
REPORT = ROOT / "benchmarks/textmate/catalog-performance.json"
BINARY = ROOT / "target/release/examples/profile-cold"
REPORT_SCHEMA_VERSION = 1


def run(command, *, capture=False):
    return subprocess.run(
        command,
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=capture,
    )


def root_path(path):
    return path if path.is_absolute() else ROOT / path


def load_policy_floor(path=VALIDATION_POLICY):
    try:
        policy = json.loads(path.read_text())
        floor = policy["minimumStressMbPerSecond"]
    except (OSError, json.JSONDecodeError, KeyError) as error:
        raise ValueError(f"cannot read performance floor from {path}: {error}") from error
    if isinstance(floor, bool) or not isinstance(floor, (int, float)):
        raise ValueError(f"{path}: minimumStressMbPerSecond must be a number")
    floor = float(floor)
    if not math.isfinite(floor) or floor < 0:
        raise ValueError(f"{path}: minimumStressMbPerSecond must be finite and nonnegative")
    return floor


def checked_corpora(path=CHECKED_MANIFEST):
    result = {}
    for block in re.split(r"(?m)^\[\[corpus\]\]\s*$", path.read_text())[1:]:
        record = {}
        for key, raw in re.findall(r'(?m)^([A-Za-z0-9_]+)\s*=\s*("[^"]*"|\d+)\s*$', block):
            record[key] = raw[1:-1] if raw.startswith('"') else int(raw)
        if "id" in record:
            result[record["id"]] = record
    return result


def aggregate_mb_per_second(results, iterations):
    """Return total bytes / total measured time."""
    if not results or any(
        record["mbPerSecond"] is None or record["mbPerSecond"] <= 0 for record in results
    ):
        return None
    total_bytes = sum(record["bytes"] * iterations for record in results)
    if all(record.get("elapsedNanoseconds") for record in results):
        elapsed_nanoseconds = sum(record["elapsedNanoseconds"] for record in results)
        return round(total_bytes * 1_000 / elapsed_nanoseconds, 3)
    # Retained for direct unit use; persisted reports always use exact timings.
    total_seconds = sum(record["bytes"] * iterations / (record["mbPerSecond"] * 1_000_000) for record in results)
    return round(total_bytes / total_seconds / 1_000_000, 3)


def make_report(corpus, results, floor, iterations):
    aggregate = aggregate_mb_per_second(results, iterations)
    failures = [record for record in results if not record["passed"]]
    return {
        "schemaVersion": REPORT_SCHEMA_VERSION,
        "corpus": {
            "id": corpus["id"],
            "bytes": corpus["bytes"],
            "sha256": corpus["sha256"],
            "languages": [record["language"] for record in results],
        },
        "measurement": {
            "mode": "process-cold",
            "iterations": iterations,
            "floorMbPerSecond": floor,
            "aggregateMbPerSecond": aggregate,
        },
        "passed": len(results) - len(failures),
        "failed": len(failures),
        "results": results,
    }


def write_report(path, report):
    """Atomically write stable JSON; callers opt in because rates vary by machine."""
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.{os.getpid()}.tmp")
    try:
        temporary.write_text(
            json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
        )
        temporary.replace(path)
    finally:
        temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--policy",
        type=Path,
        default=VALIDATION_POLICY,
        help="validation policy containing the default floor",
    )
    parser.add_argument(
        "--floor",
        type=float,
        help="override the policy floor (normally only useful for diagnostics)",
    )
    parser.add_argument("--iterations", type=int, default=1)
    parser.add_argument("--binary", type=Path, default=BINARY)
    parser.add_argument(
        "--write-report",
        nargs="?",
        type=Path,
        const=REPORT,
        metavar="PATH",
        help=f"persist measurements (default path: {REPORT.relative_to(ROOT)})",
    )
    args = parser.parse_args()
    policy_path = root_path(args.policy)
    try:
        policy_floor = load_policy_floor(policy_path)
    except ValueError as error:
        parser.error(str(error))
    floor = policy_floor if args.floor is None else args.floor
    if not math.isfinite(floor) or floor < 0 or args.iterations < 1:
        parser.error("--floor must be finite and nonnegative and --iterations must be positive")

    built_corpora = run([sys.executable, "tools/build-textmate-corpora.py"], capture=True)
    if built_corpora.returncode:
        print("failed to build TextMate corpora", file=sys.stderr)
        if built_corpora.stderr:
            print(built_corpora.stderr.rstrip(), file=sys.stderr)
        return 1
    binary = root_path(args.binary)
    if binary == BINARY:
        built = run(
            [
                "cargo", "build", "-q", "-p", "syntaxmate", "--release", "--locked",
                "--example", "profile-cold",
            ]
        )
        if built.returncode:
            return built.returncode

    manifest = json.loads(MANIFEST.read_text())
    checked = checked_corpora()
    for generated in manifest["corpora"]:
        committed = checked.get(generated["id"])
        if committed is None:
            print(f"{CHECKED_MANIFEST}: missing corpus {generated['id']}", file=sys.stderr)
            return 1
        if committed.get("content_contract") == "dynamic":
            continue
        for key in ("bytes", "tokenizer_lines", "sha256", "languages", "generated_languages"):
            if key in generated and committed.get(key) != generated[key]:
                print(
                    f"{CHECKED_MANIFEST}: stale {generated['id']}.{key}: "
                    f"{committed.get(key)!r} != {generated[key]!r}",
                    file=sys.stderr,
                )
                return 1
    corpus = next(item for item in manifest["corpora"] if item["id"] == "catalog-repeated")
    results = []
    for fixture in corpus["files"]:
        measured = run(
            [
                str(binary),
                "--mode", "process-cold",
                "--json",
                "--assets", "assets/grammars/languages",
                "--scope", fixture["scope"],
                fixture["path"],
                str(args.iterations),
            ],
            capture=True,
        )
        protocol = None
        try:
            candidate = json.loads(measured.stdout)
            if (
                candidate.get("schemaVersion") == 1
                and candidate.get("mode") == "process-cold"
                and candidate.get("iterations") == args.iterations
                and candidate.get("bytesPerIteration") == fixture["bytes"]
                and candidate.get("processedBytes") == fixture["bytes"] * args.iterations
                and isinstance(candidate.get("elapsedNanoseconds"), int)
                and not isinstance(candidate.get("elapsedNanoseconds"), bool)
                and candidate["elapsedNanoseconds"] > 0
            ):
                protocol = candidate
        except (json.JSONDecodeError, AttributeError):
            pass
        elapsed_nanoseconds = protocol["elapsedNanoseconds"] if protocol else None
        exact_mb_s = (
            protocol["processedBytes"] * 1_000 / elapsed_nanoseconds if protocol else None
        )
        mb_s = round(exact_mb_s, 3) if exact_mb_s is not None else None
        passed = measured.returncode == 0 and exact_mb_s is not None and exact_mb_s >= floor
        record = {
            "language": fixture["language"],
            "scope": fixture["scope"],
            "bytes": fixture["bytes"],
            "sha256": fixture["sha256"],
            "processedBytes": fixture["bytes"] * args.iterations,
            "elapsedNanoseconds": elapsed_nanoseconds,
            "mbPerSecond": mb_s,
            "passed": passed,
        }
        results.append(record)
        if not passed:
            detail = measured.stderr.strip() or measured.stdout.strip() or "no benchmark output"
            print(f"{fixture['language']}: performance measurement failed\n{detail}", file=sys.stderr)

    report = make_report(corpus, results, floor, args.iterations)
    rendered = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
    print(rendered, end="")
    if args.write_report is not None:
        if report["measurement"]["aggregateMbPerSecond"] is None:
            print("refusing to write an incomplete performance report", file=sys.stderr)
            return 1
        report_path = root_path(args.write_report)
        write_report(report_path, report)
        try:
            display_path = report_path.relative_to(ROOT)
        except ValueError:
            display_path = report_path
        print(f"wrote {display_path}", file=sys.stderr)
    return int(report["failed"] != 0)


if __name__ == "__main__":
    raise SystemExit(main())
