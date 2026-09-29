//! C ABI for Syntaxmate over [`syntaxmate_boundary`].
//!
//! `include/syntaxmate.h` is generated from this file by cbindgen; the doc
//! comments below are the C documentation.
//!
//! Every pointer argument is checked for null, every text input for UTF-8, and
//! every status-returning function runs inside `catch_unwind`, so a Rust panic
//! becomes `SM_INTERNAL` instead of unwinding into C.

#![allow(non_camel_case_types)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::{
    cell::RefCell,
    ffi::{CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    ptr,
};

use syntaxmate_boundary::{
    self as boundary, AnsiOptions, BoundaryError, Engine, ErrorKind, HtmlOptions, OffsetUnit,
    PackedStyle, Session, ThemeHandle, TokenBuffer, TokenOptions,
};

/// Result of every fallible `sm_` function. `SM_OK` is zero; other values
/// are stable and never renumbered. Read `sm_last_error_message` for detail.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum sm_status {
    /// Success.
    SM_OK = 0,
    /// The language ID or alias is not in the catalog.
    SM_UNKNOWN_LANGUAGE = 1,
    /// The bundled theme name is unknown.
    SM_UNKNOWN_THEME = 2,
    /// A grammar could not be parsed or prepared.
    SM_INVALID_GRAMMAR = 3,
    /// A custom theme could not be parsed.
    SM_INVALID_THEME = 4,
    /// A grammar bundle could not be decoded.
    SM_INVALID_BUNDLE = 5,
    /// A null pointer, invalid UTF-8, an out-of-range value, or other rejected input.
    SM_INVALID_INPUT = 6,
    /// Output rendering failed.
    SM_RENDER = 7,
    /// An unexpected internal failure, including a caught Rust panic.
    SM_INTERNAL = 8,
}

const _: () = {
    use sm_status::*;
    assert!(ErrorKind::UnknownLanguage as u32 == SM_UNKNOWN_LANGUAGE as u32);
    assert!(ErrorKind::UnknownTheme as u32 == SM_UNKNOWN_THEME as u32);
    assert!(ErrorKind::InvalidGrammar as u32 == SM_INVALID_GRAMMAR as u32);
    assert!(ErrorKind::InvalidTheme as u32 == SM_INVALID_THEME as u32);
    assert!(ErrorKind::InvalidBundle as u32 == SM_INVALID_BUNDLE as u32);
    assert!(ErrorKind::InvalidInput as u32 == SM_INVALID_INPUT as u32);
    assert!(ErrorKind::Render as u32 == SM_RENDER as u32);
    assert!(ErrorKind::Internal as u32 == SM_INTERNAL as u32);
    assert!(boundary::BOLD as u32 == SM_BOLD);
    assert!(boundary::ITALIC as u32 == SM_ITALIC);
    assert!(boundary::UNDERLINE as u32 == SM_UNDERLINE);
    assert!(boundary::STRIKETHROUGH as u32 == SM_STRIKETHROUGH);
    assert!(boundary::NO_COLOR == SM_NO_COLOR);
};

// Backs the documented thread-safety of `sm_engine` and `sm_theme`.
const _: fn() = || {
    fn shareable<T: Send + Sync>() {}
    shareable::<Engine>();
    shareable::<ThemeHandle>();
};

/// Unit of every offset and length in a token buffer.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum sm_offset_unit {
    /// UTF-8 bytes (the default for C and C++).
    SM_OFFSET_UTF8 = 0,
    /// UTF-16 code units.
    SM_OFFSET_UTF16 = 1,
    /// Unicode scalar values.
    SM_OFFSET_CODE_POINT = 2,
}

/// Value of `sm_style.foreground` / `sm_style.background` when the color is absent.
pub const SM_NO_COLOR: u32 = u32::MAX;
/// Bold bit in `sm_style.modifiers`.
pub const SM_BOLD: u32 = 1;
/// Italic bit in `sm_style.modifiers`.
pub const SM_ITALIC: u32 = 2;
/// Underline bit in `sm_style.modifiers`.
pub const SM_UNDERLINE: u32 = 4;
/// Strikethrough bit in `sm_style.modifiers`.
pub const SM_STRIKETHROUGH: u32 = 8;

/// A resolved style.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct sm_style {
    /// `0xRRGGBB`, or `SM_NO_COLOR`.
    pub foreground: u32,
    /// `0xRRGGBB`, or `SM_NO_COLOR`.
    pub background: u32,
    /// Bitset of `SM_BOLD`, `SM_ITALIC`, `SM_UNDERLINE`, `SM_STRIKETHROUGH`.
    pub modifiers: u32,
}

impl From<PackedStyle> for sm_style {
    fn from(style: PackedStyle) -> Self {
        Self {
            foreground: style.foreground,
            background: style.background,
            modifiers: u32::from(style.modifiers),
        }
    }
}

/// A borrowed UTF-8 string: `len` bytes at `ptr`, followed by a NUL byte.
/// `ptr` is NULL only where a function documents an absent value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct sm_str {
    /// First byte; `ptr[len]` is NUL.
    pub ptr: *const c_char,
    /// Length in bytes, excluding the NUL.
    pub len: usize,
}

