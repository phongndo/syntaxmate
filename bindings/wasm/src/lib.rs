//! WebAssembly exports behind the `syntaxmate` npm package.
//!
//! These are low-level handles over [`syntaxmate_boundary`]; `lib/core.js`
//! wraps them in the public JavaScript API. Token offsets are always UTF-16
//! code units, the unit JavaScript strings index by.

use std::{cell::RefCell, collections::HashMap};

use js_sys::{Array, Function, Reflect};
use syntaxmate_boundary::{
    AnsiOptions, BoundaryError, Engine, HtmlOptions, OffsetUnit, PackedStyle, Session, ThemeHandle,
    TokenBuffer, TokenOptions,
};
use wasm_bindgen::prelude::*;

thread_local! {
    static ERROR_CLASS: RefCell<Option<Function>> = const { RefCell::new(None) };
}

/// Registers the constructor used for thrown errors: `new Class(message, code)`.
#[wasm_bindgen(js_name = setErrorClass)]
pub fn set_error_class(class: Function) {
    ERROR_CLASS.with(|slot| *slot.borrow_mut() = Some(class));
}

/// Returns the version of the underlying engine.
#[wasm_bindgen]
pub fn version() -> String {
    syntaxmate_boundary::VERSION.to_owned()
}

fn to_js(error: BoundaryError) -> JsValue {
    let message = JsValue::from_str(&error.message);
    let code = JsValue::from(error.kind as u32);
    ERROR_CLASS.with(|slot| match &*slot.borrow() {
        Some(class) => {
            Reflect::construct(class, &Array::of2(&message, &code)).unwrap_or_else(|e| e)
        }
        None => {
            let fallback = js_sys::Error::new(&error.message);
            // Setting a property on a fresh Error object cannot fail.
            let _ = Reflect::set(&fallback, &"code".into(), &code);
            fallback.into()
        }
    })
}

fn options(include_scopes: bool) -> TokenOptions {
    TokenOptions {
        unit: OffsetUnit::Utf16,
        include_scopes,
    }
}

fn style(style: PackedStyle) -> [u32; 3] {
    [
        style.foreground,
        style.background,
        u32::from(style.modifiers),
    ]
}

/// A grammar catalog loaded from bundle bytes.
#[wasm_bindgen(js_name = RawEngine)]
pub struct RawEngine(Engine);

#[wasm_bindgen(js_class = RawEngine)]
impl RawEngine {
    /// Decodes a grammar bundle; the bytes are copied.
    #[wasm_bindgen(js_name = fromBundle)]
    pub fn from_bundle(bytes: &[u8]) -> Result<RawEngine, JsValue> {
        Engine::from_bundle(bytes).map(Self).map_err(to_js)
    }

    /// Lists canonical language IDs.
    pub fn languages(&self) -> Vec<String> {
        self.0.languages()
    }

    /// Lists bundled theme names.
    pub fn themes(&self) -> Vec<String> {
        self.0.themes()
    }

    /// Resolves an ID or alias to its canonical language ID.
    #[wasm_bindgen(js_name = canonicalLanguage)]
    pub fn canonical_language(&self, language: &str) -> Option<String> {
        self.0.canonical_language(language)
    }

    /// Detects a language from an optional path and the source's first line.
    pub fn detect(&self, path: Option<String>, source: &str) -> Option<String> {
        self.0.detect(path.as_deref(), source)
    }

    /// Renders escaped HTML.
    #[allow(clippy::too_many_arguments)]
    pub fn html(
        &self,
        language: &str,
        source: &str,
        theme: &RawTheme,
        include_wrapper: bool,
        class: Option<String>,
        include_scopes: bool,
        class_prefix: Option<String>,
    ) -> Result<String, JsValue> {
        let options = HtmlOptions {
            include_wrapper,
            class,
            include_scopes,
            class_prefix,
        };
        self.0
            .html(language, source, &theme.0, &options)
            .map(|output| output.text)
            .map_err(to_js)
    }

    /// Renders 24-bit ANSI text.
    pub fn ansi(
        &self,
        language: &str,
        source: &str,
        theme: &RawTheme,
        colors: bool,
        sanitize_control_characters: bool,
        include_default_background: bool,
    ) -> Result<String, JsValue> {
        let options = AnsiOptions {
            colors,
            sanitize_control_characters,
            include_default_background,
        };
        self.0
            .ansi(language, source, &theme.0, &options)
            .map(|output| output.text)
            .map_err(to_js)
    }

    /// Highlights a document into a packed UTF-16 token buffer (see [`pack`]).
    pub fn tokens(
        &self,
        language: &str,
        source: &str,
        theme: &RawTheme,
        include_scopes: bool,
        names: &Array,
    ) -> Result<Vec<u32>, JsValue> {
        self.0
            .tokens(language, source, &theme.0, options(include_scopes))
            .map(|buffer| pack(buffer, names))
            .map_err(to_js)
    }

