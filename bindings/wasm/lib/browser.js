// Browser and bundler entry point: call `await init()` (or use
// `createHighlighter()`, which does) before any other API.

import initWasm from '../dist/syntaxmate.js'
import { Highlighter, isReady, markReady, toBytes } from './core.js'

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

let pending

/**
 * Fetches and instantiates the WebAssembly module once. Pass a URL, Response,
 * bytes, or `WebAssembly.Module` to override the default sibling `.wasm` file.
 */
export function init(wasm) {
  if (isReady()) return Promise.resolve()
  pending ??= initWasm(wasm === undefined ? undefined : { module_or_path: wasm }).then(
    () => markReady(),
    (error) => {
      pending = undefined
      throw error
    },
  )
  return pending
}

/** Fetches a grammar bundle; defaults to the package's full `grammars.bundle`. */
export async function loadBundle(url = BUNDLE) {
  const response = await fetch(url)
  if (!response.ok) {
    throw new Error(`syntaxmate: failed to fetch ${url}: ${response.status} ${response.statusText}`)
  }
  return new Uint8Array(await response.arrayBuffer())
}

/** Initializes WebAssembly and creates a highlighter from bundle bytes or a URL. */
export async function createHighlighter({ bundle, wasm } = {}) {
  const [, bytes] = await Promise.all([
    init(wasm),
    bundle === undefined || typeof bundle === 'string' || bundle instanceof URL
      ? loadBundle(bundle)
      : toBytes(bundle),
  ])
  return Highlighter.fromBundle(bytes)
}