const NULL_STR: sm_str = sm_str {
    ptr: ptr::null(),
    len: 0,
};

/// Options for `sm_engine_tokens` and `sm_engine_session`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct sm_token_options {
    /// An `sm_offset_unit` value; others fail with `SM_INVALID_INPUT`.
    pub unit: u32,
    /// Fill `sm_token_view.token_scopes` and the scope stacks.
    pub include_scopes: bool,
}

/// Options for `sm_engine_html`. Strings are (pointer, length) pairs; a NULL
/// pointer means "absent".
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct sm_html_options {
    /// Wrap output in `<pre><code>…</code></pre>`.
    pub include_wrapper: bool,
    /// Add a `data-scopes` attribute with each token's scope stack.
    pub include_scopes: bool,
    /// Class on the `<pre>` wrapper (default `"syntaxmate"`); NULL for none.
    pub wrapper_class: *const c_char,
    /// Length of `wrapper_class` in bytes.
    pub wrapper_class_len: usize,
    /// Emit scope classes with this prefix instead of inline styles; NULL for
    /// inline styles. Pair with `sm_theme_stylesheet` using the same prefix.
    pub class_prefix: *const c_char,
    /// Length of `class_prefix` in bytes.
    pub class_prefix_len: usize,
}

/// Options for `sm_engine_ansi`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct sm_ansi_options {
    /// Emit 24-bit color and modifier SGR sequences.
    pub colors: bool,
    /// Replace control characters in the source with visible control pictures.
    /// Keep enabled for untrusted input.
    pub sanitize_control_characters: bool,
    /// Paint the theme's default background.
    pub include_default_background: bool,
}

/// Borrowed flat arrays of an `sm_tokens` buffer, valid until it is freed.
///
/// Lines split on `\n` only. Token `i` covers
/// `[token_starts[i], token_starts[i] + token_lengths[i])` in `unit`s of the
/// whole input (a session buffer covers one line). Tokens of line `l` are
/// indices `[line_token_ranges[l], line_token_ranges[l + 1])`. Text outside
/// every token uses `default_style`. Array pointers are non-NULL even when
/// their count is zero, except `token_scopes`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct sm_token_view {
    /// The `sm_offset_unit` of every offset and length.
    pub unit: u32,
    /// False if tokenization stopped at a resource limit.
    pub complete: bool,
    /// Number of logical lines.
    pub line_count: usize,
    /// `line_count` offsets of each line's first character.
    pub line_starts: *const u32,
    /// `line_count + 1` token index boundaries.
    pub line_token_ranges: *const u32,
    /// Number of tokens.
    pub token_count: usize,
    /// `token_count` start offsets.
    pub token_starts: *const u32,
    /// `token_count` lengths.
    pub token_lengths: *const u32,
    /// `token_count` indices into `styles`.
    pub token_styles: *const u32,
    /// `token_count` indices into the scope stacks; NULL when scopes were not
    /// requested or there are no tokens.
    pub token_scopes: *const u32,
    /// Number of distinct styles.
    pub style_count: usize,
    /// `style_count` styles.
    pub styles: *const sm_style,
    /// Number of distinct scope stacks; read them with `sm_tokens_scope_stack`.
    pub scope_stack_count: usize,
    /// The theme's default foreground and background.
    pub default_style: sm_style,
}

/// Owned text with a trailing NUL that `len` excludes.
struct CText(Box<str>);

impl CText {
    fn new(mut text: String) -> Self {
        text.push('\0');
        Self(text.into_boxed_str())
    }

    fn as_str(&self) -> sm_str {
        sm_str {
            ptr: self.0.as_ptr().cast(),
            len: self.0.len() - 1,
        }
    }
}

/// A grammar catalog and highlighter. Immutable and safe to share across
/// threads; free once with `sm_engine_free` after all users finish.
pub struct sm_engine(Engine);

/// A bundled or custom theme. Immutable and safe to share across threads.
pub struct sm_theme {
    theme: ThemeHandle,
    name: CText,
}

/// An incremental highlighting session. Not thread-safe: use one session
/// from one thread at a time. It does not borrow the engine or theme it was
/// created from, which may be freed first.
pub struct sm_session(Session);

/// An owned token buffer; read it with `sm_tokens_view`.
pub struct sm_tokens {
    buffer: TokenBuffer,
    styles: Vec<sm_style>,
    // Every scope name, each followed by a NUL. `scope_names` points into it;
    // the heap text never moves or changes after construction.
    _scope_text: Box<str>,
    // The names of every stack, concatenated; stack `i` is
    // `scope_names[scope_stack_starts[i]..scope_stack_starts[i + 1]]`.
    // `scope_stack_starts` is empty when there are no stacks.
    scope_names: Vec<sm_str>,
    scope_stack_starts: Vec<usize>,
}

/// An owned UTF-8 string. It may contain interior NUL bytes (for example
/// ANSI output with sanitization disabled), so use `sm_string_len`.
pub struct sm_string(CText);

