#!/usr/bin/env python3
"""Run reproducible engine and end-to-end highlighter comparisons."""

import argparse
import datetime
import json
import math
import platform
import statistics
import subprocess
import sys
import time
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CASES = ROOT / "tests/fixtures/textmate/cases.toml"
ASSETS = ROOT / "assets/grammars/languages"
COMPETITORS = ROOT / "benchmarks/competitors"
SYNTAXMATE_ENGINE = ROOT / "target/release/examples/profile-engine"
SYNTAXMATE_PRODUCT = ROOT / "target/release/examples/profile-product"
SHIKI = COMPETITORS / "shiki-driver.mjs"
VSCODE = COMPETITORS / "vscode-textmate-driver.mjs"
SYNTECT = (
    COMPETITORS
    / "syntect-driver/target/release/syntaxmate-syntect-benchmark-driver"
)
DEFAULT_LANGUAGES = (
    "bash",
    "cpp",
    "html",
    "java",
    "json",
    "markdown",
    "python",
    "rust",
    "typescript",
    "yaml",
)
SYNTECT_EXTENSIONS = {
    "bash": "sh",
    "cpp": "cpp",
    "html": "html",
    "java": "java",
    "json": "json",
    "markdown": "md",
    "python": "py",
    "rust": "rs",
    "yaml": "yaml",
}
TRACKS = {
    "engine": {
        "phases": ("first", "steady", "replay"),
        "engines": ("syntaxmate", "vscode-textmate"),
    },
    "end-to-end": {
        "phases": ("cold", "steady", "replay"),
        "engines": ("syntaxmate", "shiki", "syntect"),
    },
}


def run_checked(command):
    subprocess.run(command, cwd=ROOT, check=True)


def prepare():
    run_checked(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--example",
            "profile-engine",
            "--example",
            "profile-product",
        ]
    )
    run_checked(["npm", "ci", "--prefix", str(COMPETITORS), "--ignore-scripts"])
    run_checked(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--manifest-path",
            str(COMPETITORS / "syntect-driver/Cargo.toml"),
        ]
    )


def stress_cases():
    records = tomllib.loads(CASES.read_text())["case"]
    result = {}
    for record in records:
        if Path(record["fixture"]).name.startswith("stress."):
            result[record["language"]] = record
    return result


def balanced_order(engines, sample):
    if len(engines) == 2:
        return engines if sample % 2 == 0 else tuple(reversed(engines))
    if len(engines) == 3:
        orders = (
            engines,
            (engines[1], engines[2], engines[0]),
            (engines[2], engines[0], engines[1]),
            tuple(reversed(engines)),
            (engines[0], engines[2], engines[1]),
            (engines[1], engines[0], engines[2]),
        )
        return orders[sample % len(orders)]
    offset = sample % len(engines)
    return engines[offset:] + engines[:offset]


def command_for(track, engine, phase, case, minimum_time_ms):
    fixture = str(ROOT / case["fixture"])
    timing = ["--minimum-time-ms", str(minimum_time_ms)]
    if track == "engine" and engine == "syntaxmate":
        return [
            str(SYNTAXMATE_ENGINE),
            "--assets",
            str(ASSETS),
            "--scope",
            case["scope"],
            "--file",
            fixture,
            "--phase",
            phase,
            *timing,
        ]
    if track == "engine" and engine == "vscode-textmate":
        return [
            "node",
            str(VSCODE),
            "--assets",
            str(ASSETS),
            "--scope",
            case["scope"],
            "--file",
            fixture,
            "--phase",
            phase,
            *timing,
        ]
    if track == "end-to-end" and engine == "syntaxmate":
        return [
            str(SYNTAXMATE_PRODUCT),
            "--language",
            case["language"],
            "--file",
            fixture,
            "--phase",
            phase,
            *timing,
        ]
    if track == "end-to-end" and engine == "shiki":
        return [
            "node",
            str(SHIKI),
            "--language",
            case["language"],
            "--file",
            fixture,
            "--phase",
            phase,
            *timing,
        ]
    if track == "end-to-end" and engine == "syntect":
        return [
            str(SYNTECT),
            "--extension",
            SYNTECT_EXTENSIONS[case["language"]],
            "--file",
            fixture,
            "--phase",
            phase,
            *timing,
        ]
    raise ValueError(f"unsupported driver: {track}/{engine}")


