#!/usr/bin/env python3
"""Compare Syntaxmate and vscode-textmate on identical grammars and sources."""

import argparse
import json
import platform
import subprocess
import sys
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CASES = ROOT / "tests/fixtures/textmate/cases.toml"
ASSETS = ROOT / "assets/grammars/languages"
SYNTAXMATE = ROOT / "target/release/examples/profile-cold"
VSCODE = ROOT / "tools/textmate-bench.mjs"
ORACLE_PACKAGE = ROOT / "tools/golden-oracle/package.json"
DEFAULT_LANGUAGES = (
    "bash", "cpp", "html", "java", "json", "markdown", "python", "rust",
    "typescript", "yaml",
)


def run(command):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(map(str, command))}\n"
            f"{result.stderr or result.stdout}"
        )
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"invalid JSON from {command[0]}: {result.stdout}") from error


def stress_cases():
    records = tomllib.loads(CASES.read_text())["case"]
    return {
        record["language"]: record
        for record in records
        if Path(record["fixture"]).name.startswith("stress.")
    }


def aggregate(records, engine):
    bytes_processed = sum(record["bytes"] for record in records)
    elapsed_ns = sum(record["elapsedNanoseconds"] for record in records)
    return {
        "engine": engine,
        "bytes": bytes_processed,
        "elapsedNanoseconds": elapsed_ns,
        "megabytesPerSecond": round(bytes_processed * 1_000 / elapsed_ns, 3),
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--iterations", type=int, default=5)
    parser.add_argument(
        "--languages",
        default=",".join(DEFAULT_LANGUAGES),
        help="comma-separated public language IDs",
    )
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    if args.iterations < 1:
        parser.error("--iterations must be positive")
    languages = tuple(dict.fromkeys(filter(None, args.languages.split(","))))
    available = stress_cases()
    oracle_dependencies = json.loads(ORACLE_PACKAGE.read_text())["dependencies"]
    oracle_versions = {
        "vscodeTextmate": oracle_dependencies["vscode-textmate"],
        "vscodeOniguruma": oracle_dependencies["vscode-oniguruma"],
    }
    missing = sorted(set(languages) - set(available))
    if missing:
        parser.error(f"languages lack stress fixtures: {', '.join(missing)}")

    subprocess.run(
        ["cargo", "build", "--release", "--locked", "--example", "profile-cold"],
        cwd=ROOT,
        check=True,
    )
    results = []
    for language in languages:
        case = available[language]
        syntaxmate = run([
            str(SYNTAXMATE), "--mode", "same-driver", "--json",
            "--assets", str(ASSETS), "--scope", case["scope"],
            case["fixture"], str(args.iterations),
        ])
        vscode = run([
            "node", str(VSCODE), "--mode", "same-driver", "--json",
            "--assets", str(ASSETS), "--scope", case["scope"],
            "--file", case["fixture"], "--iterations", str(args.iterations),
        ])
        source_bytes = syntaxmate["bytesPerIteration"]
        if source_bytes != vscode["sourceBytes"]:
            raise RuntimeError(f"{language}: benchmark drivers read different source bytes")
        processed = source_bytes * args.iterations
        if (
            syntaxmate["mode"] != "same-driver"
            or syntaxmate["iterations"] != args.iterations
            or syntaxmate["processedBytes"] != processed
            or vscode["mode"] != "same-driver"
            or vscode["iterations"] != args.iterations
            or vscode["bytes"] != processed
        ):
            raise RuntimeError(f"{language}: benchmark driver accounting mismatch")
        if vscode["stoppedEarly"]:
            raise RuntimeError(f"{language}: vscode-textmate stopped before completing the source")
        for key, expected in oracle_versions.items():
            if vscode["versions"][key] != expected:
                raise RuntimeError(
                    f"{language}: {key} version {vscode['versions'][key]!r} "
                    f"does not match pinned {expected!r}"
                )
        results.append({
            "language": language,
            "scope": case["scope"],
            "fixture": case["fixture"],
            "sourceBytes": source_bytes,
            "syntaxmate": {
                "bytes": processed,
                "elapsedNanoseconds": syntaxmate["elapsedNanoseconds"],
                "megabytesPerSecond": round(
                    processed * 1_000 / syntaxmate["elapsedNanoseconds"], 3
                ),
                "tokens": syntaxmate["tokens"],
            },
            "vscodeTextmate": {
                "bytes": vscode["bytes"],
                "elapsedNanoseconds": vscode["highlightMicros"] * 1_000,
                "megabytesPerSecond": round(vscode["megabytesPerSecond"], 3),
                "tokens": vscode["tokens"],
                "stoppedEarly": vscode["stoppedEarly"],
            },
        })

    syntaxmate_records = [record["syntaxmate"] for record in results]
    vscode_records = [record["vscodeTextmate"] for record in results]
    report = {
        "schemaVersion": 1,
        "mode": "same-driver",
        "iterations": args.iterations,
        "contract": "identical TextMate grammar assets and source fixtures; setup excluded",
        "oracle": oracle_versions,
        "environment": {
            "platform": platform.platform(),
            "python": platform.python_version(),
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "node": subprocess.check_output(["node", "--version"], text=True).strip(),
        },
        "aggregate": {
            "syntaxmate": aggregate(syntaxmate_records, "syntaxmate"),
            "vscodeTextmate": aggregate(vscode_records, "vscode-textmate"),
        },
        "results": results,
    }
    rendered = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.out:
        output = args.out if args.out.is_absolute() else ROOT / args.out
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(rendered)
        print(f"wrote {output.relative_to(ROOT) if output.is_relative_to(ROOT) else output}")
    else:
        print(rendered, end="")


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.CalledProcessError) as error:
        print(f"performance comparison failed: {error}", file=sys.stderr)
        raise SystemExit(1)
