//! Python extension module `syntaxmate._native`.
//!
//! A thin layer over `syntaxmate-boundary`: every highlighting call copies its
//! input once, releases the GIL, and returns either a `str` or a [`Tokens`]
//! buffer whose flat arrays are exposed as `memoryview`s rather than one Python
//! object per token.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, PoisonError};

use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::pybacked::{PyBackedBytes, PyBackedStr};
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyBytes, PyMemoryView, PyString, PyTuple};
use syntaxmate_boundary as boundary;
use syntaxmate_boundary::{
    AnsiOptions, BoundaryError, Engine, ErrorKind, HtmlOptions, NO_COLOR, OffsetUnit, PackedStyle,
    ThemeHandle, TokenBuffer, TokenOptions,
};

const DEFAULT_THEME: &str = "github-dark";

/// Converts a boundary error into the matching `syntaxmate` exception class.
fn py_error(py: Python<'_>, error: BoundaryError) -> PyErr {
    let class = match error.kind {
        ErrorKind::UnknownLanguage => "UnknownLanguageError",
        ErrorKind::UnknownTheme => "UnknownThemeError",
        ErrorKind::InvalidGrammar => "InvalidGrammarError",
        ErrorKind::InvalidTheme => "InvalidThemeError",
        ErrorKind::InvalidBundle => "InvalidBundleError",
        ErrorKind::InvalidInput => "InvalidInputError",
        ErrorKind::Render => "RenderError",
        ErrorKind::Internal => "InternalError",
    };
    match py
        .import("syntaxmate._errors")
        .and_then(|module| module.getattr(class))
    {
        Ok(class) => match class.call1((error.message,)) {
            Ok(instance) => PyErr::from_value(instance),
            Err(err) => err,
        },
        Err(err) => err,
    }
}

/// Runs `f` without the GIL, turning Rust panics into `InternalError`.
fn detached<T: Send>(
    py: Python<'_>,
    f: impl FnOnce() -> boundary::Result<T> + Send,
) -> PyResult<T> {
    py.detach(|| {
        catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|payload| {
            let detail = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic".to_owned());
            Err(BoundaryError::new(
                ErrorKind::Internal,
                format!("internal error: {detail}"),
            ))
        })
    })
    .map_err(|error| py_error(py, error))
}

fn parse_unit(unit: &str) -> PyResult<OffsetUnit> {
    match unit {
        "codepoint" => Ok(OffsetUnit::CodePoint),
        "utf8" => Ok(OffsetUnit::Utf8),
        "utf16" => Ok(OffsetUnit::Utf16),
        _ => Err(PyValueError::new_err(format!(
            "unit must be 'codepoint', 'utf8', or 'utf16', not {unit:?}"
        ))),
    }
}

fn unit_name(unit: OffsetUnit) -> &'static str {
    match unit {
        OffsetUnit::CodePoint => "codepoint",
        OffsetUnit::Utf8 => "utf8",
        OffsetUnit::Utf16 => "utf16",
    }
}

/// A theme argument: a bundled theme name or a [`Theme`].
fn resolve_theme(theme: &Bound<'_, PyAny>) -> PyResult<ThemeHandle> {
    if let Ok(theme) = theme.cast::<Theme>() {
        return Ok(theme.get().handle.clone());
    }
    let name: PyBackedStr = theme.extract().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("theme must be a bundled theme name or a Theme")
    })?;
    ThemeHandle::bundled(&name).map_err(|error| py_error(theme.py(), error))
}

/// A resolved style: colors are `0xRRGGBB` integers or `None`.
#[pyclass(frozen, eq, hash, skip_from_py_object, module = "syntaxmate")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Style {
    packed: PackedStyle,
}

fn color(value: u32) -> Option<u32> {
    (value != NO_COLOR).then_some(value)
}

#[pymethods]
impl Style {
    /// Foreground color as `0xRRGGBB`, or `None`.
    #[getter]
    fn foreground(&self) -> Option<u32> {
        color(self.packed.foreground)
    }

    /// Background color as `0xRRGGBB`, or `None`.
    #[getter]
    fn background(&self) -> Option<u32> {
        color(self.packed.background)
    }

    /// Bitset of `BOLD`, `ITALIC`, `UNDERLINE`, and `STRIKETHROUGH`.
    #[getter]
    fn modifiers(&self) -> u8 {
        self.packed.modifiers
    }

