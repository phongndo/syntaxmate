#!/usr/bin/env python3
"""Check or regenerate public language-count documentation snippets."""

import argparse
import datetime
import json
import math
import re
import sys
from dataclasses import dataclass
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
STATUS = Path("docs/language-status.md")
COVERAGE = Path("assets/grammars/coverage.toml")
CASES = Path("tests/fixtures/textmate/cases.toml")
CORPORA = Path("benchmarks/textmate/corpora.toml")
SCALE_POLICY = Path("tools/textmate-golden-scale-policy.json")
VALIDATION_POLICY = Path("benchmarks/textmate/validation-policy.json")
FIXTURE_README = Path("tests/fixtures/textmate/README.md")
MANAGED_DOCS = (
    Path("docs/textmate-engine.md"),
    FIXTURE_README,
)
TOP_README = Path("README.md")
COUNT_START = "<!-- BEGIN GENERATED: language-counts -->"
COUNT_END = "<!-- END GENERATED: language-counts -->"
SCALE_START = "<!-- BEGIN GENERATED: golden-scale-policy -->"
SCALE_END = "<!-- END GENERATED: golden-scale-policy -->"
TOP_README_COUNT_CLAIM = re.compile(
    r"(?is)(?:"
    r"\b(?:supports?|bundles?)\b[^\n]{0,60}\b\d+\b[^\n]{0,40}\blanguages?\b|"
    r"\b\d+\b[^\n]{0,35}\b(?:supported|validated|oracle-covered)\b|"
    r"\b\d+\b[^\n]{0,20}\blanguages?\b[^\n]{0,35}\b(?:supported|validated|oracle)\b|"
    r"\b\d{3,}\b[^\n]{0,12}\b(?:public\s+)?languages\b"
    r")"
)


@dataclass(frozen=True)
class Counts:
    supported: int
    validated: int
    oracle: int
    corpus: int
    manifest_cases: int
    validated_ids: tuple[str, ...]

    @property
    def supported_only(self):
        return self.supported - self.validated


def blocks(text, header):
    return re.split(rf"(?m)^\[\[{re.escape(header)}\]\]\s*$", text)[1:]


def string_value(block, key):
    match = re.search(rf'(?m)^{re.escape(key)}\s*=\s*"([^"]*)"', block)
    return match.group(1) if match else None


def int_value(block, key):
    match = re.search(rf"(?m)^{re.escape(key)}\s*=\s*(\d+)", block)
    return int(match.group(1)) if match else None


def public_catalog(root):
    text = (root / COVERAGE).read_text()
    count = re.search(r"(?m)^public_language_count\s*=\s*(\d+)$", text)
    kept = re.search(r"(?ms)^kept\s*=\s*\[(.*?)^\]", text)
    if not count or not kept:
        raise ValueError(f"{COVERAGE} lacks public_language_count or kept")
    languages = set(re.findall(r'"([^"]+)"', kept.group(1)))
    expected = int(count.group(1))
    if len(languages) != expected:
        raise ValueError(f"{COVERAGE} says {expected} public IDs but lists {len(languages)}")
    return languages


def manifest_counts(root, public):
    cases = blocks((root / CASES).read_text(), "case")
    languages = set()
    for block in cases:
        language = string_value(block, "language")
        grammar = string_value(block, "grammar")
        if not language or not grammar:
            raise ValueError(f"{CASES} has a case without language or grammar")
        grammar_id = Path(grammar).name.removesuffix(".tmLanguage.json")
        public_id = language if language in public else grammar_id
        if public_id not in public:
            raise ValueError(f"{CASES} cannot map {language!r} to a public ID")
        languages.add(public_id)
    return len(cases), len(languages)


def corpus_count(root):
    for block in blocks((root / CORPORA).read_text(), "corpus"):
        if string_value(block, "id") == "catalog-repeated":
            count = int_value(block, "languages")
            if count is None:
                raise ValueError("catalog-repeated lacks languages")
            return count
    raise ValueError(f"{CORPORA} lacks catalog-repeated")


