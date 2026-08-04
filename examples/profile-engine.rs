use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fmt::Write as _,
    fs,
    hint::black_box,
    path::PathBuf,
    time::Instant,
};

use syntaxmate::{GrammarId, GrammarRegistry, TokenizedDocument, Tokenizer, TokenizerOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse()?;
    let source = fs::read_to_string(&args.file)?;

    let setup_started = Instant::now();
    let (registry, root) = load_asset_closure(&args.assets, &args.scope)?;
    let options = if args.phase == "steady" {
        TokenizerOptions {
            line_cache_entries: 0,
            ..TokenizerOptions::default()
        }
    } else {
        TokenizerOptions::default()
    };
    let mut tokenizer = Tokenizer::new(&registry, root, options)?;
    let setup_nanos = nanos(setup_started.elapsed());

    let (iterations, elapsed_nanos, document) = match args.phase.as_str() {
        "first" => {
            let started = Instant::now();
            let document = tokenizer.tokenize(&source);
            (1, nanos(started.elapsed()), document)
        }
        "steady" | "replay" => {
            let warmup = tokenizer.tokenize(&source);
            if !warmup.status().is_complete() {
                return Err("warmup tokenization degraded".into());
            }
            calibrate(args.minimum_time_ms, || tokenizer.tokenize(&source))
        }
        phase => return Err(format!("unsupported phase {phase:?}").into()),
    };
    if !document.status().is_complete() {
        return Err("tokenization degraded".into());
    }
    let token_count = document
        .lines()
        .iter()
        .map(|line| line.spans().len())
        .sum::<usize>();

    println!(
        "{}",
        serde_json::json!({
            "schemaVersion": 1,
            "track": "engine",
            "engine": "syntaxmate",
            "version": env!("CARGO_PKG_VERSION"),
            "phase": args.phase,
            "iterations": iterations,
            "sourceBytes": source.len(),
            "processedBytes": source.len().saturating_mul(iterations),
            "setupNanoseconds": setup_nanos,
            "elapsedNanoseconds": elapsed_nanos,
            "tokens": token_count,
            "scopeDigest": scope_digest(&document),
            "complete": true,
        })
    );
    Ok(())
}

fn calibrate<F>(minimum_time_ms: u64, mut operation: F) -> (usize, u64, TokenizedDocument)
where
    F: FnMut() -> TokenizedDocument,
{
    let target_nanos = u128::from(minimum_time_ms) * 1_000_000;
    let mut iterations = 1usize;
    loop {
        let started = Instant::now();
        let mut last = None;
        for _ in 0..iterations {
            let document = operation();
            black_box(document.lines().len());
            last = Some(document);
        }
        let elapsed = started.elapsed();
        if elapsed.as_nanos() >= target_nanos || iterations >= 16_384 {
            return (
                iterations,
                nanos(elapsed),
                last.expect("positive iteration count"),
            );
        }
        iterations = iterations.saturating_mul(2);
    }
}

fn scope_digest(document: &TokenizedDocument) -> String {
    let mut canonical = String::new();
    for (line_index, line) in document.lines().iter().enumerate() {
        let mut coalesced: Vec<(std::ops::Range<usize>, Vec<String>)> = Vec::new();
        for span in line.spans() {
            let range = span.range();
            if range.start >= range.end {
                continue;
            }
            let scopes = line
                .scope_names(span.scope_stack())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            if let Some((previous_range, previous_scopes)) = coalesced.last_mut()
                && previous_range.end == range.start
                && *previous_scopes == scopes
            {
                previous_range.end = range.end;
            } else {
                coalesced.push((range, scopes));
            }
        }
        for (range, scopes) in coalesced {
            let _ = write!(canonical, "{line_index}:{}:{}:", range.start, range.end);
            for scope in scopes {
                canonical.push_str(&scope);
                canonical.push('\u{1f}');
            }
            canonical.push('\n');
        }
    }
    fnv1a(canonical.as_bytes())
}

fn fnv1a(bytes: &[u8]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn nanos(duration: std::time::Duration) -> u64 {
    u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX)
}

fn load_asset_closure(
    assets: &PathBuf,
    scope: &str,
) -> Result<(GrammarRegistry, GrammarId), Box<dyn std::error::Error>> {
    let mut sources = BTreeMap::new();
    let mut entries = fs::read_dir(assets)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let contents = fs::read_to_string(&path)?;
        let parsed: serde_json::Value = serde_json::from_str(&contents)?;
        if let Some(scope_name) = parsed.get("scopeName").and_then(|value| value.as_str()) {
            sources.insert(scope_name.to_owned(), (contents, parsed));
        }
    }

    let mut selected = BTreeSet::new();
    let mut pending = vec![scope.to_owned()];
    while let Some(requested) = pending.pop() {
        if !selected.insert(requested.clone()) {
            continue;
        }
        if let Some((_, grammar)) = sources.get(&requested) {
            collect_external_scopes(grammar, &sources, &mut pending);
        }
    }

    let mut registry = GrammarRegistry::new();
    let mut root = None;
    for requested in selected {
        let Some((contents, _)) = sources.get(&requested) else {
            continue;
        };
        let id = registry.add_json(contents)?;
        if requested == scope {
            root = Some(id);
        }
    }
    Ok((
        registry,
        root.ok_or_else(|| format!("scope {scope:?} not found"))?,
    ))
}

fn collect_external_scopes(
    value: &serde_json::Value,
    sources: &BTreeMap<String, (String, serde_json::Value)>,
    pending: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(include) = object.get("include").and_then(|value| value.as_str())
                && !include.starts_with('#')
                && !matches!(include, "$self" | "$base")
            {
                let scope = include.split('#').next().unwrap_or(include);
                if sources.contains_key(scope) {
                    pending.push(scope.to_owned());
                }
            }
            for child in object.values() {
                collect_external_scopes(child, sources, pending);
            }
        }
        serde_json::Value::Array(array) => {
            for child in array {
                collect_external_scopes(child, sources, pending);
            }
        }
        _ => {}
    }
}

struct Args {
    assets: PathBuf,
    scope: String,
    file: String,
    phase: String,
    minimum_time_ms: u64,
}

impl Args {
    fn parse() -> Result<Self, Box<dyn std::error::Error>> {
        let mut assets = None;
        let mut scope = None;
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
                "--assets" => assets = Some(PathBuf::from(value)),
                "--scope" => scope = Some(value.clone()),
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
            assets: assets.ok_or("--assets is required")?,
            scope: scope.ok_or("--scope is required")?,
            file: file.ok_or("--file is required")?,
            phase: phase.ok_or("--phase is required")?,
            minimum_time_ms,
        })
    }
}
