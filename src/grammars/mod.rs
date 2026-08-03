//! Grammar asset registry and embedded bundle access.
//!
//! The embedded bundle is the authoritative grammar catalog: production
//! highlighting, language metadata, path detection, and bundle diagnostics
//! all resolve through it.

use std::{borrow::Cow, collections::HashMap, path::Path, sync::OnceLock};

pub mod bundle;
pub mod catalog;
#[cfg(test)]
pub(crate) mod registry;

use bundle::{Bundle, BundleError, BundleGrammarRegistry, LicenseEntry};

static EMBEDDED_BUNDLE: OnceLock<Bundle> = OnceLock::new();
static CATALOG_INDEX: OnceLock<CatalogIndex> = OnceLock::new();

struct CatalogIndex {
    canonical: HashMap<String, &'static str>,
    basenames: HashMap<String, &'static str>,
    extensions: HashMap<String, &'static str>,
}
#[cfg(feature = "bundled-grammars")]
static EMBEDDED_BUNDLE_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/assets/grammars.bundle"
));
#[cfg(not(feature = "bundled-grammars"))]
static EMBEDDED_BUNDLE_BYTES: &[u8] = &[];

pub fn embedded_bundle() -> &'static Bundle {
    EMBEDDED_BUNDLE.get_or_init(|| {
        if cfg!(feature = "bundled-grammars") {
            Bundle::parse(embedded_bundle_bytes())
                .expect("embedded Syntaxmate grammar bundle should parse")
        } else {
            Bundle::default()
        }
    })
}

pub fn embedded_bundle_bytes() -> &'static [u8] {
    EMBEDDED_BUNDLE_BYTES
}

pub fn embedded_bundle_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();
    VERSION
        .get_or_init(|| {
            if cfg!(feature = "bundled-grammars") {
                embedded_bundle().version_stamp()
            } else {
                "disabled".to_owned()
            }
        })
        .as_str()
}

pub fn bundle_summary() -> BundleSummary {
    BundleSummary::from_bundle(embedded_bundle())
}

pub fn bundled_licenses() -> &'static [LicenseEntry] {
    &embedded_bundle().licenses
}

pub fn available_languages() -> Vec<String> {
    embedded_bundle()
        .available_languages()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

pub fn canonical_language(language: &str) -> Option<String> {
    let token = normalized_catalog_token(language);
    catalog_index()
        .canonical
        .get(token.as_ref())
        .map(|language| (*language).to_owned())
}

pub fn has_language(language: &str) -> bool {
    let token = normalized_catalog_token(language);
    catalog_index().canonical.contains_key(token.as_ref())
}

pub fn detect_language_from_path(path: &str) -> Option<String> {
    let name = Path::new(path).file_name()?.to_str()?;
    let lower_name = name.to_ascii_lowercase();
    let index = catalog_index();
    if let Some(language) = index.basenames.get(lower_name.as_str()) {
        return Some((*language).to_owned());
    }

    // Compound TextMate file types are suffixes (`blade.php`, `html.erb`,
    // ...). Trying suffixes at each dot from left to right naturally chooses
    // the longest match without scanning every language in the catalog.
    for (dot, _) in lower_name.match_indices('.') {
        let suffix = &lower_name[dot + 1..];
        if let Some(language) = index.extensions.get(suffix) {
            return Some((*language).to_owned());
        }
    }
    None
}

fn catalog_index() -> &'static CatalogIndex {
    CATALOG_INDEX.get_or_init(|| {
        let bundle = embedded_bundle();
        let mut canonical = HashMap::new();
        let mut basenames = HashMap::new();
        let mut extensions = HashMap::new();

        // Canonical IDs take precedence over an identical alias, matching the
        // bundle's public lookup contract.
        for language in &bundle.languages {
            canonical.insert(language.canonical.clone(), language.canonical.as_str());
        }
        for language in &bundle.languages {
            for alias in &language.aliases {
                canonical
                    .entry(alias.clone())
                    .or_insert(language.canonical.as_str());
            }
            for basename in &language.basenames {
                basenames.insert(basename.to_ascii_lowercase(), language.canonical.as_str());
                if basename.contains('.') {
                    extensions.insert(
                        basename.trim_start_matches('.').to_ascii_lowercase(),
                        language.canonical.as_str(),
                    );
                }
            }
            for extension in &language.extensions {
                extensions.insert(
                    extension.trim_start_matches('.').to_ascii_lowercase(),
                    language.canonical.as_str(),
                );
            }
        }
        CatalogIndex {
            canonical,
            basenames,
            extensions,
        }
    })
}

