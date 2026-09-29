// Public API shared by the Node and browser entry points. The entry point
// instantiates the WebAssembly module and then calls `markReady()`.

import * as raw from '../dist/syntaxmate.js'

/** Bold bit in `Style.modifiers`. */
export const BOLD = 1
/** Italic bit in `Style.modifiers`. */
export const ITALIC = 2
/** Underline bit in `Style.modifiers`. */
export const UNDERLINE = 4
/** Strikethrough bit in `Style.modifiers`. */
export const STRIKETHROUGH = 8

// Index = stable numeric code shared with the C ABI.
const KINDS = [
  'Internal',
  'UnknownLanguage',
  'UnknownTheme',
  'InvalidGrammar',
  'InvalidTheme',
  'InvalidBundle',
  'InvalidInput',
  'Render',
  'Internal',
]

/** An engine error with a stable `kind` and numeric `code`. */
export class SyntaxmateError extends Error {
  constructor(message, code) {
    super(message)
    this.name = 'SyntaxmateError'
    this.code = code
    this.kind = KINDS[code] ?? 'Internal'
  }
}

let ready = false

export function markReady() {
  if (!ready) {
    raw.setErrorClass(SyntaxmateError)
    ready = true
  }
}

export function isReady() {
  return ready
}

/** Version of the underlying Syntaxmate engine. */
export function version() {
  assertReady()
  return raw.version()
}

function assertReady() {
  if (!ready) {
    throw new Error('syntaxmate: WebAssembly is not initialized; call `await init()` first')
  }
}

const dispose = Symbol.dispose ?? Symbol.for('Symbol.dispose')
const NO_COLOR = 0xffffffff

function color(value) {
  return value === NO_COLOR ? null : value
}

function styleAt(flat, index) {
  return {
    foreground: color(flat[index]),
    background: color(flat[index + 1]),
    modifiers: flat[index + 2],
  }
}

export function toBytes(bytes) {
  if (bytes instanceof Uint8Array) return bytes
  if (bytes instanceof ArrayBuffer) return new Uint8Array(bytes)
  if (ArrayBuffer.isView(bytes)) {
    return new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  }
  throw new TypeError('syntaxmate: expected a Uint8Array, ArrayBuffer, or ArrayBufferView')
}

// wasm-bindgen does not type-check string arguments: a number or object
// reaches Rust as a bogus pointer and traps with "memory access out of bounds".
function string(value, name) {
  if (typeof value !== 'string') {
    throw new TypeError(`syntaxmate: ${name} must be a string, got ${value === null ? 'null' : typeof value}`)
  }
  return value
}

function optionalString(value, name) {
  return value === undefined || value === null ? undefined : string(value, name)
}

// Owns one wasm-bindgen handle; `free()` is idempotent and use-after-free throws.
class Handle {
  #raw

  constructor(handle) {
    this.#raw = handle
  }

  get _raw() {
    if (this.#raw === undefined) {
      throw new Error(`syntaxmate: ${this.constructor.name} has been freed`)
    }
    return this.#raw
  }

  /** Releases WebAssembly memory now instead of at garbage collection. */
  free() {
    this.#raw?.free()
    this.#raw = undefined
  }

  [dispose]() {
    this.free()
  }
}

/** A bundled or custom TextMate theme. */
export class Theme extends Handle {
  /** Loads a bundled theme by name, such as `github-dark`. */
  static bundled(name) {
    assertReady()
    return new Theme(raw.RawTheme.bundled(string(name, 'name')))
  }

  /** Parses a TextMate/VS Code JSON theme from a string or parsed object. */
  static fromJson(json) {
    assertReady()
    const text = typeof json === 'string' ? json : JSON.stringify(json)
    return new Theme(raw.RawTheme.fromJson(string(text, 'json')))
  }

  get name() {
    return this._raw.name()
  }

  /** The theme's default foreground and background. */
  get defaultStyle() {
    return styleAt(this._raw.defaultStyle(), 0)
  }

  /** CSS for HTML rendered in class mode with the same `classPrefix`. */
  stylesheet(classPrefix) {
    return this._raw.stylesheet(string(classPrefix, 'classPrefix'))
  }
}

/**
 * Styled tokens as flat arrays. Offsets are UTF-16 code units, so they index
 * JavaScript strings directly. Token `i` covers
 * `source.slice(tokenStarts[i], tokenStarts[i] + tokenLengths[i])`, and line
 * `l` owns tokens `lineTokenRanges[l] .. lineTokenRanges[l + 1]`.
 */
