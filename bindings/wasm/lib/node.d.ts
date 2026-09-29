export * from './core.js'
import type { Highlighter } from './core.js'

/** Where to load WebAssembly from: a path, URL, bytes, or compiled module. */
export type WasmSource = string | URL | Uint8Array | ArrayBuffer | ArrayBufferView | WebAssembly.Module

/**
 * Instantiates WebAssembly from `wasm`. Importing the package already does
 * this from the package's own `.wasm`; call it only when that file was not
 * found, such as after bundling. A no-op once initialized.
 */
export function init(wasm?: WasmSource): Promise<void>
/** Synchronous {@link init}. */
export function initSync(wasm?: WasmSource): void
/** Reads a grammar bundle; defaults to the package's full `grammars.bundle`. */
export function loadBundle(path?: string | URL): Promise<Uint8Array>
/** Synchronous {@link loadBundle}. */
export function loadBundleSync(path?: string | URL): Uint8Array
/**
 * Creates a highlighter from bundle bytes or a path; defaults to the full
 * bundle. `wasm` is used only if WebAssembly is not initialized yet.
 */
export function createHighlighter(options?: {
  bundle?: Uint8Array | ArrayBuffer | ArrayBufferView | string | URL
  wasm?: WasmSource
}): Promise<Highlighter>
