use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
};

#[path = "../src/grammars/catalog.rs"]
mod catalog;

const MAGIC: &[u8; 4] = b"MRKB";
const FORMAT_VERSION: u16 = 1;
const CODEC_NONE: u32 = 0;
const CODEC_DEFLATE_ZLIB: u32 = 1;
const SECTION_STRINGS: u32 = 3;
const SECTION_SCOPES: u32 = 4;
const SECTION_LANGUAGES: u32 = 5;
const SECTION_GRAMMAR_BLOBS: u32 = 6;
const SECTION_LICENSES: u32 = 7;
const HEADER_LEN: usize = 32;
const SECTION_ENTRY_LEN: usize = 24;
const NO_STRING: u32 = u32::MAX;

#[derive(Debug, Clone)]
struct GrammarAsset {
    language: String,
    path: String,
    scope_name: String,
    first_line_pattern: Option<String>,
    bytes: Vec<u8>,
    scopes: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct LanguageMetadataManifest {
    languages: Vec<LanguageMetadata>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct LanguageMetadata {
    id: String,
    #[serde(default)]
    asset: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    extensions: Vec<String>,
    #[serde(default)]
    basenames: Vec<String>,
}

#[derive(Debug, Clone)]
struct LanguageEntry {
    canonical: String,
    scope_name: String,
    aliases: Vec<String>,
    extensions: Vec<String>,
    basenames: Vec<String>,
    first_line_pattern: Option<String>,
    grammar_blob: u32,
    license: u32,
}

#[derive(Debug, Clone)]
struct GrammarBlob {
    language: String,
    scope_name: String,
    codec: u32,
    flags: u32,
    raw_len: u32,
    bytes: Vec<u8>,
    pattern_count: u32,
    dfa_count: u32,
    fallback_count: u32,
}

#[derive(Debug, Clone)]
struct LicenseEntry {
    language: String,
    source_path: String,
    upstream_url: String,
    spdx_id: String,
    license_text: String,
    source_revision: String,
}

#[derive(Debug, Clone)]
struct AssetLicense {
    source_path: String,
    upstream_url: String,
    spdx_id: String,
    license_text: String,
    source_revision: String,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let assets = manifest_dir.join("assets/grammars");
    let default_output = manifest_dir.join("assets/grammars.bundle");
    let mut output = default_output;
    let mut check = false;
    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--check" => check = true,
            "--out" => {
                output = PathBuf::from(args.next().ok_or("--out requires a path")?);
            }
            "--help" | "-h" => {
                println!(
                    "usage: cargo run --bin syntaxmate-bundle --features bundle-tools -- [--check] [--out PATH]"
                );
                return Ok(());
            }
            other => return Err(format!("unexpected argument {other:?}")),
        }
    }

    let mut hash_bytes = Vec::new();
    hash_bytes.extend_from_slice(&hash_inputs(&assets).to_le_bytes());
    hash_bytes.extend_from_slice(
        &fs::read(manifest_dir.join("tools/build-bundle.rs")).unwrap_or_default(),
    );
    hash_bytes.extend_from_slice(
        &fs::read(manifest_dir.join("src/grammars/catalog.rs")).unwrap_or_default(),
    );
    let input_hash = fnv1a64(&hash_bytes);
    let bytes = build_bundle(&assets, input_hash)?;
    let version = read_bundle_hash(&bytes).unwrap_or(0);

    if check {
        let existing =
            fs::read(&output).map_err(|error| format!("read {}: {error}", output.display()))?;
        if existing != bytes {
            return Err(format!(
                "{} is stale; regenerate it with syntaxmate-bundle",
                output.display()
            ));
        }
        println!("{} is current ({version:016x})", output.display());
    } else {
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&output, &bytes).map_err(|error| error.to_string())?;
        println!(
            "wrote {} bytes to {} ({version:016x})",
            bytes.len(),
            output.display()
        );
    }
    Ok(())
}

