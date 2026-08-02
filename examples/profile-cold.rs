use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::PathBuf,
    time::Instant,
};

use syntaxmate::{Catalog, GrammarId, GrammarRegistry, Tokenizer, TokenizerOptions};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut assets = None;
    let mut scope = None;
    let mut mode = "process-cold".to_owned();
    let mut json = false;
    let mut positional = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--scope" => {
                scope = Some(
                    args.get(index + 1)
                        .ok_or("--scope requires a value")?
                        .clone(),
                );
                index += 2;
            }
            "--mode" => {
                mode = args
                    .get(index + 1)
                    .ok_or("--mode requires a value")?
                    .clone();
                index += 2;
            }
            "--json" => {
                json = true;
                index += 1;
            }
            "--assets" => {
                assets = Some(PathBuf::from(
                    args.get(index + 1).ok_or("--assets requires a value")?,
                ));
                index += 2;
            }
            option if option.starts_with("--") => {
                return Err(format!("unexpected option {option}").into());
            }
            value => {
                positional.push(value.to_owned());
                index += 1;
            }
        }
    }
    let fixture = positional.first().ok_or("missing source file")?;
    let iterations = positional
        .get(1)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    let scope = scope.ok_or("--scope is required")?;
    if !matches!(mode.as_str(), "process-cold" | "same-driver") {
        return Err(format!("unsupported mode {mode:?}").into());
    }
    let source = fs::read_to_string(fixture)?;

    // Catalog performance uses source assets so grammar discovery and file IO
    // remain outside the measured process-cold tokenizer construction, matching
    // the extraction baseline. Normal users take the bundled branch.
    let custom = assets
        .as_ref()
        .map(|assets| load_asset_closure(assets, &scope))
        .transpose()?;
    let bundled_language = if custom.is_none() {
        Some(
            Catalog::bundled()
                .language_for_scope(&scope)
                .ok_or_else(|| format!("unknown bundled scope {scope:?}"))?,
        )
    } else {
        None
    };

    let create_tokenizer = || -> Result<Tokenizer, syntaxmate::Error> {
        if let Some((registry, root)) = &custom {
            Tokenizer::new(registry, *root, TokenizerOptions::default())
        } else {
            Tokenizer::for_bundled_language(
                bundled_language
                    .as_deref()
                    .expect("bundled language selected"),
                TokenizerOptions::default(),
            )
        }
    };
    let mut same_driver = if mode == "same-driver" {
        Some(create_tokenizer()?)
    } else {
        None
    };

    let started = Instant::now();
    let mut token_count = 0usize;
    for _ in 0..iterations {
        let document = if let Some(tokenizer) = same_driver.as_mut() {
            tokenizer.tokenize(&source)
        } else {
            create_tokenizer()?.tokenize(&source)
        };
        token_count += document
            .lines()
            .iter()
            .map(|line| line.spans().len())
            .sum::<usize>();
    }
    let elapsed = started.elapsed();
    if json {
        println!(
            "{}",
            serde_json::json!({
                "schemaVersion": 1,
                "mode": mode,
                "iterations": iterations,
                "bytesPerIteration": source.len(),
                "processedBytes": source.len() * iterations,
                "elapsedNanoseconds": u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
                "tokens": token_count,
            })
        );
    } else {
        let megabytes_per_second =
            (source.len() * iterations) as f64 / elapsed.as_secs_f64() / 1_000_000.0;
        println!("{megabytes_per_second:.2} MB/s; {token_count} tokens");
    }
    Ok(())
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
    let root = root.ok_or_else(|| format!("scope {scope:?} not found"))?;
    Ok((registry, root))
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
