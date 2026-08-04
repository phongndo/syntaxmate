use std::{env, fs, hint::black_box, time::Instant};

use syntect::{highlighting::ThemeSet, html::highlighted_html_for_string, parsing::SyntaxSet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse()?;
    let source = fs::read_to_string(&args.file)?;

    let setup_started = Instant::now();
    let syntaxes = SyntaxSet::load_defaults_newlines();
    let themes = ThemeSet::load_defaults();
    let syntax = syntaxes
        .find_syntax_by_extension(&args.extension)
        .ok_or_else(|| format!("no default Syntect syntax for {:?}", args.extension))?;
    let theme = themes
        .themes
        .get("base16-ocean.dark")
        .ok_or("Syntect lacks the base16-ocean.dark default theme")?;
    let setup_nanos = nanos(setup_started.elapsed());

    let operation = || highlighted_html_for_string(&source, &syntaxes, syntax, theme);
    let (iterations, elapsed_nanos, output) = match args.phase.as_str() {
        "cold" => {
            let started = Instant::now();
            let output = operation()?;
            (1, nanos(started.elapsed()), output)
        }
        "steady" | "replay" => {
            black_box(operation()?);
            calibrate(args.minimum_time_ms, operation)?
        }
        phase => return Err(format!("unsupported phase {phase:?}").into()),
    };

    println!(
        "{}",
        serde_json::json!({
            "schemaVersion": 1,
            "track": "end-to-end",
            "engine": "syntect",
            "version": "5.3.0",
            "regexEngine": "onig (default-onig feature)",
            "phase": args.phase,
            "iterations": iterations,
            "sourceBytes": source.len(),
            "processedBytes": source.len().saturating_mul(iterations),
            "setupNanoseconds": setup_nanos,
            "elapsedNanoseconds": elapsed_nanos,
            "outputBytes": output.len(),
            "outputDigest": fnv1a(output.as_bytes()),
            "complete": true,
        })
    );
    Ok(())
}

fn calibrate<F>(
    minimum_time_ms: u64,
    mut operation: F,
) -> Result<(usize, u64, String), syntect::Error>
where
    F: FnMut() -> Result<String, syntect::Error>,
{
    let target_nanos = u128::from(minimum_time_ms) * 1_000_000;
    let mut iterations = 1usize;
    loop {
        let started = Instant::now();
        let mut last = None;
        for _ in 0..iterations {
            let output = operation()?;
            black_box(output.len());
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
    extension: String,
    file: String,
    phase: String,
    minimum_time_ms: u64,
}

impl Args {
    fn parse() -> Result<Self, Box<dyn std::error::Error>> {
        let mut extension = None;
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
                "--extension" => extension = Some(value.clone()),
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
            extension: extension.ok_or("--extension is required")?,
            file: file.ok_or("--file is required")?,
            phase: phase.ok_or("--phase is required")?,
            minimum_time_ms,
        })
    }
}
