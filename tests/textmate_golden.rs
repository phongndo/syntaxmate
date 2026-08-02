use std::{
    collections::HashSet,
    fs,
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    path::{Path, PathBuf},
};

use crate::{
    SyntaxClass, SyntaxSegment,
    engine::{
        grammar::load_dev_grammar_from_str,
        regex::{AnchorContext, FallbackError, RegexMatcher},
        scopes::ScopeClassifier,
        state::GrammarId,
        tokenizer::{GrammarSet, TextMateTokenizer, TokenizedLine, TokenizerState},
    },
};
use serde::Deserialize;

const MANIFEST_PATH: &str = "tests/fixtures/textmate/cases.toml";
const DIVERGENCES_PATH: &str = "tests/fixtures/textmate/divergences.toml";
const SHARD_INDEX_ENV: &str = "SYNTAXMATE_SHARD_INDEX";
const SHARD_TOTAL_ENV: &str = "SYNTAXMATE_SHARD_TOTAL";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GoldenShard {
    index: usize,
    total: usize,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    #[serde(default, rename = "case")]
    cases: Vec<CaseSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct CaseSpec {
    language: String,
    scope: String,
    grammar: String,
    fixture: String,
    golden: String,
    #[serde(default)]
    embedded: Vec<EmbeddedSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct EmbeddedSpec {
    #[allow(dead_code)]
    scope: String,
    grammar: String,
}

#[derive(Debug, Deserialize, Default)]
struct DivergenceFile {
    #[serde(default, rename = "divergence")]
    divergences: Vec<DivergenceSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct DivergenceSpec {
    language: String,
    grammar: String,
    fixture: String,
    line_start: usize,
    line_end: usize,
    mode: DivergenceMode,
    reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
enum DivergenceMode {
    Exact,
    Coarse,
    Any,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComparisonMode {
    Exact,
    Coarse,
}

#[derive(Debug)]
struct RuntimeDivergence {
    spec: DivergenceSpec,
    hits: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenLine {
    language: String,
    scope_name: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    line_number: Option<usize>,
    line: String,
    tokens: Vec<GoldenToken>,
    #[serde(default)]
    rule_stack: Option<String>,
    #[serde(default)]
    rule_stack_hash: Option<String>,
    stopped_early: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenToken {
    start_index: usize,
    end_index: usize,
    scopes: Vec<String>,
}

fn utf16_range_to_utf8(
    line: &str,
    start_utf16: usize,
    end_utf16: usize,
) -> Option<std::ops::Range<usize>> {
    Some(utf16_offset_to_utf8(line, start_utf16)?..utf16_offset_to_utf8(line, end_utf16)?)
}

fn utf16_offset_to_utf8(line: &str, target: usize) -> Option<usize> {
    let mut utf16 = 0usize;
    for (byte, ch) in line.char_indices() {
        if utf16 == target {
            return Some(byte);
        }
        utf16 += ch.len_utf16();
        if utf16 > target {
            return None;
        }
    }
    // vscode-textmate represents the final token as extending through its
    // synthetic line terminator, so its end index may be one UTF-16 code unit
    // past the input string. Clamp that sentinel to the real byte length.
    (utf16 == target || utf16.checked_add(1) == Some(target)).then_some(line.len())
}

fn coalesce_scope_tokens(
    tokens: impl IntoIterator<Item = (std::ops::Range<usize>, Vec<String>)>,
) -> Vec<(std::ops::Range<usize>, Vec<String>)> {
    let mut coalesced: Vec<(std::ops::Range<usize>, Vec<String>)> = Vec::new();
    for (range, scopes) in tokens {
        if range.start >= range.end {
            continue;
        }
        if let Some((last_range, last_scopes)) = coalesced.last_mut()
            && last_range.end == range.start
            && *last_scopes == scopes
        {
            last_range.end = range.end;
            continue;
        }
        coalesced.push((range, scopes));
    }
    coalesced
}

#[test]
fn parses_golden_jsonl_record() {
    let line: GoldenLine = serde_json::from_str(
        r#"{"language":"json","scopeName":"source.json","file":"fixture.json","lineNumber":7,"line":"{\"ok\":true}","tokens":[{"startIndex":0,"endIndex":1,"scopes":["source.json","punctuation.definition.dictionary.begin.json"]}],"ruleStack":"[(1, source.json, source.json)]","ruleStackHash":"abc","stoppedEarly":false}"#,
    )
    .unwrap();
    assert_eq!(line.language, "json");
    assert_eq!(line.scope_name, "source.json");
    assert_eq!(line.file.as_deref(), Some("fixture.json"));
    assert_eq!(line.line_number, Some(7));
    assert_eq!(line.line, "{\"ok\":true}");
    assert_eq!(line.tokens[0].start_index, 0);
    assert_eq!(line.tokens[0].end_index, 1);
    assert_eq!(line.tokens[0].scopes[0], "source.json");
    assert_eq!(line.rule_stack_hash.as_deref(), Some("abc"));
    assert!(!line.stopped_early);
}

#[test]
fn golden_jsonl_record_requires_stopped_early_false() {
    let record = r#"{"language":"json","scopeName":"source.json","line":"{}","tokens":[]}"#;
    let error = serde_json::from_str::<GoldenLine>(record).unwrap_err();
    assert!(
        error.to_string().contains("stoppedEarly"),
        "unexpected deserialization error: {error}"
    );

    let record: GoldenLine = serde_json::from_str(
        r#"{"language":"json","scopeName":"source.json","lineNumber":0,"line":"{}","tokens":[],"stoppedEarly":true}"#,
    )
    .unwrap();
    let case = CaseSpec {
        language: "json".to_owned(),
        scope: "source.json".to_owned(),
        grammar: "grammar.json".to_owned(),
        fixture: "fixture.json".to_owned(),
        golden: "fixture.golden.jsonl".to_owned(),
        embedded: Vec::new(),
    };
    let mut failures = Vec::new();
    validate_record(&case, &record, 0, &mut failures);
    assert_eq!(failures.len(), 1);
    assert!(failures[0].contains("oracle stopped early"));
}

#[test]
fn converts_utf16_offsets_to_utf8_byte_ranges() {
    let line = "aπ𝌆z";
    assert_eq!(utf16_offset_to_utf8(line, 0), Some(0));
    assert_eq!(utf16_offset_to_utf8(line, 1), Some(1));
    assert_eq!(utf16_offset_to_utf8(line, 2), Some(3));
    assert_eq!(utf16_offset_to_utf8(line, 3), None);
    assert_eq!(utf16_offset_to_utf8(line, 4), Some(7));
    assert_eq!(utf16_range_to_utf8(line, 1, 4), Some(1..7));
    assert_eq!(utf16_offset_to_utf8(line, 6), Some(line.len()));
}

#[test]
fn parses_textmate_shard_configuration() {
    assert_eq!(parse_shard(None, None).unwrap(), None);
    assert_eq!(
        parse_shard(Some("2"), Some("4")).unwrap(),
        Some(GoldenShard { index: 2, total: 4 })
    );

    for (index, total, message) in [
        (Some("0"), None, "must be set together"),
        (None, Some("4"), "must be set together"),
        (Some("x"), Some("4"), "non-negative integer"),
        (Some("0"), Some("0"), "must be at least 1"),
        (Some("4"), Some("4"), "must be less than"),
    ] {
        assert!(
            parse_shard(index, total).unwrap_err().contains(message),
            "expected {message:?} for index={index:?} total={total:?}"
        );
    }
}

#[test]
fn textmate_shards_use_stable_sha256_language_assignment() {
    assert_eq!(
        sha256(b"abc"),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );
    assert_eq!(language_shard("rust", 4), 3);
    assert_eq!(language_shard("shellscript", 4), 3);
    assert_eq!(language_shard("json", 4), 2);
    assert_eq!(language_shard("typescript", 4), 0);

    let cases = [
        shard_test_case("rust", "rust"),
        shard_test_case("bash", "shellscript"),
        shard_test_case("json", "json"),
        shard_test_case("typescript", "typescript"),
    ];
    for case in cases {
        let owners = (0..4)
            .filter(|index| {
                case_belongs_to_shard(
                    &case,
                    GoldenShard {
                        index: *index,
                        total: 4,
                    },
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            owners.len(),
            1,
            "{} must have exactly one owner",
            case.language
        );
    }

    let manifest = load_manifest();
    let shard_sizes = (0..4)
        .map(|index| {
            manifest
                .iter()
                .filter(|case| case_belongs_to_shard(case, GoldenShard { index, total: 4 }))
                .count()
        })
        .collect::<Vec<_>>();
    assert_eq!(shard_sizes.iter().sum::<usize>(), manifest.len());
    assert!(shard_sizes.iter().all(|size| *size > 0), "{shard_sizes:?}");
}

fn shard_test_case(language: &str, grammar: &str) -> CaseSpec {
    CaseSpec {
        language: language.to_owned(),
        scope: format!("source.{language}"),
        grammar: format!("assets/grammars/languages/{grammar}.tmLanguage.json"),
        fixture: String::new(),
        golden: String::new(),
        embedded: Vec::new(),
    }
}

#[test]
fn manifest_golden_cases_match_or_are_allowlisted() {
    let cases = load_manifest_for_current_shard();
    assert!(
        !cases.is_empty(),
        "golden manifest should list at least one case"
    );
    let mut divergences = load_divergences();
    validate_divergences(&divergences);

    let mut failures = Vec::new();
    for case in &cases {
        assert_case_files_exist(case);
        let records = load_golden_records(case);
        compare_exact_scopes(case, &records, &mut divergences, &mut failures);
        compare_coarse_highlights(case, &records, &mut divergences, &mut failures);
    }

    for divergence in &divergences {
        if divergence.hits == 0 {
            failures.push(format!(
                "stale divergence: language={} grammar={} fixture={} lines {}-{} mode={:?} reason={}",
                divergence.spec.language,
                divergence.spec.grammar,
                divergence.spec.fixture,
                divergence.spec.line_start,
                divergence.spec.line_end,
                divergence.spec.mode,
                divergence.spec.reason,
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn divergence_allowlist_is_empty() {
    let file = read_divergence_file();
    assert!(
        file.divergences.is_empty(),
        "{DIVERGENCES_PATH} must not contain audit bypasses"
    );
}

#[test]
fn manifest_golden_cases_replay_incremental_state_deterministically() {
    for case in load_manifest_for_current_shard() {
        let records = load_golden_records(&case);
        let mut tokenizer = tokenizer_for_case(&case);
        let mut state = TokenizerState::default();
        for (line_index, record) in records.iter().enumerate() {
            let parse_line = format!("{}\n", record.line);
            let first =
                tokenizer.tokenize_line_scopes_at_line(&parse_line, state.clone(), line_index);
            let replay =
                tokenizer.tokenize_line_scopes_at_line(&parse_line, state.clone(), line_index);
            assert_eq!(
                first,
                replay,
                "{} {} line {} changed when replayed from identical state",
                case.language,
                case.fixture,
                line_index + 1,
            );
            state = first.state.clone();
        }
    }
}

#[test]
fn manifest_golden_cases_have_no_budget_degradation() {
    for case in load_manifest_for_current_shard() {
        let mut tokenizer = tokenizer_for_case(&case);
        tokenizer.set_counters_enabled(true);
        let source = fs::read_to_string(repo_path(&case.fixture)).expect("fixture source");
        let _ = tokenizer.tokenize_source(&source);
        let counters = tokenizer.take_counters();
        assert_eq!(
            counters.degraded_lines, 0,
            "{} degraded committed fixture lines",
            case.fixture
        );
        assert_eq!(
            counters.fallback_budget_kills, 0,
            "{} exhausted fallback budget",
            case.fixture
        );
    }
}

#[test]
fn bsl_basic_and_stress_are_exact_and_budget_safe_in_the_full_grammar_set() {
    let cases = load_manifest()
        .into_iter()
        .filter(|case| case.language == "bsl")
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 2, "BSL must keep basic and stress coverage");

    // Keep the oracle checks on the manifest-declared dependency closure. This
    // is the same grammar set vscode-textmate uses for the checked-in goldens.
    for case in &cases {
        let records = load_golden_records(case);
        let mut failures = Vec::new();
        compare_exact_scopes(case, &records, &mut [], &mut failures);
        compare_coarse_highlights(case, &records, &mut [], &mut failures);
        assert!(
            failures.is_empty(),
            "{} diverged from the BSL oracle:\n{}",
            case.fixture,
            failures.join("\n")
        );
    }

    // Production embeds the complete grammar catalog. Loading that closure
    // activates injection candidates which are absent from the small oracle
    // set and historically amplified BSL's Unicode keyword alternations until
    // the source-wide fallback budget was exhausted.
    let grammars = full_asset_grammar_set();
    let root = grammars
        .grammar_by_scope("source.bsl")
        .expect("full catalog contains BSL")
        .id;
    for case in &cases {
        let source = fs::read_to_string(repo_path(&case.fixture)).expect("BSL fixture source");
        let mut tokenizer = TextMateTokenizer::new(grammars.clone(), root);
        tokenizer.set_counters_enabled(true);
        let _ = tokenizer.tokenize_source(&source);
        let counters = tokenizer.take_counters();
        assert_eq!(
            counters.degraded_lines, 0,
            "{}: {counters:#?}",
            case.fixture
        );
        assert_eq!(
            counters.fallback_budget_kills, 0,
            "{}: {counters:#?}",
            case.fixture
        );
    }
}

fn full_asset_grammar_set() -> GrammarSet {
    let directory = repo_path("assets/grammars/languages");
    let mut entries = fs::read_dir(directory)
        .expect("grammar asset directory")
        .collect::<Result<Vec<_>, _>>()
        .expect("grammar asset entries");
    entries.sort_by_key(|entry| entry.file_name());

    let mut set = GrammarSet::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let source = fs::read_to_string(&path).expect("grammar asset");
        let id = GrammarId(set.grammars().len() as u16);
        if let Ok(grammar) = load_dev_grammar_from_str(id, &source) {
            set.add(grammar);
        }
    }
    set
}

#[test]
fn libcxx_cpp_fixture_has_zero_oracle_divergence() {
    let cases = load_manifest();
    let case = cases
        .iter()
        .find(|case| case.fixture.contains("cpp/libcxx_"))
        .expect("manifest should contain a dedicated libc++ C++ fixture");
    let allowlisted: Vec<_> = load_divergences()
        .into_iter()
        .filter(|divergence| divergence.spec.fixture == case.fixture)
        .collect();
    assert!(
        allowlisted.is_empty(),
        "libc++ fixture must not rely on divergence allowlist entries"
    );

    let records = load_golden_records(case);
    let mut divergences = Vec::new();
    let mut failures = Vec::new();
    compare_exact_scopes(case, &records, &mut divergences, &mut failures);
    compare_coarse_highlights(case, &records, &mut divergences, &mut failures);
    assert!(
        failures.is_empty(),
        "libc++ fixture diverged from oracle:\n{}",
        failures.join("\n")
    );
}

#[test]
fn missing_external_scope_matches_oracle_no_op_behavior() {
    // vscode-textmate drops an unresolvable include. In particular, a
    // begin/end rule whose only child is unavailable is not entered at all;
    // this subtle behavior affects optional host-extension dependencies in
    // dozens of the vendored grammars.
    let case = CaseSpec {
        language: "missing-scope-conformance".to_owned(),
        scope: "source.missing-scope-conformance".to_owned(),
        grammar: "tests/fixtures/textmate/conformance/missing-scope.tmLanguage.json".to_owned(),
        fixture: "tests/fixtures/textmate/conformance/missing-scope.txt".to_owned(),
        golden: "tests/fixtures/textmate/conformance/missing-scope.golden.jsonl".to_owned(),
        embedded: Vec::new(),
    };
    assert_case_files_exist(&case);
    let records = load_golden_records(&case);
    let mut failures = Vec::new();
    compare_exact_scopes(&case, &records, &mut [], &mut failures);
    compare_coarse_highlights(&case, &records, &mut [], &mut failures);
    assert!(
        failures.is_empty(),
        "missing-scope behavior diverged from oracle:\n{}",
        failures.join("\n")
    );
}

#[test]
fn jison_basic_and_stress_match_oracle_with_missing_optional_includes() {
    for case in jison_cases() {
        assert_case_files_exist(&case);
        let records = load_golden_records(&case);
        let mut failures = Vec::new();
        compare_exact_scopes(&case, &records, &mut [], &mut failures);
        compare_coarse_highlights(&case, &records, &mut [], &mut failures);
        assert!(
            failures.is_empty(),
            "{} diverged from the oracle:\n{}",
            case.fixture,
            failures.join("\n")
        );

        let source = fs::read_to_string(repo_path(&case.fixture)).expect("Jison fixture source");
        let mut tokenizer = tokenizer_for_case(&case);
        tokenizer.set_counters_enabled(true);
        let _ = tokenizer.tokenize_source(&source);
        let counters = tokenizer.take_counters();
        assert_eq!(
            counters.degraded_lines, 0,
            "{}: {counters:#?}",
            case.fixture
        );
        assert_eq!(
            counters.fallback_budget_kills, 0,
            "{}: {counters:#?}",
            case.fixture
        );
        assert!(
            counters
                .pattern_compile_counts
                .iter()
                .all(|pattern| pattern.count == 1),
            "Jison patterns must compile once per tokenizer: {}: {counters:#?}",
            case.fixture
        );
    }
}

fn jison_cases() -> [CaseSpec; 2] {
    ["basic", "stress"].map(|fixture| CaseSpec {
        language: "jison".to_owned(),
        scope: "source.jison".to_owned(),
        grammar: "assets/grammars/languages/jison.tmLanguage.json".to_owned(),
        fixture: format!("tests/fixtures/textmate/jison/{fixture}.jison"),
        golden: format!("tests/fixtures/textmate/jison/{fixture}.golden.jsonl"),
        // The oracle can resolve source.js but not source.jisonlex or the
        // absent source.js#string_escapes repository entry.
        embedded: vec![EmbeddedSpec {
            scope: "source.js".to_owned(),
            grammar: "assets/grammars/languages/javascript.tmLanguage.json".to_owned(),
        }],
    })
}

#[test]
fn ara_basic_and_stress_match_oracle_exactly_without_divergences() {
    for case in ara_cases() {
        assert_case_files_exist(&case);
        let records = load_golden_records(&case);
        let mut failures = Vec::new();
        compare_exact_scopes(&case, &records, &mut [], &mut failures);
        compare_coarse_highlights(&case, &records, &mut [], &mut failures);
        assert!(
            failures.is_empty(),
            "{} diverged from the oracle:\n{}",
            case.fixture,
            failures.join("\n")
        );

        let source = fs::read_to_string(repo_path(&case.fixture)).expect("Ara fixture source");
        let mut tokenizer = tokenizer_for_case(&case);
        tokenizer.set_counters_enabled(true);
        let _ = tokenizer.tokenize_source(&source);
        let counters = tokenizer.take_counters();
        assert_eq!(counters.degraded_lines, 0, "{}", case.fixture);
        assert_eq!(counters.fallback_budget_kills, 0, "{}", case.fixture);
    }
}

fn ara_cases() -> [CaseSpec; 2] {
    ["basic", "stress"].map(|fixture| CaseSpec {
        language: "ara".to_owned(),
        scope: "source.ara".to_owned(),
        grammar: "assets/grammars/languages/ara.tmLanguage.json".to_owned(),
        fixture: format!("tests/fixtures/textmate/ara/{fixture}.ara"),
        golden: format!("tests/fixtures/textmate/ara/{fixture}.golden.jsonl"),
        embedded: Vec::new(),
    })
}

#[test]
fn tsv_basic_and_stress_match_oracle_exactly_on_native_path() {
    for case in tsv_cases() {
        assert_case_files_exist(&case);
        let records = load_golden_records(&case);
        let mut failures = Vec::new();
        compare_exact_scopes(&case, &records, &mut [], &mut failures);
        compare_coarse_highlights(&case, &records, &mut [], &mut failures);
        assert!(
            failures.is_empty(),
            "{} diverged from the oracle:\n{}",
            case.fixture,
            failures.join("\n")
        );

        let source = fs::read_to_string(repo_path(&case.fixture)).expect("TSV fixture source");
        let mut tokenizer = tokenizer_for_case(&case);
        tokenizer.set_counters_enabled(true);
        let _ = tokenizer.tokenize_source(&source);
        let counters = tokenizer.take_counters();
        assert_eq!(
            counters.degraded_lines, 0,
            "{}: {counters:#?}",
            case.fixture
        );
        assert_eq!(
            counters.fallback_budget_kills, 0,
            "{}: {counters:#?}",
            case.fixture
        );
        assert_eq!(
            counters.regex_fallback_attempts, 0,
            "TSV must stay on the native matcher: {}: {counters:#?}",
            case.fixture
        );
        assert!(
            counters.regex_dfa_attempts > 0,
            "TSV native matcher was not exercised: {}: {counters:#?}",
            case.fixture
        );
        assert!(
            counters
                .pattern_compile_counts
                .iter()
                .all(|pattern| pattern.count == 1),
            "TSV patterns must compile once per tokenizer: {}: {counters:#?}",
            case.fixture
        );
    }
}

fn tsv_cases() -> [CaseSpec; 2] {
    ["basic", "stress"].map(|fixture| CaseSpec {
        language: "tsv".to_owned(),
        scope: "text.tsv".to_owned(),
        grammar: "assets/grammars/languages/tsv.tmLanguage.json".to_owned(),
        fixture: format!("tests/fixtures/textmate/tsv/{fixture}.tsv"),
        golden: format!("tests/fixtures/textmate/tsv/{fixture}.golden.jsonl"),
        embedded: Vec::new(),
    })
}

#[test]
fn gherkin_basic_and_stress_match_oracle_around_unicode_table_cells() {
    for case in gherkin_cases() {
        assert_case_files_exist(&case);
        let records = load_golden_records(&case);
        let mut failures = Vec::new();
        compare_exact_scopes(&case, &records, &mut [], &mut failures);
        compare_coarse_highlights(&case, &records, &mut [], &mut failures);
        assert!(
            failures.is_empty(),
            "{} diverged from the oracle:\n{}",
            case.fixture,
            failures.join("\n")
        );

        let source = fs::read_to_string(repo_path(&case.fixture)).expect("Gherkin fixture source");
        let mut tokenizer = tokenizer_for_case(&case);
        tokenizer.set_counters_enabled(true);
        let _ = tokenizer.tokenize_source(&source);
        let counters = tokenizer.take_counters();
        assert_eq!(
            counters.degraded_lines, 0,
            "{}: {counters:#?}",
            case.fixture
        );
        assert_eq!(
            counters.fallback_budget_kills, 0,
            "{}: {counters:#?}",
            case.fixture
        );
    }
}

fn gherkin_cases() -> [CaseSpec; 2] {
    ["basic", "stress"].map(|fixture| CaseSpec {
        language: "gherkin".to_owned(),
        scope: "text.gherkin.feature".to_owned(),
        grammar: "assets/grammars/languages/gherkin.tmLanguage.json".to_owned(),
        fixture: format!("tests/fixtures/textmate/gherkin/{fixture}.feature"),
        golden: format!("tests/fixtures/textmate/gherkin/{fixture}.golden.jsonl"),
        embedded: Vec::new(),
    })
}

#[test]
fn wikitext_basic_and_stress_match_oracle_exactly() {
    for case in wikitext_cases() {
        assert_case_files_exist(&case);
        let records = load_golden_records(&case);
        let mut failures = Vec::new();
        compare_exact_scopes(&case, &records, &mut [], &mut failures);
        assert!(
            failures.is_empty(),
            "{} diverged from the oracle:\n{}",
            case.fixture,
            failures.join("\n")
        );
    }
}

fn wikitext_cases() -> [CaseSpec; 2] {
    ["basic", "stress"].map(|fixture| CaseSpec {
        language: "wikitext".to_owned(),
        scope: "source.wikitext".to_owned(),
        grammar: "assets/grammars/languages/wikitext.tmLanguage.json".to_owned(),
        fixture: format!("tests/fixtures/textmate/wikitext/{fixture}.wiki"),
        golden: format!("tests/fixtures/textmate/wikitext/{fixture}.golden.jsonl"),
        embedded: [
            ("source.css", "css"),
            ("source.js", "javascript"),
            ("text.html.basic", "html"),
        ]
        .map(|(scope, grammar)| EmbeddedSpec {
            scope: scope.to_owned(),
            grammar: format!("assets/grammars/languages/{grammar}.tmLanguage.json"),
        })
        .to_vec(),
    })
}

#[test]
fn reusable_parity_regressions_match_oracle_basic_and_stress() {
    for case in reusable_parity_cases() {
        assert_case_files_exist(&case);
        let records = load_golden_records(&case);
        let mut failures = Vec::new();
        compare_exact_scopes(&case, &records, &mut [], &mut failures);
        compare_coarse_highlights(&case, &records, &mut [], &mut failures);
        assert!(
            failures.is_empty(),
            "{} diverged from the oracle:\n{}",
            case.fixture,
            failures.join("\n")
        );

        let source = fs::read_to_string(repo_path(&case.fixture)).expect("fixture source");
        let mut tokenizer = tokenizer_for_case(&case);
        tokenizer.set_counters_enabled(true);
        let _ = tokenizer.tokenize_source(&source);
        let counters = tokenizer.take_counters();
        assert_eq!(counters.degraded_lines, 0, "{}", case.fixture);
        assert_eq!(counters.fallback_budget_kills, 0, "{}", case.fixture);
    }
}

fn reusable_parity_cases() -> Vec<CaseSpec> {
    let specs = [
        ("sas", "source.sas", "sas", "sas", &["sql"][..]),
        ("sdbl", "source.sdbl", "sdbl", "sdbl", &[][..]),
        (
            "shellsession",
            "text.shell-session",
            "shellsession",
            "sh-session",
            &[][..],
        ),
        ("sparql", "source.sparql", "sparql", "rq", &["turtle"][..]),
        ("splunk", "source.splunk_search", "splunk", "spl", &[][..]),
    ];
    specs
        .into_iter()
        .flat_map(|(language, scope, grammar, extension, embedded)| {
            ["basic", "stress"].map(move |fixture| CaseSpec {
                language: language.to_owned(),
                scope: scope.to_owned(),
                grammar: format!("assets/grammars/languages/{grammar}.tmLanguage.json"),
                fixture: format!("tests/fixtures/textmate/{language}/{fixture}.{extension}"),
                golden: format!("tests/fixtures/textmate/{language}/{fixture}.golden.jsonl"),
                embedded: embedded
                    .iter()
                    .map(|grammar| EmbeddedSpec {
                        scope: String::new(),
                        grammar: format!("assets/grammars/languages/{grammar}.tmLanguage.json"),
                    })
                    .collect(),
            })
        })
        .collect()
}

#[test]
fn tokenizer_never_panics_on_generated_utf8_inputs() {
    let grammar = fs::read_to_string(repo_path("assets/grammars/languages/json.tmLanguage.json"))
        .expect("json grammar");
    let mut tokenizer = TextMateTokenizer::from_grammar(&grammar).unwrap();
    let mut state = TokenizerState::default();
    for input in generated_utf8_inputs() {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let tokenized = tokenizer.tokenize_line_scopes(&input, state.clone());
            assert_token_invariants(&input, &tokenized);
            state = tokenized.state;
        }));
        assert!(result.is_ok(), "tokenizer panicked for input {input:?}");
    }
}

#[test]
fn zero_width_match_stops_line_without_looping() {
    let grammar = r##"{
        "scopeName": "source.zero-width",
        "patterns": [
            {"match":"", "name":"meta.empty.zero-width"},
            {"match":"λ|🚀|[A-Za-z]+", "name":"keyword.visible.zero-width"}
        ]
    }"##;
    let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
    let line = "λ🚀abc";
    let tokenized = tokenizer.tokenize_line_scopes(line, TokenizerState::default());
    assert_token_invariants(line, &tokenized);
    assert!(tokenized.state.is_initial());
    assert!(tokenized.tokens.iter().all(|token| {
        !token
            .scopes
            .iter()
            .any(|scope| scope == "keyword.visible.zero-width")
    }));
}

#[test]
fn parser_state_replay_from_checkpoint_matches_replay_from_zero() {
    let grammar = r##"{
        "scopeName": "source.checkpoint",
        "patterns": [
            {"begin":"/\\*", "end":"\\*/", "name":"comment.block.checkpoint"},
            {"begin":"\"", "end":"\"", "name":"string.quoted.double.checkpoint"},
            {"match":"\\b(let|return)\\b", "name":"keyword.control.checkpoint"}
        ]
    }"##;
    let lines = [
        "let before = 1;",
        "/* comment starts",
        "still in comment λ🚀",
        "ends */ let after = \"ok\";",
        "return after;",
    ];

    let mut full = TextMateTokenizer::from_grammar(grammar).unwrap();
    let mut state = TokenizerState::default();
    let checkpoint_after_line = 1usize;
    let mut replay = None;
    let mut full_states = Vec::new();
    let mut full_tokens = Vec::new();
    for (line_index, line) in lines.into_iter().enumerate() {
        let tokenized = full.tokenize_line_scopes(line, state);
        state = tokenized.state.clone();
        full_tokens.push(tokenized.tokens);
        full_states.push(state.clone());
        if line_index == checkpoint_after_line {
            replay = Some(full.clone());
        }
    }

    let checkpoint_state = full_states[checkpoint_after_line].clone();
    let mut replay = replay.expect("checkpoint tokenizer clone");
    let mut state = checkpoint_state;
    for line_index in checkpoint_after_line + 1..lines.len() {
        let tokenized = replay.tokenize_line_scopes(lines[line_index], state);
        assert_eq!(
            tokenized.tokens, full_tokens[line_index],
            "line {line_index}"
        );
        state = tokenized.state;
    }
    assert_eq!(state, *full_states.last().unwrap());
}

#[test]
fn fallback_budget_kills_pathological_pattern_and_tokenizer_continues_line() {
    let pathological = "a".repeat(256);
    let matcher = RegexMatcher::new(r"(?=(a+)+b)(a+)+b");
    let report = matcher.find_report(&pathological, 0, AnchorContext::line_start());
    assert!(
        matches!(report, Err(FallbackError::BudgetExceeded { .. })),
        "expected fallback budget kill, got {report:?}"
    );

    let grammar = r##"{
        "scopeName": "source.budget",
        "patterns": [
            {"match":"(?=(a+)+b)(a+)+b", "name":"invalid.pathological.budget"},
            {"match":"ok", "name":"keyword.control.budget"}
        ]
    }"##;
    let mut tokenizer = TextMateTokenizer::from_grammar(grammar).unwrap();
    let line = format!("{} ok", "a".repeat(256));
    let tokenized = tokenizer.tokenize_line_scopes(&line, TokenizerState::default());
    assert_token_invariants(&line, &tokenized);
    assert!(
        tokenized
            .tokens
            .iter()
            .any(|token| token.range == (257..259)
                && token
                    .scopes
                    .iter()
                    .any(|scope| scope == "keyword.control.budget")),
        "tokenizer should degrade the pathological prefix but still highlight later safe ranges: {:#?}",
        tokenized.tokens
    );
}

fn compare_exact_scopes(
    case: &CaseSpec,
    records: &[GoldenLine],
    divergences: &mut [RuntimeDivergence],
    failures: &mut Vec<String>,
) {
    let mut tokenizer = tokenizer_for_case(case);
    let mut state = TokenizerState::default();
    for (index, record) in records.iter().enumerate() {
        validate_record(case, record, index, failures);
        if is_allowlisted(divergences, case, index + 1, ComparisonMode::Exact) {
            continue;
        }
        // vscode-textmate tokenizes each logical line with a synthetic `\n`;
        // many real grammars close line comments with a literal `\n` pattern.
        let parse_line = format!("{}\n", record.line);
        let tokenized = tokenizer.tokenize_line_scopes_at_line(&parse_line, state, index);
        state = tokenized.state.clone();
        let actual = coalesce_scope_tokens(tokenized.tokens.iter().map(|token| {
            (
                token.range.start.min(record.line.len())..token.range.end.min(record.line.len()),
                token.scopes.clone(),
            )
        }));
        let expected = coalesce_scope_tokens(record.tokens.iter().filter_map(|token| {
            let range = utf16_range_to_utf8(&record.line, token.start_index, token.end_index)
                .or_else(|| (record.line.is_empty() && token.start_index == 0).then_some(0..0))?;
            Some((range, token.scopes.clone()))
        }));
        if actual != expected {
            failures.push(format!(
                "exact mismatch: {} {} line {} ruleStackHash={} expected={} actual={} line={:?}",
                case.language,
                case.fixture,
                index + 1,
                record.rule_stack_hash.as_deref().unwrap_or("<missing>"),
                summarize_scoped(&expected),
                summarize_scoped(&actual),
                record.line,
            ));
        }
    }
}

fn compare_coarse_highlights(
    case: &CaseSpec,
    records: &[GoldenLine],
    divergences: &mut [RuntimeDivergence],
    failures: &mut Vec<String>,
) {
    let source = fs::read_to_string(repo_path(&case.fixture)).expect("fixture source");
    let allowed_lines = records
        .iter()
        .enumerate()
        .filter_map(|(index, _)| {
            is_allowlisted(divergences, case, index + 1, ComparisonMode::Coarse).then_some(index)
        })
        .collect::<HashSet<_>>();
    if allowed_lines.len() == records.len() {
        return;
    }
    let mut tokenizer = tokenizer_for_case(case);
    let highlighted = tokenizer.tokenize_source(&source);
    let expected = expected_coarse_segments(records);
    if highlighted.lines.len() != expected.len() {
        failures.push(format!(
            "coarse line count mismatch: {} {} expected {} actual {}",
            case.language,
            case.fixture,
            expected.len(),
            highlighted.lines.len()
        ));
        return;
    }
    for (index, expected_segments) in expected.iter().enumerate() {
        if allowed_lines.contains(&index) {
            continue;
        }
        // Highlighted output now retains exact scope-stack boundaries and IDs.
        // This compatibility assertion intentionally checks only the old
        // coarse-class projection.
        let actual_segments = coarse_projection(&highlighted.lines[index].segments);
        if actual_segments != *expected_segments {
            failures.push(format!(
                "coarse mismatch: {} {} line {} expected={:?} actual={:?} line={:?}",
                case.language,
                case.fixture,
                index + 1,
                expected_segments,
                actual_segments,
                records
                    .get(index)
                    .map(|record| record.line.as_str())
                    .unwrap_or(""),
            ));
        }
    }
}

fn coarse_projection(segments: &[SyntaxSegment]) -> Vec<SyntaxSegment> {
    let mut projected = Vec::with_capacity(segments.len());
    for segment in segments {
        push_segment(
            &mut projected,
            segment.byte_start..segment.byte_end,
            segment.class,
        );
    }
    projected
}

fn validate_record(case: &CaseSpec, record: &GoldenLine, index: usize, failures: &mut Vec<String>) {
    if record.language != case.language {
        failures.push(format!(
            "golden language mismatch for {} line {}: expected {}, got {}",
            case.fixture,
            index + 1,
            case.language,
            record.language
        ));
    }
    if record.scope_name != case.scope {
        failures.push(format!(
            "golden scope mismatch for {} line {}: expected {}, got {}",
            case.fixture,
            index + 1,
            case.scope,
            record.scope_name
        ));
    }
    if record.line_number != Some(index) {
        failures.push(format!(
            "golden lineNumber mismatch for {} line {}: got {:?}",
            case.fixture,
            index + 1,
            record.line_number
        ));
    }
    if record.stopped_early {
        failures.push(format!(
            "oracle stopped early for {} line {} ruleStack={:?}",
            case.fixture,
            index + 1,
            record.rule_stack
        ));
    }
}

fn expected_coarse_segments(records: &[GoldenLine]) -> Vec<Vec<SyntaxSegment>> {
    let mut classifier = ScopeClassifier::default();
    records
        .iter()
        .map(|record| {
            let mut segments = Vec::new();
            for token in &record.tokens {
                let Some(mut range) =
                    utf16_range_to_utf8(&record.line, token.start_index, token.end_index)
                else {
                    continue;
                };
                range.start = range.start.min(record.line.len());
                range.end = range.end.min(record.line.len());
                if range.start >= range.end {
                    continue;
                }
                let class = classifier.class_for_stack(&token.scopes);
                push_segment(&mut segments, range, class);
            }
            segments
        })
        .collect()
}

fn push_segment(
    segments: &mut Vec<SyntaxSegment>,
    range: Range<usize>,
    class: Option<SyntaxClass>,
) {
    if range.start >= range.end {
        return;
    }
    if let Some(last) = segments.last_mut()
        && last.class == class
        && last.byte_end == range.start
    {
        last.byte_end = range.end;
        return;
    }
    segments.push(SyntaxSegment::new(range.start, range.end, class));
}

fn tokenizer_for_case(case: &CaseSpec) -> TextMateTokenizer {
    let mut set = GrammarSet::new();
    let root_source = fs::read_to_string(repo_path(&case.grammar)).expect("root grammar");
    let root = set.load_and_add(&root_source).expect("root grammar parses");
    for embedded in &case.embedded {
        let source = fs::read_to_string(repo_path(&embedded.grammar)).expect("embedded grammar");
        set.load_and_add(&source).expect("embedded grammar parses");
    }
    TextMateTokenizer::new(set, root)
}

fn load_manifest() -> Vec<CaseSpec> {
    let text = fs::read_to_string(repo_path(MANIFEST_PATH)).expect("textmate cases manifest");
    let manifest: Manifest = toml::from_str(&text).expect("valid textmate cases manifest");
    manifest.cases
}

fn load_manifest_for_current_shard() -> Vec<CaseSpec> {
    let cases = load_manifest();
    match configured_shard() {
        Some(shard) => cases
            .into_iter()
            .filter(|case| case_belongs_to_shard(case, shard))
            .collect(),
        None => cases,
    }
}

fn configured_shard() -> Option<GoldenShard> {
    let index = std::env::var(SHARD_INDEX_ENV).ok();
    let total = std::env::var(SHARD_TOTAL_ENV).ok();
    parse_shard(index.as_deref(), total.as_deref()).unwrap_or_else(|error| panic!("{error}"))
}

fn parse_shard(index: Option<&str>, total: Option<&str>) -> Result<Option<GoldenShard>, String> {
    let (Some(index), Some(total)) = (index, total) else {
        return if index.is_none() && total.is_none() {
            Ok(None)
        } else {
            Err(format!(
                "{SHARD_INDEX_ENV} and {SHARD_TOTAL_ENV} must be set together"
            ))
        };
    };
    let index = index
        .parse::<usize>()
        .map_err(|_| format!("{SHARD_INDEX_ENV} must be a non-negative integer"))?;
    let total = total
        .parse::<usize>()
        .map_err(|_| format!("{SHARD_TOTAL_ENV} must be a positive integer"))?;
    if total == 0 {
        return Err(format!("{SHARD_TOTAL_ENV} must be at least 1"));
    }
    if index >= total {
        return Err(format!(
            "{SHARD_INDEX_ENV} ({index}) must be less than {SHARD_TOTAL_ENV} ({total})"
        ));
    }
    Ok(Some(GoldenShard { index, total }))
}

fn case_belongs_to_shard(case: &CaseSpec, shard: GoldenShard) -> bool {
    language_shard(canonical_case_language_id(case), shard.total) == shard.index
}

fn canonical_case_language_id(case: &CaseSpec) -> &str {
    Path::new(&case.grammar)
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".tmLanguage.json"))
        .unwrap_or_else(|| panic!("root grammar path has no language ID: {}", case.grammar))
}

fn language_shard(language: &str, total: usize) -> usize {
    // Streaming the unsigned big-endian digest through the modulus is
    // equivalent to interpreting all 256 bits as one integer, without a
    // big-integer dependency.
    sha256(language.as_bytes())
        .iter()
        .fold(0usize, |remainder, byte| {
            (remainder * 256 + usize::from(*byte)) % total
        })
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (input.len() as u64) * 8;
    let mut padded = input.to_vec();
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut words = [0u32; 64];
        for (word, bytes) in words.iter_mut().zip(chunk.chunks_exact(4)) {
            *word = u32::from_be_bytes(bytes.try_into().expect("four-byte SHA-256 word"));
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let sum1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(sum1)
                .wrapping_add(choice)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let sum0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = sum0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = slot.wrapping_add(value);
        }
    }

    let mut digest = [0u8; 32];
    for (bytes, word) in digest.chunks_exact_mut(4).zip(state) {
        bytes.copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn load_divergences() -> Vec<RuntimeDivergence> {
    if std::env::var_os("SYNTAXMATE_STRICT").is_some() {
        return Vec::new();
    }
    let sharded_languages = configured_shard().map(|_| {
        load_manifest_for_current_shard()
            .into_iter()
            .map(|case| case.language)
            .collect::<HashSet<_>>()
    });
    read_divergence_file()
        .divergences
        .into_iter()
        .filter(|spec| {
            sharded_languages
                .as_ref()
                .is_none_or(|languages| languages.contains(&spec.language))
        })
        .map(|spec| RuntimeDivergence { spec, hits: 0 })
        .collect()
}

fn read_divergence_file() -> DivergenceFile {
    let text = fs::read_to_string(repo_path(DIVERGENCES_PATH)).expect("textmate divergences");
    toml::from_str(&text).expect("valid textmate divergences")
}

fn load_golden_records(case: &CaseSpec) -> Vec<GoldenLine> {
    let text = fs::read_to_string(repo_path(&case.golden))
        .unwrap_or_else(|error| panic!("{} should be readable: {error}", case.golden));
    text.lines()
        .map(|line| serde_json::from_str::<GoldenLine>(line).expect("golden record"))
        .collect()
}

fn assert_case_files_exist(case: &CaseSpec) {
    for file in [&case.grammar, &case.fixture, &case.golden] {
        assert!(
            repo_path(file).exists(),
            "manifest path should exist: {file}"
        );
    }
    for embedded in &case.embedded {
        assert!(
            repo_path(&embedded.grammar).exists(),
            "embedded grammar path should exist: {}",
            embedded.grammar
        );
    }
}

fn validate_divergences(divergences: &[RuntimeDivergence]) {
    for divergence in divergences {
        let spec = &divergence.spec;
        assert!(
            !spec.language.trim().is_empty(),
            "divergence language required"
        );
        assert!(
            !spec.grammar.trim().is_empty(),
            "divergence grammar required"
        );
        assert!(
            !spec.fixture.trim().is_empty(),
            "divergence fixture required"
        );
        assert!(!spec.reason.trim().is_empty(), "divergence reason required");
        assert!(spec.line_start > 0, "divergence line_start is 1-based");
        assert!(
            spec.line_start <= spec.line_end,
            "divergence line range must be ordered: {spec:?}"
        );
    }
}

fn is_allowlisted(
    divergences: &mut [RuntimeDivergence],
    case: &CaseSpec,
    line: usize,
    mode: ComparisonMode,
) -> bool {
    let mut matched = false;
    for divergence in divergences {
        if divergence_matches(&divergence.spec, case, line, mode) {
            divergence.hits += 1;
            matched = true;
        }
    }
    matched
}

fn divergence_matches(
    spec: &DivergenceSpec,
    case: &CaseSpec,
    line: usize,
    mode: ComparisonMode,
) -> bool {
    spec.language == case.language
        && spec.grammar == case.scope
        && spec.fixture == case.fixture
        && spec.line_start <= line
        && line <= spec.line_end
        && matches!(
            (spec.mode, mode),
            (DivergenceMode::Any, _)
                | (DivergenceMode::Exact, ComparisonMode::Exact)
                | (DivergenceMode::Coarse, ComparisonMode::Coarse)
        )
}

fn summarize_scoped(tokens: &[(Range<usize>, Vec<String>)]) -> String {
    const LIMIT: usize = 6;
    let mut parts = tokens
        .iter()
        .take(LIMIT)
        .map(|(range, scopes)| format!("{range:?}:{scopes:?}"))
        .collect::<Vec<_>>();
    if tokens.len() > LIMIT {
        parts.push(format!("… {} more", tokens.len() - LIMIT));
    }
    parts.join(" | ")
}

fn repo_path(path: impl AsRef<Path>) -> PathBuf {
    workspace_root().join(path)
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn generated_utf8_inputs() -> Vec<String> {
    let mut out = vec![
        String::new(),
        "plain ascii".to_owned(),
        "λ🚀 café".to_owned(),
        "\0\u{1f600}\tcontrol".to_owned(),
        "a/=/regex?/中文/עברית".to_owned(),
    ];
    let mut seed = 0x05ee_dcaf_ed15_ca11_u64;
    let alphabet = [
        'a', 'Z', '0', '_', ' ', '\t', '/', '*', '\\', '"', '\'', '`', '{', '}', '[', ']', '(',
        ')', '<', '>', '=', '+', '-', ':', ';', ',', '.', '#', 'λ', 'π', 'é', '🚀', '中', '✓',
    ];
    for len in [1usize, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233] {
        let mut s = String::new();
        for _ in 0..len {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            s.push(alphabet[(seed as usize) % alphabet.len()]);
        }
        out.push(s);
    }
    out
}

fn assert_token_invariants(line: &str, tokenized: &TokenizedLine) {
    let mut cursor = 0usize;
    for token in tokenized.tokens.iter() {
        assert!(token.range.start <= token.range.end, "bad range {token:?}");
        assert!(token.range.start >= cursor, "overlap in {token:?}");
        assert!(token.range.end <= line.len(), "out of bounds in {token:?}");
        assert!(
            line.is_char_boundary(token.range.start),
            "bad start {token:?}"
        );
        assert!(line.is_char_boundary(token.range.end), "bad end {token:?}");
        cursor = token.range.end;
    }

    let expanded_end = tokenized
        .tokens
        .last()
        .map(|token| token.range.end)
        .unwrap_or(0)
        .max(line.len());
    assert_eq!(expanded_end, line.len(), "expanded spans should cover line");

    let unique_boundaries = tokenized
        .tokens
        .iter()
        .flat_map(|token| [token.range.start, token.range.end])
        .collect::<HashSet<_>>();
    assert!(
        unique_boundaries
            .iter()
            .all(|offset| line.is_char_boundary(*offset))
    );
}
