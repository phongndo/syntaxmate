// Edge cases at the JavaScript/WebAssembly boundary: UTF-16 offsets for
// arbitrary strings, argument validation, and object lifetimes.

import assert from 'node:assert/strict'
import { test } from 'node:test'
import * as browser from '../lib/browser.js'
import * as node from '../lib/node.js'
import { createHighlighter, SyntaxmateError, Theme } from '../lib/node.js'

const highlighter = await createHighlighter()
const theme = 'github-dark'

// Every token lies inside its line, in order, without overlap; lines split on
// `\n` exactly as `String.prototype.split` does.
function assertWellFormed(source, tokens) {
  const lines = source.split('\n')
  assert.equal(tokens.lineCount, lines.length)
  assert.equal(tokens.lineTokenRanges.length, lines.length + 1)
  assert.equal(tokens.tokenCount, tokens.lineTokenRanges[lines.length])
  let start = 0
  for (let line = 0; line < lines.length; line++) {
    assert.equal(tokens.lineStarts[line], start, `line ${line} start`)
    const end = start + lines[line].length
    let previous = start
    for (let i = tokens.lineTokenRanges[line]; i < tokens.lineTokenRanges[line + 1]; i++) {
      const tokenStart = tokens.tokenStarts[i]
      const tokenEnd = tokenStart + tokens.tokenLengths[i]
      assert.ok(tokenStart >= previous && tokenEnd <= end, `token ${i} [${tokenStart}, ${tokenEnd}) outside line ${line} [${previous}, ${end})`)
      assert.ok(tokens.tokenStyles[i] < tokens.styles.length)
      previous = tokenEnd
    }
    start = end + 1
  }
}