fn build_bundle(assets: &Path, input_hash: u64) -> Result<Vec<u8>, String> {
    let source_text = fs::read_to_string(assets.join("SOURCE.toml")).map_err(|e| e.to_string())?;
    let coverage_text =
        fs::read_to_string(assets.join("coverage.toml")).map_err(|e| e.to_string())?;
    let licenses_text =
        fs::read_to_string(assets.join("licenses.json")).map_err(|e| e.to_string())?;
    let metadata_text =
        fs::read_to_string(assets.join("language-metadata.json")).map_err(|e| e.to_string())?;
    let coverage = toml::from_str::<toml::Value>(&coverage_text).map_err(|e| e.to_string())?;
    let licenses =
        serde_json::from_str::<serde_json::Value>(&licenses_text).map_err(|e| e.to_string())?;
    let metadata = serde_json::from_str::<LanguageMetadataManifest>(&metadata_text)
        .map_err(|e| e.to_string())?;
    let metadata_by_language = metadata
        .languages
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect::<BTreeMap<_, _>>();
    let source = licenses.get("source").and_then(|v| v.as_object());
    let upstream_url = source
        .and_then(|s| s.get("repository"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned();
    let default_license = source
        .and_then(|s| s.get("license"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned();
    let source_revision = source
        .and_then(|s| s.get("version"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_owned();
    let mut license_by_language = BTreeMap::new();
    for asset in licenses
        .get("assets")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        let Some(language) = asset
            .get("language")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
        else {
            continue;
        };
        let license_text = asset
            .get("licenseTextPath")
            .and_then(|v| v.as_str())
            .filter(|path| !path.is_empty())
            .map(|path| {
                fs::read_to_string(assets.join(path))
                    .map_err(|error| format!("read license notice {path}: {error}"))
            })
            .transpose()?
            .unwrap_or_default();
        let asset_repository = asset
            .get("repository")
            .and_then(|v| v.as_str())
            .filter(|repository| !repository.is_empty());
        let asset_revision = asset
            .get("version")
            .and_then(|v| v.as_str())
            .filter(|version| !version.is_empty());
        if asset_repository.is_none()
            && asset_revision
                .map(|version| version != source_revision.as_str())
                .unwrap_or(false)
        {
            return Err(format!(
                "license asset {language} overrides the source version without a repository"
            ));
        }
        if asset_repository.is_some() && asset_revision.is_none() {
            return Err(format!(
                "license asset {language} overrides the source repository without a version"
            ));
        }
        license_by_language.insert(
            language,
            AssetLicense {
                source_path: asset
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned(),
                upstream_url: asset_repository.unwrap_or(upstream_url.as_str()).to_owned(),
                spdx_id: asset
                    .get("license")
                    .and_then(|v| v.as_str())
                    .filter(|license| !license.is_empty())
                    .unwrap_or(default_license.as_str())
                    .to_owned(),
                license_text,
                source_revision: asset_revision
                    .unwrap_or(source_revision.as_str())
                    .to_owned(),
            },
        );
    }

    let grammars = collect_grammars(assets)?;
    let grammar_by_language = grammars
        .iter()
        .enumerate()
        .map(|(index, grammar)| (grammar.language.as_str(), (index as u32, grammar)))
        .collect::<BTreeMap<_, _>>();

    let mut scopes = BTreeSet::new();
    for grammar in &grammars {
        scopes.insert(grammar.scope_name.clone());
        scopes.extend(grammar.scopes.iter().cloned());
    }

    let mut licenses_out = Vec::new();
    for grammar in &grammars {
        let license = license_by_language
            .get(&grammar.language)
            .cloned()
            .unwrap_or_else(|| AssetLicense {
                source_path: grammar.path.clone(),
                upstream_url: upstream_url.clone(),
                spdx_id: default_license.clone(),
                license_text: String::new(),
                source_revision: source_revision.clone(),
            });
        licenses_out.push(LicenseEntry {
            language: grammar.language.clone(),
            source_path: license.source_path,
            upstream_url: license.upstream_url,
            spdx_id: license.spdx_id,
            license_text: license.license_text,
            source_revision: license.source_revision,
        });
    }

    let grammar_blobs = grammars
        .iter()
        .map(|grammar| {
            let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&grammar.bytes, 6);
            let (codec, bytes) = if compressed.len() < grammar.bytes.len() {
                (CODEC_DEFLATE_ZLIB, compressed)
            } else {
                (CODEC_NONE, grammar.bytes.clone())
            };
            GrammarBlob {
                language: grammar.language.clone(),
                scope_name: grammar.scope_name.clone(),
                codec,
                flags: 0,
                raw_len: grammar.bytes.len() as u32,
                bytes,
                pattern_count: count_key(&grammar.bytes, b"\"match\"")
                    + count_key(&grammar.bytes, b"\"begin\"")
                    + count_key(&grammar.bytes, b"\"end\"")
                    + count_key(&grammar.bytes, b"\"while\""),
                dfa_count: 0,
                fallback_count: 0,
            }
        })
        .collect::<Vec<_>>();

    let mut language_entries = Vec::new();
    let mut seen = BTreeSet::new();
    for language in string_array(&coverage, "kept") {
        if grammar_by_language.contains_key(language.as_str()) {
            language_entries.push(language_entry(
                &language,
                &language,
                &grammar_by_language,
                &metadata_by_language,
                &mut seen,
            )?);
        }
    }
    if let Some(remapped) = coverage.get("remapped").and_then(|v| v.as_array()) {
        for remap in remapped {
            let language = remap
                .get("language")
                .and_then(|v| v.as_str())
                .ok_or("bad remapped language")?;
            let asset = remap
                .get("asset")
                .and_then(|v| v.as_str())
                .ok_or("bad remapped asset")?;
            // Only package remaps whose recovered asset is present on disk.
            if !grammar_by_language.contains_key(asset) {
                continue;
            }
            language_entries.push(language_entry(
                language,
                asset,
                &grammar_by_language,
                &metadata_by_language,
                &mut seen,
            )?);
        }
    }
    language_entries.sort_by(|l, r| l.canonical.cmp(&r.canonical));

    let mut source_hash_input = Vec::new();
    source_hash_input.extend_from_slice(source_text.as_bytes());
    source_hash_input.extend_from_slice(coverage_text.as_bytes());
    source_hash_input.extend_from_slice(licenses_text.as_bytes());
    source_hash_input.extend_from_slice(metadata_text.as_bytes());
    source_hash_input.extend_from_slice(&input_hash.to_le_bytes());

    Ok(bundle_to_bytes(
        fnv1a64(&source_hash_input),
        scopes.into_iter().collect(),
        language_entries,
        grammar_blobs,
        licenses_out,
    ))
}

fn collect_grammars(assets: &Path) -> Result<Vec<GrammarAsset>, String> {
    let languages_dir = assets.join("languages");
    let mut entries = fs::read_dir(&languages_dir)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort_by_key(|entry| entry.file_name());
    let mut grammars = Vec::new();
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let source_bytes = fs::read(&path).map_err(|e| e.to_string())?;
        let json = serde_json::from_slice::<serde_json::Value>(&source_bytes)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        // The runtime parses individual blobs lazily. Store canonical compact
        // JSON so whitespace in vendored, reviewable sources does not inflate
        // every Syntaxmate consumer.
        let bytes = serde_json::to_vec(&json).map_err(|e| e.to_string())?;
        let scope_name = json
            .get("scopeName")
            .and_then(|v| v.as_str())
            .ok_or_else(|| format!("{}: missing scopeName", path.display()))?
            .to_owned();
        let first_line_pattern = json
            .get("firstLineMatch")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        let language = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .trim_end_matches(".tmLanguage.json")
            .to_owned();
        let mut scopes = BTreeSet::new();
        collect_scope_names(&json, &mut scopes);
        grammars.push(GrammarAsset {
            language,
            path: normalize_path(&path),
            scope_name,
            first_line_pattern,
            bytes,
            scopes: scopes.into_iter().collect(),
        });
    }
    Ok(grammars)
}

fn language_entry(
    language: &str,
    asset: &str,
    grammar_by_language: &BTreeMap<&str, (u32, &GrammarAsset)>,
    metadata_by_language: &BTreeMap<&str, &LanguageMetadata>,
    seen: &mut BTreeSet<String>,
) -> Result<LanguageEntry, String> {
    if !seen.insert(language.to_owned()) {
        return Err(format!("duplicate language in coverage: {language}"));
    }
    let (grammar_blob, grammar) = grammar_by_language
        .get(asset)
        .ok_or_else(|| format!("coverage maps {language} to missing grammar asset {asset}"))?;
    let metadata = metadata_by_language
        .get(language)
        .ok_or_else(|| format!("language metadata is missing public id {language}"))?;
    if metadata.asset.as_deref().unwrap_or(language) != asset {
        return Err(format!(
            "language metadata maps {language} to the wrong asset"
        ));
    }
    let mut aliases = catalog::aliases_for_language(language)
        .into_iter()
        .filter(|alias| alias != language)
        .collect::<BTreeSet<_>>();
    aliases.extend(
        metadata
            .aliases
            .iter()
            .map(|alias| catalog::normalize_language_token(alias))
            .filter(|alias| !alias.is_empty() && alias != language),
    );
    if asset != language {
        aliases.insert(catalog::normalize_language_token(asset));
    }
    let mut extensions = catalog::extensions_for_language(language)
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>();
    let mut basenames = catalog::basenames_for_language(language)
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<BTreeSet<_>>();
    extensions.extend(
        metadata
            .extensions
            .iter()
            .filter(|extension| catalog::extension_is_allowed(extension, language))
            .cloned(),
    );
    basenames.extend(metadata.basenames.iter().cloned());
    if language == "tsx" {
        aliases.insert("typescriptreact".to_owned());
        extensions.insert("tsx".to_owned());
    }
    if language == "jsx" {
        aliases.insert("javascript-babel".to_owned());
        extensions.insert("jsx".to_owned());
    }
    if language == "c" {
        extensions.insert("h".to_owned());
    }
    if language == "cpp" {
        extensions.insert("hpp".to_owned());
        extensions.insert("hh".to_owned());
        extensions.insert("hxx".to_owned());
    }
    if language == "dockerfile" {
        basenames.insert("Dockerfile".to_owned());
        aliases.insert("docker".to_owned());
    }
    if language == "make" {
        basenames.insert("Makefile".to_owned());
        basenames.insert("GNUmakefile".to_owned());
        basenames.insert("BSDmakefile".to_owned());
    }
    if language == "bash" {
        aliases.insert("shellscript".to_owned());
        aliases.insert("shell".to_owned());
        aliases.insert("sh".to_owned());
        aliases.insert("zsh".to_owned());
    }
    Ok(LanguageEntry {
        canonical: language.to_owned(),
        scope_name: grammar.scope_name.clone(),
        aliases: aliases.into_iter().collect(),
        extensions: extensions.into_iter().collect(),
        basenames: basenames.into_iter().collect(),
        first_line_pattern: grammar.first_line_pattern.clone(),
        grammar_blob: *grammar_blob,
        license: *grammar_blob,
    })
}

fn collect_scope_names(value: &serde_json::Value, out: &mut BTreeSet<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if (key == "name" || key == "contentName")
                    && let Some(scope) = value.as_str()
                {
                    for part in scope.split_whitespace() {
                        if part.contains('.') {
                            out.insert(part.to_owned());
                        }
                    }
                }
                collect_scope_names(value, out);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_scope_names(value, out);
            }
        }
        _ => {}
    }
}