def status_counts(root):
    text = (root / STATUS).read_text()
    summary = re.search(
        r"Syntaxmate bundles \*\*(\d+) supported public language IDs\*\*\. "
        r"\*\*(\d+) are validated\*\*.*?\*\*(\d+) more are supported\*\*",
        text,
    )
    coverage = re.search(
        r"oracle manifest currently covers \*\*(\d+)\*\* public IDs\. "
        r"`catalog-repeated` contains the \*\*(\d+)\*\* IDs",
        text,
    )
    if not summary or not coverage:
        raise ValueError(f"{STATUS} lacks its generated count summary")
    supported, validated, supported_only = map(int, summary.groups())
    oracle, corpus = map(int, coverage.groups())
    if supported - validated != supported_only:
        raise ValueError(f"{STATUS} has an inconsistent supported/validated split")

    rows = []
    validated_ids = []
    for line in text.splitlines():
        match = re.match(r"\| `([^`]+)` \| [^|]+ \| (supported|validated) \|", line)
        if match:
            rows.append(match.group(1))
            if match.group(2) == "validated":
                validated_ids.append(match.group(1))
    if len(rows) != supported or len(set(rows)) != supported:
        raise ValueError(f"{STATUS} table does not contain {supported} unique public IDs")
    if len(validated_ids) != validated:
        raise ValueError(f"{STATUS} table disagrees with its validated count")
    return supported, validated, oracle, corpus, tuple(validated_ids), set(rows)


def collect_counts(root=ROOT):
    public = public_catalog(root)
    cases, oracle = manifest_counts(root, public)
    corpus = corpus_count(root)
    supported, validated, _status_oracle, _status_corpus, validated_ids, status_ids = status_counts(root)
    if supported != len(public) or status_ids != public:
        raise ValueError(f"{STATUS} public IDs disagree with {COVERAGE}")
    # Validation is the status ledger's complete (including measured perf)
    # decision. Oracle and corpus membership come directly from their generated
    # manifests so this tool can refresh public snippets immediately after a
    # fixture batch, before the more expensive status/performance regeneration.
    counts = Counts(supported, validated, oracle, corpus, cases, validated_ids)
    policy = json.loads((root / VALIDATION_POLICY).read_text())
    expected = policy.get("expectedCounts")
    actual = {
        "publicLanguages": counts.supported,
        "validatedLanguages": counts.validated,
        "oracleLanguages": counts.oracle,
        "stressCorpusLanguages": counts.corpus,
    }
    if not isinstance(expected, dict):
        raise ValueError(f"{VALIDATION_POLICY} lacks expectedCounts")
    for key, value in actual.items():
        if expected.get(key) != value:
            raise ValueError(
                f"{VALIDATION_POLICY} requires {key}={expected.get(key)!r}, found {value}"
            )
    return counts


def replace_snippet(text, start, end, replacement, path):
    if text.count(start) != 1 or text.count(end) != 1:
        raise ValueError(f"{path} must contain exactly one {start!r}/{end!r} pair")
    before, remainder = text.split(start, 1)
    _, after = remainder.split(end, 1)
    return before + replacement + after


