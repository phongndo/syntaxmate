#!/usr/bin/env node
/** Compare every committed fixture scope stack with the vscode-textmate theme oracle. */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const fixtureRoot = path.join(root, 'tests/fixtures/textmate')
const output = path.join(root, 'benchmarks/textmate/theme-parity.json')
const check = process.argv.includes('--check')
const themeName = 'github-dark-high-contrast'
const theme = JSON.parse(await fs.readFile(path.join(root, `assets/themes/${themeName}.json`), 'utf8'))
const files = await recursiveGoldens(fixtureRoot)
const coverage = await fs.readFile(path.join(root, 'assets/grammars/coverage.toml'), 'utf8')
const publicLanguages = Number(coverage.match(/^public_language_count\s*=\s*(\d+)$/m)?.[1])
if (!Number.isInteger(publicLanguages)) throw new Error('coverage.toml lacks public_language_count')
const unique = new Map()
const languages = new Set()
let fixtureTokens = 0
for (const file of files) {
  for (const line of (await fs.readFile(file, 'utf8')).split('\n')) {
    if (!line) continue
    const record = JSON.parse(line)
    if (record.language) languages.add(record.language)
    for (const token of record.tokens ?? []) {
      fixtureTokens++
      unique.set(JSON.stringify(token.scopes), token.scopes)
    }
  }
}
const stacks = [...unique.values()]
const separator = '\x1f'
const maxDepth = Math.max(...stacks.map(stack => stack.length))
const patterns = []
for (let depth = 1; depth <= maxDepth; depth++) {
  patterns.push({
    match: `^${Array(depth).fill('([^\\x1f\\n]+)').join('\\x1f')}$`,
    name: Array.from({ length: depth }, (_, index) => `$${index + 1}`).join(' '),
  })
}
const grammarSource = { scopeName: 'mark.theme.oracle', patterns }
const require = createRequire(import.meta.url)
const resolvePaths = [path.join(root, 'tools/golden-oracle')]
const resolvePackage = name => require.resolve(name, { paths: resolvePaths })
const importPackage = async name => import(pathToFileURL(resolvePackage(name)).href)
const vsctmModule = await importPackage('vscode-textmate')
const vsctm = vsctmModule.default ?? vsctmModule
const onigModule = await importPackage('vscode-oniguruma')
const onig = onigModule.default ?? onigModule
const onigMain = resolvePackage('vscode-oniguruma')
const wasm = await fs.readFile(path.join(path.dirname(onigMain), 'onig.wasm'))
await onig.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength))
const registry = new vsctm.Registry({
  theme: {
    settings: [
      { settings: {
        foreground: theme.colors['editor.foreground'],
        background: theme.colors['editor.background'],
      } },
      ...theme.tokenColors,
    ],
  },
  onigLib: Promise.resolve({
    createOnigScanner(patterns) { return new onig.OnigScanner(patterns) },
    createOnigString(source) { return new onig.OnigString(source) },
  }),
  loadGrammar: async scope => scope === grammarSource.scopeName ? grammarSource : null,
})
const grammar = await registry.loadGrammar(grammarSource.scopeName)
const colorMap = registry.getColorMap()
const expected = stacks.map(scopes => {
  const result = grammar.tokenizeLine2(scopes.join(separator), vsctm.INITIAL)
  return decode(result.tokens[1] >>> 0, colorMap)
})
const rustInput = stacks.map(scopes => JSON.stringify([grammarSource.scopeName, ...scopes])).join('\n') + '\n'
const rust = spawnSync(
  'cargo',
  ['run', '--quiet', '-p', 'syntaxmate', '--example', 'theme-resolve', '--', themeName],
  { cwd: root, input: rustInput, encoding: 'utf8', maxBuffer: 128 * 1024 * 1024 },
)
if (rust.status !== 0) throw new Error(rust.stderr || `Rust resolver exited ${rust.status}`)
const actual = rust.stdout.trimEnd().split('\n').map(JSON.parse)
const mismatches = []
for (let index = 0; index < stacks.length; index++) {
  if (!sameStyle(actual[index], expected[index])) {
    mismatches.push({ scopes: stacks[index], expected: expected[index], actual: actual[index] })
  }
}
const report = `${JSON.stringify({
  schemaVersion: 1,
  oracle: { vscodeTextmate: '9.2.0', vscodeOniguruma: '1.7.0', semanticHighlighting: false },
  theme: themeName,
  fixtureFiles: files.length,
  publicLanguages,
  fixtureLanguageLabels: languages.size,
  fixtureTokens,
  uniqueScopeStacks: stacks.length,
  mismatches,
}, null, 2)}\n`
if (check) {
  const committed = await fs.readFile(output, 'utf8')
  if (committed !== report) throw new Error('theme parity report is stale; run node tools/theme-catalog-parity.mjs')
} else {
  await fs.mkdir(path.dirname(output), { recursive: true })
  await fs.writeFile(output, report)
}
if (mismatches.length) throw new Error(`${mismatches.length} catalog theme mismatches`)
console.log(`ok: ${stacks.length} unique stacks, ${fixtureTokens} tokens, ${publicLanguages} public languages`)

async function recursiveGoldens(directory) {
  const results = []
  for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
    const item = path.join(directory, entry.name)
    if (entry.isDirectory()) results.push(...await recursiveGoldens(item))
    else if (entry.name.endsWith('.golden.jsonl') && !entry.name.includes('.theme.')) results.push(item)
  }
  return results.sort()
}

function decode(metadata, colorMap) {
  const fontStyle = (metadata & 0x00007800) >>> 11
  const modifiers = []
  if (fontStyle & 1) modifiers.push('italic')
  if (fontStyle & 2) modifiers.push('bold')
  if (fontStyle & 4) modifiers.push('underline')
  if (fontStyle & 8) modifiers.push('strikethrough')
  return {
    foreground: normalizeColor(colorMap[(metadata & 0x00ff8000) >>> 15]),
    background: normalizeColor(colorMap[(metadata & 0xff000000) >>> 24]),
    modifiers,
  }
}

function normalizeColor(value) {
  if (!value) return null
  const hex = value.toLowerCase()
  if (/^#[0-9a-f]{3,4}$/.test(hex)) return `#${[...hex.slice(1, 4)].map(c => c + c).join('')}`
  return hex.length === 9 ? hex.slice(0, 7) : hex
}

function sameStyle(left, right) {
  return left?.foreground === right?.foreground && left?.background === right?.background &&
    JSON.stringify(left?.modifiers) === JSON.stringify(right?.modifiers)
}
