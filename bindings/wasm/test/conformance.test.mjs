// Runs every shared binding conformance case through the public JS API.

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { Highlighter, loadBundleSync, Theme } from '../lib/node.js'

const dir = new URL('../../conformance/', import.meta.url)
const { customTheme, cases } = JSON.parse(readFileSync(new URL('cases.json', dir), 'utf8'))
const expected = JSON.parse(readFileSync(new URL('expected.json', dir), 'utf8'))

const NO_COLOR = 0xffffffff
const style = (s) => [s.foreground ?? NO_COLOR, s.background ?? NO_COLOR, s.modifiers]

function buffer(tokens) {
  return {
    complete: tokens.complete,
    lineStarts: Array.from(tokens.lineStarts),
    lineTokenRanges: Array.from(tokens.lineTokenRanges),
    tokenStarts: Array.from(tokens.tokenStarts),
    tokenLengths: Array.from(tokens.tokenLengths),
    tokenStyles: Array.from(tokens.tokenStyles),
    styles: tokens.styles.map(style),
    defaultStyle: style(tokens.defaultStyle),
  }
}

const highlighter = Highlighter.fromBundle(loadBundleSync())
const custom = Theme.fromJson(customTheme)

test('conformance covers every case', () => {
  assert.deepEqual(cases.map((c) => c.name).sort(), Object.keys(expected).sort())
})

for (const { name, language: lang, theme: themeName, source } of cases) {
  test(`conformance: ${name}`, () => {
    const want = expected[name]
    const theme = themeName ?? custom
    assert.equal(highlighter.html(source, { lang, theme }), want.html, 'html')
    assert.equal(
      highlighter.html(source, { lang, theme, classPrefix: 'sm' }),
      want.htmlClasses,
      'htmlClasses',
    )
    assert.equal(highlighter.ansi(source, { lang, theme }), want.ansi, 'ansi')

    const tokens = highlighter.tokens(source, { lang, theme })
    assert.ok(tokens.lineStarts instanceof Uint32Array)
    assert.deepEqual(buffer(tokens), want.tokens.utf16, 'tokens.utf16')

    const scoped = highlighter.tokens(source, { lang, theme, includeScopes: true })
    assert.deepEqual(
      { tokenScopes: Array.from(scoped.tokenScopes), scopeStacks: scoped.scopeStacks },
      want.scopes,
      'scopes',
    )

    const session = highlighter.session({ lang, theme })
    try {
      const lines = source.split('\n').map((line) => buffer(session.line(line)))
      assert.deepEqual(lines, want.session, 'session')
    } finally {
      session.free()
    }
  })
}