def count_snippet(path, counts):
    if path == Path("docs/configuration.md"):
        body = (
            f"The bundled native backend supports **{counts.supported} public language IDs**. "
            f"**{counts.validated} are validated** by the complete generated contract; "
            f"**{counts.supported_only} more are supported** by real bundled grammars and "
            "the catalog-wide smoke/budget gate. See "
            "[`language-status.md`](language-status.md) for the generated per-language "
            "ledger, or inspect `Catalog::bundled().languages()` at runtime."
        )
    elif path == FIXTURE_README:
        validated = ", ".join(f"`{language}`" for language in counts.validated_ids) or "none"
        body = (
            f"The generated manifest has **{counts.manifest_cases} cases** covering "
            f"**{counts.oracle} public language IDs** in the "
            f"**{counts.supported}-ID supported catalog**. **{counts.validated} IDs are "
            f"validated** by the complete generated contract; **{counts.corpus} IDs** are "
            f"in `catalog-repeated`. The current validated IDs are {validated}."
        )
    elif path == TOP_README:
        body = (
            f"Current generated coverage: **{counts.supported} supported public language "
            f"IDs**, **{counts.validated} validated**, **{counts.oracle} oracle-covered**, "
            f"and **{counts.corpus} in the catalog stress corpus**. The final quality target "
            f"is {counts.supported}/{counts.supported} validated; see "
            "[`docs/language-status.md`](docs/language-status.md) for the generated ledger."
        )
    else:
        body = (
            f"Completed generated coverage: **{counts.supported} supported public language "
            f"IDs**, **{counts.validated} validated**, **{counts.oracle} oracle-covered**, "
            f"and **{counts.corpus} in the catalog stress corpus**. The locked quality "
            f"contract is {counts.supported}/{counts.supported} validated; the deterministic "
            "validation policy locks all four counts and the exact catalog identity "
            "(SHA-256 of the sorted public-ID list), so regeneration cannot make a lost "
            "public-ID basic/stress contract look complete or swap one language for "
            "another. See [`language-status.md`](language-status.md) for the generated "
            "ledger."
        )
    return f"{COUNT_START}\n{body}\n{COUNT_END}"


def finite(value, label):
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise ValueError(f"{label} must be a finite number")
    return float(value)


