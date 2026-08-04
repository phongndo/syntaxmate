use std::{env, fs, hint::black_box, time::Instant};

use syntaxmate::{Highlighter, TokenizerOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse()?;
    let source = fs::read_to_string(&args.file)?;

    let setup_started = Instant::now();
    let mut highlighter = if args.phase == "steady" {
        Highlighter::with_options(TokenizerOptions {
            line_cache_entries: 0,
            ..TokenizerOptions::default()
        })?
    } else {
        Highlighter::bundled()?
    };
    let setup_nanos = nanos(setup_started.elapsed());

    let (iterations, elapsed_nanos, output) = match args.phase.as_str() {
        "cold" => {
            let started = Instant::now();
            let output = highlighter.highlight_html(&args.language, &source, "github-dark")?;
            (1, nanos(started.elapsed()), output)
        }
        "steady" | "replay" => {
            let warmup = highlighter.highlight_html(&args.language, &source, "github-dark")?;
            if !warmup.status().is_complete() {
                return Err("warmup highlighting degraded".into());
            }
            calibrate(args.minimum_time_ms, || {
                highlighter.highlight_html(&args.language, &source, "github-dark")
            })?
        }
        phase => return Err(format!("unsupported phase {phase:?}").into()),
    };
    if !output.status().is_complete() {
        return Err("highlighting degraded".into());
    }

    println!(
        "{}",
        serde_json::json!({
            "schemaVersion": 1,
            "track": "end-to-end",
            "engine": "syntaxmate",
            "version": env!("CARGO_PKG_VERSION"),
            "phase": args.phase,
            "iterations": iterations,
            "sourceBytes": source.len(),
            "processedBytes": source.len().saturating_mul(iterations),
            "setupNanoseconds": setup_nanos,
            "elapsedNanoseconds": elapsed_nanos,
            "outputBytes": output.as_str().len(),
            "outputDigest": fnv1a(output.as_str().as_bytes()),
            "complete": true,
        })
    );
    Ok(())
}

fn calibrate<F>(
    minimum_time_ms: u64,
    mut operation: F,
) -> Result<(usize, u64, syntaxmate::RenderedOutput), syntaxmate::Error>
where
    F: FnMut() -> syntaxmate::Result<syntaxmate::RenderedOutput>,
{
    let target_nanos = u128::from(minimum_time_ms) * 1_000_000;
    let mut iterations = 1usize;
    loop {
        let started = Instant::now();
        let mut last = None;
        for _ in 0..iterations {
            let output = operation()?;
            black_box(output.as_str().len());
            last = Some(output);
        }
        let elapsed = started.elapsed();
        if elapsed.as_nanos() >= target_nanos || iterations >= 16_384 {
            return Ok((
                iterations,
                nanos(elapsed),
                last.expect("positive iteration count"),
            ));
        }
        iterations = iterations.saturating_mul(2);
    }
}

fn nanos(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn fnv1a(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

struct Args {
    language: String,
    file: String,
    phase: String,
    minimum_time_ms: u64,
}

impl Args {
    fn parse() -> Result<Self, Box<dyn std::error::Error>> {
        let mut language = None;
        let mut file = None;
        let mut phase = None;
        let mut minimum_time_ms = 100;
        let raw = env::args().skip(1).collect::<Vec<_>>();
        let mut index = 0;
        while index < raw.len() {
            let value = raw
                .get(index + 1)
                .ok_or_else(|| format!("{} requires a value", raw[index]))?;
            match raw[index].as_str() {
                "--language" => language = Some(value.clone()),
                "--file" => file = Some(value.clone()),
                "--phase" => phase = Some(value.clone()),
                "--minimum-time-ms" => minimum_time_ms = value.parse()?,
                option => return Err(format!("unknown option {option:?}").into()),
            }
            index += 2;
        }
        if minimum_time_ms == 0 {
            return Err("--minimum-time-ms must be positive".into());
        }
        Ok(Self {
            language: language.ok_or("--language is required")?,
            file: file.ok_or("--file is required")?,
            phase: phase.ok_or("--phase is required")?,
            minimum_time_ms,
        })
    }
}
