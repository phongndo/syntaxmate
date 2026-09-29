import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const pkg = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8'))

// The Node entry instantiates WebAssembly at import time. If package.json
// declared it side-effect-free, bundlers such as Rollup would skip it when a
// consumer imports only re-exported names, leaving WebAssembly uninitialized.
test('sideEffects retains the Node entry, which initializes at import', () => {
  const nodeEntry = pkg.exports['.'].node.default
  assert.equal(nodeEntry, './lib/node.js')
  assert.ok(Array.isArray(pkg.sideEffects), 'sideEffects must list side-effectful files')
  assert.ok(pkg.sideEffects.includes(nodeEntry), `sideEffects must include ${nodeEntry}`)
})
