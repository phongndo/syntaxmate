use std::{fmt, sync::Arc};

/// Error returned by a fallible Syntaxmate operation.
///
/// Match the payload's kind to select recovery without parsing display text.
/// JSON causes remain available through [`std::error::Error::source`]. Clones
/// share those causes; equality compares their category, position and message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The requested language ID or alias is not present in the catalog.
    UnknownLanguage(String),
    /// The requested bundled theme is not present in the catalog.
    UnknownTheme(String),
    /// A grammar could not be parsed, validated, or prepared.
    Grammar(GrammarError),
    /// A theme could not be parsed or compiled.
    Theme(ThemeError),
    /// A bundled asset could not be decoded or validated.
    Bundle(BundleError),
    /// A feature-gated diagnostic operation failed.
    Diagnostic(DiagnosticError),
    /// Source validation or output writing failed.
    Render(RenderError),
    /// Incremental state belongs to another tokenizer.
    StateMismatch,
    /// Incremental input contained more than one logical line.
    InvalidLine,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLanguage(value) => write!(f, "unknown TextMate language `{value}`"),
            Self::UnknownTheme(value) => write!(f, "unknown TextMate theme `{value}`"),
            Self::Grammar(error) => error.fmt(f),
            Self::Theme(error) => error.fmt(f),
            Self::Bundle(error) => error.fmt(f),
            Self::Diagnostic(error) => error.fmt(f),
            Self::Render(error) => error.fmt(f),
            Self::StateMismatch => {
                f.write_str("tokenizer state belongs to a different Syntaxmate tokenizer")
            }
            Self::InvalidLine => {
                f.write_str("tokenize_line expects one logical line without a newline terminator")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Grammar(error) => Some(error),
            Self::Theme(error) => Some(error),
            Self::Bundle(error) => Some(error),
            Self::Diagnostic(error) => Some(error),
            Self::Render(error) => Some(error),
            _ => None,
        }
    }
}

/// Result type for fallible Syntaxmate operations.
pub type Result<T> = std::result::Result<T, Error>;

/// A JSON decoding failure with its original serde cause.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct JsonError(Arc<serde_json::Error>);

impl JsonError {
    pub(crate) fn new(error: serde_json::Error) -> Self {
        Self(Arc::new(error))
    }
    /// One-based input line reported by serde.
    pub fn line(&self) -> usize {
        self.0.line()
    }
    /// One-based input column reported by serde, or zero before any input.
    pub fn column(&self) -> usize {
        self.0.column()
    }
}
impl PartialEq for JsonError {
    fn eq(&self, other: &Self) -> bool {
        self.0.classify() == other.0.classify()
            && self.line() == other.line()
            && self.column() == other.column()
            && self.0.to_string() == other.0.to_string()
    }
}
impl Eq for JsonError {}
impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
impl std::error::Error for JsonError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.0.as_ref())
    }
}

/// A missing local or external grammar include.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MissingInclude {
    scope: Option<String>,
    repository: Option<String>,
}
impl MissingInclude {
    pub(crate) fn new(scope: Option<String>, repository: Option<String>) -> Self {
        Self { scope, repository }
    }
    /// Referenced external scope, or `None` for a local repository include.
    pub fn scope_name(&self) -> Option<&str> {
        self.scope.as_deref()
    }
    /// Referenced repository key without the `#`, if present.
    pub fn repository_key(&self) -> Option<&str> {
        self.repository.as_deref()
    }
}

/// A regex parser diagnostic. Offsets count Unicode scalar values, not bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RegexError {
    pattern: String,
    position: usize,
    message: String,
}
impl RegexError {
    pub(crate) fn new(pattern: String, position: usize, message: String) -> Self {
        Self {
            pattern,
            position,
            message,
        }
    }
    /// Original regex pattern.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
    /// Zero-based Unicode scalar offset, possibly at the end of the pattern.
    pub fn position(&self) -> usize {
        self.position
    }
    /// Human-readable parser diagnostic.
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// The custom grammar resource whose configured limit was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GrammarResource {
    /// JSON input bytes for one grammar.
    GrammarBytes,
    /// Number of grammars in the registry.
    GrammarCount,
}

/// A configured grammar limit and the attempted resource usage.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct LimitExceeded {
    resource: GrammarResource,
    limit: usize,
    actual: usize,
}
impl LimitExceeded {
    pub(crate) fn new(resource: GrammarResource, limit: usize, actual: usize) -> Self {
        Self {
            resource,
            limit,
            actual,
        }
    }
    /// Resource being measured; byte limits use UTF-8 bytes.
    pub fn resource(&self) -> GrammarResource {
        self.resource
    }
    /// Maximum permitted value.
    pub fn limit(&self) -> usize {
        self.limit
    }
    /// Attempted value, including the grammar being added.
    pub fn actual(&self) -> usize {
        self.actual
    }
}

