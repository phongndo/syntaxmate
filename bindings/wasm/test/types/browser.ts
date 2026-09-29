// Type-checked, never run, against lib/*.d.ts by `npm run typecheck`.
import { createHighlighter, init, loadBundle, type WasmInput } from '../../lib/browser.js'

export async function main(): Promise<void> {
  const wasm: WasmInput = new URL('https://example.com/syntaxmate.wasm')
  await init(wasm)
  await init(fetch('/syntaxmate.wasm'))
  const bundle: Uint8Array = await loadBundle('/grammars.bundle')
  const highlighter = await createHighlighter({ bundle, wasm: new Uint8Array() })
  highlighter.free()
}
