//! Measure the shared binding API, including flat-buffer construction.
//!
//! Usage: profile-boundary OP LANGUAGE FILE UNIT SCOPES ITERATIONS MODE
//! OP: tokens|session|html|ansi; UNIT: utf8|utf16|codepoint; SCOPES: true|false;
//! MODE: first|replay|evicted. `first` requires one iteration. Session mode
//! aggregates sequential line calls and resets between documents. `evicted`
//! displaces the engine's line-result cache outside measured intervals; unlike
//! a disabled cache, duplicate lines inside a document can still hit.
//!
//! Normal builds use System with compile-time-eliminated allocation counters.
//! Build a separate instrumented binary with SYNTAXMATE_PROFILE_ALLOC=1.
//! Report its allocation counters separately; do not use its times for claims.
//! Timed intervals include returned output construction, exclude output drop,
//! validation, digest computation, setup, and eviction. Process startup is not
//! measured here. JSON output includes every sample, complete-output digest,
//! cumulative requested bytes, API-boundary live bytes, peak additional live
//! bytes, and live bytes after dropping returned output (possibly cache growth).

#[cfg(feature = "bundled-grammars")]
mod profiler {
    use std::{
        alloc::{GlobalAlloc, Layout, System},
        hint::black_box,
        sync::atomic::{AtomicU64, Ordering::Relaxed},
        time::Instant,
    };
    use syntaxmate_boundary::{
        AnsiOptions, Engine, HtmlOptions, OffsetUnit, Rendered, ThemeHandle, TokenBuffer,
        TokenOptions,
    };