/// Matchable cause of a grammar failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GrammarErrorKind {
    /// Invalid JSON syntax or grammar JSON structure.
    InvalidJson(JsonError),
    /// A local repository or external grammar include was not found.
    MissingInclude(MissingInclude),
    /// Explicit regex validation found a parser diagnostic.
    InvalidRegex(RegexError),
    /// A configured registry limit was exceeded.
    LimitExceeded(LimitExceeded),
    /// The root grammar ID does not belong to this registry.
    ForeignGrammarId,
    /// Preparation exceeded an internal graph-walk bound; use a tokenizer directly.
    PreparationLimit(String),
    /// A compiled reference is invalid for a reason other than a missing include.
    InvalidReference(String),
}

/// Grammar failure with the originating scope when available.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct GrammarError {
    scope: Option<String>,
    kind: GrammarErrorKind,
}
impl GrammarError {
    pub(crate) fn new(scope: Option<String>, kind: GrammarErrorKind) -> Self {
        Self { scope, kind }
    }
    /// Grammar scope, or `None` when parsing/limits failed before it was known.
    pub fn scope_name(&self) -> Option<&str> {
        self.scope.as_deref()
    }
    /// Matchable failure cause.
    pub fn kind(&self) -> &GrammarErrorKind {
        &self.kind
    }
}
impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(scope) = &self.scope {
            write!(f, "{scope}: ")?;
        }
        match &self.kind {
            GrammarErrorKind::InvalidJson(error) => write!(f, "JSON parse error: {error}"),
            GrammarErrorKind::MissingInclude(include) => write!(
                f,
                "unknown include {}{}{}",
                include.scope_name().unwrap_or(""),
                if include.repository.is_some() {
                    "#"
                } else {
                    ""
                },
                include.repository_key().unwrap_or("")
            ),
            GrammarErrorKind::InvalidRegex(error) => {
                write!(f, "invalid regex `{}`: {}", error.pattern, error.message)
            }
            GrammarErrorKind::LimitExceeded(error) => write!(
                f,
                "grammar {:?} value {} exceeds limit {}",
                error.resource, error.actual, error.limit
            ),
            GrammarErrorKind::ForeignGrammarId => {
                f.write_str("root grammar does not belong to this registry")
            }
            GrammarErrorKind::PreparationLimit(detail) => write!(
                f,
                "grammar exceeds PreparedLanguage preparation bounds ({detail}); use Tokenizer directly"
            ),
            GrammarErrorKind::InvalidReference(message) => f.write_str(message),
        }
    }
}
impl std::error::Error for GrammarError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            GrammarErrorKind::InvalidJson(error) => Some(error),
            _ => None,
        }
    }
}

/// Matchable cause of a theme failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ThemeErrorKind {
    /// Invalid JSON syntax or theme JSON structure.
    InvalidJson(JsonError),
    /// Unsupported color syntax or alpha/background combination; contains the input value.
    InvalidColor(String),
    /// Invalid selector, font style, or empty rule settings.
    InvalidRule,
}

/// Theme parsing or rule compilation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ThemeError {
    kind: ThemeErrorKind,
    message: String,
}
impl ThemeError {
    pub(crate) fn rule(message: String) -> Self {
        Self::new(ThemeErrorKind::InvalidRule, message)
    }
    pub(crate) fn new(kind: ThemeErrorKind, message: String) -> Self {
        Self { kind, message }
    }
    pub(crate) fn json(error: serde_json::Error) -> Self {
        let message = format!("invalid TextMate theme JSON: {error}");
        Self::new(ThemeErrorKind::InvalidJson(JsonError::new(error)), message)
    }
    pub(crate) fn color(value: &str, message: String) -> Self {
        Self::new(ThemeErrorKind::InvalidColor(value.to_owned()), message)
    }
    /// Matchable failure cause, including the invalid color when applicable.
    pub fn kind(&self) -> &ThemeErrorKind {
        &self.kind
    }
}
impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for ThemeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            ThemeErrorKind::InvalidJson(error) => Some(error),
            _ => None,
        }
    }
}

/// Matchable cause of a bundled asset failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BundleErrorKind {
    /// The embedded catalog has no languages.
    EmptyCatalog,
    /// A requested grammar or its closure is absent.
    MissingGrammar,
    /// Compiled grammar data could not be decoded.
    Decode,
}

