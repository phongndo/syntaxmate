// The browser entry point, driven from Node with explicit bytes (Node's fetch
// cannot read file: URLs). Runs in its own process, so WebAssembly starts
// uninitialized.

import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import { createHighlighter, Highlighter, init, Theme } from '../lib/browser.js'

test('APIs require init() first', () => {
  assert.throws(() => Theme.bundled('github-dark'), /init\(\)/)
  assert.throws(() => Highlighter.fromBundle(new Uint8Array()), /init\(\)/)
})

test('init() and createHighlighter() with explicit bytes', async () => {
  const wasm = readFileSync(new URL('../dist/syntaxmate_bg.wasm', import.meta.url))
  const bundle = readFileSync(new URL('../grammars.bundle', import.meta.url))
  await Promise.all([init(wasm), init(wasm)])
  const highlighter = await createHighlighter({ bundle: bundle.buffer.slice(bundle.byteOffset, bundle.byteOffset + bundle.byteLength) })
  assert.match(highlighter.html('fn main() {}', { lang: 'rust', theme: 'github-dark' }), /^<pre class="syntaxmate"/)
  highlighter.free()
})