/// An owned list of strings.
pub struct sm_string_list {
    _owned: Vec<CText>,
    items: Vec<sm_str>,
}

impl sm_string_list {
    fn new(strings: Vec<String>) -> Self {
        let owned: Vec<CText> = strings.into_iter().map(CText::new).collect();
        let items = owned.iter().map(CText::as_str).collect();
        Self {
            _owned: owned,
            items,
        }
    }
}

impl sm_tokens {
    fn new(mut buffer: TokenBuffer) -> Self {
        // The C style table replaces this storage; don't retain the emptied
        // source allocation for the lifetime of the returned token handle.
        let styles = std::mem::take(&mut buffer.styles)
            .into_iter()
            .map(sm_style::from)
            .collect();
        // One text allocation for all names instead of one per name.
        let stacks = std::mem::take(&mut buffer.scope_stacks);
        let mut text =
            String::with_capacity(stacks.iter().flatten().map(|name| name.len() + 1).sum());
        let mut spans = Vec::with_capacity(stacks.iter().map(Vec::len).sum());
        // Stays empty (no allocation) when scopes were not requested.
        let mut scope_stack_starts = Vec::new();
        if !stacks.is_empty() {
            scope_stack_starts.reserve_exact(stacks.len() + 1);
            scope_stack_starts.push(0);
        }
        for stack in &stacks {
            for name in stack {
                spans.push((text.len(), name.len()));
                text.push_str(name);
                text.push('\0');
            }
            scope_stack_starts.push(spans.len());
        }
        drop(stacks);
        let text = text.into_boxed_str();
        let scope_names = spans
            .into_iter()
            .map(|(start, len)| sm_str {
                // In bounds: `start + len` is the NUL that follows the name.
                ptr: text[start..].as_ptr().cast(),
                len,
            })
            .collect();
        Self {
            buffer,
            styles,
            _scope_text: text,
            scope_names,
            scope_stack_starts,
        }
    }

    fn scope_stack_count(&self) -> usize {
        self.scope_stack_starts.len().saturating_sub(1)
    }

    fn scope_stack(&self, index: usize) -> Option<&[sm_str]> {
        let start = *self.scope_stack_starts.get(index)?;
        let end = *self.scope_stack_starts.get(index + 1)?;
        Some(&self.scope_names[start..end])
    }
}

// ---------------------------------------------------------------------------
// Errors and the panic guard

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(message: Option<&str>) {
    let message = message
        .map(|text| CString::new(text.replace('\0', "\u{fffd}")).expect("NUL bytes were replaced"));
    // Ignore access during thread-local teardown.
    let _ = LAST_ERROR.try_with(|slot| *slot.borrow_mut() = message);
}

type Result<T> = std::result::Result<T, BoundaryError>;

fn invalid(message: impl Into<String>) -> BoundaryError {
    BoundaryError::new(ErrorKind::InvalidInput, message)
}

fn status(kind: ErrorKind) -> sm_status {
    use sm_status::*;
    match kind {
        ErrorKind::UnknownLanguage => SM_UNKNOWN_LANGUAGE,
        ErrorKind::UnknownTheme => SM_UNKNOWN_THEME,
        ErrorKind::InvalidGrammar => SM_INVALID_GRAMMAR,
        ErrorKind::InvalidTheme => SM_INVALID_THEME,
        ErrorKind::InvalidBundle => SM_INVALID_BUNDLE,
        ErrorKind::InvalidInput => SM_INVALID_INPUT,
        ErrorKind::Render => SM_RENDER,
        ErrorKind::Internal => SM_INTERNAL,
    }
}

/// Runs `body`, converting errors and panics into a status and last error.
fn guard(body: impl FnOnce() -> Result<()>) -> sm_status {
    set_last_error(None);
    let (code, message) = match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => return sm_status::SM_OK,
        Ok(Err(error)) => (status(error.kind), error.message),
        Err(payload) => {
            let detail = payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "unknown panic payload".to_owned());
            (sm_status::SM_INTERNAL, format!("internal panic: {detail}"))
        }
    };
    set_last_error(Some(&message));
    code
}

/// Frees a boxed handle, containing any panic from its destructor.
///
/// # Safety
/// `handle` is null or came from `Box::into_raw` and is not used again.
unsafe fn free<T>(handle: *mut T) {
    if !handle.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: guaranteed by the caller.
            drop(unsafe { Box::from_raw(handle) });
        }));
    }
}

// ---------------------------------------------------------------------------
// Argument conversion

/// # Safety
/// If non-null, `ptr` must be valid for reads of `len` bytes for `'a`.
unsafe fn bytes<'a>(ptr: *const u8, len: usize, what: &str) -> Result<&'a [u8]> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(invalid(format!("{what} is NULL with nonzero length")));
    }
    if len > isize::MAX as usize {
        return Err(invalid(format!("{what} length is too large")));
    }
    // SAFETY: non-null and valid for `len` bytes per the caller; `len` fits `isize`.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// # Safety
