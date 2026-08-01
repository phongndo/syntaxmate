#!/usr/bin/env python3
"""Build or check deterministic Markdown and repeated TextMate corpora."""

import argparse
import hashlib
import json
import re
import shutil
import sys
from pathlib import Path

try:
    from textmate_validation import assert_locked_counts, contract_snapshot, load_policy
except ModuleNotFoundError:  # Imported as tools.build_textmate_corpora in tests.
    from tools.textmate_validation import assert_locked_counts, contract_snapshot, load_policy


ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "target" / "textmate-performance" / "corpora"
MARKDOWN_SOURCES = [
    "README.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "docs/compatibility.md",
    "docs/textmate-engine.md",
    "docs/textmate-theme-engine.md",
]
CASES = ROOT / "tests/fixtures/textmate/cases.toml"
CATALOG = ROOT / "assets/grammars/coverage.toml"
CHECKED_MANIFEST = ROOT / "benchmarks/textmate/corpora.toml"
# Keep new catalog members the same approximate size as the historical
# core-repeated members. This deliberately does not resize core-repeated.
CORE_TARGET_BYTES = 3_240_000
CORE_BASELINE_LANGUAGES = 39
# Frozen for cross-round comparability. New stress fixtures belong only to
# catalog-repeated; do not grow or rebalance this historical corpus.
CORE_LANGUAGES = {
    "asm", "bash", "c", "cpp", "csharp", "css", "docker", "go", "html",
    "java", "javascript", "json", "jsx", "kotlin", "llvm", "lua", "make",
    "markdown", "mipsasm", "mlir", "mojo", "nix", "ocaml", "odin", "php",
    "powershell", "python", "riscv", "ruby", "rust", "scss", "sql", "swift",
    "terraform", "toml", "tsx", "typescript", "yaml", "zig",
}


def tokenizer_lines(data: bytes) -> int:
    return data.count(b"\n") + 1


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def build_markdown() -> dict:
    chunks = []
    for relative in MARKDOWN_SOURCES:
        data = (ROOT / relative).read_bytes()
        chunks.append(f"<!-- corpus-source: {relative} -->\n".encode())
        chunks.append(data)
        if not data.endswith(b"\n"):
            chunks.append(b"\n")
    data = b"".join(chunks)
    path = OUT / "representative-markdown.md"
    path.write_bytes(data)
    return {
        "id": "representative-markdown",
        "path": str(path.relative_to(ROOT)),
        "language": "markdown",
        "scope": "text.html.markdown",
        "bytes": len(data),
        "tokenizer_lines": tokenizer_lines(data),
        "sha256": digest(data),
        "sources": MARKDOWN_SOURCES,
    }