    #[getter]
    fn bold(&self) -> bool {
        self.packed.modifiers & boundary::BOLD != 0
    }

    #[getter]
    fn italic(&self) -> bool {
        self.packed.modifiers & boundary::ITALIC != 0
    }

    #[getter]
    fn underline(&self) -> bool {
        self.packed.modifiers & boundary::UNDERLINE != 0
    }

    #[getter]
    fn strikethrough(&self) -> bool {
        self.packed.modifiers & boundary::STRIKETHROUGH != 0
    }

    fn __repr__(&self) -> String {
        let hex = |value: u32| color(value).map_or("None".to_owned(), |c| format!("0x{c:06x}"));
        format!(
            "Style(foreground={}, background={}, modifiers={})",
            hex(self.packed.foreground),
            hex(self.packed.background),
            self.packed.modifiers
        )
    }
}

/// A bundled or custom TextMate theme.
#[pyclass(frozen, module = "syntaxmate")]
struct Theme {
    handle: ThemeHandle,
}

#[pymethods]
impl Theme {
    /// Loads a bundled theme by name.
    #[staticmethod]
    fn bundled(py: Python<'_>, name: &str) -> PyResult<Self> {
        let handle = ThemeHandle::bundled(name).map_err(|error| py_error(py, error))?;
        Ok(Self { handle })
    }

    /// Parses a TextMate JSON theme from `str`, `bytes`, or a JSON-compatible mapping.
    #[staticmethod]
    fn from_json(py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Self> {
        let json: PyBackedStr = if data.is_instance_of::<PyString>() {
            data.extract()?
        } else if let Ok(bytes) = data.extract::<PyBackedBytes>() {
            let text = std::str::from_utf8(&bytes)
                .map_err(|error| PyValueError::new_err(format!("theme is not UTF-8: {error}")))?;
            PyString::new(py, text).extract()?
        } else {
            py.import("json")?
                .call_method1("dumps", (data,))?
                .extract()?
        };
        let handle = detached(py, || ThemeHandle::from_json(&json))?;
        Ok(Self { handle })
    }

    /// The theme's name.
    #[getter]
    fn name(&self) -> &str {
        self.handle.name()
    }

    /// The theme's default foreground and background.
    #[getter]
    fn default_style(&self) -> Style {
        Style {
            packed: self.handle.default_style(),
        }
    }

    /// CSS for HTML rendered with `class_prefix` (class mode).
    fn stylesheet(&self, class_prefix: &str) -> String {
        self.handle.stylesheet(class_prefix)
    }

    fn __repr__(&self) -> String {
        format!("Theme({:?})", self.handle.name())
    }
}

/// One token from [`Tokens`] iteration.
#[pyclass(frozen, get_all, module = "syntaxmate")]
struct Token {
    /// Start offset in the buffer's unit.
    start: u32,
    /// End offset (exclusive) in the buffer's unit.
    end: u32,
    /// Resolved style.
    style: Py<Style>,
    /// Scope stack, outermost first, or `None` when scopes were not requested.
    scopes: Option<Py<PyTuple>>,
}

#[pymethods]
impl Token {
    fn __repr__(&self, py: Python<'_>) -> String {
        let scopes = self
            .scopes
            .as_ref()
            .map(|scopes| format!(", scopes={}", scopes.bind(py)))
            .unwrap_or_default();
        format!(
            "Token(start={}, end={}, style={}{scopes})",
            self.start,
            self.end,
            self.style.get().__repr__()
        )
    }
}

/// Highlighted tokens as flat arrays.
///
/// Array properties return read-only `memoryview`s of unsigned 32-bit integers
/// (format `"I"`), usable with `numpy.frombuffer` or indexed directly.
#[pyclass(frozen, sequence, module = "syntaxmate")]
struct Tokens {
    buffer: TokenBuffer,
    views: [PyOnceLock<Py<PyAny>>; 6],
    styles: PyOnceLock<Py<PyTuple>>,
    scope_stacks: PyOnceLock<Py<PyTuple>>,
}

impl Tokens {
    fn new(buffer: TokenBuffer) -> Self {
        Self {
            buffer,
            views: std::array::from_fn(|_| PyOnceLock::new()),
            styles: PyOnceLock::new(),
            scope_stacks: PyOnceLock::new(),
        }
    }

