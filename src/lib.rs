//! Rust-native TextMate syntax highlighting with bundled grammars and themes.
//!
//! [`Highlighter`] is the default batteries-included entry point, with
//! structured spans plus safe HTML and ANSI convenience output.
//! [`GrammarRegistry`] and [`Tokenizer`] provide the custom-grammar API.
//!
//! ```
//! use syntaxmate::Highlighter;
//!
//! let mut highlighter = Highlighter::bundled()?;
//! let document = highlighter.highlight("rust", "fn main() {}", "github-dark")?;
//! assert!(document.status().is_complete());
//! # Ok::<(), syntaxmate::Error>(())
//! ```

#![forbid(unsafe_code)]

mod catalog;
#[allow(dead_code, unused_imports)]
mod engine;
mod error;
#[allow(dead_code)]
mod grammars;
mod highlighter;
mod render;
mod theme;
mod tokenizer;
#[allow(dead_code)]
mod types;

pub use catalog::{AssetLicense, Catalog, CatalogSummary};
pub use error::{Error, Result};
#[cfg(feature = "bundled-grammars")]
pub use highlighter::{HighlightSession, Highlighter};
pub use highlighter::{
    HighlightedDocument, HighlightedLine, HighlightedSpan, IncrementalHighlightedLine,
    IncrementalHighlightedSpan, Theme, style_document,
};
pub use render::RenderedOutput;
#[cfg(feature = "ansi")]
pub use render::{AnsiOptions, render_ansi};
#[cfg(feature = "html")]
pub use render::{HtmlOptions, render_html};
pub use theme::{
    ResolvedSyntaxStyle as Style, ResolvedThemeStyle, RgbColor, SyntaxModifiers as FontModifiers,
    TextMateTheme, ThemeMatch, ThemeSelectorScore,
};
pub use tokenizer::{
    CheckpointTable, DocumentLine, GrammarId, GrammarLimits, GrammarRegistry, HighlightStatus,
    PreparedLanguage, PreparedLanguageStats, ScopedToken, TokenSpan, TokenizedDocument,
    TokenizedLine, Tokenizer, TokenizerState,
};
pub use types::{
    DEFAULT_LINE_CACHE_ENTRIES, DEFAULT_MAX_LINE_BYTES, HighlightScopeTable, ScopeAtomId,
    ScopeStackRef, ThemeRule, TokenizerOptions,
};
pub use types::{HighlightScopeTable as ScopeTable, ScopeStackRef as ScopeStackId};

// Internal engine modules use these compact output types directly. They are
// deliberately not part of the top-level documented facade.
pub(crate) use types::{
    HighlightedLine as EngineHighlightedLine, HighlightedText, LineTextFingerprint, SyntaxClass,
    SyntaxSegment,
};
/// Returns the canonical bundled language ID for an ID or alias.
pub fn canonical_language(language: &str) -> Option<String> {
    grammars::canonical_language(language)
}

/// Detects a bundled language from a path.
pub fn detect_language_from_path(path: impl AsRef<std::path::Path>) -> Option<String> {
    grammars::detect_language_from_path(&path.as_ref().to_string_lossy())
}

/// Lists the bundled public language IDs.
pub fn available_languages() -> Vec<String> {
    grammars::available_languages()
}

#[cfg(test)]
#[path = "../tests/engine_capture_quality.rs"]
mod engine_capture_quality;
#[cfg(all(test, feature = "bundled-grammars", feature = "bundled-themes"))]
mod public_api_tests;
#[cfg(test)]
#[path = "../tests/textmate_golden.rs"]
mod textmate_golden;
#[cfg(test)]
#[path = "../tests/theme_golden.rs"]
mod theme_golden;

#[cfg(feature = "diagnostics")]
pub mod diagnostics {
    use std::ops::Range;

    pub use crate::engine::counters::{EngineCounters, PatternCompileCount, PatternHotspot};
    use crate::engine::regex::{
        AnchorContext, AutomataMatcher, FallbackMatcher, Matcher, RegexMatcher, parse, translate,
    };
    use crate::{Error, Result};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegexEngine {
        Auto,
        Dfa,
        Fallback,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct RegexAnchorContext {
        pub allow_start_of_file: bool,
        pub continuation_position: Option<usize>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RegexInspection {
        pub parsed: String,
        pub translated_pattern: String,
        pub anchor_strategy: String,
        pub route: String,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RegexMatchReport {
        pub engine: &'static str,
        pub matched: Option<Range<usize>>,
        pub captures: Vec<Option<Range<usize>>>,
        pub steps: Option<usize>,
    }

    pub fn inspect_regex(pattern: &str) -> RegexInspection {
        let parsed = parse(pattern);
        let translation = translate(pattern);
        RegexInspection {
            parsed: parsed.to_string(),
            translated_pattern: translation.pattern,
            anchor_strategy: format!("{:?}", translation.anchor_strategy),
            route: format!("{:?}", translation.route),
        }
    }

    pub fn match_regex(
        pattern: &str,
        line: &str,
        from: usize,
        anchors: RegexAnchorContext,
        engine: RegexEngine,
        fallback_budget: usize,
    ) -> Result<RegexMatchReport> {
        let context = AnchorContext {
            allow_a: anchors.allow_start_of_file,
            allow_g: anchors.continuation_position.is_some(),
            g_pos: anchors.continuation_position.unwrap_or(0),
        };
        let (engine_name, result, steps) = match engine {
            RegexEngine::Auto => {
                let matcher = RegexMatcher::new(pattern);
                let (result, steps) = matcher
                    .find_report(line, from, context)
                    .map_err(|error| Error::Diagnostic(format!("fallback error: {error:?}")))?;
                (matcher.engine_name(), result, steps)
            }
            RegexEngine::Dfa => {
                let matcher = AutomataMatcher::new(pattern)
                    .map_err(|error| Error::Diagnostic(error.to_string()))?;
                ("dfa", matcher.find(line, from, context), None)
            }
            RegexEngine::Fallback => {
                let matcher = FallbackMatcher::with_budget(pattern, fallback_budget);
                let report = matcher
                    .try_find(line, from, context)
                    .map_err(|error| Error::Diagnostic(format!("fallback error: {error:?}")))?;
                ("fallback", report.result, Some(report.steps))
            }
        };
        Ok(RegexMatchReport {
            engine: engine_name,
            matched: result.as_ref().map(|matched| matched.start..matched.end),
            captures: result.map_or_else(Vec::new, |matched| matched.captures),
            steps,
        })
    }
}
