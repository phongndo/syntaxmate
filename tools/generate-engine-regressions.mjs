#!/usr/bin/env node
// Small custom-grammar regressions, independent of the bundled-catalog contract
// and the scanner-execution difference ledger. Never hand-edit the output.
import fs from 'node:fs/promises'
import { createRequire } from 'node:module'
import path from 'node:path'

const require = createRequire(new URL('./golden-oracle/package.json', import.meta.url))
const onig = require('vscode-oniguruma')
const textmate = require('vscode-textmate')
const wasm = await fs.readFile(require.resolve('vscode-oniguruma/release/onig.wasm'))
await onig.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength))
const directory = new URL('../tests/fixtures/engine-regressions/', import.meta.url)
const cases = JSON.parse(await fs.readFile(new URL('cases.json', directory), 'utf8'))
const output = []
for (const testCase of cases) {
  for (const source of testCase.sources) {
    const registry = new textmate.Registry({
      onigLib: Promise.resolve({
        createOnigScanner: patterns => new onig.OnigScanner(patterns),
        createOnigString: value => new onig.OnigString(value),
      }),
      loadGrammar: async scope => testCase.grammars.find(grammar => grammar.scopeName === scope) ?? null,
    })
    try {
      const grammar = await registry.loadGrammar(testCase.grammars[0].scopeName)
      let state = textmate.INITIAL
      const lines = []
      for (const line of source.split('\n')) {
        const result = grammar.tokenizeLine(line, state, 0)
        if (result.stoppedEarly) throw new Error(`oracle stopped: ${testCase.name}`)
        state = result.ruleStack
        lines.push(result.tokens.flatMap(token => {
          // Public Rust line spans omit the oracle's synthetic newline and
          // empty spans. Convert UTF-16 boundaries, not code point counts.
          const start = Buffer.byteLength(line.slice(0, token.startIndex))
          const end = Buffer.byteLength(line.slice(0, token.endIndex))
          return start < end ? [{ start, end, scopes: token.scopes }] : []
        }))
      }
      output.push({ name: testCase.name, source, lines })
    } finally {
      registry.dispose()
    }
  }
}
const destination = new URL('scopes.golden.jsonl', directory)
const generated = output.map(record => JSON.stringify(record)).join('\n') + '\n'
if (process.argv.includes('--check')) {
  if (await fs.readFile(destination, 'utf8') !== generated) {
    throw new Error(`stale generated engine regressions: ${path.basename(destination.pathname)}`)
  }
} else {
  await fs.writeFile(destination, generated)
}
console.log(`engine regressions: ${output.length} oracle documents${process.argv.includes('--check') ? ' checked' : ' generated'}`)