    fn view(&self, py: Python<'_>, index: usize) -> PyResult<Py<PyAny>> {
        let b = &self.buffer;
        let data: &[u32] = match index {
            0 => &b.line_starts,
            1 => &b.line_token_ranges,
            2 => &b.token_starts,
            3 => &b.token_lengths,
            4 => &b.token_styles,
            _ => &b.token_scopes,
        };
        self.views[index]
            .get_or_try_init(py, || {
                let bytes = PyBytes::new_with(py, data.len() * 4, |out| {
                    for (chunk, value) in out.as_chunks_mut::<4>().0.iter_mut().zip(data) {
                        *chunk = value.to_ne_bytes();
                    }
                    Ok(())
                })?;
                let view = PyMemoryView::from(bytes.as_any())?.call_method1("cast", ("I",))?;
                Ok(view.unbind())
            })
            .map(|view| view.clone_ref(py))
    }

    fn style_objects<'py>(&self, py: Python<'py>) -> PyResult<&Bound<'py, PyTuple>> {
        self.styles
            .get_or_try_init(py, || {
                let styles = self
                    .buffer
                    .styles
                    .iter()
                    .map(|&packed| Py::new(py, Style { packed }))
                    .collect::<PyResult<Vec<_>>>()?;
                PyTuple::new(py, styles).map(Bound::unbind)
            })
            .map(|tuple| tuple.bind(py))
    }

    fn scope_objects<'py>(&self, py: Python<'py>) -> PyResult<&Bound<'py, PyTuple>> {
        self.scope_stacks
            .get_or_try_init(py, || {
                let stacks = self
                    .buffer
                    .scope_stacks
                    .iter()
                    .map(|stack| PyTuple::new(py, stack))
                    .collect::<PyResult<Vec<_>>>()?;
                PyTuple::new(py, stacks).map(Bound::unbind)
            })
            .map(|tuple| tuple.bind(py))
    }

    fn token(&self, py: Python<'_>, index: usize) -> PyResult<Token> {
        let b = &self.buffer;
        let start = b.token_starts[index];
        let style = self
            .style_objects(py)?
            .get_item(b.token_styles[index] as usize)?
            .cast_into::<Style>()?
            .unbind();
        let scopes = match b.token_scopes.get(index) {
            Some(&stack) => Some(
                self.scope_objects(py)?
                    .get_item(stack as usize)?
                    .cast_into::<PyTuple>()?
                    .unbind(),
            ),
            None => None,
        };
        Ok(Token {
            start,
            end: start + b.token_lengths[index],
            style,
            scopes,
        })
    }
}

#[pymethods]
impl Tokens {
    /// Offset unit: `"codepoint"`, `"utf8"`, or `"utf16"`.
    #[getter]
    fn unit(&self) -> &'static str {
        unit_name(self.buffer.unit)
    }

    /// Whether tokenization finished within resource limits.
    #[getter]
    fn complete(&self) -> bool {
        self.buffer.complete
    }

    /// The theme's default style, for text not covered by a token.
    #[getter]
    fn default_style(&self) -> Style {
        Style {
            packed: self.buffer.default_style,
        }
    }

    /// Offset of each `\n`-separated line's first character.
    #[getter]
    fn line_starts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, 0)
    }

    /// Tokens of line `l` are `line_token_ranges[l]:line_token_ranges[l + 1]`.
    #[getter]
    fn line_token_ranges(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, 1)
    }

    /// Token start offsets.
    #[getter]
    fn starts(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, 2)
    }

    /// Token lengths.
    #[getter]
    fn lengths(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, 3)
    }

    /// Index into `styles` per token.
    #[getter]
    fn style_ids(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, 4)
    }

    /// Index into `scope_stacks` per token; empty unless scopes were requested.
    #[getter]
    fn scope_ids(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.view(py, 5)
    }

    /// Distinct styles referenced by `style_ids`.
    #[getter]
    fn styles<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.style_objects(py).cloned()
    }

    /// Distinct scope stacks referenced by `scope_ids`, outermost scope first.
    #[getter]
    fn scope_stacks<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        self.scope_objects(py).cloned()
    }

    fn __len__(&self) -> usize {
        self.buffer.token_starts.len()
    }

    fn __getitem__(&self, py: Python<'_>, index: isize) -> PyResult<Token> {
        let len = self.buffer.token_starts.len() as isize;
        let resolved = if index < 0 { index + len } else { index };
        if !(0..len).contains(&resolved) {
            return Err(PyIndexError::new_err("token index out of range"));
        }
        self.token(py, resolved as usize)
    }

    fn __iter__(slf: Bound<'_, Self>) -> TokenIter {
        TokenIter {
            tokens: slf.unbind(),
            next: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "<Tokens tokens={} lines={} unit={:?} complete={}>",
            self.buffer.token_starts.len(),
            self.buffer.line_starts.len(),
            unit_name(self.buffer.unit),
            if self.buffer.complete {
                "True"
            } else {
                "False"
            }
        )
    }
}

