use std::fmt;

/// Error returned by Syntaxmate's batteries-included API.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The requested language ID or alias is not present in the catalog.
    UnknownLanguage(String),
    /// The requested bundled theme is not present in the catalog.
    UnknownTheme(String),
    /// A grammar could not be decoded, parsed, or linked.
    Grammar(String),
    /// A theme could not be decoded or parsed.
    Theme(String),
    /// A bundled asset could not be decoded or validated.
    Bundle(String),
    /// A feature-gated diagnostic operation failed.
    Diagnostic(String),
    /// Highlighted byte ranges did not match the source supplied to a renderer.
    Render(String),
    /// Incremental state was used with a tokenizer that did not create it.
    StateMismatch,
    /// Incremental input contained more than one logical line.
    InvalidLine,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownLanguage(language) => {
                write!(formatter, "unknown TextMate language `{language}`")
            }
            Self::UnknownTheme(theme) => write!(formatter, "unknown TextMate theme `{theme}`"),
            Self::Grammar(message) | Self::Theme(message) => formatter.write_str(message),
            Self::Bundle(message) | Self::Diagnostic(message) | Self::Render(message) => {
                formatter.write_str(message)
            }
            Self::StateMismatch => {
                formatter.write_str("tokenizer state belongs to a different Syntaxmate tokenizer")
            }
            Self::InvalidLine => formatter
                .write_str("tokenize_line expects one logical line without a newline terminator"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;