fn normalized_catalog_token(language: &str) -> Cow<'_, str> {
    let token = language.trim().trim_start_matches('.');
    if token.bytes().all(|byte| !byte.is_ascii_uppercase()) {
        Cow::Borrowed(token)
    } else {
        Cow::Owned(token.to_ascii_lowercase())
    }
}

pub fn grammar_registry() -> BundleGrammarRegistry {
    BundleGrammarRegistry::new(embedded_bundle().clone())
}

pub fn parse_embedded_bundle() -> Result<Bundle, BundleError> {
    Bundle::parse(embedded_bundle_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleSummary {
    pub version: String,
    pub source_hash: u64,
    pub grammar_count: usize,
    pub language_count: usize,
    pub scope_count: usize,
    pub license_count: usize,
    pub source_revision: Option<String>,
}

impl BundleSummary {
    pub fn from_bundle(bundle: &Bundle) -> Self {
        let source_revision = bundle.licenses.iter().find_map(|license| {
            (!license.source_revision.is_empty()).then(|| license.source_revision.clone())
        });
        Self {
            version: bundle.version_stamp(),
            source_hash: bundle.source_hash,
            grammar_count: bundle.grammar_blobs.len(),
            language_count: bundle.languages.len(),
            scope_count: bundle.scopes.len(),
            license_count: bundle.licenses.len(),
            source_revision,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        state::GrammarId,
        tokenizer::{GrammarSet, TextMateTokenizer},
    };
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn embedded_bundle_parses_and_exposes_catalog() {
        let bundle = embedded_bundle();
        // Full public catalog plus private dependency blobs. `coverage.toml`
        // decides which embedded blobs are public catalog entries.
        assert_eq!(bundle.languages.len(), 264);
        assert_eq!(bundle.grammar_blobs.len(), 268);
        assert!(
            bundle
                .grammar_blob_for_scope("source.cpp.embedded.macro")
                .is_some()
        );
        assert!(bundle.grammar_blob_for_scope("source.yang").is_some());
        assert!(bundle.grammar_blob_for_scope("source.twig").is_some());
        assert!(bundle.grammar_blob_for_scope("source.yaml.1.2").is_some());
        assert!(
            bundle
                .grammar_blob_for_scope("source.yaml.embedded")
                .is_some()
        );
        assert!(bundle.has_language("cpp-macro"));
        assert_eq!(bundle.canonical_language("rs"), Some("rust"));
        assert_eq!(
            bundle.canonical_language("shellscript"),
            Some("shellscript")
        );
        assert_eq!(bundle.canonical_language("bash"), Some("shellscript"));
        assert_eq!(bundle.canonical_language("sh"), Some("shellscript"));
        assert_eq!(bundle.canonical_language("git-ignore"), Some("ignore"));
        assert_eq!(
            bundle.detect_language_from_path("script.sh"),
            Some("shellscript")
        );
        assert_eq!(
            bundle.detect_language_from_path("httpd.conf"),
            Some("apache")
        );
        assert_eq!(
            bundle.detect_language_from_path("module.v"),
            Some("verilog")
        );
        assert_eq!(
            bundle.detect_language_from_path("generated.js"),
            Some("javascript")
        );
        // Extension precedence must not hide an explicitly selected language.
        assert_eq!(bundle.canonical_language("bird2"), Some("bird2"));
        // Aliases shipped by Shiki's grammar metadata are part of the bundle,
        // not a second hand-maintained catalog. `properties` is declared by
        // the INI grammar and has historically been easy to omit.
        assert_eq!(bundle.canonical_language("properties"), Some("ini"));
        assert_eq!(bundle.canonical_language("docker"), Some("docker"));
        assert_eq!(bundle.detect_language_from_path("src/lib.rs"), Some("rust"));
        assert_eq!(
            bundle.detect_language_from_path("component.tsx"),
            Some("tsx")
        );
        assert_eq!(bundle.detect_language_from_path("include/foo.h"), Some("c"));
        assert_eq!(
            bundle.detect_language_from_path("Dockerfile"),
            Some("docker")
        );
        assert_eq!(bundle.detect_language_from_path("Makefile"), Some("make"));
        assert_eq!(bundle.detect_language_from_path("BUILD"), Some("starlark"));
        assert_eq!(bundle.detect_language_from_path(".bazelrc"), None);
        assert_eq!(
            bundle.detect_language_from_path("kernels/scale.cu"),
            Some("cuda")
        );
        assert_eq!(
            bundle.detect_language_from_path("shaders/module.spvasm"),
            Some("spirv")
        );
        assert_eq!(
            bundle.detect_language_from_path("kernels/scale.cl"),
            Some("opencl")
        );
        assert_eq!(
            bundle.detect_language_from_path("shaders/main.metal"),
            Some("metal")
        );
        assert_eq!(
            bundle.detect_language_from_path("policy/authz.rego"),
            Some("rego")
        );
        assert_eq!(
            bundle.detect_language_from_path("interfaces/catalog.webidl"),
            Some("webidl")
        );
        assert_eq!(bundle.canonical_language("asc"), Some("assemblyscript"));
        assert_eq!(
            bundle.detect_language_from_path(".gitignore"),
            Some("ignore")
        );
        assert_eq!(
            bundle.detect_language_from_path("models/catalog.yang"),
            Some("yang")
        );
        assert_eq!(
            bundle.detect_language_from_path("main.tf"),
            Some("terraform")
        );
        // Compound fileTypes are suffixes, not only literal basenames, and
        // outrank the shorter plain-extension owner.
        assert_eq!(
            bundle.detect_language_from_path("resources/views/index.blade.php"),
            Some("blade")
        );
        assert_eq!(
            bundle.detect_language_from_path("app/views/show.html.erb"),
            Some("erb")
        );
        assert_eq!(
            bundle.detect_language_from_path("widget.html.haml"),
            Some("haml")
        );
        assert_eq!(
            bundle.detect_language_from_path("notes.adoc.txt"),
            Some("asciidoc")
        );
        assert_eq!(bundle.detect_language_from_path("blade.php"), Some("blade"));
        assert_eq!(bundle.detect_language_from_path("index.php"), Some("php"));
        assert_eq!(bundle.licenses.len(), bundle.grammar_blobs.len());
    }

    #[test]
    fn embedded_bundle_round_trips_deterministically() {
        let parsed = parse_embedded_bundle().unwrap();
        let reparsed = Bundle::parse(&parsed.to_bytes()).unwrap();
        assert_eq!(parsed.to_bytes(), reparsed.to_bytes());
    }

    #[test]
    fn embedded_bundle_preserves_external_license_provenance() {
        let expected = [
            (
                "assemblyscript",
                "https://github.com/AssemblyScript/assemblyscript",
                "0.28.19",
                "ASSEMBLYSCRIPT NOTICE",
            ),
            (
                "cuda",
                "https://github.com/kriegalex/vscode-cuda",
                "0.1.1",
                "Copyright (c) 2017 Marco L.",
            ),
            (
                "metal",
                "https://github.com/computer-graphics-tools/metal-analyzer",
                "0.1.22",
                "Copyright (c) 2026 Computer Graphics Tools",
            ),
            (
                "opencl",
                "https://github.com/Galarius/vscode-opencl",
                "0.10.0",
                "Copyright (c) 2017 Ilya Shoshin",
            ),
            (
                "rego",
                "https://github.com/open-policy-agent/vscode-opa",
                "0.23.0",
                "Apache License",
            ),
            (
                "spirv",
                "https://github.com/KhronosGroup/SPIRV-Tools",
                "0.0.1",
                "Apache License",
            ),
            (
                "starlark",
                "https://github.com/bazel-contrib/vscode-bazel",
                "0.14.0",
                "Copyright (c) 2015 MagicStack Inc.",
            ),
            (
                "webidl",
                "https://github.com/nberlette/vscode-webidl",
                "0.2.0",
                "Copyright (c) 2026 Nicholas Berlette.",
            ),
        ];
        let licenses = bundled_licenses();
        for (language, repository, revision, notice) in expected {
            let license = licenses
                .iter()
                .find(|license| license.language == language)
                .unwrap_or_else(|| panic!("missing license metadata for {language}"));
            assert_eq!(license.upstream_url, repository, "{language}");
            assert_eq!(license.source_revision, revision, "{language}");
            assert!(license.license_text.contains(notice), "{language}");
        }

        let existing_external = [
            ("dart", "https://github.com/microsoft/vscode", "1.128.0"),
            (
                "handlebars",
                "https://github.com/microsoft/vscode",
                "1.128.0",
            ),
            ("ignore", "https://github.com/microsoft/vscode", "1.128.0"),
            (
                "js-regexp",
                "https://github.com/microsoft/vscode",
                "1.128.0",
            ),
            ("php", "https://github.com/microsoft/vscode", "1.128.0"),
            ("pug", "https://github.com/microsoft/vscode", "1.128.0"),
            ("r", "https://github.com/microsoft/vscode", "1.128.0"),
            ("rst", "https://github.com/microsoft/vscode", "1.128.0"),
            ("yaml", "https://github.com/microsoft/vscode", "1.128.0"),
            ("yaml-1.2", "https://github.com/microsoft/vscode", "1.128.0"),
            (
                "yaml-embedded",
                "https://github.com/microsoft/vscode",
                "1.128.0",
            ),
            (
                "yang",
                "https://github.com/marko2276/yang-vscode-syntax",
                "0.1.3",
            ),
            (
                "mlir",
                "https://github.com/llvm/llvm-project",
                "llvmorg-18.1.8",
            ),
        ];
        for (language, repository, revision) in existing_external {
            let license = licenses
                .iter()
                .find(|license| license.language == language)
                .unwrap_or_else(|| panic!("missing license metadata for {language}"));
            assert_eq!(license.upstream_url, repository, "{language}");
            assert_eq!(license.source_revision, revision, "{language}");
        }
    }

    #[test]
    fn pinned_language_metadata_matches_every_public_catalog_entry() {
        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Manifest {
            schema_version: u32,
            languages: Vec<Metadata>,
        }
        #[derive(serde::Deserialize)]
        struct Metadata {
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

        let manifest: Manifest =
            serde_json::from_str(include_str!("../../assets/grammars/language-metadata.json"))
                .unwrap();
        let bundle = embedded_bundle();
        assert_eq!(manifest.schema_version, 1);
        assert_eq!(manifest.languages.len(), 264);
        assert_eq!(bundle.languages.len(), 264);

        let metadata_by_id = manifest
            .languages
            .iter()
            .map(|entry| (entry.id.as_str(), entry))
            .collect::<BTreeMap<_, _>>();
        let public_ids = bundle
            .languages
            .iter()
            .map(|entry| entry.canonical.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            metadata_by_id.keys().copied().collect::<BTreeSet<_>>(),
            public_ids
        );
        assert!(public_ids.contains("bird2"));

        let mut alias_owners = BTreeMap::<String, BTreeSet<&str>>::new();
        let mut extension_owners = BTreeMap::<String, BTreeSet<&str>>::new();
        let mut basename_owners = BTreeMap::<String, BTreeSet<&str>>::new();
        let mut raw_extension_owners = BTreeMap::<String, BTreeSet<&str>>::new();

        for language in &bundle.languages {
            let metadata = metadata_by_id[language.canonical.as_str()];
            assert_eq!(
                metadata.asset.as_deref().unwrap_or(&metadata.id),
                bundle.grammar_blobs[language.grammar_blob as usize].language
            );

            raw_extension_owners
                .entry(catalog::normalize_language_token(&language.canonical))
                .or_default()
                .insert(&language.canonical);
            for (extension, target) in catalog::LANGUAGE_ALIASES {
                if *target == language.canonical {
                    raw_extension_owners
                        .entry(catalog::normalize_language_token(extension))
                        .or_default()
                        .insert(&language.canonical);
                }
            }
            for (extension, target) in catalog::EXTENSION_ALIASES {
                if *target == language.canonical {
                    raw_extension_owners
                        .entry(catalog::normalize_language_token(extension))
                        .or_default()
                        .insert(&language.canonical);
                }
            }

            let mut expected_aliases = catalog::aliases_for_language(&language.canonical)
                .into_iter()
                .filter(|alias| alias != &language.canonical)
                .collect::<BTreeSet<_>>();
            expected_aliases.extend(metadata.aliases.iter().cloned());
            if metadata
                .asset
                .as_deref()
                .is_some_and(|asset| asset != metadata.id)
            {
                expected_aliases.insert(metadata.asset.clone().unwrap());
            }
            if language.canonical == "tsx" {
                expected_aliases.insert("typescriptreact".to_owned());
            }
            if language.canonical == "jsx" {
                expected_aliases.insert("javascript-babel".to_owned());
            }

            let mut expected_extensions = catalog::extensions_for_language(&language.canonical)
                .into_iter()
                .collect::<BTreeSet<_>>();
            for extension in &metadata.extensions {
                raw_extension_owners
                    .entry(extension.clone())
                    .or_default()
                    .insert(&language.canonical);
                if catalog::extension_is_allowed(extension, &language.canonical) {
                    expected_extensions.insert(extension.clone());
                } else {
                    assert!(
                        catalog::extension_precedence(extension).is_some()
                            || catalog::extension_override(extension).is_some(),
                        "{} silently drops metadata extension {extension}",
                        language.canonical
                    );
                }
            }
            if language.canonical == "tsx" {
                expected_extensions.insert("tsx".to_owned());
                raw_extension_owners
                    .entry("tsx".to_owned())
                    .or_default()
                    .insert(&language.canonical);
            }
            if language.canonical == "jsx" {
                expected_extensions.insert("jsx".to_owned());
                raw_extension_owners
                    .entry("jsx".to_owned())
                    .or_default()
                    .insert(&language.canonical);
            }
            if language.canonical == "c" {
                expected_extensions.insert("h".to_owned());
                raw_extension_owners
                    .entry("h".to_owned())
                    .or_default()
                    .insert(&language.canonical);
            }
            if language.canonical == "cpp" {
                expected_extensions.extend(["hh", "hpp", "hxx"].map(str::to_owned));
                for extension in ["hh", "hpp", "hxx"] {
                    raw_extension_owners
                        .entry(extension.to_owned())
                        .or_default()
                        .insert(&language.canonical);
                }
            }

            let mut expected_basenames = catalog::basenames_for_language(&language.canonical)
                .into_iter()
                .collect::<BTreeSet<_>>();
            expected_basenames.extend(metadata.basenames.iter().cloned());
            if language.canonical == "make" {
                expected_basenames
                    .extend(["Makefile", "GNUmakefile", "BSDmakefile"].map(str::to_owned));
            }

            assert_eq!(
                language.aliases.iter().cloned().collect::<BTreeSet<_>>(),
                expected_aliases,
                "alias metadata drift for {}",
                language.canonical
            );
            assert_eq!(
                language.extensions.iter().cloned().collect::<BTreeSet<_>>(),
                expected_extensions,
                "extension metadata drift for {}",
                language.canonical
            );
            assert_eq!(
                language.basenames.iter().cloned().collect::<BTreeSet<_>>(),
                expected_basenames,
                "basename metadata drift for {}",
                language.canonical
            );

            for alias in &language.aliases {
                alias_owners
                    .entry(alias.clone())
                    .or_default()
                    .insert(&language.canonical);
            }
            for extension in &language.extensions {
                extension_owners
                    .entry(extension.clone())
                    .or_default()
                    .insert(&language.canonical);
            }
            for basename in &language.basenames {
                basename_owners
                    .entry(basename.to_ascii_lowercase())
                    .or_default()
                    .insert(&language.canonical);
            }
        }

        for (extension, owners) in &raw_extension_owners {
            if owners.len() > 1 {
                assert!(
                    catalog::extension_precedence(extension).is_some()
                        || catalog::extension_override(extension).is_some(),
                    "unresolved metadata extension collision {extension}: {owners:?}"
                );
            }
        }
        assert!(
            extension_owners.values().all(|owners| owners.len() == 1),
            "final extension catalog still has collisions: {:?}",
            extension_owners
                .iter()
                .filter(|(_, owners)| owners.len() > 1)
                .collect::<Vec<_>>()
        );
        assert!(basename_owners.values().all(|owners| owners.len() == 1));
        assert!(alias_owners.values().all(|owners| owners.len() == 1));

        for (extension, winner) in catalog::EXTENSION_PRECEDENCE {
            let raw_owners = raw_extension_owners
                .get(*extension)
                .unwrap_or_else(|| panic!("stale extension precedence for {extension}"));
            assert!(
                raw_owners.len() > 1,
                "stale extension precedence for {extension}: {raw_owners:?}"
            );
            if winner.is_empty() {
                assert!(
                    !extension_owners.contains_key(*extension),
                    "suppressed extension {extension} still has an owner"
                );
            } else {
                assert!(
                    raw_owners.contains(winner),
                    "extension precedence winner {winner} does not advertise {extension}"
                );
                assert_eq!(
                    extension_owners.get(*extension),
                    Some(&BTreeSet::from([*winner])),
                    "extension precedence for {extension}"
                );
            }
        }

        for (alias, owners) in alias_owners {
            let owner = *owners.first().unwrap();
            let expected = public_ids.get(alias.as_str()).copied().unwrap_or(owner);
            assert_eq!(
                bundle.canonical_language(&alias),
                Some(expected),
                "alias {alias}"
            );
        }
        for (extension, owners) in extension_owners {
            if extension.contains('/') {
                // TextMate can publish path-qualified fileTypes. The current
                // path API receives a basename and still retains these values
                // exactly in catalog metadata for future path-aware matching.
                continue;
            }
            assert_eq!(
                bundle.detect_language_from_path(&format!("fixture.{extension}")),
                owners.first().copied(),
                "extension {extension}"
            );
        }
        for (basename, owners) in basename_owners {
            assert_eq!(
                bundle.detect_language_from_path(&basename),
                owners.first().copied(),
                "basename {basename}"
            );
        }
    }

    #[test]
    fn indexed_catalog_lookup_matches_bundle_contract() {
        let bundle = embedded_bundle();
        for language in &bundle.languages {
            assert_eq!(
                canonical_language(&language.canonical).as_deref(),
                bundle.canonical_language(&language.canonical)
            );
            for alias in &language.aliases {
                assert_eq!(
                    canonical_language(alias).as_deref(),
                    bundle.canonical_language(alias),
                    "alias {alias}"
                );
            }
            for extension in &language.extensions {
                if extension.contains('/') {
                    continue;
                }
                let path = format!("fixture.{extension}");
                assert_eq!(
                    detect_language_from_path(&path).as_deref(),
                    bundle.detect_language_from_path(&path),
                    "extension {extension}"
                );
            }
            for basename in &language.basenames {
                assert_eq!(
                    detect_language_from_path(basename).as_deref(),
                    bundle.detect_language_from_path(basename),
                    "basename {basename}"
                );
                if basename.contains('.') {
                    let path = format!("prefix.{basename}");
                    assert_eq!(
                        detect_language_from_path(&path).as_deref(),
                        bundle.detect_language_from_path(&path),
                        "compound basename {basename}"
                    );
                }
            }
        }
    }

    #[test]
    fn curated_extension_precedence_is_in_the_generated_catalog() {
        let bundle = embedded_bundle();
        let owners = |extension: &str| {
            bundle
                .languages
                .iter()
                .filter(|entry| entry.extensions.iter().any(|item| item == extension))
                .map(|entry| entry.canonical.as_str())
                .collect::<Vec<_>>()
        };

        assert_eq!(owners("conf"), vec!["apache"]);
        assert_eq!(owners("v"), vec!["verilog"]);
        assert_eq!(owners("js"), vec!["javascript"]);
        assert_eq!(
            bundle.detect_language_from_path("router.conf"),
            Some("apache")
        );
        assert_eq!(
            bundle.detect_language_from_path("module.v"),
            Some("verilog")
        );
        assert_eq!(
            bundle.detect_language_from_path("generated.js"),
            Some("javascript")
        );
        assert_eq!(bundle.canonical_language("bird2"), Some("bird2"));
    }

    #[test]
    fn bundle_registry_decodes_lazily() {
        let mut registry = grammar_registry();
        assert_eq!(registry.cached_grammar_count(), 0);
        let grammar = registry.grammar("json").unwrap();
        assert_eq!(grammar.scope_name, "source.json");
        assert_eq!(registry.cached_grammar_count(), 1);
    }

    #[test]
    fn bundled_yaml_loads_private_dependency_closure() {
        let mut highlighter = crate::Highlighter::bundled().unwrap();
        let highlighted = highlighter
            .tokenize(
                "yaml",
                "%YAML 1.2\n---\ntitle: \"Syntaxmate\"\nenabled: true\n",
            )
            .unwrap();

        assert!(
            highlighted.lines().iter().any(|line| line
                .spans()
                .iter()
                .any(|span| line.scope_names(span.scope_stack()).count() > 1)),
            "YAML should not collapse to root-scope-only output"
        );
    }

    #[test]
    fn every_public_language_has_a_smoke_fixture_budget_gate() {
        let bundle = embedded_bundle();
        let mut grammars = GrammarSet::new();
        for (index, blob) in bundle.grammar_blobs.iter().enumerate() {
            let grammar = blob
                .compiled_grammar(GrammarId(index as u16))
                .unwrap_or_else(|error| panic!("{} failed to decode: {error:?}", blob.language));
            grammars.add(grammar);
        }

        let mut failures = Vec::new();
        for language in &bundle.languages {
            let root = grammars
                .grammar_by_scope(&language.scope_name)
                .unwrap_or_else(|| panic!("missing grammar for {}", language.canonical))
                .id;
            let mut tokenizer = TextMateTokenizer::new(grammars.clone(), root);
            tokenizer.set_counters_enabled(true);
            let _ = tokenizer.tokenize_source(language_smoke_fixture(&language.canonical));
            let counters = tokenizer.take_counters();
            if counters.degraded_lines != 0 || counters.fallback_budget_kills != 0 {
                failures.push(format!(
                    "{} degraded={} fallback_budget_kills={}",
                    language.canonical, counters.degraded_lines, counters.fallback_budget_kills
                ));
            }
        }
        assert!(
            failures.is_empty(),
            "public language smoke fixture budget failures:\n{}",
            failures.join("\n")
        );
    }

    fn language_smoke_fixture(language: &str) -> &'static str {
        match language {
            "csv" => "a,b,c\n",
            "tsv" => "a\tb\tc\n",
            "json" | "jsonc" => "{\"key\": 1}\n",
            "xml" | "html" => "<root>text</root>\n",
            "yaml" => "key: value\n",
            _ => "x\n",
        }
    }
}