/// Iterator over [`Token`]s.
#[pyclass(frozen, module = "syntaxmate")]
struct TokenIter {
    tokens: Py<Tokens>,
    next: std::sync::atomic::AtomicUsize,
}

#[pymethods]
impl TokenIter {
    fn __iter__(slf: Bound<'_, Self>) -> Bound<'_, Self> {
        slf
    }

    fn __next__(&self, py: Python<'_>) -> PyResult<Option<Token>> {
        use std::sync::atomic::Ordering;
        let tokens = self.tokens.get();
        let index = self.next.fetch_add(1, Ordering::Relaxed);
        if index >= tokens.buffer.token_starts.len() {
            self.next
                .store(tokens.buffer.token_starts.len(), Ordering::Relaxed);
            return Ok(None);
        }
        tokens.token(py, index).map(Some)
    }
}

/// Incremental highlighter: feed logical lines in order, without terminators.
#[pyclass(frozen, module = "syntaxmate")]
struct Session {
    inner: Mutex<boundary::Session>,
}

#[pymethods]
impl Session {
    /// Highlights the next line; offsets in the result are line-relative.
    fn line(&self, py: Python<'_>, text: PyBackedStr) -> PyResult<Tokens> {
        let buffer = detached(py, || {
            let mut session = self.inner.lock().unwrap_or_else(PoisonError::into_inner);
            session.line(&text)
        })?;
        Ok(Tokens::new(buffer))
    }

    /// Returns to the start-of-document state, keeping caches.
    fn reset(&self, py: Python<'_>) {
        py.detach(|| {
            self.inner
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .reset();
        });
    }
}

/// Thread-safe highlighter over one grammar catalog.
#[pyclass(frozen, module = "syntaxmate")]
struct Highlighter {
    engine: Engine,
}

#[pymethods]
impl Highlighter {
    /// Creates a highlighter over the embedded grammar catalog.
    #[new]
    fn new(py: Python<'_>) -> PyResult<Self> {
        #[cfg(feature = "bundled-grammars")]
        {
            let engine = detached(py, Engine::bundled)?;
            Ok(Self { engine })
        }
        #[cfg(not(feature = "bundled-grammars"))]
        {
            Err(py_error(
                py,
                BoundaryError::new(
                    ErrorKind::InvalidInput,
                    "built without embedded grammars; use Highlighter.from_bundle",
                ),
            ))
        }
    }

    /// Creates a highlighter from grammar-bundle bytes.
    #[staticmethod]
    fn from_bundle(py: Python<'_>, data: PyBackedBytes) -> PyResult<Self> {
        let engine = detached(py, || Engine::from_bundle(&data))?;
        Ok(Self { engine })
    }

    /// Canonical language IDs.
    fn languages(&self) -> Vec<String> {
        self.engine.languages()
    }

    /// Bundled theme names.
    fn themes(&self) -> Vec<String> {
        self.engine.themes()
    }

    /// Resolves an ID or alias to its canonical language ID.
    fn canonical_language(&self, language: &str) -> Option<String> {
        self.engine.canonical_language(language)
    }

    /// Detects a language from an optional path and the source's first line.
    #[pyo3(signature = (source = "", *, path = None))]
    fn detect(&self, source: &str, path: Option<std::path::PathBuf>) -> Option<String> {
        let path = path.map(|path| path.to_string_lossy().into_owned());
        self.engine.detect(path.as_deref(), source)
    }