export class TokenBuffer {
  // `packed` and `names` come from a single wasm call; `pack` in src/lib.rs
  // documents the layout. The typed arrays are views of one JavaScript-owned
  // buffer (a copy, not WebAssembly memory), so they stay valid indefinitely.
  constructor(packed, names) {
    const [complete, lines, tokens, scoped, styles, stacks] = packed
    let offset = 6
    const take = (length) => packed.subarray(offset, (offset += length))
    this.complete = complete === 1
    this.lineStarts = take(lines)
    this.lineTokenRanges = take(lines + 1)
    this.tokenStarts = take(tokens)
    this.tokenLengths = take(tokens)
    this.tokenStyles = take(tokens)
    this.tokenScopes = take(scoped)
    this.styles = new Array(styles)
    for (let index = 0; index < styles; index++, offset += 3) {
      this.styles[index] = styleAt(packed, offset)
    }
    this.defaultStyle = styleAt(packed, offset)
    offset += 3
    this.scopeStacks = new Array(stacks)
    for (let index = 0; index < stacks; index++) {
      const stack = new Array(packed[offset++])
      for (let scope = 0; scope < stack.length; scope++) stack[scope] = names[packed[offset++]]
      this.scopeStacks[index] = stack
    }
  }

  get lineCount() {
    return this.lineStarts.length
  }

  get tokenCount() {
    return this.tokenStarts.length
  }

  /**
   * Yields one object per token. Convenient, but allocates; index the typed
   * arrays directly on hot paths.
   */
  *[Symbol.iterator]() {
    const scoped = this.tokenScopes.length > 0
    for (let line = 0; line < this.lineCount; line++) {
      const end = this.lineTokenRanges[line + 1]
      for (let index = this.lineTokenRanges[line]; index < end; index++) {
        yield {
          line,
          start: this.tokenStarts[index],
          length: this.tokenLengths[index],
          style: this.styles[this.tokenStyles[index]],
          scopes: scoped ? this.scopeStacks[this.tokenScopes[index]] : undefined,
        }
      }
    }
  }
}

/** Highlights one line per call, carrying grammar state between lines. */
export class Session extends Handle {
  /** Highlights the next line; pass it without its `\n`. Offsets are line-relative. */
  line(text) {
    const names = []
    return new TokenBuffer(this._raw.line(string(text, 'text'), names), names)
  }

  /** Returns to the start-of-document state. */
  reset() {
    this._raw.reset()
  }
}

function required(options, name) {
  const value = options?.[name]
  if (value === undefined || value === null) {
    throw new TypeError(`syntaxmate: options.${name} is required`)
  }
  return value
}

function lang(options) {
  return string(required(options, 'lang'), 'options.lang')
}

/** A highlighter over one grammar catalog. */
export class Highlighter extends Handle {
  #themes = new Map()

  /** Decodes grammar-bundle bytes (for example, the package's `grammars.bundle`). */
  static fromBundle(bytes) {
    assertReady()
    return new Highlighter(raw.RawEngine.fromBundle(toBytes(bytes)))
  }

  #theme(options) {
    this._raw // Throw use-after-free before caching a theme.
    const theme = required(options, 'theme')
    if (theme instanceof Theme) return theme._raw
    string(theme, 'options.theme')
    let handle = this.#themes.get(theme)
    if (handle === undefined) {
      handle = raw.RawTheme.bundled(theme)
      this.#themes.set(theme, handle)
    }
    return handle
  }

  /** Canonical language IDs in this bundle. */
  languages() {
    return this._raw.languages()
  }

  /** Bundled theme names. */
  themes() {
    return this._raw.themes()
  }

  /** Resolves an ID or alias to its canonical ID, or `null`. */
  canonicalLanguage(language) {
    return this._raw.canonicalLanguage(string(language, 'language')) ?? null
  }

  /** Detects a language from a file path and/or the source's first line, or `null`. */
  detect({ path, source = '' } = {}) {
    return this._raw.detect(optionalString(path, 'path'), string(source, 'source')) ?? null
  }

  /** Highlights `code` to escaped HTML. */
  html(code, options) {
    const theme = this.#theme(options)
    return this._raw.html(
      lang(options),
      string(code, 'code'),
      theme,
      options.includeWrapper ?? true,
      options.class === undefined ? 'syntaxmate' : optionalString(options.class, 'options.class'),
      options.includeScopes ?? false,
      optionalString(options.classPrefix, 'options.classPrefix'),
    )
  }

  /** Highlights `code` to 24-bit ANSI text for terminals. */
  ansi(code, options) {
    const theme = this.#theme(options)
    return this._raw.ansi(
      lang(options),
      string(code, 'code'),
      theme,
      options.colors ?? true,
      options.sanitizeControlCharacters ?? true,
      options.includeDefaultBackground ?? false,
    )
  }

  /** Highlights `code` into a {@link TokenBuffer}. */
  tokens(code, options) {
    const theme = this.#theme(options)
    const names = []
    const packed = this._raw.tokens(
      lang(options), string(code, 'code'), theme, options.includeScopes ?? false, names,
    )
    return new TokenBuffer(packed, names)
  }

  /** Starts an incremental {@link Session}. */
  session(options) {
    const theme = this.#theme(options)
    return new Session(
      this._raw.session(lang(options), theme, options.includeScopes ?? false),
    )
  }

  free() {
    for (const handle of this.#themes.values()) handle.free()
    this.#themes.clear()
    super.free()
  }
}
