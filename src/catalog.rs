use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, OnceLock},
};

use crate::engine::regex::{AnchorContext, FallbackMatcher};
#[cfg(feature = "bundled-themes")]
use crate::theme::BuiltinTextMateTheme;
use crate::{
    Error, Result,
    grammars::bundle::{Bundle, LanguageEntry},
};

/// Cheaply cloned, read-only access to a grammar bundle and its metadata.
///
/// Clones share metadata and detection matchers. No filesystem access occurs.
#[derive(Debug, Clone)]
pub struct Catalog(Arc<CatalogInner>);

#[derive(Debug)]
struct CatalogInner {
    bundle: Arc<Bundle>,
    bytes: usize,
    version: String,
    canonical: HashMap<String, usize>,
    basenames: HashMap<String, usize>,
    extensions: HashMap<String, usize>,
    first_lines: Vec<OnceLock<FallbackMatcher>>,
}

/// Borrowed metadata for one public language in a [`Catalog`].
#[derive(Debug, Clone, Copy)]
pub struct LanguageInfo<'a> {
    entry: &'a LanguageEntry,
}

impl<'a> LanguageInfo<'a> {
    /// Canonical public language ID.
    pub fn id(self) -> &'a str {
        &self.entry.canonical
    }
    /// Alternate names accepted by catalog lookup.
    pub fn aliases(self) -> &'a [String] {
        &self.entry.aliases
    }
    /// Filename suffixes, without a leading dot; may contain multiple dots.
    pub fn extensions(self) -> &'a [String] {
        &self.entry.extensions
    }
    /// Recognized complete filenames.
    pub fn basenames(self) -> &'a [String] {
        &self.entry.basenames
    }
    /// Root TextMate scope name.
    pub fn root_scope(self) -> &'a str {
        &self.entry.scope_name
    }
}

impl Catalog {
    /// Returns a shared handle to the embedded catalog.
    #[cfg(feature = "bundled-grammars")]
    pub fn bundled() -> Self {
        static CATALOG: OnceLock<Catalog> = OnceLock::new();
        CATALOG
            .get_or_init(|| {
                Self::from_bundle(
                    Arc::clone(crate::grammars::embedded_bundle_shared()),
                    crate::grammars::embedded_bundle_bytes().len(),
                )
            })
            .clone()
    }

    /// Loads a bundle from static storage, such as `include_bytes!`.
    ///
    /// Validates container tables, references, compressed grammars and compiled
    /// IR before returning. Uncompressed metadata borrows the input. Requires
    /// no bundled-asset features. Bundle encoding is versioned but not a stable
    /// interchange format; generate it with the matching `syntaxmate-bundle`.
    /// Input is limited to 64 MiB, 4,096 languages/grammars, 16 MiB decoded per
    /// grammar and 256 MiB decoded in total. Invalid assets return [`Error::Bundle`].
    pub fn from_static(bytes: &'static [u8]) -> Result<Self> {
        Self::parse(bytes, Bundle::parse_static)
    }

