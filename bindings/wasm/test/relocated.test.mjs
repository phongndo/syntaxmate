// A bundler that inlines lib/node.js elsewhere leaves `../dist/syntaxmate_bg.wasm`
// behind. Simulate that layout: import must still succeed, first use must say
// how to recover, and an explicit wasm location must work.
import assert from 'node:assert/strict'
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath, pathToFileURL } from 'node:url'

const pkg = fileURLToPath(new URL('..', import.meta.url))
const wasm = join(pkg, 'dist', 'syntaxmate_bg.wasm')
const bundle = join(pkg, 'grammars.bundle')

async function relocated() {
  const root = mkdtempSync(join(tmpdir(), 'syntaxmate-relocated-'))
  mkdirSync(join(root, 'lib'))
  mkdirSync(join(root, 'dist'))
  for (const file of ['node.js', 'core.js']) {
    copyFileSync(join(pkg, 'lib', file), join(root, 'lib', file))
  }
  copyFileSync(join(pkg, 'dist', 'syntaxmate.js'), join(root, 'dist', 'syntaxmate.js'))
  const module = await import(pathToFileURL(join(root, 'lib', 'node.js')).href)
  return { module, cleanup: () => rmSync(root, { recursive: true, force: true }) }
}

test('a missing default .wasm defers initialization with a clear error', async () => {
  const { module, cleanup } = await relocated()
  try {
    assert.throws(() => module.version(), /was not found .*init\(wasm\)/)
    await module.init(wasm)
    assert.equal(typeof module.version(), 'string')
    const highlighter = module.Highlighter.fromBundle(readFileSync(bundle))
    assert.match(highlighter.html('fn main() {}', { lang: 'rust', theme: 'github-dark' }), /<pre/)
  } finally {
    cleanup()
  }
})

test('createHighlighter accepts wasm bytes and a bundle path when relocated', async () => {
  const { module, cleanup } = await relocated()
  try {
    const highlighter = await module.createHighlighter({ wasm: readFileSync(wasm), bundle })
    assert.ok(highlighter.languages().includes('rust'))
  } finally {
    cleanup()
  }
})
