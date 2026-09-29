//! Native Rust baseline for `bench.cpp`: the same workloads through
//! `syntaxmate_boundary` directly, so the difference is the C ABI's cost.
//! Run with `make -C bindings/c bench`; see the README.

use std::{env, fs, hint::black_box, time::Instant};

use syntaxmate_boundary::{Engine, HtmlOptions, OffsetUnit, ThemeHandle, TokenOptions};

const THEME: &str = "github-dark";

/// Each line gets a trailing-whitespace suffix encoding the variant number, so
/// cycling through the variants misses the engine's per-tokenizer line cache
/// (1,024 lines): a cycle covers at least 4,096 distinct lines.
fn suffix_bits(lines: usize) -> u32 {
    (0..16).find(|bits| (lines << bits) >= 4096).unwrap_or(16)
}

fn variant(source: &str, number: usize, bits: u32) -> String {
    let suffix: String = (0..bits)
        .map(|bit| if number >> bit & 1 == 1 { '\t' } else { ' ' })
        .collect();
    let mut out = String::with_capacity(source.len() + source.len() / 4);
    for (index, line) in source.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(line);
        out.push_str(&suffix);
    }
    out
}

struct Case {
    language: String,
    variants: Vec<String>,
    lines: Vec<Vec<String>>,
}

/// Runs `call` until `min_ms` elapses per sample; returns the median ns/call.
fn measure(samples: usize, min_ms: u64, mut call: impl FnMut(usize)) -> f64 {
    let mut results = Vec::with_capacity(samples);
    let mut index = 0;
    call(index); // warm-up
    for _ in 0..samples {
        let started = Instant::now();
        let mut calls = 0u64;
        loop {
            index += 1;
            call(index);
            calls += 1;
            if started.elapsed().as_millis() as u64 >= min_ms {
                break;
            }
        }
        results.push(started.elapsed().as_nanos() as f64 / calls as f64);
    }
    results.sort_by(f64::total_cmp);
    results[results.len() / 2]
}

/// Measures and prints one mode unless `BENCH_MODES` (comma-separated) omits it.
fn run(
    samples: usize,
    min_ms: u64,
    language: &str,
    mode: &str,
    bytes: usize,
    call: impl FnMut(usize),
) {
    if let Ok(only) = env::var("BENCH_MODES")
        && !only.is_empty()
        && !only.split(',').any(|m| m == mode)
    {
        return;
    }
    let ns = measure(samples, min_ms, call);
    let mbps = bytes as f64 / ns * 1e3;
    println!("{language:<12} {mode:<17} {ns:>12.0} {mbps:>9.1}");
}

fn main() {
    let mut args = env::args().skip(1);
    let samples: usize = args.next().and_then(|s| s.parse().ok()).expect("samples");
    let min_ms: u64 = args.next().and_then(|s| s.parse().ok()).expect("min-ms");
    let cases: Vec<Case> = args
        .map(|spec| {
            let (language, path) = spec.split_once('=').expect("LANGUAGE=PATH");
            let source = fs::read_to_string(path).expect("readable fixture");
            let bits = suffix_bits(source.split('\n').count());
            let variants: Vec<String> = (0..1usize << bits)
                .map(|n| variant(&source, n, bits))
                .collect();
            let lines = variants
                .iter()
                .map(|v| v.split('\n').map(str::to_owned).collect())
                .collect();
            Case {
                language: language.to_owned(),
                variants,
                lines,
            }
        })
        .collect();

    let engine = Engine::bundled().expect("engine");
    let theme = ThemeHandle::bundled(THEME).expect("theme");
    let html = HtmlOptions::default();
    let utf8 = TokenOptions::default();
    let scopes = TokenOptions {
        unit: OffsetUnit::Utf8,
        include_scopes: true,
    };
    println!(
        "{:<12} {:<17} {:>12} {:>9}",
        "language", "mode", "ns/call", "MB/s"
    );
    for case in &cases {
        let language = case.language.as_str();
        let n = case.variants.len();
        let bytes = case.variants[0].len();
        let bench = |mode: &str, call: &mut dyn FnMut(usize)| {
            run(samples, min_ms, language, mode, bytes, call);
        };
        bench("html-cold", &mut |_| {
            let fresh = Engine::bundled().expect("engine");
            let theme = ThemeHandle::bundled(THEME).expect("theme");
            black_box(
                fresh
                    .html(language, &case.variants[0], &theme, &html)
                    .unwrap(),
            );
        });
        bench("html-steady", &mut |i| {
            black_box(
                engine
                    .html(language, &case.variants[i % n], &theme, &html)
                    .unwrap(),
            );
        });
        bench("html-replay", &mut |_| {
            black_box(
                engine
                    .html(language, &case.variants[0], &theme, &html)
                    .unwrap(),
            );
        });
        bench("tokens-steady", &mut |i| {
            black_box(
                engine
                    .tokens(language, &case.variants[i % n], &theme, utf8)
                    .unwrap(),
            );
        });
        bench("scopes-steady", &mut |i| {
            black_box(
                engine
                    .tokens(language, &case.variants[i % n], &theme, scopes)
                    .unwrap(),
            );
        });
        let mut session = engine.session(language, &theme, utf8).expect("session");
        bench("session-steady", &mut |i| {
            session.reset();
            for line in &case.lines[i % n] {
                black_box(session.line(line).unwrap());
            }
        });
    }
    run(samples, min_ms, "rust", "html-tiny", 10, |_| {
        black_box(engine.html("rust", "let x = 1;", &theme, &html).unwrap());
    });
}