fn string_array(value: &toml::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn bundle_to_bytes(
    source_hash: u64,
    scopes: Vec<String>,
    languages: Vec<LanguageEntry>,
    grammar_blobs: Vec<GrammarBlob>,
    licenses: Vec<LicenseEntry>,
) -> Vec<u8> {
    let strings = interned_strings(&scopes, &languages, &grammar_blobs, &licenses);
    let sections = vec![
        (SECTION_STRINGS, encode_string_table(&strings)),
        (SECTION_SCOPES, encode_scope_table(&scopes, &strings)),
        (
            SECTION_LANGUAGES,
            encode_language_table(&languages, &strings),
        ),
        (
            SECTION_GRAMMAR_BLOBS,
            encode_grammar_blobs(&grammar_blobs, &strings),
        ),
        (SECTION_LICENSES, encode_license_table(&licenses, &strings)),
    ];
    let bundle_hash = hash_sections(&sections);
    write_container(source_hash, bundle_hash, sections)
}

fn interned_strings(
    scopes: &[String],
    languages: &[LanguageEntry],
    grammar_blobs: &[GrammarBlob],
    licenses: &[LicenseEntry],
) -> Vec<String> {
    let mut strings = BTreeMap::<String, ()>::new();
    for scope in scopes {
        strings.insert(scope.clone(), ());
    }
    for language in languages {
        strings.insert(language.canonical.clone(), ());
        strings.insert(language.scope_name.clone(), ());
        if let Some(first_line) = &language.first_line_pattern {
            strings.insert(first_line.clone(), ());
        }
        for value in language
            .aliases
            .iter()
            .chain(language.extensions.iter())
            .chain(language.basenames.iter())
        {
            strings.insert(value.clone(), ());
        }
    }
    for blob in grammar_blobs {
        strings.insert(blob.language.clone(), ());
        strings.insert(blob.scope_name.clone(), ());
    }
    for license in licenses {
        strings.insert(license.language.clone(), ());
        strings.insert(license.source_path.clone(), ());
        strings.insert(license.upstream_url.clone(), ());
        strings.insert(license.spdx_id.clone(), ());
        strings.insert(license.license_text.clone(), ());
        strings.insert(license.source_revision.clone(), ());
    }
    strings.into_keys().collect()
}

fn encode_string_table(strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut payload = Vec::new();
    let mut offsets = Vec::with_capacity(strings.len());
    for string in strings {
        offsets.push(payload.len() as u32);
        payload.extend_from_slice(string.as_bytes());
    }
    write_u32(&mut bytes, strings.len() as u32);
    write_u32(&mut bytes, payload.len() as u32);
    for offset in offsets {
        write_u32(&mut bytes, offset);
    }
    bytes.extend_from_slice(&payload);
    bytes
}

fn encode_scope_table(scopes: &[String], strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, scopes.len() as u32);
    for scope in scopes {
        write_u32(&mut bytes, string_id(strings, scope));
    }
    bytes
}

