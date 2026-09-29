export * from './core.js'
import type { Highlighter } from './core.js'

/** Where to load the `.wasm` from; defaults to the file beside the package module. */
export type WasmInput =
  | string
  | URL
  | Request
  | Response
  | BufferSource
  | WebAssembly.Module
  | Promise<Response>

/** Fetches and instantiates WebAssembly once; required before other APIs. */
export function init(wasm?: WasmInput): Promise<void>
/** Fetches a grammar bundle; defaults to the package's full `grammars.bundle`. */
export function loadBundle(url?: string | URL): Promise<Uint8Array>
/** Initializes WebAssembly and creates a highlighter from bundle bytes or a URL. */
export function createHighlighter(options?: {
  bundle?: Uint8Array | ArrayBuffer | ArrayBufferView | string | URL
  wasm?: WasmInput
}): Promise<Highlighter>
