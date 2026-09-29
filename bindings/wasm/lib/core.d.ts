/** Bold bit in {@link Style.modifiers}. */
export const BOLD: 1
/** Italic bit in {@link Style.modifiers}. */
export const ITALIC: 2
/** Underline bit in {@link Style.modifiers}. */
export const UNDERLINE: 4
/** Strikethrough bit in {@link Style.modifiers}. */
export const STRIKETHROUGH: 8

/** Stable error categories; `code` is the matching C ABI number. */
export type ErrorKind =
  | 'UnknownLanguage'
  | 'UnknownTheme'
  | 'InvalidGrammar'
  | 'InvalidTheme'
  | 'InvalidBundle'
  | 'InvalidInput'
  | 'Render'
  | 'Internal'

/** Thrown for engine failures such as unknown languages or invalid themes. */
export class SyntaxmateError extends Error {
  readonly name: 'SyntaxmateError'
  readonly kind: ErrorKind
  readonly code: number
}

/** A resolved style. Colors are `0xRRGGBB` integers, or `null` when unset. */
export interface Style {
  foreground: number | null
  background: number | null
  /** Bitset of {@link BOLD}, {@link ITALIC}, {@link UNDERLINE}, {@link STRIKETHROUGH}. */
  modifiers: number
}

declare global {
  interface SymbolConstructor {
    readonly dispose: unique symbol
  }
}

/** WebAssembly-backed objects free on garbage collection, or now with `free()`/`using`. */
interface Freeable {
  /** Releases WebAssembly memory now. Idempotent; later use throws. */
  free(): void
  [Symbol.dispose](): void
}

/** A bundled or custom TextMate theme. */
export class Theme implements Freeable {
  private constructor()
  /** Loads a bundled theme by name, such as `github-dark`. */
  static bundled(name: string): Theme
  /** Parses a TextMate/VS Code JSON theme from a string or parsed object. */
  static fromJson(json: string | object): Theme
  readonly name: string
  /** The theme's default foreground and background. */
  readonly defaultStyle: Style
  /** CSS for HTML rendered in class mode with the same `classPrefix`. */
  stylesheet(classPrefix: string): string
  free(): void
  [Symbol.dispose](): void
}

/** A bundled theme name or a {@link Theme}. */
export type ThemeInput = string | Theme

export interface LanguageOptions {
  /** Language ID or alias, such as `rust`, `ts`, or `py`. */
  lang: string
  theme: ThemeInput
}

export interface HtmlOptions extends LanguageOptions {
  /** Wrap in `<pre><code>`, with default colors on `<pre>`. Default `true`. */
  includeWrapper?: boolean
  /** Class on the `<pre>` wrapper; `null` for none. Default `'syntaxmate'`. */
  class?: string | null
  /** Add `data-scopes` attributes with the TextMate scope stack. Default `false`. */
  includeScopes?: boolean
  /** Emit theme-independent scope classes; style them with `Theme.stylesheet(classPrefix)`. */
  classPrefix?: string
}

export interface AnsiOptions extends LanguageOptions {
  /** Emit 24-bit color and modifier sequences. Default `true`. */
  colors?: boolean
  /** Replace control characters with visible pictures. Default `true`; disable only for trusted input. */
  sanitizeControlCharacters?: boolean
  /** Paint the theme's default background. Default `false`. */
  includeDefaultBackground?: boolean
}

export interface TokenOptions extends LanguageOptions {
  /** Fill `tokenScopes` and `scopeStacks`. Default `false`. */
  includeScopes?: boolean
}

/** One token from the {@link TokenBuffer} iterator. */
export interface Token {
  line: number
  start: number
  length: number
  style: Style
  /** Present when scopes were requested; outermost first. */
  scopes: readonly string[] | undefined
}

/**
 * Styled tokens as flat arrays. Offsets and lengths are UTF-16 code units, so
 * they index JavaScript strings directly. Token `i` covers
 * `source.slice(tokenStarts[i], tokenStarts[i] + tokenLengths[i])`. Line `l`
 * (split on `\n` only) owns tokens `lineTokenRanges[l] .. lineTokenRanges[l + 1]`.
 * Text outside every token uses `defaultStyle`. The typed arrays are views of
 * one JavaScript-owned buffer, not of WebAssembly memory, so they stay valid.
 */
export class TokenBuffer implements Iterable<Token> {
  private constructor()
  /** Whether tokenization finished within resource limits. */
  readonly complete: boolean
  /** Offset of each line's first character. */
  readonly lineStarts: Uint32Array
  /** Token index boundaries per line; length `lineCount + 1`. */
  readonly lineTokenRanges: Uint32Array
  readonly tokenStarts: Uint32Array
  readonly tokenLengths: Uint32Array
  /** Index into `styles` per token. */
  readonly tokenStyles: Uint32Array
  /** Index into `scopeStacks` per token; empty unless `includeScopes`. */
  readonly tokenScopes: Uint32Array
  /** Distinct styles referenced by `tokenStyles`. */
  readonly styles: readonly Style[]
  /** Distinct scope stacks, outermost first; empty unless `includeScopes`. */
  readonly scopeStacks: readonly (readonly string[])[]
  readonly defaultStyle: Style
  readonly lineCount: number
  readonly tokenCount: number
  /** Allocates one object per token; index the typed arrays on hot paths. */
  [Symbol.iterator](): Iterator<Token>
}

/** Highlights one line per call, carrying grammar state between lines. */
export class Session implements Freeable {
  private constructor()
  /** Highlights the next line, passed without its `\n`. Offsets are line-relative. */
  line(text: string): TokenBuffer
  /** Returns to the start-of-document state. */
  reset(): void
  free(): void
  [Symbol.dispose](): void
}

/** A highlighter over one grammar catalog. Bundled theme names are cached per instance. */
export class Highlighter implements Freeable {
  private constructor()
  /** Decodes grammar-bundle bytes; the bytes are copied. */
  static fromBundle(bytes: Uint8Array | ArrayBuffer | ArrayBufferView): Highlighter
  /** Canonical language IDs in this bundle. */
  languages(): string[]
  /** Bundled theme names. */
  themes(): string[]
  /** Resolves an ID or alias to its canonical ID. */
  canonicalLanguage(language: string): string | null
  /** Detects a language from a file path and/or the source's first line. */
  detect(input?: { path?: string | null; source?: string }): string | null
  html(code: string, options: HtmlOptions): string
  ansi(code: string, options: AnsiOptions): string
  tokens(code: string, options: TokenOptions): TokenBuffer
  session(options: TokenOptions): Session
  free(): void
  [Symbol.dispose](): void
}

/** Version of the underlying Syntaxmate engine. */
export function version(): string