// Deterministic PRNG so failures reproduce.
function rng(seed) {
  return () => {
    seed = (seed + 0x6d2b79f5) | 0
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const PIECES = [
  'fn', 'let', ' ', '  ', '\t', '"', "'", '//', '/*', '*/', '#', '{', '}', '(', ')', ';', '=', '<', '>',
  '0x1F', '42', 'é', 'ß', '中文', 'Ω', 'é', '😀', '𝒳', '👩‍💻', '\r', '\n', '\r\n', '\n\n',
  '\u0000', '\u001b', ' ', '﻿', '\ud800', '\udc00',
]

function randomSource(random) {
  let source = ''
  const length = Math.floor(random() * 40)
  for (let i = 0; i < length; i++) source += PIECES[Math.floor(random() * PIECES.length)]
  return source
}

test('token offsets are well-formed UTF-16 ranges for random mixed-script input', () => {
  const random = rng(0x5eed)
  for (const lang of ['rust', 'javascript', 'python', 'markdown', 'html']) {
    for (let iteration = 0; iteration < 150; iteration++) {
      const source = randomSource(random)
      try {
        const tokens = highlighter.tokens(source, { lang, theme, includeScopes: iteration % 2 === 0 })
        assertWellFormed(source, tokens)
        // A session over the same lines yields the same tokens, line-relative.
        const session = highlighter.session({ lang, theme })
        try {
          source.split('\n').forEach((text, line) => {
            const one = session.line(text)
            const from = tokens.lineTokenRanges[line]
            const to = tokens.lineTokenRanges[line + 1]
            assert.deepEqual(
              Array.from(one.tokenStarts, (s) => s + tokens.lineStarts[line]),
              Array.from(tokens.tokenStarts.subarray(from, to)),
            )
            assert.deepEqual(one.tokenLengths, tokens.tokenLengths.subarray(from, to))
          })
        } finally {
          session.free()
        }
      } catch (error) {
        error.message += `\n  lang=${lang} source=${JSON.stringify(source)}`
        throw error
      }
    }
  }
})

test('astral characters, CRLF, and empty or final lines', () => {
  const source = '😀x\r\n\r\n"👩‍💻"\n'
  const tokens = highlighter.tokens(source, { lang: 'javascript', theme })
  assertWellFormed(source, tokens)
  assert.deepEqual(Array.from(tokens.lineStarts), [0, 5, 7, 15])
  const empty = highlighter.tokens('', { lang: 'rust', theme })
  assert.deepEqual(Array.from(empty.lineStarts), [0])
  assert.deepEqual(Array.from(empty.lineTokenRanges), [0, 0])
})

// wasm-bindgen encodes strings with TextEncoder, which replaces a lone
// surrogate (one UTF-16 unit) with U+FFFD (also one unit), so offsets still
// index the caller's original string.
test('lone surrogates keep offsets aligned and render as U+FFFD', () => {
  const source = 'let a = "x\ud800y"; // \udc00'
  const tokens = highlighter.tokens(source, { lang: 'javascript', theme })
  assertWellFormed(source, tokens)
  const texts = [...tokens].map((t) => source.slice(t.start, t.start + t.length))
  assert.ok(texts.includes('x\ud800y'), JSON.stringify(texts))
  const html = highlighter.html(source, { lang: 'javascript', theme, includeWrapper: false })
  assert.match(html, /x�y/)
  assert.doesNotMatch(html, /[\ud800-\udfff]/)
  const ansi = highlighter.ansi(source, { lang: 'javascript', theme, colors: false })
  assert.equal(ansi, source.replace(/[\ud800-\udfff]/g, '�'))
})

test('typed arrays are copies that survive WebAssembly memory growth', () => {
  const tokens = highlighter.tokens('fn main() { let x = 1; }', { lang: 'rust', theme })
  const before = Array.from(tokens.tokenStarts)
  assert.ok(!(tokens.tokenStarts.buffer instanceof SharedArrayBuffer))
  // Large enough to force the wasm heap to grow, detaching any old views.
  highlighter.tokens('fn f() { "😀" }\n'.repeat(200_000), { lang: 'rust', theme })
  assert.equal(tokens.tokenStarts.byteLength, before.length * 4)
  assert.deepEqual(Array.from(tokens.tokenStarts), before)
})

test('non-string arguments throw TypeError instead of trapping', () => {
  const options = { lang: 'rust', theme }
  for (const bad of [123, undefined, null, {}, ['fn'], Symbol('x')]) {
    assert.throws(() => highlighter.html(bad, options), TypeError)
    assert.throws(() => highlighter.ansi(bad, options), TypeError)
    assert.throws(() => highlighter.tokens(bad, options), TypeError)
  }
  assert.throws(() => highlighter.html('x', { lang: 5, theme }), TypeError)
  assert.throws(() => highlighter.html('x', { lang: 'rust', theme: 5 }), TypeError)
  assert.throws(() => highlighter.html('x', { ...options, class: 5 }), TypeError)
  assert.throws(() => highlighter.html('x', { ...options, classPrefix: {} }), TypeError)
  assert.throws(() => highlighter.canonicalLanguage(1), TypeError)
  assert.throws(() => highlighter.detect({ path: 1 }), TypeError)
  assert.throws(() => highlighter.detect({ source: null }), TypeError)
  assert.throws(() => Theme.bundled(1), TypeError)
  assert.throws(() => Theme.fromJson(undefined), TypeError)
  assert.throws(() => Theme.bundled('github-dark').stylesheet(), TypeError)
  const session = highlighter.session(options)
  assert.throws(() => session.line(1), TypeError)
  session.free()
  // The module is still healthy afterwards.
  assert.match(highlighter.html('fn x() {}', options), /<span/)
})

test('sessions and themes outlive the objects that created them', async () => {
  const own = await createHighlighter()
  const custom = Theme.bundled('github-light')
  const byName = own.session({ lang: 'rust', theme: 'github-dark' })
  const byTheme = own.session({ lang: 'rust', theme: custom })
  own.free()
  custom.free()
  assert.ok(byName.line('fn main() {}').tokenCount > 0)
  assert.ok(byTheme.line('fn main() {}').tokenCount > 0)
  byName.free()
  byTheme.free()
  assert.throws(() => byName.line('x'), /freed/)
  assert.throws(() => own.html('x', { lang: 'rust', theme: 'github-dark' }), /freed/)
  assert.throws(() => highlighter.html('x', { lang: 'rust', theme: custom }), /freed/)
})

test('unknown error codes map to Internal', () => {
  assert.equal(new SyntaxmateError('m', 0).kind, 'Internal')
  assert.equal(new SyntaxmateError('m', 99).kind, 'Internal')
  assert.equal(new SyntaxmateError('m', 8).kind, 'Internal')
  assert.equal(new SyntaxmateError('m', 1).kind, 'UnknownLanguage')
})

test('entry points export the declared API', () => {
  const shared = [
    'BOLD', 'Highlighter', 'ITALIC', 'STRIKETHROUGH', 'Session', 'SyntaxmateError', 'Theme',
    'TokenBuffer', 'UNDERLINE', 'createHighlighter', 'init', 'loadBundle', 'version',
  ]
  assert.deepEqual(Object.keys(browser).sort(), shared.sort())
  assert.deepEqual(Object.keys(node).sort(), [...shared, 'initSync', 'loadBundleSync'].sort())
})
