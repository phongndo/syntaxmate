//! WebAssembly exports behind the `syntaxmate` npm package.
//!
//! These are low-level handles over [`syntaxmate_boundary`]; `lib/core.js`
//! wraps them in the public JavaScript API. Token offsets are always UTF-16
//! code units, the unit JavaScript strings index by.

use std::cell::RefCell;

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

    /// Highlights a document into UTF-16 token arrays.
    pub fn tokens(
        &self,
        language: &str,
        source: &str,
        theme: &RawTheme,
        include_scopes: bool,
    ) -> Result<RawTokens, JsValue> {
        self.0
            .tokens(language, source, &theme.0, options(include_scopes))
            .map(RawTokens)
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
    /// Highlights the next line (without its terminator).
    pub fn line(&mut self, line: &str) -> Result<RawTokens, JsValue> {
        self.0.line(line).map(RawTokens).map_err(to_js)
    }

    /// Returns to the start-of-document state.
    pub fn reset(&mut self) {
        self.0.reset();
    }
}

/// A token buffer whose arrays are moved out once each by the JS wrapper.
#[wasm_bindgen(js_name = RawTokens)]
pub struct RawTokens(TokenBuffer);

#[wasm_bindgen(js_class = RawTokens)]
impl RawTokens {
    /// Whether tokenization finished within resource limits.
    pub fn complete(&self) -> bool {
        self.0.complete
    }

    /// Takes the line start offsets.
    #[wasm_bindgen(js_name = takeLineStarts)]
    pub fn take_line_starts(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.0.line_starts)
    }

    /// Takes the per-line token index boundaries.
    #[wasm_bindgen(js_name = takeLineTokenRanges)]
    pub fn take_line_token_ranges(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.0.line_token_ranges)
    }

    /// Takes the token start offsets.
    #[wasm_bindgen(js_name = takeTokenStarts)]
    pub fn take_token_starts(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.0.token_starts)
    }

    /// Takes the token lengths.
    #[wasm_bindgen(js_name = takeTokenLengths)]
    pub fn take_token_lengths(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.0.token_lengths)
    }

    /// Takes the per-token style indices.
    #[wasm_bindgen(js_name = takeTokenStyles)]
    pub fn take_token_styles(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.0.token_styles)
    }

    /// Takes the per-token scope-stack indices.
    #[wasm_bindgen(js_name = takeTokenScopes)]
    pub fn take_token_scopes(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.0.token_scopes)
    }

    /// Returns the style table flattened as `[foreground, background, modifiers]` triples.
    pub fn styles(&self) -> Vec<u32> {
        self.0.styles.iter().flat_map(|s| style(*s)).collect()
    }

    /// Returns `[foreground, background, modifiers]` for uncovered text.
    #[wasm_bindgen(js_name = defaultStyle)]
    pub fn default_style(&self) -> Vec<u32> {
        style(self.0.default_style).to_vec()
    }

    /// Returns the number of scopes in each stack.
    #[wasm_bindgen(js_name = scopeStackLengths)]
    pub fn scope_stack_lengths(&self) -> Vec<u32> {
        self.0
            .scope_stacks
            .iter()
            .map(|stack| stack.len() as u32)
            .collect()
    }

    /// Takes every stack's scopes, concatenated outermost first.
    #[wasm_bindgen(js_name = takeScopeNames)]
    pub fn take_scope_names(&mut self) -> Vec<String> {
        std::mem::take(&mut self.0.scope_stacks)
            .into_iter()
            .flatten()
            .collect()
    }
}