def load_scale_policy(root, counts):
    policy = json.loads((root / SCALE_POLICY).read_text())
    required = {
        "schemaVersion", "targetPublicLanguages", "targetMinimumGoldenCases",
        "measurementGateCases", "warmupRuns", "timedRuns", "interactiveP95Seconds",
        "shardTargetP95Seconds", "maximumShards", "shardStrategy", "referenceRunner",
        "percentileMethod", "timingCommand", "decision",
    }
    missing = sorted(required - policy.keys())
    if missing:
        raise ValueError(f"{SCALE_POLICY} lacks: {', '.join(missing)}")
    if policy["schemaVersion"] != 1:
        raise ValueError(f"{SCALE_POLICY} schemaVersion must be 1")
    if policy["targetPublicLanguages"] != counts.supported:
        raise ValueError("scale target must equal the public catalog count")
    if policy["targetMinimumGoldenCases"] != counts.supported * 2:
        raise ValueError("scale target must reserve basic + stress for every public ID")
    if not 1 <= policy["measurementGateCases"] <= policy["targetMinimumGoldenCases"]:
        raise ValueError("measurementGateCases is outside the target range")
    if policy["warmupRuns"] < 1 or policy["timedRuns"] < 5:
        raise ValueError("scale timing requires at least one warmup and five timed runs")
    interactive = finite(policy["interactiveP95Seconds"], "interactiveP95Seconds")
    shard_target = finite(policy["shardTargetP95Seconds"], "shardTargetP95Seconds")
    if not 0 < shard_target <= interactive:
        raise ValueError("shard target must be positive and no greater than interactive p95")
    if policy["maximumShards"] < 2:
        raise ValueError("scale policy must permit at least two shards")
    if policy["shardStrategy"] != "sha256-language-id-modulo":
        raise ValueError("shard strategy must be sha256-language-id-modulo")
    if policy["percentileMethod"] != "nearest-rank":
        raise ValueError("scale percentile method must be nearest-rank")
    if policy["timingCommand"] != "cargo test -p syntaxmate --lib textmate_golden::manifest_golden_cases_ --locked":
        raise ValueError("scale timing command must remain the full golden test")
    if not isinstance(policy["referenceRunner"], str) or not policy["referenceRunner"].strip():
        raise ValueError("scale referenceRunner must be documented")

    decision = policy["decision"]
    state = decision.get("state") if isinstance(decision, dict) else None
    if state == "pending":
        # Static CI deliberately does not turn a documentation check into a
        # full-suite timing run. Once the checked-in manifest reaches the
        # configured trigger, however, a reviewed measurement is mandatory.
        if counts.manifest_cases >= policy["measurementGateCases"]:
            raise ValueError(
                "scale decision cannot remain pending at or above the manifest-case trigger"
            )
    elif state == "measured":
        needed = {
            "manifestCases", "fullSuiteP95Seconds", "interactiveThresholdSeconds",
            "shardCount", "measuredAt", "runner", "evidence",
        }
        missing = sorted(needed - decision.keys())
        if missing:
            raise ValueError(f"scale decision lacks: {', '.join(missing)}")
        measured_cases = decision["manifestCases"]
        if isinstance(measured_cases, bool) or not isinstance(measured_cases, int):
            raise ValueError("scale decision manifestCases must be an integer")
        if measured_cases < policy["measurementGateCases"]:
            raise ValueError("scale decision was measured below the scale gate")
        if measured_cases != counts.manifest_cases:
            raise ValueError("scale decision manifestCases is stale for the current manifest")
        try:
            if datetime.date.fromisoformat(decision["measuredAt"]).isoformat() != decision["measuredAt"]:
                raise ValueError
        except (TypeError, ValueError):
            raise ValueError("scale decision measuredAt must be an ISO date") from None
        if decision["runner"] != policy["referenceRunner"]:
            raise ValueError("scale decision runner must match referenceRunner")
        if not isinstance(decision["evidence"], str) or not decision["evidence"].strip():
            raise ValueError("scale decision evidence must be documented")
        measured_threshold = finite(
            decision["interactiveThresholdSeconds"], "interactiveThresholdSeconds"
        )
        if not math.isclose(measured_threshold, interactive):
            raise ValueError("scale decision threshold must match interactiveP95Seconds")
        full_p95 = finite(decision["fullSuiteP95Seconds"], "fullSuiteP95Seconds")
        shards = decision["shardCount"]
        if isinstance(shards, bool) or not isinstance(shards, int) or not 1 <= shards <= policy["maximumShards"]:
            raise ValueError("scale shard count is outside policy")
        if full_p95 <= interactive and shards != 1:
            raise ValueError("suite below the interactive limit must remain unsharded")
        if full_p95 > interactive:
            if shards == 1:
                raise ValueError("suite above the interactive limit must be sharded")
            candidates = decision.get("maximumShardP95SecondsByShardCount")
            # Candidate shard timings are optional evidence. When recorded,
            # validate the complete smallest-compliant-count claim; the
            # trigger measurement alone is sufficient to require CI sharding.
            if candidates is not None:
                expected_keys = {str(count) for count in range(1, shards + 1)}
                if not isinstance(candidates, dict) or set(candidates) != expected_keys:
                    raise ValueError(
                        "scale decision must record every shard count through the selected count"
                    )
                candidate_p95 = {
                    int(count): finite(value, f"shard count {count} p95")
                    for count, value in candidates.items()
                }
                if not math.isclose(candidate_p95[1], full_p95):
                    raise ValueError("one-shard p95 must equal fullSuiteP95Seconds")
                if candidate_p95[shards] > shard_target:
                    raise ValueError("recorded shards do not meet the shard p95 target")
                if any(candidate_p95[count] <= shard_target for count in range(1, shards)):
                    raise ValueError("selected shard count is not the smallest compliant count")
            per_shard = decision.get("shardP95SecondsByIndex")
            # Measured per-shard evidence for the selected shard count. When
            # recorded, every selected shard index must have a p95 at or below
            # the shard target.
            if per_shard is not None:
                expected_indexes = {str(index) for index in range(shards)}
                if not isinstance(per_shard, dict) or set(per_shard) != expected_indexes:
                    raise ValueError(
                        "per-shard evidence must cover exactly the selected shard indexes"
                    )
                for index, value in per_shard.items():
                    if finite(value, f"shard {index} p95") > shard_target:
                        raise ValueError(
                            f"shard {index} p95 evidence exceeds the shard p95 target"
                        )
    else:
        raise ValueError("scale decision.state must be pending or measured")
    return policy


