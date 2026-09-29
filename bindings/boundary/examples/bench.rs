//! Measures the boundary's overhead on top of the raw `syntaxmate` API.
//!
//! ```sh
//! cd bindings && cargo run --release -p syntaxmate-boundary --example bench -- \
//!     [--ms 50] [--rounds 15] [--only <case substring>] [--no-cold] [lang...]
//! ```
//!
//! `--only` runs just the matching cases (plus their baselines), which is
//! convenient under a profiler.
//!
//! Inputs are the competitive benchmarks' stress fixtures. Steady timings
//! rotate trailing-space variants of every line so the engine's line-result
//! cache never hits (matching a `line_cache_entries: 0` highlighter); both
//! sides process identical inputs. Cases run interleaved in rounds and each is
//! reported as the median of its per-round ratio to a raw-API baseline, which
//! keeps the comparison stable on a busy machine. Cold timings build a fresh
//! engine and time its first call, including grammar preparation.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use syntaxmate::{Highlighter, Theme};
use syntaxmate_boundary::{Engine, HtmlOptions, OffsetUnit, ThemeHandle, TokenOptions};

const LANGUAGES: &[&str] = &[
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
];
const THEME: &str = "github-dark";
// Enough variants that their lines exceed the engine's 1024-entry line cache.
const VARIANTS: usize = 16;

type Run<'a> = Box<dyn Fn(&str, &str) -> usize + 'a>;

struct Case<'a> {
    name: &'static str,
    /// Index of the raw-API case this one is compared with.
    baseline: usize,
    run: Run<'a>,
}