    /// Starts an incremental session.
    pub fn session(
        &self,
        language: &str,
        theme: &RawTheme,
        include_scopes: bool,
    ) -> Result<RawSession, JsValue> {
        self.0
            .session(language, &theme.0, options(include_scopes))
            .map(RawSession)
            .map_err(to_js)
    }
}

/// A bundled or custom theme.
#[wasm_bindgen(js_name = RawTheme)]
pub struct RawTheme(ThemeHandle);

#[wasm_bindgen(js_class = RawTheme)]
impl RawTheme {
    /// Loads a bundled theme by name.
    pub fn bundled(name: &str) -> Result<RawTheme, JsValue> {
        ThemeHandle::bundled(name).map(Self).map_err(to_js)
    }

    /// Parses a TextMate JSON theme.
    #[wasm_bindgen(js_name = fromJson)]
    pub fn from_json(json: &str) -> Result<RawTheme, JsValue> {
        ThemeHandle::from_json(json).map(Self).map_err(to_js)
    }

    /// Returns the theme's name.
    pub fn name(&self) -> String {
        self.0.name().to_owned()
    }

    /// Returns `[foreground, background, modifiers]`.
    #[wasm_bindgen(js_name = defaultStyle)]
    pub fn default_style(&self) -> Vec<u32> {
        style(self.0.default_style()).to_vec()
    }

    /// Returns CSS for class-mode HTML rendered with `class_prefix`.
    pub fn stylesheet(&self, class_prefix: &str) -> String {
        self.0.stylesheet(class_prefix)
    }
}

/// Incremental line-at-a-time highlighting.
#[wasm_bindgen(js_name = RawSession)]
pub struct RawSession(Session);

#[wasm_bindgen(js_class = RawSession)]
impl RawSession {
    /// Highlights the next line (without its terminator) into a packed buffer.
    pub fn line(&mut self, line: &str, names: &Array) -> Result<Vec<u32>, JsValue> {
        self.0
            .line(line)
            .map(|buffer| pack(buffer, names))
            .map_err(to_js)
    }

    /// Returns to the start-of-document state.
    pub fn reset(&mut self) {
        self.0.reset();
    }
}

/// Number of `u32` header fields at the start of a packed token buffer.
const HEADER: usize = 6;

/// Packs `buffer` into one array so the JS wrapper copies it out in a single
/// call; `TokenBuffer` in `lib/core.js` decodes this layout:
///
/// ```text
/// header       complete, lines, tokens, scoped tokens, styles, stacks
/// lineStarts         [lines]
/// lineTokenRanges    [lines + 1]
/// tokenStarts        [tokens]
/// tokenLengths       [tokens]
/// tokenStyles        [tokens]
/// tokenScopes        [scoped tokens]
/// styles             [styles * 3], then defaultStyle [3]
/// per stack          length, then that many indices into `names`
/// ```
///
/// Scope names are deduplicated and appended to `names`.
fn pack(buffer: TokenBuffer, names: &Array) -> Vec<u32> {
    let TokenBuffer {
        complete,
        line_starts,
        line_token_ranges,
        token_starts,
        token_lengths,
        token_styles,
        token_scopes,
        styles,
        scope_stacks,
        default_style,
        ..
    } = buffer;
    let stack_words: usize = scope_stacks.iter().map(|stack| stack.len() + 1).sum();
    let mut out = Vec::with_capacity(
        HEADER
            + line_starts.len()
            + line_token_ranges.len()
            + 3 * token_starts.len()
            + token_scopes.len()
            + 3 * (styles.len() + 1)
            + stack_words,
    );
    // Lengths are bounded by the boundary's u32 offset limit.
    out.extend([
        u32::from(complete),
        line_starts.len() as u32,
        token_starts.len() as u32,
        token_scopes.len() as u32,
        styles.len() as u32,
        scope_stacks.len() as u32,
    ]);
    for array in [
        &line_starts,
        &line_token_ranges,
        &token_starts,
        &token_lengths,
        &token_styles,
        &token_scopes,
    ] {
        out.extend_from_slice(array);
    }
    for packed in styles.iter().chain([&default_style]) {
        out.extend(style(*packed));
    }
    let mut seen = HashMap::<&str, u32>::new();
    for stack in &scope_stacks {
        out.push(stack.len() as u32);
        for name in stack {
            let index = *seen.entry(name).or_insert_with(|| {
                names.push(&JsValue::from_str(name));
                names.length() - 1
            });
            out.push(index);
        }
    }
    out
}