    /// Loads a bundle from temporary or caller-owned bytes, copying retained data.
    ///
    /// The input may be dropped on return. Validation is the same as
    /// [`Self::from_static`]; catalog clones share the resulting allocation.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        Self::parse(bytes, Bundle::parse)
    }

    fn parse<'a>(
        bytes: &'a [u8],
        parse: impl FnOnce(
            &'a [u8],
        ) -> std::result::Result<Bundle, crate::grammars::bundle::BundleError>,
    ) -> Result<Self> {
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(Error::Bundle(crate::BundleError::new(
                crate::BundleErrorKind::TooLarge,
                None,
                "bundle exceeds 64 MiB".to_owned(),
            )));
        }
        let bundle = parse(bytes).map_err(|error| {
            Error::Bundle(crate::BundleError::new(
                crate::BundleErrorKind::Invalid,
                None,
                format!("invalid Syntaxmate bundle: {error:?}"),
            ))
        })?;
        bundle.validate().map_err(|error| {
            Error::Bundle(crate::BundleError::new(
                crate::BundleErrorKind::Invalid,
                None,
                format!("invalid Syntaxmate bundle: {error:?}"),
            ))
        })?;
        Ok(Self::from_bundle(Arc::new(bundle), bytes.len()))
    }

    fn from_bundle(bundle: Arc<Bundle>, bytes: usize) -> Self {
        let mut canonical = HashMap::new();
        let mut basenames = HashMap::new();
        let mut extensions = HashMap::new();
        for (index, language) in bundle.languages.iter().enumerate() {
            canonical.insert(language.canonical.clone(), index);
        }
        for (index, language) in bundle.languages.iter().enumerate() {
            for alias in &language.aliases {
                canonical.entry(alias.clone()).or_insert(index);
            }
            for basename in &language.basenames {
                basenames.insert(basename.to_ascii_lowercase(), index);
                if basename.contains('.') {
                    extensions.insert(basename.trim_start_matches('.').to_ascii_lowercase(), index);
                }
            }
            for extension in &language.extensions {
                extensions.insert(
                    extension.trim_start_matches('.').to_ascii_lowercase(),
                    index,
                );
            }
        }
        let first_lines = bundle.languages.iter().map(|_| OnceLock::new()).collect();
        Self(Arc::new(CatalogInner {
            version: bundle.version_stamp(),
            bundle,
            bytes,
            canonical,
            basenames,
            extensions,
            first_lines,
        }))
    }

    pub(crate) fn bundle(&self) -> &Arc<Bundle> {
        &self.0.bundle
    }

    /// Lists canonical public IDs, borrowing catalog storage.
    pub fn languages(&self) -> Vec<&str> {
        self.0
            .bundle
            .languages
            .iter()
            .map(|entry| entry.canonical.as_str())
            .collect()
    }

    /// Looks up public metadata by canonical ID or alias (ASCII case insensitive).
    pub fn language(&self, language: &str) -> Option<LanguageInfo<'_>> {
        let token = language.trim().trim_start_matches('.');
        let index = if token.bytes().any(|byte| byte.is_ascii_uppercase()) {
            self.0.canonical.get(&token.to_ascii_lowercase())
        } else {
            self.0.canonical.get(token)
        }?;
        Some(LanguageInfo {
            entry: &self.0.bundle.languages[*index],
        })
    }

    /// Resolves a public ID or alias to its borrowed canonical ID.
    pub fn canonical_language(&self, language: &str) -> Option<&str> {
        self.language(language).map(LanguageInfo::id)
    }

    /// Resolves a public language ID from its root TextMate scope name.
    pub fn language_for_scope(&self, scope: &str) -> Option<&str> {
        self.0
            .bundle
            .languages
            .iter()
            .find(|entry| entry.scope_name == scope)
            .map(|entry| entry.canonical.as_str())
    }

    /// Detects by complete basename, then longest matching filename suffix.
    /// Matching ignores ASCII case; catalog order breaks metadata ties (last wins).
    pub fn detect_path(&self, path: impl AsRef<Path>) -> Option<&str> {
        let name = path.as_ref().file_name()?.to_str()?.to_ascii_lowercase();
        let index = self.0.basenames.get(&name).copied().or_else(|| {
            name.match_indices('.')
                .find_map(|(dot, _)| self.0.extensions.get(&name[dot + 1..]).copied())
        })?;
        Some(&self.0.bundle.languages[index].canonical)
    }

    /// Detects by path, modeline, grammar `firstLineMatch`, then shebang interpreter.
    ///
    /// Pass `None` for an unknown path. Only the first logical line is inspected,
    /// up to 4 KiB at a UTF-8 boundary. Emacs `-*- mode -*-`/`mode:`, Vim `ft=`
    /// and `filetype=` names resolve through aliases. Grammar patterns use the
    /// Oniguruma-compatible engine. Patterns longer than 4 KiB are
    /// skipped; each search gets 10,000 steps and at most 100 patterns are tried.
    /// Unsupported or exhausted patterns are nonmatches. First matching pattern
    /// in catalog order wins. As a fallback, shebang interpreter basenames
    /// (also through `env -S`) resolve as IDs/aliases, stripping numeric version
    /// suffixes; `node`/`nodejs` map to JavaScript. Detection is a bounded heuristic,
    /// not validation.
    pub fn detect(&self, path: Option<&Path>, source: &str) -> Option<&str> {
        if let Some(language) = path.and_then(|path| self.detect_path(path)) {
            return Some(language);
        }
        let mut end = source.len().min(4096);
        while !source.is_char_boundary(end) {
            end -= 1;
        }
        let line = source[..end].split(['\n', '\r']).next().unwrap_or_default();
        if let Some((_, rest)) = line.split_once("-*-")
            && let Some((mode, _)) = rest.split_once("-*-")
        {
            let mode = mode
                .trim()
                .strip_prefix("mode:")
                .unwrap_or(mode)
                .split(';')
                .next()?
                .trim();
            if let Some(language) = self.canonical_language(mode) {
                return Some(language);
            }
        }
        if line.contains("vim:") || line.contains("vi:") || line.contains("ex:") {
            for word in line.split_whitespace() {
                if let Some(mode) = word
                    .strip_prefix("ft=")
                    .or_else(|| word.strip_prefix("filetype="))
                    && let Some(language) = self.canonical_language(mode.trim_end_matches(':'))
                {
                    return Some(language);
                }
            }
        }
        for (index, entry) in self
            .0
            .bundle
            .languages
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry
                    .first_line_pattern
                    .as_ref()
                    .is_some_and(|pattern| pattern.len() <= 4096)
            })
            .take(100)
        {
            let matcher = self.0.first_lines[index].get_or_init(|| {
                FallbackMatcher::with_budget(entry.first_line_pattern.as_deref().unwrap(), 10_000)
            });
            if matcher
                .try_find(line, 0, AnchorContext::start_of_file())
                .ok()
                .is_some_and(|report| report.result.is_some())
            {
                return Some(&entry.canonical);
            }
        }
        let mut words = line.strip_prefix("#!")?.split_whitespace();
        let mut interpreter = words.next()?.rsplit('/').next()?;
        if interpreter == "env" {
            interpreter = words
                .find(|word| !word.starts_with('-') && !word.contains('='))?
                .rsplit('/')
                .next()?;
        }
        let interpreter = interpreter.trim_end_matches(|ch: char| ch.is_ascii_digit() || ch == '.');
        let interpreter = match interpreter {
            "node" | "nodejs" => "javascript",
            other => other,
        };
        self.canonical_language(interpreter)
    }

    /// Lists bundled theme names (independent of this grammar catalog).
    #[cfg(feature = "bundled-themes")]
    pub fn themes(&self) -> Vec<&'static str> {
        BuiltinTextMateTheme::all()
            .iter()
            .map(|theme| theme.name())
            .collect()
    }

    /// Returns the bundle's version stamp.
    pub fn bundle_version(&self) -> &str {
        &self.0.version
    }

    /// Returns bundle size, counts, and provenance.
    pub fn bundle_summary(&self) -> CatalogSummary {
        let summary = crate::grammars::BundleSummary::from_bundle(&self.0.bundle);
        CatalogSummary {
            version: summary.version,
            bundle_bytes: self.0.bytes,
            source_hash: summary.source_hash,
            grammar_count: summary.grammar_count,
            language_count: summary.language_count,
            scope_count: summary.scope_count,
            license_count: summary.license_count,
            source_revision: summary.source_revision,
        }
    }

    /// Returns asset licenses and provenance.
    pub fn licenses(&self) -> Vec<AssetLicense> {
        self.0
            .bundle
            .licenses
            .iter()
            .map(|license| AssetLicense {
                language: license.language.clone(),
                source_path: license.source_path.clone(),
                upstream_url: license.upstream_url.clone(),
                spdx_id: license.spdx_id.clone(),
                license_text: license.license_text.clone(),
                source_revision: license.source_revision.clone(),
            })
            .collect()
    }
}

