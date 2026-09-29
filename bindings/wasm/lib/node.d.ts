export * from './core.js'
import type { Highlighter } from './core.js'

/** No-op in Node: importing the package already instantiated WebAssembly. */
export function init(): Promise<void>
/** Reads a grammar bundle; defaults to the package's full `grammars.bundle`. */
export function loadBundle(path?: string | URL): Promise<Uint8Array>
/** Synchronous {@link loadBundle}. */
export function loadBundleSync(path?: string | URL): Uint8Array
/** Creates a highlighter from bundle bytes or a path; defaults to the full bundle. */
export function createHighlighter(options?: {
  bundle?: Uint8Array | ArrayBuffer | ArrayBufferView | string | URL
}): Promise<Highlighter>