fn encode_language_table(languages: &[LanguageEntry], strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, languages.len() as u32);
    for language in languages {
        write_u32(&mut bytes, string_id(strings, &language.canonical));
        write_u32(&mut bytes, string_id(strings, &language.scope_name));
        write_u32(&mut bytes, language.grammar_blob);
        write_u32(&mut bytes, language.license);
        write_u32(
            &mut bytes,
            optional_string_id(strings, language.first_line_pattern.as_deref()),
        );
        write_string_id_vec(&mut bytes, strings, &language.aliases);
        write_string_id_vec(&mut bytes, strings, &language.extensions);
        write_string_id_vec(&mut bytes, strings, &language.basenames);
    }
    bytes
}

fn encode_grammar_blobs(blobs: &[GrammarBlob], strings: &[String]) -> Vec<u8> {
    let record_len = 48usize;
    let payload_start = 4 + blobs.len() * record_len;
    let mut records = Vec::new();
    let mut payload = Vec::new();
    write_u32(&mut records, blobs.len() as u32);
    for blob in blobs {
        write_u32(&mut records, string_id(strings, &blob.language));
        write_u32(&mut records, string_id(strings, &blob.scope_name));
        write_u32(&mut records, blob.codec);
        write_u32(&mut records, blob.flags);
        write_u32(&mut records, blob.raw_len);
        write_u64(&mut records, (payload_start + payload.len()) as u64);
        write_u64(&mut records, blob.bytes.len() as u64);
        write_u32(&mut records, blob.pattern_count);
        write_u32(&mut records, blob.dfa_count);
        write_u32(&mut records, blob.fallback_count);
        payload.extend_from_slice(&blob.bytes);
    }
    records.extend_from_slice(&payload);
    records
}