/// Summary of the embedded bundle and its provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSummary {
    /// Bundle format version.
    pub version: String,
    /// Encoded size of the embedded bundle in bytes.
    pub bundle_bytes: usize,
    /// Bundle source identity hash.
    pub source_hash: u64,
    /// Number of grammars, including private dependencies.
    pub grammar_count: usize,
    /// Number of public languages.
    pub language_count: usize,
    /// Number of indexed scope names.
    pub scope_count: usize,
    /// Number of bundled license records.
    pub license_count: usize,
    /// Upstream source revision, when recorded.
    pub source_revision: Option<String>,
}

/// License and source provenance for one bundled language asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLicense {
    /// Canonical language ID.
    pub language: String,
    /// Upstream asset path.
    pub source_path: String,
    /// Upstream repository URL.
    pub upstream_url: String,
    /// SPDX license identifier.
    pub spdx_id: String,
    /// Full license text.
    pub license_text: String,
    /// Upstream source revision, when recorded.
    pub source_revision: String,
}

#[cfg(all(test, feature = "bundled-grammars"))]
mod tests {
    use super::*;

    #[test]
    fn detection_precedence_and_bounded_first_lines() {
        let catalog = Catalog::bundled();
        assert_eq!(
            catalog.detect(Some(Path::new("script.rs")), "#!/usr/bin/env python3"),
            Some("rust")
        );
        assert_eq!(
            catalog.detect(None, "#!/usr/bin/env python3"),
            Some("python")
        );
        assert_eq!(catalog.detect(None, "#!/bin/bash"), Some("shellscript"));
        assert_eq!(catalog.detect(None, "// -*- mode: rust; -*-"), Some("rust"));
        assert_eq!(
            catalog.detect(None, "// vim: set filetype=rust:"),
            Some("rust")
        );
        assert_eq!(catalog.detect(None, "\n#!/bin/bash"), None);
        assert_eq!(
            catalog.detect(None, &format!("{}#!/bin/bash", "é".repeat(4096))),
            None
        );
        assert_eq!(catalog.detect(None, "<script>\0\\"), None);
        assert_eq!(catalog.detect_path("a.blade.php"), Some("blade"));
        assert_eq!(catalog.detect_path("Dockerfile"), Some("docker"));
    }

    #[test]
    fn first_line_match_uses_engine_regex_syntax() {
        let mut bundle = crate::grammars::embedded_bundle().clone();
        bundle.languages.truncate(1);
        bundle.languages[0].first_line_pattern = Some(r"(?<=^#!)custom([0-9])\1$".into());
        let catalog = Catalog::from_bundle(Arc::new(bundle), 0);
        assert_eq!(
            catalog.detect(None, "#!custom33"),
            Some(catalog.languages()[0])
        );
        assert_eq!(catalog.detect(None, "#!custom34"), None);
    }

    #[test]
    fn pathological_first_line_patterns_are_bounded() {
        let mut bundle = crate::grammars::embedded_bundle().clone();
        bundle.languages.truncate(1);
        bundle.languages[0].first_line_pattern = Some("^(a+)+$".into());
        let catalog = Catalog::from_bundle(Arc::new(bundle), 0);
        assert_eq!(
            catalog.detect(None, &format!("{}!", "a".repeat(4000))),
            None
        );
    }
}