/// As for [`bytes`].
unsafe fn text<'a>(ptr: *const c_char, len: usize, what: &str) -> Result<&'a str> {
    // SAFETY: forwarded from the caller.
    let bytes = unsafe { bytes(ptr.cast(), len, what) }?;
    std::str::from_utf8(bytes).map_err(|_| invalid(format!("{what} is not valid UTF-8")))
}

/// Like [`text`], but a null pointer means `None`.
///
/// # Safety
/// As for [`bytes`].
unsafe fn optional_text<'a>(ptr: *const c_char, len: usize, what: &str) -> Result<Option<&'a str>> {
    if ptr.is_null() {
        if len != 0 {
            return Err(invalid(format!("{what} is NULL with nonzero length")));
        }
        return Ok(None);
    }
    // SAFETY: forwarded from the caller.
    unsafe { text(ptr, len, what) }.map(Some)
}

/// # Safety
/// If non-null, `handle` must point to a live `T` for `'a`.
unsafe fn handle<'a, T>(handle: *const T, what: &str) -> Result<&'a T> {
    // SAFETY: guaranteed by the caller.
    unsafe { handle.as_ref() }.ok_or_else(|| invalid(format!("{what} is NULL")))
}

/// Checks an out-parameter and returns it for a later write.
fn out<T>(out: *mut T, what: &str) -> Result<*mut T> {
    if out.is_null() {
        Err(invalid(format!("{what} is NULL")))
    } else {
        Ok(out)
    }
}

/// Checks a handle out-parameter and clears it so failures leave it NULL.
///
/// # Safety
/// If non-null, `slot` must be valid for writes.
unsafe fn out_handle<T>(slot: *mut *mut T, what: &str) -> Result<*mut *mut T> {
    let slot = out(slot, what)?;
    // SAFETY: non-null and writable per the caller.
    unsafe { slot.write(ptr::null_mut()) };
    Ok(slot)
}

/// # Safety
/// `slot` came from [`out_handle`] and is still writable.
unsafe fn give<T>(slot: *mut *mut T, value: T) {
    // SAFETY: guaranteed by the caller.
    unsafe { slot.write(Box::into_raw(Box::new(value))) };
}

fn token_options(options: Option<&sm_token_options>) -> Result<TokenOptions> {
    let Some(options) = options else {
        return Ok(TokenOptions::default());
    };
    let unit = match options.unit {
        0 => OffsetUnit::Utf8,
        1 => OffsetUnit::Utf16,
        2 => OffsetUnit::CodePoint,
        other => return Err(invalid(format!("unknown offset unit {other}"))),
    };
    Ok(TokenOptions {
        unit,
        include_scopes: options.include_scopes,
    })
}

// ---------------------------------------------------------------------------
// Library

/// Returns the library version as a static NUL-terminated string.
#[unsafe(no_mangle)]
pub extern "C" fn sm_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr().cast()
}

/// Returns the message of the last failed `sm_` call on the calling thread,
/// or NULL if the last status-returning call succeeded. The NUL-terminated
/// message stays valid until the next `sm_` call on this thread that returns
/// `sm_status`. Error detail is thread-local, so concurrent callers never see
/// each other's messages.
#[unsafe(no_mangle)]
pub extern "C" fn sm_last_error_message() -> *const c_char {
    LAST_ERROR
        .try_with(|slot| slot.borrow().as_ref().map_or(ptr::null(), |m| m.as_ptr()))
        .unwrap_or(ptr::null())
}

/// Default token options: UTF-8 offsets, no scopes.
#[unsafe(no_mangle)]
pub extern "C" fn sm_token_options_default() -> sm_token_options {
    sm_token_options {
        unit: sm_offset_unit::SM_OFFSET_UTF8 as u32,
        include_scopes: false,
    }
}

/// Default HTML options: wrapper with class `"syntaxmate"`, inline styles.
#[unsafe(no_mangle)]
pub extern "C" fn sm_html_options_default() -> sm_html_options {
    const CLASS: &str = "syntaxmate\0";
    sm_html_options {
        include_wrapper: true,
        include_scopes: false,
        wrapper_class: CLASS.as_ptr().cast(),
        wrapper_class_len: CLASS.len() - 1,
        class_prefix: ptr::null(),
        class_prefix_len: 0,
    }
}

/// Default ANSI options: colors on, control characters sanitized.
#[unsafe(no_mangle)]
pub extern "C" fn sm_ansi_options_default() -> sm_ansi_options {
    let defaults = AnsiOptions::default();
    sm_ansi_options {
        colors: defaults.colors,
        sanitize_control_characters: defaults.sanitize_control_characters,
        include_default_background: defaults.include_default_background,
    }
}

// ---------------------------------------------------------------------------
// Strings

/// Returns the bytes of `string`, followed by a NUL; NULL if `string` is NULL.
///
/// # Safety
/// `string` is NULL or a live `sm_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_string_data(string: *const sm_string) -> *const c_char {
    // SAFETY: guaranteed by the caller.
    unsafe { string.as_ref() }.map_or(ptr::null(), |s| s.0.as_str().ptr)
}

