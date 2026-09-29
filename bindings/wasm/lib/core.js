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
    return new Theme(raw.RawTheme.bundled(name))
  }

  /** Parses a TextMate/VS Code JSON theme from a string or parsed object. */
  static fromJson(json) {
    assertReady()
    const text = typeof json === 'string' ? json : JSON.stringify(json)
    return new Theme(raw.RawTheme.fromJson(text))
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
    return this._raw.stylesheet(classPrefix)
  }
}

/**
 * Styled tokens as flat arrays. Offsets are UTF-16 code units, so they index
 * JavaScript strings directly. Token `i` covers
 * `source.slice(tokenStarts[i], tokenStarts[i] + tokenLengths[i])`, and line
 * `l` owns tokens `lineTokenRanges[l] .. lineTokenRanges[l + 1]`.
 */
export class TokenBuffer {
  constructor(handle) {
    try {
      this.complete = handle.complete()
      this.lineStarts = handle.takeLineStarts()
      this.lineTokenRanges = handle.takeLineTokenRanges()
      this.tokenStarts = handle.takeTokenStarts()
      this.tokenLengths = handle.takeTokenLengths()
      this.tokenStyles = handle.takeTokenStyles()
      this.tokenScopes = handle.takeTokenScopes()
      const flat = handle.styles()
      this.styles = []
      for (let index = 0; index < flat.length; index += 3) {
        this.styles.push(styleAt(flat, index))
      }
      this.defaultStyle = styleAt(handle.defaultStyle(), 0)
      const lengths = handle.scopeStackLengths()
      const names = handle.takeScopeNames()
      this.scopeStacks = []
      let offset = 0
      for (const length of lengths) {
        this.scopeStacks.push(names.slice(offset, offset + length))
        offset += length
      }
    } finally {
      handle.free()
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
    return new TokenBuffer(this._raw.line(text))
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

/** A thread-safe highlighter over one grammar catalog. */
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
    return this._raw.canonicalLanguage(language) ?? null
  }

  /** Detects a language from a file path and/or the source's first line, or `null`. */
  detect({ path, source = '' } = {}) {
    return this._raw.detect(path ?? undefined, source) ?? null
  }

  /** Highlights `code` to escaped HTML. */
  html(code, options) {
    const theme = this.#theme(options)
    return this._raw.html(
      required(options, 'lang'),
      code,
      theme,
      options.includeWrapper ?? true,
      options.class === undefined ? 'syntaxmate' : options.class,
      options.includeScopes ?? false,
      options.classPrefix ?? undefined,
    )
  }

  /** Highlights `code` to 24-bit ANSI text for terminals. */
  ansi(code, options) {
    const theme = this.#theme(options)
    return this._raw.ansi(
      required(options, 'lang'),
      code,
      theme,
      options.colors ?? true,
      options.sanitizeControlCharacters ?? true,
      options.includeDefaultBackground ?? false,
    )
  }

  /** Highlights `code` into a {@link TokenBuffer}. */
  tokens(code, options) {
    const theme = this.#theme(options)
    return new TokenBuffer(
      this._raw.tokens(required(options, 'lang'), code, theme, options.includeScopes ?? false),
    )
  }

  /** Starts an incremental {@link Session}. */
  session(options) {
    const theme = this.#theme(options)
    return new Session(
      this._raw.session(required(options, 'lang'), theme, options.includeScopes ?? false),
    )
  }

  free() {
    for (const handle of this.#themes.values()) handle.free()
    this.#themes.clear()
    super.free()
  }
}
