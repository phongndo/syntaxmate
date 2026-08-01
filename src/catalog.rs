#[cfg(feature = "bundled-themes")]
use crate::theme::BuiltinTextMateTheme;

/// Read-only access to Syntaxmate's bundled languages, detection metadata,
/// themes, versions, and third-party provenance.
#[derive(Debug, Clone, Copy, Default)]
pub struct Catalog;

impl Catalog {
    pub fn bundled() -> Self {
        Self
    }

    pub fn languages(self) -> Vec<String> {
        crate::grammars::available_languages()
    }

    #[cfg(feature = "bundled-themes")]
    pub fn themes(self) -> Vec<&'static str> {
        BuiltinTextMateTheme::all()
            .iter()
            .map(|theme| theme.name())
            .collect()
    }

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

    pub fn detect_path(self, path: impl AsRef<std::path::Path>) -> Option<String> {
        crate::grammars::detect_language_from_path(&path.as_ref().to_string_lossy())
    }

    pub fn bundle_version(self) -> &'static str {
        crate::grammars::embedded_bundle_version()
    }

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSummary {
    pub version: String,
    pub bundle_bytes: usize,
    pub source_hash: u64,
    pub grammar_count: usize,
    pub language_count: usize,
    pub scope_count: usize,
    pub license_count: usize,
    pub source_revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetLicense {
    pub language: String,
    pub source_path: String,
    pub upstream_url: String,
    pub spdx_id: String,
    pub license_text: String,
    pub source_revision: String,
}