/// Returns the length of `string` in bytes, excluding the NUL; 0 if NULL.
///
/// # Safety
/// `string` is NULL or a live `sm_string`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_string_len(string: *const sm_string) -> usize {
    // SAFETY: guaranteed by the caller.
    unsafe { string.as_ref() }.map_or(0, |s| s.0.as_str().len)
}

/// Frees a string. NULL is ignored.
///
/// # Safety
/// `string` is NULL or an `sm_string` not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_string_free(string: *mut sm_string) {
    // SAFETY: guaranteed by the caller.
    unsafe { free(string) }
}

/// Returns the number of strings in `list`; 0 if NULL.
///
/// # Safety
/// `list` is NULL or a live `sm_string_list`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_string_list_len(list: *const sm_string_list) -> usize {
    // SAFETY: guaranteed by the caller.
    unsafe { list.as_ref() }.map_or(0, |list| list.items.len())
}

/// Returns string `index` of `list`, borrowed until the list is freed;
/// `{NULL, 0}` if `list` is NULL or `index` is out of range.
///
/// # Safety
/// `list` is NULL or a live `sm_string_list`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_string_list_get(list: *const sm_string_list, index: usize) -> sm_str {
    // SAFETY: guaranteed by the caller.
    unsafe { list.as_ref() }
        .and_then(|list| list.items.get(index).copied())
        .unwrap_or(NULL_STR)
}

/// Frees a string list. NULL is ignored.
///
/// # Safety
/// `list` is NULL or an `sm_string_list` not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_string_list_free(list: *mut sm_string_list) {
    // SAFETY: guaranteed by the caller.
    unsafe { free(list) }
}

// ---------------------------------------------------------------------------
// Engine

/// Creates an engine over the embedded grammar catalog. Fails with
/// `SM_INVALID_INPUT` if the library was built without bundled grammars.
///
/// # Safety
/// `out` is NULL or valid for writes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_bundled(out: *mut *mut sm_engine) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        #[cfg(feature = "bundled-grammars")]
        {
            let engine = Engine::bundled()?;
            // SAFETY: `slot` came from `out_handle`.
            unsafe { give(slot, sm_engine(engine)) };
            Ok(())
        }
        #[cfg(not(feature = "bundled-grammars"))]
        {
            let _ = slot;
            Err(invalid("built without bundled grammars"))
        }
    })
}

/// Creates an engine from grammar-bundle bytes. The bytes are copied.
///
/// # Safety
/// `bytes` is valid for `len` bytes (or `len` is 0); `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_from_bundle(
    bytes: *const u8,
    len: usize,
    out: *mut *mut sm_engine,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let bytes = unsafe { self::bytes(bytes, len, "bundle") }?;
        let engine = Engine::from_bundle(bytes)?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_engine(engine)) };
        Ok(())
    })
}

/// Frees an engine. NULL is ignored. Sessions, themes, and buffers created
/// from it stay valid.
///
/// # Safety
/// `engine` is NULL or an `sm_engine` not yet freed and no longer in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_free(engine: *mut sm_engine) {
    // SAFETY: guaranteed by the caller.
    unsafe { free(engine) }
}

/// Lists canonical language IDs.
///
/// # Safety
/// `engine` is NULL or live; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_languages(
    engine: *const sm_engine,
    out: *mut *mut sm_string_list,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let engine = unsafe { handle(engine, "engine") }?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_string_list::new(engine.0.languages())) };
        Ok(())
    })
}

/// Lists bundled theme names.
///
/// # Safety
/// `engine` is NULL or live; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_themes(
    engine: *const sm_engine,
    out: *mut *mut sm_string_list,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let engine = unsafe { handle(engine, "engine") }?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_string_list::new(engine.0.themes())) };
        Ok(())
    })
}

/// Resolves a language ID or alias. On success `*out` is the canonical ID, or
/// NULL if the language is unknown.
///
/// # Safety
/// `language` is valid for `language_len` bytes; `engine` is NULL or live;
/// `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_canonical_language(
    engine: *const sm_engine,
    language: *const c_char,
    language_len: usize,
    out: *mut *mut sm_string,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let engine = unsafe { handle(engine, "engine") }?;
        // SAFETY: guaranteed by the caller.
        let language = unsafe { text(language, language_len, "language") }?;
        if let Some(id) = engine.0.canonical_language(language) {
            // SAFETY: `slot` came from `out_handle`.
            unsafe { give(slot, sm_string(CText::new(id))) };
        }
        Ok(())
    })
}

/// Detects a language from an optional path (NULL for none) and the source's
/// first line. On success `*out` is the language ID, or NULL if undetected.
///
/// # Safety
/// `path` is NULL or valid for `path_len` bytes; `source` is valid for
/// `source_len` bytes; `engine` is NULL or live; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_detect(
    engine: *const sm_engine,
    path: *const c_char,
    path_len: usize,
    source: *const c_char,
    source_len: usize,
    out: *mut *mut sm_string,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let engine = unsafe { handle(engine, "engine") }?;
        // SAFETY: guaranteed by the caller.
        let path = unsafe { optional_text(path, path_len, "path") }?;
        // SAFETY: guaranteed by the caller.
        let source = unsafe { text(source, source_len, "source") }?;
        if let Some(id) = engine.0.detect(path, source) {
            // SAFETY: `slot` came from `out_handle`.
            unsafe { give(slot, sm_string(CText::new(id))) };
        }
        Ok(())
    })
}