fn encode_license_table(licenses: &[LicenseEntry], strings: &[String]) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_u32(&mut bytes, licenses.len() as u32);
    for license in licenses {
        write_u32(&mut bytes, string_id(strings, &license.language));
        write_u32(&mut bytes, string_id(strings, &license.source_path));
        write_u32(&mut bytes, string_id(strings, &license.upstream_url));
        write_u32(&mut bytes, string_id(strings, &license.spdx_id));
        write_u32(&mut bytes, string_id(strings, &license.license_text));
        write_u32(&mut bytes, string_id(strings, &license.source_revision));
    }
    bytes
}

fn write_string_id_vec(out: &mut Vec<u8>, strings: &[String], values: &[String]) {
    write_u32(out, values.len() as u32);
    for value in values {
        write_u32(out, string_id(strings, value));
    }
}

fn string_id(strings: &[String], value: &str) -> u32 {
    strings
        .binary_search_by(|s| s.as_str().cmp(value))
        .expect("interned string") as u32
}

fn optional_string_id(strings: &[String], value: Option<&str>) -> u32 {
    value.map_or(NO_STRING, |value| string_id(strings, value))
}

fn write_container(source_hash: u64, bundle_hash: u64, sections: Vec<(u32, Vec<u8>)>) -> Vec<u8> {
    let table_len = sections.len() * SECTION_ENTRY_LEN;
    let mut offset = align_to(HEADER_LEN + table_len, 8);
    let mut entries = Vec::with_capacity(sections.len());
    for (id, bytes) in &sections {
        entries.push((*id, offset as u64, bytes.len() as u64));
        offset = align_to(offset + bytes.len(), 8);
    }
    let mut out = Vec::with_capacity(offset);
    out.extend_from_slice(MAGIC);
    write_u16(&mut out, FORMAT_VERSION);
    write_u16(&mut out, sections.len() as u16);
    write_u64(&mut out, source_hash);
    write_u64(&mut out, bundle_hash);
    write_u64(&mut out, 0);
    for (id, offset, len) in &entries {
        write_u32(&mut out, *id);
        write_u32(&mut out, 0);
        write_u64(&mut out, *offset);
        write_u64(&mut out, *len);
    }
    for ((_, bytes), (_, offset, _)) in sections.into_iter().zip(entries) {
        while out.len() < offset as usize {
            out.push(0);
        }
        out.extend_from_slice(&bytes);
    }
    out
}