def build_core() -> dict:
    cases = []
    for block in re.split(r"(?m)^\[\[case\]\]\s*$", CASES.read_text())[1:]:
        language = re.search(r'(?m)^language\s*=\s*"([^"]+)"', block)
        fixture = re.search(r'(?m)^fixture\s*=\s*"([^"]+)"', block)
        if language and fixture and language.group(1) in CORE_LANGUAGES:
            cases.append({"language": language.group(1), "fixture": fixture.group(1)})
    largest = {}
    for case in cases:
        path = ROOT / case["fixture"]
        size = path.stat().st_size
        if case["language"] not in largest or size > largest[case["language"]][0]:
            largest[case["language"]] = (size, path)
    if set(largest) != CORE_LANGUAGES:
        missing = sorted(CORE_LANGUAGES - set(largest))
        unexpected = sorted(set(largest) - CORE_LANGUAGES)
        raise ValueError(
            f"core-repeated fixture set changed: missing={missing} unexpected={unexpected}"
        )

    target_per_language = CORE_TARGET_BYTES // CORE_BASELINE_LANGUAGES
    output_dir = OUT / "core-repeated"
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    files = []
    aggregate = hashlib.sha256()
    generated_aggregate = hashlib.sha256()
    total_bytes = 0
    total_lines = 0
    for language, (_, source) in sorted(largest.items()):
        seed = source.read_bytes()
        if not seed.endswith(b"\n"):
            seed += b"\n"
        repeats = max(1, (target_per_language + len(seed) - 1) // len(seed))
        data = seed * repeats
        language_dir = output_dir / language
        language_dir.mkdir(parents=True, exist_ok=True)
        path = language_dir / source.name
        path.write_bytes(data)
        relative = str(path.relative_to(ROOT))
        generated_aggregate.update(relative.encode())
        generated_aggregate.update(b"\0")
        generated_aggregate.update(data)
        # The checked-in Make smoke fixture uses a .mk basename that Mark does
        # not detect. Keep generating it for coverage, but preserve the
        # historical detected-language acceptance corpus.
        detected = language != "make"
        if detected:
            aggregate.update(relative.encode())
            aggregate.update(b"\0")
            aggregate.update(data)
            total_bytes += len(data)
            total_lines += tokenizer_lines(data)
        files.append(
            {
                "language": language,
                "path": relative,
                "bytes": len(data),
                "tokenizer_lines": tokenizer_lines(data),
                "sha256": digest(data),
                "detected": detected,
            }
        )
    return {
        "id": "core-repeated",
        "languages": sum(file["detected"] for file in files),
        "generated_languages": len(files),
        "bytes": total_bytes,
        "tokenizer_lines": total_lines,
        "sha256": aggregate.hexdigest(),
        "generated_sha256": generated_aggregate.hexdigest(),
        "files": files,
    }


def public_languages() -> set:
    text = CATALOG.read_text()
    match = re.search(r"(?ms)^kept\s*=\s*\[(.*?)^\]", text)
    if not match:
        raise ValueError(f"missing kept language list in {CATALOG}")
    return set(re.findall(r'"([^"]+)"', match.group(1)))


def build_catalog() -> dict:
    """Build the complete sweep from explicitly named stress fixtures only."""
    public = public_languages()
    stress_cases = {}
    for block in re.split(r"(?m)^\[\[case\]\]\s*$", CASES.read_text())[1:]:
        language = re.search(r'(?m)^language\s*=\s*"([^"]+)"', block)
        scope = re.search(r'(?m)^scope\s*=\s*"([^"]+)"', block)
        grammar = re.search(r'(?m)^grammar\s*=\s*"([^"]+)"', block)
        fixture = re.search(r'(?m)^fixture\s*=\s*"([^"]+)"', block)
        if not all((language, scope, grammar, fixture)):
            continue
        source = ROOT / fixture.group(1)
        if source.name.split(".", 1)[0] != "stress":
            continue

        fixture_language = language.group(1)
        grammar_language = Path(grammar.group(1)).name.removesuffix(".tmLanguage.json")
        if fixture_language in public:
            catalog_language = fixture_language
        elif grammar_language in public:
            # The historical `bash` fixtures exercise the public `shellscript`
            # grammar id. Keep core-repeated's old path, but use the public id
            # in this catalog-oriented corpus.
            catalog_language = grammar_language
        else:
            raise ValueError(
                f"stress fixture {source} cannot be mapped to a public language"
            )
        if catalog_language in stress_cases:
            raise ValueError(f"duplicate stress fixture for {catalog_language}")
        stress_cases[catalog_language] = {
            "fixture_language": fixture_language,
            "scope": scope.group(1),
            "source": source,
        }

    target_per_language = CORE_TARGET_BYTES // CORE_BASELINE_LANGUAGES
    output_dir = OUT / "catalog-repeated"
    if output_dir.exists():
        shutil.rmtree(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    files = []
    aggregate = hashlib.sha256()
    total_bytes = 0
    total_lines = 0
    for language, case in sorted(stress_cases.items()):
        source = case["source"]
        seed = source.read_bytes()
        if not seed.endswith(b"\n"):
            seed += b"\n"
        repeats = max(1, (target_per_language + len(seed) - 1) // len(seed))
        data = seed * repeats
        language_dir = output_dir / language
        language_dir.mkdir(parents=True, exist_ok=True)
        path = language_dir / source.name
        path.write_bytes(data)
        relative = str(path.relative_to(ROOT))
        aggregate.update(relative.encode())
        aggregate.update(b"\0")
        aggregate.update(data)
        total_bytes += len(data)
        total_lines += tokenizer_lines(data)
        files.append(
            {
                "language": language,
                "fixture_language": case["fixture_language"],
                "scope": case["scope"],
                "source": str(source.relative_to(ROOT)),
                "path": relative,
                "bytes": len(data),
                "tokenizer_lines": tokenizer_lines(data),
                "sha256": digest(data),
            }
        )
    return {
        "id": "catalog-repeated",
        "languages": len(files),
        "bytes": total_bytes,
        "tokenizer_lines": total_lines,
        "sha256": aggregate.hexdigest(),
        "files": files,
    }


def checked_corpora(path=CHECKED_MANIFEST):
    result = {}
    for block in re.split(r"(?m)^\[\[corpus\]\]\s*$", path.read_text())[1:]:
        record = {}
        for key, raw in re.findall(
            r'(?m)^([A-Za-z0-9_]+)\s*=\s*("[^"]*"|\d+)\s*$', block
        ):
            record[key] = raw[1:-1] if raw.startswith('"') else int(raw)
        if "id" in record:
            result[record["id"]] = record
    return result


def check_committed_manifest(manifest, checked=None):
    checked = checked if checked is not None else checked_corpora()
    for generated in manifest["corpora"]:
        committed = checked.get(generated["id"])
        if committed is None:
            raise ValueError(f"{CHECKED_MANIFEST}: missing corpus {generated['id']}")
        if committed.get("content_contract") == "dynamic":
            continue
        for key in (
            "bytes",
            "tokenizer_lines",
            "sha256",
            "languages",
            "generated_languages",
        ):
            if key in generated and committed.get(key) != generated[key]:
                raise ValueError(
                    f"{CHECKED_MANIFEST}: stale {generated['id']}.{key}: "
                    f"{committed.get(key)!r} != {generated[key]!r}"
                )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the committed corpus manifest or locked membership is stale",
    )
    args = parser.parse_args()
    try:
        policy = load_policy(ROOT)
        snapshot = contract_snapshot(ROOT)
        assert_locked_counts(
            policy,
            public=len(snapshot.public_ids),
            validated=len(snapshot.validated_ids),
            oracle=len(snapshot.oracle_ids),
            stress_corpus=len(snapshot.stress_ids),
        )
    except (OSError, ValueError) as error:
        print(f"cannot build TextMate corpora: {error}", file=sys.stderr)
        return 1

    OUT.mkdir(parents=True, exist_ok=True)
    manifest = {
        "version": 1,
        "corpora": [build_markdown(), build_core(), build_catalog()],
    }
    path = OUT / "manifest.json"
    path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    if args.check:
        try:
            check_committed_manifest(manifest)
        except (OSError, ValueError) as error:
            print(error, file=sys.stderr)
            return 1
        count = len(snapshot.public_ids)
        print(f"TextMate corpus manifest and locked {count}/{count} membership are current")
        return 0
    print(json.dumps({"manifest": str(path.relative_to(ROOT)), **manifest}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
