#[cfg(feature = "bundled-themes")]
use crate::theme::BuiltinTextMateTheme;

/// Read-only access to Syntaxmate's bundled languages, detection metadata,
/// themes, versions, and third-party provenance.
#[derive(Debug, Clone, Copy, Default)]
pub struct Catalog;

impl Catalog {
    /// Returns access to the bundled assets.
    pub fn bundled() -> Self {
        Self
    }

    /// Lists canonical public language IDs.
    pub fn languages(self) -> Vec<String> {
        crate::grammars::available_languages()
    }

    /// Lists bundled theme names.
    #[cfg(feature = "bundled-themes")]
    pub fn themes(self) -> Vec<&'static str> {
        BuiltinTextMateTheme::all()
            .iter()
            .map(|theme| theme.name())
            .collect()
    }

    /// Resolves a language ID or alias to its canonical public ID.
    pub fn canonical_language(self, language: &str) -> Option<String> {
        crate::grammars::canonical_language(language)
    }

    /// Resolves a public language ID from its root TextMate scope name.
    pub fn language_for_scope(self, scope: &str) -> Option<String> {
        crate::grammars::embedded_bundle()
            .languages
            .iter()
            .find(|language| language.scope_name == scope)
            .map(|language| language.canonical.clone())
    }

    /// Detects a language from a filename or path.
    pub fn detect_path(self, path: impl AsRef<std::path::Path>) -> Option<String> {
        crate::grammars::detect_language_from_path(&path.as_ref().to_string_lossy())
    }

    /// Returns the embedded bundle format version.
    pub fn bundle_version(self) -> &'static str {
        crate::grammars::embedded_bundle_version()
    }

    /// Returns bundle size, counts, and provenance.
    pub fn bundle_summary(self) -> CatalogSummary {
        let summary = crate::grammars::bundle_summary();
        CatalogSummary {
            version: summary.version,
            bundle_bytes: crate::grammars::embedded_bundle_bytes().len(),
            source_hash: summary.source_hash,
            grammar_count: summary.grammar_count,
            language_count: summary.language_count,
            scope_count: summary.scope_count,
            license_count: summary.license_count,
            source_revision: summary.source_revision,
        }
    }

    /// Returns bundled asset licenses and provenance.
    pub fn licenses(self) -> Vec<AssetLicense> {
        crate::grammars::bundled_licenses()
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