/// Highlights `source` to escaped HTML. `options` may be NULL for defaults.
/// `complete`, if non-NULL, receives false when tokenization stopped at a
/// resource limit.
///
/// # Safety
/// Text pointers are valid for their lengths; handles are NULL or live;
/// `options` is NULL or readable; `out` and `complete` are NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_html(
    engine: *const sm_engine,
    language: *const c_char,
    language_len: usize,
    source: *const c_char,
    source_len: usize,
    theme: *const sm_theme,
    options: *const sm_html_options,
    out: *mut *mut sm_string,
    complete: *mut bool,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let (engine, theme) = unsafe { (handle(engine, "engine")?, handle(theme, "theme")?) };
        // SAFETY: guaranteed by the caller.
        let language = unsafe { text(language, language_len, "language") }?;
        // SAFETY: guaranteed by the caller.
        let source = unsafe { text(source, source_len, "source") }?;
        // SAFETY: guaranteed by the caller.
        let options = match unsafe { options.as_ref() } {
            None => HtmlOptions::default(),
            // SAFETY: the option strings are valid for their lengths per the caller.
            Some(o) => unsafe {
                HtmlOptions {
                    include_wrapper: o.include_wrapper,
                    include_scopes: o.include_scopes,
                    class: optional_text(o.wrapper_class, o.wrapper_class_len, "wrapper_class")?
                        .map(str::to_owned),
                    class_prefix: optional_text(
                        o.class_prefix,
                        o.class_prefix_len,
                        "class_prefix",
                    )?
                    .map(str::to_owned),
                }
            },
        };
        let rendered = engine.0.html(language, source, &theme.theme, &options)?;
        if !complete.is_null() {
            // SAFETY: non-null and writable per the caller.
            unsafe { complete.write(rendered.complete) };
        }
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_string(CText::new(rendered.text))) };
        Ok(())
    })
}

/// Highlights `source` to 24-bit ANSI text. `options` may be NULL for
/// defaults. `complete` is as for `sm_engine_html`.
///
/// # Safety
/// As for `sm_engine_html`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_ansi(
    engine: *const sm_engine,
    language: *const c_char,
    language_len: usize,
    source: *const c_char,
    source_len: usize,
    theme: *const sm_theme,
    options: *const sm_ansi_options,
    out: *mut *mut sm_string,
    complete: *mut bool,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let (engine, theme) = unsafe { (handle(engine, "engine")?, handle(theme, "theme")?) };
        // SAFETY: guaranteed by the caller.
        let language = unsafe { text(language, language_len, "language") }?;
        // SAFETY: guaranteed by the caller.
        let source = unsafe { text(source, source_len, "source") }?;
        // SAFETY: guaranteed by the caller.
        let options =
            unsafe { options.as_ref() }.map_or_else(AnsiOptions::default, |o| AnsiOptions {
                colors: o.colors,
                sanitize_control_characters: o.sanitize_control_characters,
                include_default_background: o.include_default_background,
            });
        let rendered = engine.0.ansi(language, source, &theme.theme, &options)?;
        if !complete.is_null() {
            // SAFETY: non-null and writable per the caller.
            unsafe { complete.write(rendered.complete) };
        }
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_string(CText::new(rendered.text))) };
        Ok(())
    })
}

/// Highlights a whole document into a token buffer. `options` may be NULL
/// for defaults (UTF-8 offsets, no scopes).
///
/// # Safety
/// Text pointers are valid for their lengths; handles are NULL or live;
/// `options` is NULL or readable; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_tokens(
    engine: *const sm_engine,
    language: *const c_char,
    language_len: usize,
    source: *const c_char,
    source_len: usize,
    theme: *const sm_theme,
    options: *const sm_token_options,
    out: *mut *mut sm_tokens,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let (engine, theme) = unsafe { (handle(engine, "engine")?, handle(theme, "theme")?) };
        // SAFETY: guaranteed by the caller.
        let language = unsafe { text(language, language_len, "language") }?;
        // SAFETY: guaranteed by the caller.
        let source = unsafe { text(source, source_len, "source") }?;
        // SAFETY: guaranteed by the caller.
        let options = token_options(unsafe { options.as_ref() })?;
        let buffer = engine.0.tokens(language, source, &theme.theme, options)?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_tokens::new(buffer)) };
        Ok(())
    })
}

/// Starts an incremental session. `options` may be NULL for defaults.
///
/// # Safety
/// `language` is valid for `language_len` bytes; handles are NULL or live;
/// `options` is NULL or readable; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_engine_session(
    engine: *const sm_engine,
    language: *const c_char,
    language_len: usize,
    theme: *const sm_theme,
    options: *const sm_token_options,
    out: *mut *mut sm_session,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let (engine, theme) = unsafe { (handle(engine, "engine")?, handle(theme, "theme")?) };
        // SAFETY: guaranteed by the caller.
        let language = unsafe { text(language, language_len, "language") }?;
        // SAFETY: guaranteed by the caller.
        let options = token_options(unsafe { options.as_ref() })?;
        let session = engine.0.session(language, &theme.theme, options)?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_session(session)) };
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Themes

