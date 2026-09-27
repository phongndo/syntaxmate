//! Throughput with independent worker state and one shared highlighter.
use std::{env, fs, hint::black_box, sync::Barrier, time::Instant};
use syntaxmate::{Catalog, Highlighter, HighlighterOptions, TokenizerOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    let language = args
        .get(1)
        .ok_or("expected LANGUAGE FILE THREADS ITERATIONS")?;
    let source = fs::read_to_string(args.get(2).ok_or("missing file")?)?;
    let threads: usize = args.get(3).ok_or("missing threads")?.parse()?;
    let iterations: usize = args.get(4).ok_or("missing iterations")?.parse()?;
    if threads == 0 || iterations == 0 {
        return Err("counts must be positive".into());
    }
    let highlighter = Highlighter::with_catalog_options(
        &Catalog::bundled(),
        HighlighterOptions {
            idle_tokenizers_per_language: threads,
            tokenizer: TokenizerOptions {
                line_cache_entries: 0,
                ..TokenizerOptions::default()
            },
            ..HighlighterOptions::default()
        },
    );
    let expected = highlighter.tokenize(language, &source)?;
    let barrier = Barrier::new(threads);
    let elapsed = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    assert_eq!(highlighter.tokenize(language, &source).unwrap(), expected);
                    barrier.wait();
                    let start = Instant::now();
                    for _ in 0..iterations {
                        let document = highlighter.tokenize(language, &source).unwrap();
                        assert!(document.status().is_complete());
                        black_box(document);
                    }
                    start.elapsed()
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .max()
            .unwrap()
    });
    println!(
        "{}",
        serde_json::json!({
            "threads": threads, "iterationsPerThread": iterations,
            "elapsedSeconds": elapsed.as_secs_f64(),
            "documentsPerSecond": (threads * iterations) as f64 / elapsed.as_secs_f64(),
            "complete": true,
        })
    );
    Ok(())
}
