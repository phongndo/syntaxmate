import assert from 'node:assert/strict'
import { test } from 'node:test'
import {
  BOLD,
  createHighlighter,
  Highlighter,
  loadBundle,
  SyntaxmateError,
  Theme,
  version,
} from '../lib/node.js'

const highlighter = await createHighlighter()

function throwsKind(fn, kind, code) {
  assert.throws(fn, (error) => {
    assert.ok(error instanceof SyntaxmateError)
    assert.ok(error instanceof Error)
    assert.equal(error.kind, kind)
    assert.equal(error.code, code)
    assert.ok(error.message.length > 0)
    return true
  })
}

test('catalog queries', () => {
  assert.match(version(), /^\d+\.\d+\.\d+/)
  assert.ok(highlighter.languages().includes('rust'))
  assert.ok(highlighter.themes().includes('github-dark'))
  assert.equal(highlighter.canonicalLanguage('py'), 'python')
  assert.equal(highlighter.canonicalLanguage('no-such-language'), null)
  assert.equal(highlighter.detect({ path: 'src/main.rs' }), 'rust')
  assert.equal(highlighter.detect({ source: '#!/usr/bin/env python3\n' }), 'python')
  assert.equal(highlighter.detect({ path: 'unknown.zzz' }), null)
})

test('errors carry stable kinds and codes', () => {
  throwsKind(() => highlighter.html('x', { lang: 'nope', theme: 'github-dark' }), 'UnknownLanguage', 1)
  throwsKind(() => highlighter.html('x', { lang: 'rust', theme: 'nope' }), 'UnknownTheme', 2)
  throwsKind(() => Theme.bundled('nope'), 'UnknownTheme', 2)
  throwsKind(() => Theme.fromJson('{'), 'InvalidTheme', 4)
  throwsKind(() => Highlighter.fromBundle(new Uint8Array([1, 2, 3])), 'InvalidBundle', 5)
  const session = highlighter.session({ lang: 'rust', theme: 'github-dark' })
  throwsKind(() => session.line('a\nb'), 'InvalidInput', 6)
  session.free()
  assert.throws(() => highlighter.html('x', { theme: 'github-dark' }), TypeError)
})

test('tokens index JavaScript strings in UTF-16 code units', () => {
  const source = 'let s = "😀"; // é\nfn main() {}'
  const tokens = highlighter.tokens(source, { lang: 'rust', theme: 'github-dark', includeScopes: true })
  assert.equal(tokens.lineCount, 2)
  assert.equal(tokens.lineStarts[1], source.indexOf('\n') + 1)
  const texts = [...tokens].map((t) => source.slice(t.start, t.start + t.length))
  assert.ok(texts.includes('"'))
  assert.ok(texts.some((t) => t.includes('😀')))
  assert.ok(texts.includes('fn'))
  const fn = [...tokens].find((t) => source.slice(t.start, t.start + t.length) === 'fn')
  assert.equal(fn.line, 1)
  assert.ok(fn.scopes.includes('source.rust'))
  assert.equal(tokens.tokenCount, tokens.lineTokenRanges[tokens.lineCount])
})

test('themes', () => {
  const theme = Theme.fromJson({
    name: 'Mine',
    colors: { 'editor.foreground': '#010203', 'editor.background': '#ffffff' },
    tokenColors: [{ scope: 'keyword', settings: { foreground: '#ff0000', fontStyle: 'bold' } }],
  })
  assert.equal(theme.name, 'Mine')
  assert.deepEqual(theme.defaultStyle, { foreground: 0x010203, background: 0xffffff, modifiers: 0 })
  const tokens = highlighter.tokens('fn f() {}', { lang: 'rust', theme })
  assert.ok(tokens.styles.some((s) => s.foreground === 0xff0000 && s.modifiers & BOLD))
  const css = Theme.bundled('github-dark').stylesheet('sm')
  assert.match(css, /\.sm-/)
  const classes = highlighter.html('fn f() {}', { lang: 'rust', theme, classPrefix: 'sm' })
  assert.doesNotMatch(classes, /style=/)
  theme.free()
})

test('html and ansi options', () => {
  const bare = highlighter.html('<a>', { lang: 'rust', theme: 'github-dark', includeWrapper: false })
  assert.doesNotMatch(bare, /<pre/)
  assert.match(bare, /&lt;a&gt;/)
  const noClass = highlighter.html('x', { lang: 'rust', theme: 'github-dark', class: null })
  assert.match(noClass, /^<pre style=/)
  assert.equal(highlighter.ansi('fn x\u0007', { lang: 'rust', theme: 'github-dark', colors: false }), 'fn x␇')
})

test('sessions carry state across lines and reset', () => {
  const session = highlighter.session({ lang: 'rust', theme: 'github-dark', includeScopes: true })
  session.line('/* open')
  const inside = session.line('still comment')
  assert.ok(inside.scopeStacks[inside.tokenScopes[0]].some((s) => s.startsWith('comment.block')))
  session.reset()
  const fresh = session.line('still comment')
  assert.ok(!fresh.scopeStacks.flat().some((s) => s.startsWith('comment.block')))
  session[Symbol.dispose]()
})

test('free is idempotent and use-after-free throws', async () => {
  const own = Highlighter.fromBundle(await loadBundle())
  assert.ok(own.html('x', { lang: 'rust', theme: 'github-dark' }).length > 0)
  own.free()
  own.free()
  assert.throws(() => own.languages(), /freed/)
  let escaped
  {
    using theme = Theme.bundled('github-light')
    escaped = theme
  }
  assert.throws(() => escaped.name, /freed/)
})

test('SyntaxmateError can be constructed and subclassed by callers', () => {
  const error = new SyntaxmateError('Invalid source', 6)
  assert.equal(error.kind, 'InvalidInput')
  assert.equal(error.code, 6)
  assert.equal(error.name, 'SyntaxmateError')
  assert.equal(new SyntaxmateError('x', 99).kind, 'Internal')
  class AppError extends SyntaxmateError {}
  assert.ok(new AppError('x', 1) instanceof SyntaxmateError)
})