fn new_theme(theme: ThemeHandle) -> sm_theme {
    sm_theme {
        name: CText::new(theme.name().to_owned()),
        theme,
    }
}

/// Loads a bundled theme by name.
///
/// # Safety
/// `name` is valid for `name_len` bytes; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_theme_bundled(
    name: *const c_char,
    name_len: usize,
    out: *mut *mut sm_theme,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let name = unsafe { text(name, name_len, "name") }?;
        let theme = ThemeHandle::bundled(name)?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, new_theme(theme)) };
        Ok(())
    })
}

/// Parses a TextMate JSON theme.
///
/// # Safety
/// `json` is valid for `json_len` bytes; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_theme_from_json(
    json: *const c_char,
    json_len: usize,
    out: *mut *mut sm_theme,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let json = unsafe { text(json, json_len, "json") }?;
        let theme = ThemeHandle::from_json(json)?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, new_theme(theme)) };
        Ok(())
    })
}

/// Frees a theme. NULL is ignored. Sessions created with it stay valid.
///
/// # Safety
/// `theme` is NULL or an `sm_theme` not yet freed and no longer in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_theme_free(theme: *mut sm_theme) {
    // SAFETY: guaranteed by the caller.
    unsafe { free(theme) }
}

/// Returns the theme's name, borrowed until the theme is freed;
/// `{NULL, 0}` if `theme` is NULL.
///
/// # Safety
/// `theme` is NULL or live.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_theme_name(theme: *const sm_theme) -> sm_str {
    // SAFETY: guaranteed by the caller.
    unsafe { theme.as_ref() }.map_or(NULL_STR, |theme| theme.name.as_str())
}

/// Returns the theme's default foreground and background.
///
/// # Safety
/// `theme` is NULL or live; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_theme_default_style(
    theme: *const sm_theme,
    out: *mut sm_style,
) -> sm_status {
    guard(|| {
        let out = self::out(out, "out")?;
        // SAFETY: guaranteed by the caller.
        let theme = unsafe { handle(theme, "theme") }?;
        // SAFETY: non-null and writable per the caller.
        unsafe { out.write(theme.theme.default_style().into()) };
        Ok(())
    })
}

/// Returns CSS for HTML rendered with `class_prefix` (see `sm_html_options`).
///
/// # Safety
/// `class_prefix` is valid for `class_prefix_len` bytes; `theme` is NULL or
/// live; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_theme_stylesheet(
    theme: *const sm_theme,
    class_prefix: *const c_char,
    class_prefix_len: usize,
    out: *mut *mut sm_string,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: guaranteed by the caller.
        let theme = unsafe { handle(theme, "theme") }?;
        // SAFETY: guaranteed by the caller.
        let prefix = unsafe { text(class_prefix, class_prefix_len, "class_prefix") }?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_string(CText::new(theme.theme.stylesheet(prefix)))) };
        Ok(())
    })
}

// ---------------------------------------------------------------------------
// Sessions

/// Highlights the next logical line, without its `\n` terminator. The
/// buffer holds one line with line-relative offsets. A line containing `\n`
/// fails with `SM_INVALID_INPUT`.
///
/// # Safety
/// `line` is valid for `line_len` bytes; `session` is NULL or live and not in
/// use by another thread; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_session_line(
    session: *mut sm_session,
    line: *const c_char,
    line_len: usize,
    out: *mut *mut sm_tokens,
) -> sm_status {
    guard(|| {
        // SAFETY: guaranteed by the caller.
        let slot = unsafe { out_handle(out, "out") }?;
        // SAFETY: live and exclusively accessed per the caller.
        let session = unsafe { session.as_mut() }.ok_or_else(|| invalid("session is NULL"))?;
        // SAFETY: guaranteed by the caller.
        let line = unsafe { text(line, line_len, "line") }?;
        let buffer = session.0.line(line)?;
        // SAFETY: `slot` came from `out_handle`.
        unsafe { give(slot, sm_tokens::new(buffer)) };
        Ok(())
    })
}

/// Returns the session to the start-of-document state.
///
/// # Safety
/// `session` is NULL or live and not in use by another thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_session_reset(session: *mut sm_session) -> sm_status {
    guard(|| {
        // SAFETY: live and exclusively accessed per the caller.
        let session = unsafe { session.as_mut() }.ok_or_else(|| invalid("session is NULL"))?;
        session.0.reset();
        Ok(())
    })
}

/// Frees a session. NULL is ignored.
///
/// # Safety
/// `session` is NULL or an `sm_session` not yet freed and no longer in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_session_free(session: *mut sm_session) {
    // SAFETY: guaranteed by the caller.
    unsafe { free(session) }
}

// ---------------------------------------------------------------------------
// Token buffers