fn hash_sections(sections: &[(u32, Vec<u8>)]) -> u64 {
    let mut bytes = Vec::new();
    for (id, section) in sections {
        write_u32(&mut bytes, *id);
        write_u64(&mut bytes, section.len() as u64);
        bytes.extend_from_slice(section);
    }
    fnv1a64(&bytes)
}

fn hash_inputs(path: &Path) -> u64 {
    let mut inputs = Vec::new();
    collect_files(path, &mut inputs);
    let mut bytes = Vec::new();
    for file in inputs {
        let relative = file.strip_prefix(path).unwrap_or(file.as_path());
        bytes.extend_from_slice(normalize_path(relative).as_bytes());
        if let Ok(contents) = fs::read(&file) {
            bytes.extend_from_slice(&contents);
        }
    }
    fnv1a64(&bytes)
}

fn collect_files(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_file() {
        out.push(path.to_path_buf());
    } else if let Ok(entries) = fs::read_dir(path) {
        let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_files(&entry.path(), out);
        }
    }
}

fn read_bundle_hash(bytes: &[u8]) -> Option<u64> {
    Some(u64::from_le_bytes(bytes.get(16..24)?.try_into().ok()?))
}

fn count_key(bytes: &[u8], key: &[u8]) -> u32 {
    bytes
        .windows(key.len())
        .filter(|window| *window == key)
        .count() as u32
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn align_to(value: usize, alignment: usize) -> usize {
    value.div_ceil(alignment) * alignment
}
fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn write_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