def scale_snippet(policy):
    body = (
        f"Static gate: measure at **{policy['measurementGateCases']} manifest cases**, after "
        f"**{policy['warmupRuns']} warmup** and **{policy['timedRuns']} timed runs**. Keep the "
        f"suite unsharded at p95 ≤ **{policy['interactiveP95Seconds']} s**; above that, choose "
        f"a reviewed count of at most **{policy['maximumShards']} stable language-ID shards** "
        f"whose maximum p95 is ≤ **{policy['shardTargetP95Seconds']} s**. Final scale is at "
        f"least **{policy['targetMinimumGoldenCases']} cases** for "
        f"**{policy['targetPublicLanguages']} public IDs**. Use nearest-rank p95 on "
        f"**{policy['referenceRunner']}**."
    )
    decision = policy["decision"]
    if decision.get("state") == "measured":
        body += (
            f" Current decision: **{decision['manifestCases']} cases** measured at "
            f"**{decision['fullSuiteP95Seconds']} s p95**, above the "
            f"**{decision['interactiveThresholdSeconds']} s** trigger, so CI runs "
            f"**{decision['shardCount']} shards**."
        )
        per_shard = decision.get("shardP95SecondsByIndex")
        if per_shard:
            worst = max(per_shard.values())
            body += (
                f" Measured per-shard p95 ({decision['measuredAt']}): "
                + ", ".join(
                    f"shard {index} = {per_shard[index]} s"
                    for index in sorted(per_shard, key=int)
                )
                + f"; maximum **{worst} s** ≤ the "
                f"**{policy['shardTargetP95Seconds']} s** shard target."
            )
    return f"{SCALE_START}\n{body}\n{SCALE_END}"


def without_generated_snippets(text):
    for start, end in ((COUNT_START, COUNT_END), (SCALE_START, SCALE_END)):
        text = re.sub(re.escape(start) + r".*?" + re.escape(end), "", text, flags=re.DOTALL)
    return text


def render_docs(root, counts, policy):
    result = {}
    for path in MANAGED_DOCS:
        text = (root / path).read_text()
        if TOP_README_COUNT_CLAIM.search(without_generated_snippets(text)):
            raise ValueError(f"{path} has an unmanaged language-count claim; add generated markers")
        text = replace_snippet(text, COUNT_START, COUNT_END, count_snippet(path, counts), path)
        if path == Path("docs/textmate-engine.md"):
            text = replace_snippet(text, SCALE_START, SCALE_END, scale_snippet(policy), path)
        result[path] = text

    top = (root / TOP_README).read_text()
    if TOP_README_COUNT_CLAIM.search(without_generated_snippets(top)):
        raise ValueError(
            f"{TOP_README} has an unmanaged language-count claim; add generated markers"
        )
    if COUNT_START in top or COUNT_END in top:
        result[TOP_README] = replace_snippet(
            top, COUNT_START, COUNT_END, count_snippet(TOP_README, counts), TOP_README
        )
    return result


def main():
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--check", action="store_true", help="check snippets (the default)")
    mode.add_argument("--write", action="store_true", help="rewrite managed snippets")
    parser.add_argument("--root", type=Path, default=ROOT, help=argparse.SUPPRESS)
    args = parser.parse_args()
    try:
        counts = collect_counts(args.root)
        policy = load_scale_policy(args.root, counts)
        expected = render_docs(args.root, counts, policy)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(error, file=sys.stderr)
        return 1

    stale = [path for path, text in expected.items() if (args.root / path).read_text() != text]
    if args.write:
        for path in stale:
            (args.root / path).write_text(expected[path])
        print(f"updated {len(stale)} language-count document(s)")
        return 0
    if stale:
        for path in stale:
            print(f"{path} is stale; run {Path(__file__).name} --write", file=sys.stderr)
        return 1
    print("language-count docs and golden scale policy are current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