/// Fills `out` with borrowed pointers into `tokens` (no copy). They stay
/// valid until `sm_tokens_free`. A buffer may be read from several threads.
///
/// # Safety
/// `tokens` is NULL or live; `out` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_tokens_view(
    tokens: *const sm_tokens,
    out: *mut sm_token_view,
) -> sm_status {
    guard(|| {
        let out = self::out(out, "out")?;
        // SAFETY: guaranteed by the caller.
        let tokens = unsafe { handle(tokens, "tokens") }?;
        let buffer = &tokens.buffer;
        let view = sm_token_view {
            unit: buffer.unit as u32,
            complete: buffer.complete,
            line_count: buffer.line_starts.len(),
            line_starts: buffer.line_starts.as_ptr(),
            line_token_ranges: buffer.line_token_ranges.as_ptr(),
            token_count: buffer.token_starts.len(),
            token_starts: buffer.token_starts.as_ptr(),
            token_lengths: buffer.token_lengths.as_ptr(),
            token_styles: buffer.token_styles.as_ptr(),
            token_scopes: if buffer.token_scopes.is_empty() {
                ptr::null()
            } else {
                buffer.token_scopes.as_ptr()
            },
            style_count: tokens.styles.len(),
            styles: tokens.styles.as_ptr(),
            scope_stack_count: tokens.scope_stack_count(),
            default_style: buffer.default_style.into(),
        };
        // SAFETY: non-null and writable per the caller.
        unsafe { out.write(view) };
        Ok(())
    })
}

/// Returns scope stack `index` (outermost scope first) as `*len` borrowed
/// strings at `*names`, valid until `sm_tokens_free`. An out-of-range
/// `index` fails with `SM_INVALID_INPUT`.
///
/// # Safety
/// `tokens` is NULL or live; `names` and `len` are NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_tokens_scope_stack(
    tokens: *const sm_tokens,
    index: usize,
    names: *mut *const sm_str,
    len: *mut usize,
) -> sm_status {
    guard(|| {
        let (names, len) = (out(names, "names")?, out(len, "len")?);
        // SAFETY: guaranteed by the caller.
        let tokens = unsafe { handle(tokens, "tokens") }?;
        let stack = tokens
            .scope_stack(index)
            .ok_or_else(|| invalid(format!("scope stack {index} is out of range")))?;
        // SAFETY: both non-null and writable per the caller.
        unsafe {
            names.write(stack.as_ptr());
            len.write(stack.len());
        }
        Ok(())
    })
}

/// Frees a token buffer. NULL is ignored.
///
/// # Safety
/// `tokens` is NULL or an `sm_tokens` not yet freed and no longer in use.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sm_tokens_free(tokens: *mut sm_tokens) {
    // SAFETY: guaranteed by the caller.
    unsafe { free(tokens) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_error() -> Option<String> {
        let message = sm_last_error_message();
        // SAFETY: a non-null message is a live NUL-terminated string.
        (!message.is_null()).then(|| {
            unsafe { std::ffi::CStr::from_ptr(message) }
                .to_string_lossy()
                .into()
        })
    }

    #[test]
    fn panics_become_internal_errors() {
        let status = guard(|| panic!("boom"));
        assert_eq!(status, sm_status::SM_INTERNAL);
        assert_eq!(last_error().as_deref(), Some("internal panic: boom"));
        assert_eq!(guard(|| Ok(())), sm_status::SM_OK);
        assert_eq!(last_error(), None);
    }

    #[test]
    fn scope_stacks_round_trip_through_the_name_arena() {
        let stacks = vec![
            vec!["source.rust".to_owned(), "comment.line".to_owned()],
            vec![],
            vec![String::new(), "é".to_owned()],
        ];
        let tokens = sm_tokens::new(TokenBuffer {
            scope_stacks: stacks.clone(),
            ..TokenBuffer::default()
        });
        assert_eq!(tokens.scope_stack_count(), stacks.len());
        for (index, stack) in stacks.iter().enumerate() {
            let names = tokens.scope_stack(index).expect("in range");
            let names: Vec<&str> = names
                .iter()
                .map(|name| {
                    // SAFETY: `ptr` is valid for `len + 1` bytes while `tokens` lives.
                    let bytes =
                        unsafe { std::slice::from_raw_parts(name.ptr.cast(), name.len + 1) };
                    assert_eq!(bytes.last(), Some(&0));
                    std::str::from_utf8(&bytes[..name.len]).expect("UTF-8")
                })
                .collect();
            assert_eq!(names, *stack);
        }
        assert!(tokens.scope_stack(stacks.len()).is_none());

        let empty = sm_tokens::new(TokenBuffer::default());
        assert_eq!(empty.scope_stack_count(), 0);
        assert!(empty.scope_stack(0).is_none());
    }

    #[test]
    fn errors_keep_their_kind_and_sanitize_nul() {
        let status = guard(|| Err(BoundaryError::new(ErrorKind::UnknownTheme, "a\0b")));
        assert_eq!(status, sm_status::SM_UNKNOWN_THEME);
        assert_eq!(last_error().as_deref(), Some("a\u{fffd}b"));
    }
}
