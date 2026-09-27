//! Rust-native TextMate syntax highlighting with bundled grammars and themes.
//!
//! [`Highlighter`] is the default batteries-included entry point, with
//! structured spans plus safe HTML and ANSI convenience output.
//! [`GrammarRegistry`] and [`Tokenizer`] provide the custom-grammar API.
//!
//! ```
//! use syntaxmate::Highlighter;
//!
//! let highlighter = Highlighter::bundled()?;
//! let document = highlighter.highlight("rust", "fn main() {}", "github-dark")?;
//! assert!(document.status().is_complete());
//! # Ok::<(), syntaxmate::Error>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, doc(auto_cfg))]

// Compile the actual guide snippets as doctests instead of maintaining copies.
#[cfg(all(
    doc,
    feature = "bundled-grammars",
    feature = "bundled-themes",
    feature = "html"
))]
#[doc = include_str!("../README.md")]
mod readme_examples {}

#[cfg(all(
    doc,
    feature = "bundled-grammars",
    feature = "bundled-themes",
    feature = "html",
    feature = "ansi"
))]
#[doc = include_str!("../docs/rendering.md")]
mod rendering_examples {}

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

pub use catalog::{AssetLicense, Catalog, CatalogSummary, LanguageInfo};
pub use error::{Error, Result};
pub use highlighter::{HighlightSession, Highlighter, HighlighterOptions};
pub use highlighter::{
    HighlightedDocument, HighlightedLine, HighlightedToken, Theme, style_document,
};
pub use render::RenderedOutput;
#[cfg(feature = "ansi")]
pub use render::{AnsiOptions, render_ansi, render_ansi_to};
#[cfg(feature = "html")]
pub use render::{HtmlOptions, html_stylesheet, render_html, render_html_to};
pub use theme::{FontModifiers, RgbColor, Style};
#[cfg(feature = "diagnostics")]
pub use theme::{ResolvedThemeStyle, ThemeMatch, ThemeSelectorScore};
pub use tokenizer::{
    CheckpointTable, GrammarId, GrammarLimits, GrammarRegistry, HighlightStatus, PreparedLanguage,
    PreparedLanguageStats, Scopes, Token, TokenizedDocument, TokenizedLine, Tokenizer,
    TokenizerState,
};
pub(crate) use types::{HighlightScopeTable, ScopeAtomId};
pub use types::{ScopeStackId, ThemeRule, TokenizerOptions};

// Internal engine modules use these compact output types directly. They are
// deliberately not part of the top-level documented facade.
pub(crate) use types::{
    HighlightedLine as EngineHighlightedLine, HighlightedText, LineTextFingerprint, SyntaxClass,
    SyntaxSegment,
};
#[cfg(test)]
#[path = "../tests/engine_capture_quality.rs"]
mod engine_capture_quality;
#[cfg(test)]
#[path = "../tests/engine_regressions.rs"]
mod engine_regressions;
#[cfg(all(test, feature = "bundled-grammars", feature = "bundled-themes"))]
mod public_api_tests;
#[cfg(test)]
#[path = "../tests/textmate_golden.rs"]
mod textmate_golden;
#[cfg(test)]
#[path = "../tests/theme_golden.rs"]
mod theme_golden;

/// Feature-gated engine inspection APIs, outside the stable compatibility contract.
#[cfg(feature = "diagnostics")]
pub mod diagnostics {
    use std::ops::Range;

    pub use crate::engine::counters::{EngineCounters, PatternCompileCount, PatternHotspot};
    use crate::engine::regex::{
        AnchorContext, AutomataMatcher, FallbackMatcher, Matcher, RegexMatcher, parse, translate,
    };
    use crate::{Error, Result};

    /// Matcher selection for diagnostic regex execution.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RegexEngine {
        /// Selects a matcher using the normal engine routing rules.
        Auto,
        /// Requires the DFA matcher.
        Dfa,
        /// Uses the budgeted fallback matcher.
        Fallback,
    }

    /// Anchor context for one diagnostic regex search.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct RegexAnchorContext {
        /// Whether the start-of-file anchor may match.
        pub allow_start_of_file: bool,
        /// Byte position allowed to match the continuation anchor, if any.
        pub continuation_position: Option<usize>,
    }

    /// Human-readable regex parsing and routing information.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RegexInspection {
        /// Parsed expression rendered as diagnostic text.
        pub parsed: String,
        /// Pattern after compatibility translation.
        pub translated_pattern: String,
        /// Diagnostic anchor-handling strategy.
        pub anchor_strategy: String,
        /// Selected matcher route.
        pub route: String,
    }

    /// Result and execution metadata from a diagnostic regex search.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct RegexMatchReport {
        /// Name of the matcher used.
        pub engine: &'static str,
        /// Matched byte range, or `None` when no match was found.
        pub matched: Option<Range<usize>>,
        /// Capture byte ranges, including the full match at index zero.
        pub captures: Vec<Option<Range<usize>>>,
        /// Fallback execution steps, when the matcher reports them.
        pub steps: Option<usize>,
    }

    /// Inspects regex parsing, translation, and matcher routing.
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

    /// Searches from a byte offset with explicit anchors, matcher choice, and fallback step budget.
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