/// Bundled asset failure without exposing the private bundle format.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BundleError {
    kind: BundleErrorKind,
    language: Option<String>,
    message: String,
}
impl BundleError {
    pub(crate) fn new(kind: BundleErrorKind, language: Option<String>, message: String) -> Self {
        Self {
            kind,
            language,
            message,
        }
    }
    /// Matchable failure cause.
    pub fn kind(&self) -> BundleErrorKind {
        self.kind
    }
    /// Affected language or scope, when known.
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }
}
impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for BundleError {}

/// Matchable cause of a diagnostic regex operation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticErrorKind {
    /// The requested matcher cannot compile this pattern.
    MatcherBuild,
    /// The fallback matcher exhausted its execution budget.
    BudgetExceeded,
    /// The starting byte offset is outside the input or inside a UTF-8 character.
    InvalidStart,
}

/// Diagnostic regex failure with the original pattern and execution context.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiagnosticError {
    kind: DiagnosticErrorKind,
    pattern: String,
    position: Option<usize>,
    steps: Option<usize>,
    message: String,
}
impl DiagnosticError {
    #[cfg(feature = "diagnostics")]
    pub(crate) fn build(pattern: &str, message: String) -> Self {
        Self {
            kind: DiagnosticErrorKind::MatcherBuild,
            pattern: pattern.to_owned(),
            position: None,
            steps: None,
            message,
        }
    }
    #[cfg(feature = "diagnostics")]
    pub(crate) fn fallback(pattern: &str, error: crate::engine::regex::FallbackError) -> Self {
        use crate::engine::regex::FallbackError;
        let (kind, position, steps) = match error {
            FallbackError::InvalidStart { from } => {
                (DiagnosticErrorKind::InvalidStart, Some(from), None)
            }
            FallbackError::BudgetExceeded { steps } => {
                (DiagnosticErrorKind::BudgetExceeded, None, Some(steps))
            }
        };
        Self {
            kind,
            pattern: pattern.to_owned(),
            position,
            steps,
            message: format!("fallback error: {error:?}"),
        }
    }
    /// Matchable failure cause.
    pub fn kind(&self) -> DiagnosticErrorKind {
        self.kind
    }
    /// Original regex pattern.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
    /// Invalid starting byte offset, if applicable.
    pub fn position(&self) -> Option<usize> {
        self.position
    }
    /// Executed fallback steps when the budget was exhausted.
    pub fn steps(&self) -> Option<usize> {
        self.steps
    }
}
impl fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for DiagnosticError {}

/// Matchable cause of a render failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderErrorKind {
    /// Source line counts or byte ranges do not match the document.
    SourceMismatch,
    /// The output writer returned [`fmt::Error`]; output may be partial.
    Writer,
}

/// Render validation or writer failure.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RenderError {
    kind: RenderErrorKind,
    message: String,
    source: Option<fmt::Error>,
}
impl RenderError {
    #[cfg(any(feature = "html", feature = "ansi"))]
    pub(crate) fn mismatch(message: String) -> Self {
        Self {
            kind: RenderErrorKind::SourceMismatch,
            message,
            source: None,
        }
    }
    #[cfg(any(feature = "html", feature = "ansi"))]
    pub(crate) fn writer(error: fmt::Error) -> Self {
        Self {
            kind: RenderErrorKind::Writer,
            message: "render output writer failed".to_owned(),
            source: Some(error),
        }
    }
    /// Matchable failure cause.
    pub fn kind(&self) -> RenderErrorKind {
        self.kind
    }
}
impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|error| error as _)
    }
}

pub(crate) fn grammar_load_error(error: crate::engine::grammar::GrammarLoadError) -> Error {
    use crate::engine::grammar::GrammarLoadError;
    match error {
        GrammarLoadError::Json { source, .. } => Error::Grammar(GrammarError::new(
            None,
            GrammarErrorKind::InvalidJson(JsonError::new(source)),
        )),
        GrammarLoadError::Validation { source, .. } => grammar_validation_error(*source),
    }
}

pub(crate) fn grammar_validation_error(
    error: crate::engine::grammar::GrammarValidationError,
) -> Error {
    let message = error.to_string();
    let kind = match error.missing_include {
        Some(target) => {
            let (scope, repository) = *target;
            GrammarErrorKind::MissingInclude(MissingInclude::new(scope, repository))
        }
        None => GrammarErrorKind::InvalidReference(message),
    };
    Error::Grammar(GrammarError::new(Some(error.grammar), kind))
}

#[cfg(test)]
mod tests;
