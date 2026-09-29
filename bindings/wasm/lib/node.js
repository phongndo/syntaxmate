// Node entry point: instantiates WebAssembly synchronously at import, so the
// API is usable without awaiting `init()`. When a bundler moves this module
// away from the package's `dist/` directory, the default `.wasm` is missing;
// import still succeeds and `init(wasm)` or `createHighlighter({ wasm })`
// loads it from an explicit location.

import { readFileSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { initSync as initWasm } from '../dist/syntaxmate.js'
import { Highlighter, isReady, markReady, setNotReadyHint, toBytes } from './core.js'

export {
  BOLD,
  Highlighter,
  ITALIC,
  STRIKETHROUGH,
  Session,
  SyntaxmateError,
  Theme,
  TokenBuffer,
  UNDERLINE,
  version,
} from './core.js'

const BUNDLE = new URL('../grammars.bundle', import.meta.url)
const WASM = new URL('../dist/syntaxmate_bg.wasm', import.meta.url)

function isPath(value) {
  return typeof value === 'string' || value instanceof URL
}

function instantiate(wasm) {
  const module = wasm instanceof WebAssembly.Module
    ? wasm
    : isPath(wasm) ? readFileSync(wasm) : toBytes(wasm)
  initWasm({ module })
  markReady()
}

try {
  instantiate(WASM)
} catch (error) {
  if (error?.code !== 'ENOENT') throw error
  setNotReadyHint(
    `${WASM.pathname} was not found (was this module bundled?); ` +
      'pass the .wasm location or bytes to `init(wasm)` or `createHighlighter({ wasm })`',
  )
}

/**
 * Instantiates WebAssembly from a path, URL, bytes, or `WebAssembly.Module`.
 * Needed only when the default sibling `.wasm` was not found at import, such
 * as after bundling; otherwise a no-op.
 */
export async function init(wasm) {
  initWith(wasm)
}

/** Synchronous {@link init}. */
export function initSync(wasm) {
  initWith(wasm)
}

function initWith(wasm) {
  if (isReady()) return
  instantiate(wasm ?? WASM)
}

/** Reads a grammar bundle; defaults to the package's full `grammars.bundle`. */
export async function loadBundle(path = BUNDLE) {
  return readFile(path)
}

/** Synchronous {@link loadBundle}. */
export function loadBundleSync(path = BUNDLE) {
  return readFileSync(path)
}

/**
 * Creates a highlighter from bundle bytes or a path; defaults to the full
 * bundle. `wasm` is used only if WebAssembly is not initialized yet.
 */
export async function createHighlighter({ bundle, wasm } = {}) {
  initWith(wasm)
  const bytes = bundle === undefined || isPath(bundle) ? await loadBundle(bundle) : toBytes(bundle)
  return Highlighter.fromBundle(bytes)
}