def run_driver(command):
    started = time.perf_counter_ns()
    process = subprocess.run(
        command,
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    wall_nanos = time.perf_counter_ns() - started
    if process.returncode:
        raise RuntimeError(
            f"driver failed ({process.returncode}): {' '.join(command)}\n"
            f"{process.stderr or process.stdout}"
        )
    try:
        result = json.loads(process.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(
            f"invalid JSON from {' '.join(command)}: {process.stdout}"
        ) from error
    result["wallNanoseconds"] = wall_nanos
    return result


def nearest_rank(values, percentile):
    ordered = sorted(values)
    rank = max(1, math.ceil(percentile * len(ordered)))
    return ordered[rank - 1]


def metric_nanos(sample):
    if sample["track"] == "end-to-end" and sample["phase"] == "cold":
        return sample["wallNanoseconds"]
    return sample["elapsedNanoseconds"] / sample["iterations"]


def summarize(samples, track_languages):
    summaries = {}
    aggregates = {}
    for track, contract in TRACKS.items():
        summaries[track] = {}
        aggregates[track] = {}
        for phase in contract["phases"]:
            summaries[track][phase] = {}
            aggregates[track][phase] = {}
            corpus_medians = {engine: [] for engine in contract["engines"]}
            for language in track_languages[track]:
                summaries[track][phase][language] = {}
                for engine in contract["engines"]:
                    selected = [
                        sample
                        for sample in samples
                        if sample["track"] == track
                        and sample["phase"] == phase
                        and sample["language"] == language
                        and sample["engine"] == engine
                    ]
                    latencies = [metric_nanos(sample) for sample in selected]
                    throughputs = [
                        sample["sourceBytes"] * 1_000 / latency
                        for sample, latency in zip(selected, latencies, strict=True)
                    ]
                    median_latency = statistics.median(latencies)
                    corpus_medians[engine].append(
                        (selected[0]["sourceBytes"], median_latency)
                    )
                    summary = {
                        "samples": len(selected),
                        "latencyNanosecondsP50": round(median_latency),
                        "latencyNanosecondsP95": round(nearest_rank(latencies, 0.95)),
                        "megabytesPerSecondP50": round(statistics.median(throughputs), 3),
                        "setupNanosecondsP50": round(
                            statistics.median(
                                sample["setupNanoseconds"] for sample in selected
                            )
                        ),
                    }
                    if track == "engine":
                        summary.update(
                            {
                                "scopeDigest": selected[0]["scopeDigest"],
                                "tokens": selected[0]["tokens"],
                            }
                        )
                    else:
                        summary.update(
                            {
                                "outputBytes": selected[0]["outputBytes"],
                                "outputDigest": selected[0]["outputDigest"],
                            }
                        )
                    summaries[track][phase][language][engine] = summary
            syntaxmate_elapsed = sum(
                elapsed for _, elapsed in corpus_medians["syntaxmate"]
            )
            for engine, medians in corpus_medians.items():
                total_bytes = sum(source_bytes for source_bytes, _ in medians)
                total_elapsed = sum(elapsed for _, elapsed in medians)
                aggregates[track][phase][engine] = {
                    "corpora": len(medians),
                    "totalSourceBytes": total_bytes,
                    "aggregateMegabytesPerSecond": round(
                        total_bytes * 1_000 / total_elapsed, 3
                    ),
                    "meanLatencyMilliseconds": round(
                        total_elapsed / len(medians) / 1_000_000, 3
                    ),
                    "syntaxmateSpeedup": round(total_elapsed / syntaxmate_elapsed, 3),
                }
    return summaries, aggregates


def validate(samples, sample_count, track_languages):
    for track, contract in TRACKS.items():
        for phase in contract["phases"]:
            for language in track_languages[track]:
                selected = [
                    sample
                    for sample in samples
                    if sample["track"] == track
                    and sample["phase"] == phase
                    and sample["language"] == language
                ]
                expected = sample_count * len(contract["engines"])
                if len(selected) != expected:
                    raise RuntimeError(
                        f"{track}/{phase}/{language}: expected {expected} samples, "
                        f"found {len(selected)}"
                    )
                if not all(sample.get("complete") for sample in selected):
                    raise RuntimeError(f"{track}/{phase}/{language}: incomplete output")
                if len({sample["sourceBytes"] for sample in selected}) != 1:
                    raise RuntimeError(f"{track}/{phase}/{language}: source bytes differ")
                for engine in contract["engines"]:
                    engine_samples = [
                        sample for sample in selected if sample["engine"] == engine
                    ]
                    stable_fields = (
                        ("scopeDigest", "tokens")
                        if track == "engine"
                        else ("outputDigest", "outputBytes")
                    )
                    for field in stable_fields:
                        if len({sample[field] for sample in engine_samples}) != 1:
                            raise RuntimeError(
                                f"{track}/{phase}/{language}/{engine}: {field} is unstable"
                            )
                if track == "engine":
                    digests = {sample["scopeDigest"] for sample in selected}
                    if len(digests) != 1:
                        raise RuntimeError(
                            f"engine/{phase}/{language}: normalized scope digests differ"
                        )
    for track, contract in TRACKS.items():
        digest_field = "scopeDigest" if track == "engine" else "outputDigest"
        for language in track_languages[track]:
            for engine in contract["engines"]:
                digests = {
                    sample[digest_field]
                    for sample in samples
                    if sample["track"] == track
                    and sample["language"] == language
                    and sample["engine"] == engine
                }
                if len(digests) != 1:
                    raise RuntimeError(
                        f"{track}/{language}/{engine}: {digest_field} differs across phases"
                    )


def environment():
    details = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "python": platform.python_version(),
        "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
        "node": subprocess.check_output(["node", "--version"], text=True).strip(),
    }
    if sys.platform == "darwin":
        details["cpu"] = subprocess.check_output(
            ["sysctl", "-n", "machdep.cpu.brand_string"], text=True
        ).strip()
        details["memoryBytes"] = int(
            subprocess.check_output(["sysctl", "-n", "hw.memsize"], text=True)
        )
    elif platform.processor():
        details["cpu"] = platform.processor()
    return details


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--samples", type=int, default=7)
    parser.add_argument("--minimum-time-ms", type=int, default=100)
    parser.add_argument("--languages", default=",".join(DEFAULT_LANGUAGES))
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--include-samples", action="store_true")
    parser.add_argument("--skip-prepare", action="store_true")
    args = parser.parse_args()
    if args.samples < 3:
        parser.error("--samples must be at least 3")
    if args.minimum_time_ms < 10:
        parser.error("--minimum-time-ms must be at least 10")
    languages = tuple(dict.fromkeys(filter(None, args.languages.split(","))))
    available = stress_cases()
    missing = sorted(set(languages) - set(available))
    if missing:
        parser.error(f"languages lack stress fixtures: {', '.join(missing)}")
    track_languages = {
        "engine": languages,
        "end-to-end": tuple(
            language for language in languages if language in SYNTECT_EXTENSIONS
        ),
    }
    if not track_languages["end-to-end"]:
        parser.error("no selected language has a bundled syntax in all three products")

    if not args.skip_prepare:
        prepare()

    samples = []
    total = sum(
        len(contract["phases"])
        * len(contract["engines"])
        * len(track_languages[track])
        * args.samples
        for track, contract in TRACKS.items()
    )
    completed = 0
    set_index = 0
    for track, contract in TRACKS.items():
        for phase in contract["phases"]:
            for language in track_languages[track]:
                case = {**available[language], "language": language}
                for sample_index in range(args.samples):
                    order = balanced_order(
                        contract["engines"], sample_index + set_index
                    )
                    for order_index, engine in enumerate(order):
                        result = run_driver(
                            command_for(
                                track,
                                engine,
                                phase,
                                case,
                                args.minimum_time_ms,
                            )
                        )
                        if result.get("track") != track or result.get("engine") != engine:
                            raise RuntimeError(
                                f"driver identity mismatch for {track}/{engine}: {result}"
                            )
                        result.update(
                            {
                                "language": language,
                                "fixture": case["fixture"],
                                "scope": case["scope"],
                                "sample": sample_index,
                                "scheduleOffset": set_index,
                                "order": list(order),
                                "orderIndex": order_index,
                            }
                        )
                        samples.append(result)
                        completed += 1
                        print(
                            f"[{completed:>3}/{total}] {track} {phase} "
                            f"{language} {engine}",
                            file=sys.stderr,
                        )
                set_index += 1

    validate(samples, args.samples, track_languages)
    summaries, aggregates = summarize(samples, track_languages)
    versions = {}
    for sample in samples:
        versions[sample["engine"]] = sample["version"]
        if "regexEngine" in sample:
            versions[f"{sample['engine']}Regex"] = sample["regexEngine"]
    report = {
        "schemaVersion": 1,
        "measuredAt": datetime.date.today().isoformat(),
        "samplesPerSet": args.samples,
        "minimumWarmSampleMilliseconds": args.minimum_time_ms,
        "percentileMethod": "nearest-rank",
        "contracts": {
            "engine": (
                "identical TextMate grammar assets and source fixtures; grammar setup "
                "excluded; normalized UTF-8 scope streams must match; steady disables "
                "Syntaxmate's line-result cache; replay retains normal caches"
            ),
            "end-to-end": (
                "same source fixtures through each product's normal bundled syntax, "
                "theme, highlighting, and HTML API; process-cold includes process launch; "
                "steady disables Syntaxmate's line-result cache; replay retains normal caches"
            ),
        },
        "environment": environment(),
        "versions": versions,
        "languages": {
            track: list(selected) for track, selected in track_languages.items()
        },
        "aggregates": aggregates,
        "summaries": summaries,
        "sampleCount": len(samples),
    }
    if args.include_samples:
        report["samples"] = samples
    output = args.out if args.out.is_absolute() else ROOT / args.out
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    print(f"wrote {output}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"competitive benchmark failed: {error}", file=sys.stderr)
        raise SystemExit(1)
