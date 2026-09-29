// Type-checked, never run, against lib/*.d.ts by `npm run typecheck`.
import {
  BOLD,
  createHighlighter,
  Highlighter,
  init,
  loadBundleSync,
  SyntaxmateError,
  Theme,
  type ErrorKind,
  type Style,
  type Token,
  TokenBuffer,
  version,
} from '../../lib/node.js'

export async function main(): Promise<void> {
  await init()
  const v: string = version()
  const highlighter: Highlighter = await createHighlighter({ bundle: loadBundleSync() })
  const own = Highlighter.fromBundle(new ArrayBuffer(0))
  own.free()
  const html: string = highlighter.html('x', { lang: 'rust', theme: 'github-dark', class: null })
  const ansi: string = highlighter.ansi('x', { lang: 'rust', theme: 'github-dark', colors: false })
  const lang: string | null = highlighter.detect({ path: 'a.rs' }) ?? highlighter.detect()
  {
    using theme = Theme.fromJson({ name: 'x' })
    const tokens: TokenBuffer = highlighter.tokens('x', { lang: 'rust', theme })
    const starts: Uint32Array = tokens.tokenStarts
    const style: Style | undefined = tokens.styles[0]
    const bold: boolean = ((style?.modifiers ?? 0) & BOLD) !== 0
    for (const token of tokens) {
      const t: Token = token
      const scopes: readonly string[] | undefined = t.scopes
      void [scopes, starts, bold]
    }
  }
  using session = highlighter.session({ lang: 'rust', theme: 'github-dark', includeScopes: true })
  const line: TokenBuffer = session.line('x')
  try {
    Theme.bundled('nope')
  } catch (error) {
    if (error instanceof SyntaxmateError) {
      const kind: ErrorKind = error.kind
      const code: number = error.code
      void [kind, code]
    }
  }
  // @ts-expect-error `lang` is required.
  highlighter.html('x', { theme: 'github-dark' })
  // @ts-expect-error constructors are private.
  new TokenBuffer()
  void [v, html, ansi, lang, line]
}