    /// Highlights `code` to escaped HTML.
    #[pyo3(signature = (
        code, language, theme = None, *,
        include_wrapper = true, wrapper_class = Some("syntaxmate".to_owned()),
        include_scopes = false, class_prefix = None,
    ))]
    #[pyo3(
        text_signature = "(self, code, language, theme=None, *, include_wrapper=True, \
        wrapper_class='syntaxmate', include_scopes=False, class_prefix=None)"
    )]
    #[allow(clippy::too_many_arguments)]
    fn html(
        &self,
        py: Python<'_>,
        code: PyBackedStr,
        language: PyBackedStr,
        theme: Option<&Bound<'_, PyAny>>,
        include_wrapper: bool,
        wrapper_class: Option<String>,
        include_scopes: bool,
        class_prefix: Option<String>,
    ) -> PyResult<String> {
        let theme = theme_or_default(py, theme)?;
        let options = HtmlOptions {
            include_wrapper,
            class: wrapper_class,
            include_scopes,
            class_prefix,
        };
        detached(py, || {
            self.engine
                .html(&language, &code, &theme, &options)
                .map(|out| out.text)
        })
    }

    /// Highlights `code` to 24-bit ANSI terminal text.
    #[pyo3(signature = (
        code, language, theme = None, *,
        colors = true, sanitize_control_characters = true, include_default_background = false,
    ))]
    #[allow(clippy::too_many_arguments)]
    fn ansi(
        &self,
        py: Python<'_>,
        code: PyBackedStr,
        language: PyBackedStr,
        theme: Option<&Bound<'_, PyAny>>,
        colors: bool,
        sanitize_control_characters: bool,
        include_default_background: bool,
    ) -> PyResult<String> {
        let theme = theme_or_default(py, theme)?;
        let options = AnsiOptions {
            colors,
            sanitize_control_characters,
            include_default_background,
        };
        detached(py, || {
            self.engine
                .ansi(&language, &code, &theme, &options)
                .map(|out| out.text)
        })
    }

    /// Highlights `code` into flat token arrays.
    #[pyo3(signature = (code, language, theme = None, *, include_scopes = false, unit = "codepoint"))]
    fn tokens(
        &self,
        py: Python<'_>,
        code: PyBackedStr,
        language: PyBackedStr,
        theme: Option<&Bound<'_, PyAny>>,
        include_scopes: bool,
        unit: &str,
    ) -> PyResult<Tokens> {
        let theme = theme_or_default(py, theme)?;
        let options = TokenOptions {
            unit: parse_unit(unit)?,
            include_scopes,
        };
        let buffer = detached(py, || self.engine.tokens(&language, &code, &theme, options))?;
        Ok(Tokens::new(buffer))
    }

    /// Starts an incremental session that highlights one line per call.
    #[pyo3(signature = (language, theme = None, *, include_scopes = false, unit = "codepoint"))]
    fn session(
        &self,
        py: Python<'_>,
        language: &str,
        theme: Option<&Bound<'_, PyAny>>,
        include_scopes: bool,
        unit: &str,
    ) -> PyResult<Session> {
        let theme = theme_or_default(py, theme)?;
        let options = TokenOptions {
            unit: parse_unit(unit)?,
            include_scopes,
        };
        let session = detached(py, || self.engine.session(language, &theme, options))?;
        Ok(Session {
            inner: Mutex::new(session),
        })
    }

    fn __repr__(&self) -> String {
        format!("<Highlighter languages={}>", self.engine.languages().len())
    }
}

fn theme_or_default(py: Python<'_>, theme: Option<&Bound<'_, PyAny>>) -> PyResult<ThemeHandle> {
    match theme {
        Some(theme) => resolve_theme(theme),
        None => ThemeHandle::bundled(DEFAULT_THEME).map_err(|error| py_error(py, error)),
    }
}

#[pymodule]
mod _native {
    #[pymodule_export]
    use super::{Highlighter, Session, Style, Theme, Token, Tokens};

    #[pymodule_export]
    const BOLD: u8 = syntaxmate_boundary::BOLD;
    #[pymodule_export]
    const ITALIC: u8 = syntaxmate_boundary::ITALIC;
    #[pymodule_export]
    const UNDERLINE: u8 = syntaxmate_boundary::UNDERLINE;
    #[pymodule_export]
    const STRIKETHROUGH: u8 = syntaxmate_boundary::STRIKETHROUGH;
    #[pymodule_export]
    const DEFAULT_THEME: &str = super::DEFAULT_THEME;
    #[pymodule_export]
    const VERSION: &str = syntaxmate_boundary::VERSION;
}
