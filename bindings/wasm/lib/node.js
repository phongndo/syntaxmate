// Node entry point: instantiates WebAssembly synchronously at import, so the
// API is usable without awaiting `init()`.

import { readFileSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { initSync } from '../dist/syntaxmate.js'
import { Highlighter, markReady, toBytes } from './core.js'

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

initSync({ module: readFileSync(new URL('../dist/syntaxmate_bg.wasm', import.meta.url)) })
markReady()

/** No-op kept for parity with the browser entry; importing already initialized. */
export async function init() {}

/** Reads a grammar bundle; defaults to the package's full `grammars.bundle`. */
export async function loadBundle(path = BUNDLE) {
  return readFile(path)
}

/** Synchronous {@link loadBundle}. */
export function loadBundleSync(path = BUNDLE) {
  return readFileSync(path)
}

/** Creates a highlighter from bundle bytes or a path; defaults to the full bundle. */
export async function createHighlighter({ bundle } = {}) {
  const bytes = bundle === undefined || typeof bundle === 'string' || bundle instanceof URL
    ? await loadBundle(bundle)
    : toBytes(bundle)
  return Highlighter.fromBundle(bytes)
}