    const COUNT: bool = option_env!("SYNTAXMATE_PROFILE_ALLOC").is_some();
    static CALLS: AtomicU64 = AtomicU64::new(0);
    static REALLOCS: AtomicU64 = AtomicU64::new(0);
    static BYTES: AtomicU64 = AtomicU64::new(0);
    static LIVE: AtomicU64 = AtomicU64::new(0);
    static PEAK: AtomicU64 = AtomicU64::new(0);
    struct Allocator;
    fn add(size: usize) {
        BYTES.fetch_add(size as u64, Relaxed);
        let live = LIVE.fetch_add(size as u64, Relaxed) + size as u64;
        PEAK.fetch_max(live, Relaxed);
    }
    unsafe impl GlobalAlloc for Allocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc(layout) };
            if COUNT && !p.is_null() {
                CALLS.fetch_add(1, Relaxed);
                add(layout.size());
            }
            p
        }
        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let p = unsafe { System.alloc_zeroed(layout) };
            if COUNT && !p.is_null() {
                CALLS.fetch_add(1, Relaxed);
                add(layout.size());
            }
            p
        }
        unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
            if COUNT {
                LIVE.fetch_sub(layout.size() as u64, Relaxed);
            }
            unsafe { System.dealloc(p, layout) };
        }
        unsafe fn realloc(&self, p: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            let p = unsafe { System.realloc(p, layout, size) };
            if COUNT && !p.is_null() {
                REALLOCS.fetch_add(1, Relaxed);
                LIVE.fetch_sub(layout.size() as u64, Relaxed);
                add(size);
            }
            p
        }
    }
    #[global_allocator]
    static ALLOCATOR: Allocator = Allocator;

    #[derive(Clone, Copy)]
    struct Stats {
        calls: u64,
        reallocs: u64,
        bytes: u64,
        live: u64,
    }
    impl Stats {
        fn now() -> Self {
            Self {
                calls: CALLS.load(Relaxed),
                reallocs: REALLOCS.load(Relaxed),
                bytes: BYTES.load(Relaxed),
                live: LIVE.load(Relaxed),
            }
        }
    }

    // Boxing the large variant would add an allocation inside the timed API interval.
    #[allow(clippy::large_enum_variant)]
    #[derive(PartialEq, Eq)]
    enum Output {
        Tokens(TokenBuffer),
        Session(Vec<TokenBuffer>),
        Rendered(Rendered),
    }
    impl Output {
        fn value(&self) -> serde_json::Value {
            let buffer = |b: &TokenBuffer| {
                assert!(b.complete, "degraded output cannot establish a speedup");
                serde_json::json!({
                    "unit": b.unit as u32, "complete": b.complete,
                    "lineStarts": b.line_starts, "lineTokenRanges": b.line_token_ranges,
                    "tokenStarts": b.token_starts, "tokenLengths": b.token_lengths,
                    "tokenStyles": b.token_styles, "tokenScopes": b.token_scopes,
                    "styles": b.styles.iter().map(|s| [s.foreground, s.background, u32::from(s.modifiers)]).collect::<Vec<_>>(),
                    "scopeStacks": b.scope_stacks,
                    "defaultStyle": [b.default_style.foreground, b.default_style.background, u32::from(b.default_style.modifiers)],
                })
            };
            match self {
                Self::Tokens(b) => buffer(b),
                Self::Session(buffers) => buffers.iter().map(buffer).collect(),
                Self::Rendered(r) => {
                    assert!(r.complete, "degraded output cannot establish a speedup");
                    serde_json::json!({"text": r.text, "complete": r.complete})
                }
            }
        }
    }

    fn digest(bytes: &[u8]) -> String {
        format!(
            "{:016x}",
            bytes.iter().fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            })
        )
    }

    pub fn main() {
        let args: Vec<String> = std::env::args().skip(1).collect();
        assert_eq!(
            args.len(),
            7,
            "usage: profile-boundary OP LANGUAGE FILE UNIT SCOPES ITERATIONS MODE"
        );
        let [op, language, path, unit, scopes, iterations, mode] = args.as_slice() else {
            unreachable!()
        };
        let unit = match unit.as_str() {
            "utf8" => OffsetUnit::Utf8,
            "utf16" => OffsetUnit::Utf16,
            "codepoint" => OffsetUnit::CodePoint,
            _ => panic!("invalid offset unit"),
        };
        let options = TokenOptions {
            unit,
            include_scopes: scopes.parse().unwrap(),
        };
        let iterations: usize = iterations.parse().unwrap();
        assert!(iterations > 0);
        assert!(matches!(mode.as_str(), "first" | "replay" | "evicted"));
        assert!(mode != "first" || iterations == 1);
        assert!(
            mode != "evicted" || op != "session",
            "session owns a distinct line cache"
        );
        let source = std::fs::read_to_string(path).unwrap();
        let setup = Instant::now();
        let engine = Engine::bundled().unwrap();
        let theme = ThemeHandle::bundled("github-dark").unwrap();
        let setup_ns = setup.elapsed().as_nanos();
        let mut session =
            (op == "session").then(|| engine.session(language, &theme, options).unwrap());
        let eviction = (mode == "evicted").then(|| {
            (0..2048)
                .map(|i| format!("__syntaxmate_cache_eviction_{i}__\n"))
                .collect::<String>()
        });
        let mut call = || match op.as_str() {
            "tokens" => Output::Tokens(engine.tokens(language, &source, &theme, options).unwrap()),
            "session" => {
                let s = session.as_mut().unwrap();
                s.reset();
                Output::Session(
                    source
                        .split('\n')
                        .map(|line| s.line(line).unwrap())
                        .collect(),
                )
            }
            "html" => Output::Rendered(
                engine
                    .html(language, &source, &theme, &HtmlOptions::default())
                    .unwrap(),
            ),
            "ansi" => Output::Rendered(
                engine
                    .ansi(language, &source, &theme, &AnsiOptions::default())
                    .unwrap(),
            ),
            _ => panic!("invalid operation"),
        };
        // Keep one reference output outside measurement for exact equality on every call.
        let reference = if mode == "first" {
            None
        } else {
            let first = call();
            first.value();
            drop(call());
            drop(call());
            Some(first)
        };
        let mut elapsed = Vec::with_capacity(iterations);
        let mut allocations = Vec::with_capacity(iterations);
        let mut output_digest = None;
        for _ in 0..iterations {
            if let Some(eviction) = &eviction {
                let displaced = engine.tokens(language, eviction, &theme, options).unwrap();
                assert!(displaced.complete);
            }
            let before = Stats::now();
            PEAK.store(before.live, Relaxed);
            let start = Instant::now();
            let output = black_box(call());
            let ns = start.elapsed().as_nanos();
            let after = Stats::now();
            let peak = PEAK.load(Relaxed);
            if let Some(reference) = &reference {
                assert!(
                    *reference == output,
                    "observable output changed across calls"
                );
            }
            if output_digest.is_none() {
                output_digest = Some(digest(&serde_json::to_vec(&output.value()).unwrap()));
            }
            let before_drop = Stats::now();
            drop(output);
            let after_drop = Stats::now();
            elapsed.push(ns);
            allocations.push(serde_json::json!({
                "allocations": after.calls - before.calls,
                "reallocations": after.reallocs - before.reallocs,
                "cumulativeBytes": after.bytes - before.bytes,
                "liveAtReturn": i128::from(after.live) - i128::from(before.live),
                "peakAdditionalLive": peak.saturating_sub(before.live),
                "liveAfterOutputDrop": i128::from(after.live) - i128::from(before.live)
                    + i128::from(after_drop.live) - i128::from(before_drop.live),
            }));
        }
        println!(
            "{}",
            serde_json::json!({
                "operation": op, "language": language, "path": path,
                "sourceBytes": source.len(), "sourceDigest": digest(source.as_bytes()),
                "unit": unit as u32, "includeScopes": options.include_scopes,
                "mode": mode, "instrumented": COUNT, "iterations": iterations,
                "setupNs": setup_ns, "elapsedNs": elapsed,
                "complete": true, "outputDigest": output_digest,
                "allocations": if COUNT { Some(allocations) } else { None },
            })
        );
    }
}

#[cfg(feature = "bundled-grammars")]
fn main() {
    profiler::main();
}
#[cfg(not(feature = "bundled-grammars"))]
fn main() {
    panic!("profile-boundary requires bundled-grammars");
}