fn main() {
    let mut millis = 50;
    let mut rounds = 15;
    let mut only = None;
    let mut cold = true;
    let mut languages = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--ms" => millis = args.next().unwrap().parse().unwrap(),
            "--rounds" => rounds = args.next().unwrap().parse().unwrap(),
            "--only" => only = args.next(),
            "--no-cold" => cold = false,
            _ => languages.push(arg),
        }
    }
    if languages.is_empty() {
        languages = LANGUAGES.iter().map(|l| (*l).to_owned()).collect();
    }
    let budget = Duration::from_millis(millis);

    let engine = Engine::bundled().unwrap();
    let handle = ThemeHandle::bundled(THEME).unwrap();
    let highlighter = Highlighter::bundled().unwrap();
    let theme = Theme::bundled(THEME).unwrap();

    let tokens = |unit, include_scopes| -> Run {
        let (engine, handle) = (&engine, &handle);
        let options = TokenOptions {
            unit,
            include_scopes,
        };
        Box::new(move |language, source| {
            let buffer = engine.tokens(language, source, handle, options).unwrap();
            assert!(buffer.complete);
            buffer.token_starts.len()
        })
    };
    let session = |unit| -> Run {
        let (engine, handle) = (&engine, &handle);
        let options = TokenOptions {
            unit,
            include_scopes: false,
        };
        Box::new(move |language, source| {
            let mut session = engine.session(language, handle, options).unwrap();
            source
                .split('\n')
                .map(|line| session.line(line).unwrap().token_starts.len())
                .sum()
        })
    };
    let case = |name, baseline, run| Case {
        name,
        baseline,
        run,
    };
    let mut cases = vec![
        case(
            "raw highlight_html",
            0,
            Box::new(|language, source| {
                let output = highlighter.highlight_html(language, source, THEME);
                output.unwrap().as_str().len()
            }),
        ),
        case(
            "Engine::html",
            0,
            Box::new(|language, source| {
                let options = HtmlOptions::default();
                let output = engine.html(language, source, &handle, &options);
                output.unwrap().text.len()
            }),
        ),
        case(
            "raw highlight_with_theme",
            2,
            Box::new(|language, source| {
                let document = highlighter.highlight_with_theme(language, source, &theme);
                document.unwrap().lines().len()
            }),
        ),
        case(
            "raw tokenize",
            2,
            Box::new(|language, source| {
                let document = highlighter.tokenize(language, source);
                document.unwrap().lines().len()
            }),
        ),
        case("Engine::tokens utf8", 2, tokens(OffsetUnit::Utf8, false)),
        case("Engine::tokens utf16", 2, tokens(OffsetUnit::Utf16, false)),
        case(
            "Engine::tokens codepoint",
            2,
            tokens(OffsetUnit::CodePoint, false),
        ),
        case(
            "Engine::tokens utf8+scopes",
            2,
            tokens(OffsetUnit::Utf8, true),
        ),
        case(
            "Engine::tokens utf16+scopes",
            2,
            tokens(OffsetUnit::Utf16, true),
        ),
        case(
            "raw session lines",
            9,
            Box::new(|language, source| {
                let mut session = highlighter.session_with_theme(language, &theme).unwrap();
                let mut spans = Vec::new();
                let mut count = 0;
                for line in source.split('\n') {
                    session.highlight_line_into(line, &mut spans).unwrap();
                    count += spans.len();
                }
                count
            }),
        ),
        case("Session::line utf8", 9, session(OffsetUnit::Utf8)),
        case("Session::line utf16", 9, session(OffsetUnit::Utf16)),
    ];

    if let Some(only) = &only {
        // Keep the matching cases and the baselines they are compared with.
        let names: Vec<&str> = cases.iter().map(|case| case.name).collect();
        let keep: Vec<bool> = (0..cases.len())
            .map(|index| {
                cases.iter().any(|case| {
                    case.name.contains(only.as_str())
                        && (case.name == names[index] || case.baseline == index)
                })
            })
            .collect();
        let mut index = 0;
        cases.retain(|_| {
            index += 1;
            keep[index - 1]
        });
        let kept: Vec<&str> = names
            .iter()
            .zip(&keep)
            .filter_map(|(name, kept)| kept.then_some(*name))
            .collect();
        for case in &mut cases {
            let baseline = names[case.baseline];
            case.baseline = kept.iter().position(|name| *name == baseline).unwrap();
        }
    }

    println!(
        "steady: {rounds} interleaved rounds of >= {millis} ms per case; \
         baselines in µs/document, other rows as median ratio to their baseline"
    );
    print!("{:<28}", "case");
    for language in &languages {
        print!("{:>10}", &language[..language.len().min(9)]);
    }
    println!("{:>10}", "geomean");

    // results[language][case] = (median µs, median ratio to baseline)
    let mut results = Vec::new();
    for language in &languages {
        let variants = variants(&fixture(language));
        let mut samples = vec![Vec::new(); cases.len()];
        for case in &cases {
            for variant in &variants {
                black_box((case.run)(language, variant));
            }
        }
        for round in 0..rounds {
            for offset in 0..cases.len() {
                let index = (round + offset) % cases.len();
                let run = &cases[index].run;
                let started = Instant::now();
                let mut documents = 0usize;
                while started.elapsed() < budget {
                    for variant in &variants {
                        black_box(run(language, variant));
                    }
                    documents += variants.len();
                }
                samples[index].push(started.elapsed().as_secs_f64() * 1e6 / documents as f64);
            }
        }
        let per_case: Vec<(f64, f64)> = cases
            .iter()
            .enumerate()
            .map(|(index, case)| {
                let ratios = samples[index]
                    .iter()
                    .zip(&samples[case.baseline])
                    .map(|(sample, base)| sample / base)
                    .collect();
                (median(samples[index].clone()), median(ratios))
            })
            .collect();
        results.push(per_case);
    }
    for (index, case) in cases.iter().enumerate() {
        print!("{:<28}", case.name);
        let is_baseline = case.baseline == index;
        let values: Vec<f64> = results
            .iter()
            .map(|row| {
                let (micros, ratio) = row[index];
                if is_baseline { micros } else { ratio }
            })
            .collect();
        for value in &values {
            if is_baseline {
                print!("{value:>10.0}");
            } else {
                print!("{value:>9.2}x");
            }
        }
        let geomean = (values.iter().map(|v| v.ln()).sum::<f64>() / values.len() as f64).exp();
        if is_baseline {
            println!("{geomean:>10.0}");
        } else {
            println!("{geomean:>9.2}x");
        }
    }

    if !cold {
        return;
    }
    println!("\ncold: fresh engine and first call, µs (median of 5)");
    for language in &languages {
        let source = fixture(language);
        let mut raw = Vec::new();
        let mut boundary = Vec::new();
        for _ in 0..5 {
            let started = Instant::now();
            let highlighter = Highlighter::bundled().unwrap();
            black_box(highlighter.highlight_with_theme(language, &source, &theme)).unwrap();
            raw.push(started.elapsed().as_secs_f64() * 1e6);
            let started = Instant::now();
            let engine = Engine::bundled().unwrap();
            let options = TokenOptions {
                unit: OffsetUnit::Utf16,
                include_scopes: false,
            };
            black_box(engine.tokens(language, &source, &handle, options)).unwrap();
            boundary.push(started.elapsed().as_secs_f64() * 1e6);
        }
        println!(
            "{language:<12} raw highlight_with_theme {:>8.0}   Engine::tokens utf16 {:>8.0}",
            median(raw),
            median(boundary)
        );
    }
}

fn fixture(language: &str) -> String {
    let dir = format!(
        "{}/../../tests/fixtures/textmate/{language}",
        env!("CARGO_MANIFEST_DIR")
    );
    let path = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.starts_with("stress.") && !name.ends_with(".golden.jsonl")
        })
        .unwrap_or_else(|| panic!("no stress fixture in {dir}"));
    std::fs::read_to_string(path).unwrap()
}

/// Copies of `source` whose lines carry 0..VARIANTS trailing spaces.
fn variants(source: &str) -> Vec<String> {
    (0..VARIANTS)
        .map(|n| {
            let pad = " ".repeat(n);
            source
                .split('\n')
                .map(|line| format!("{line}{pad}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    values[values.len() / 2]
}
