#!/usr/bin/env node
/** Differentially tokenize Mark fixtures with Shiki and pinned VS Code assets. */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const output = path.join(root, 'benchmarks/textmate/vscode-grammar-behavior.json')
const checkoutIndex = process.argv.indexOf('--vscode-checkout')
if (checkoutIndex < 0 || !process.argv[checkoutIndex + 1]) {
  throw new Error('usage: vscode-grammar-behavior-audit.mjs --vscode-checkout PATH [--check]')
}
const checkout = path.resolve(process.argv[checkoutIndex + 1])
const check = process.argv.includes('--check')
const revision = spawnSync('git', ['-C', checkout, 'rev-parse', 'HEAD'], { encoding: 'utf8' }).stdout.trim()
const expectedRevision = 'fc3def6774c76082adf699d366f31a557ce5573f'
if (revision !== expectedRevision) throw new Error(`expected VS Code ${expectedRevision}, got ${revision}`)

const exported = spawnSync('python3', [path.join(root, 'tools/export-vscode-grammars.py'), checkout], {
  encoding: 'utf8', maxBuffer: 128 * 1024 * 1024,
})
if (exported.status !== 0) throw new Error(exported.stderr)
const vscodeSpecs = new Map(Object.entries(JSON.parse(exported.stdout)))
const syntaxmateSpecs = new Map()
for (const name of await fs.readdir(path.join(root, 'assets/grammars/languages'))) {
  if (!name.endsWith('.tmLanguage.json')) continue
  const grammar = JSON.parse(await fs.readFile(path.join(root, 'assets/grammars/languages', name), 'utf8'))
  syntaxmateSpecs.set(grammar.scopeName, { path: `assets/grammars/languages/${name}`, grammar })
}

const sourceAudit = JSON.parse(await fs.readFile(path.join(root, 'benchmarks/textmate/vscode-grammar-differences.json'), 'utf8'))
// Root source equality is not enough to guarantee behavior equality: external
// includes are resolved through the registry, whose dependency set may differ.
// Audit every shared root so canonical-equal grammars still exercise their
// dependency closures (YAML is one such multi-file grammar).
const languages = sourceAudit.entries

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
const onigLib = Promise.resolve({
  createOnigScanner(patterns) { return new onig.OnigScanner(patterns) },
  createOnigString(source) { return new onig.OnigString(source) },
})
const theme = JSON.parse(await fs.readFile(path.join(root, 'assets/themes/github-dark-high-contrast.json'), 'utf8'))
const rawTheme = {
  settings: [
    { settings: { foreground: theme.colors['editor.foreground'], background: theme.colors['editor.background'] } },
    ...theme.tokenColors,
  ],
}
const syntaxmateRegistry = new vsctm.Registry({ theme: rawTheme, onigLib, loadGrammar: async scope => syntaxmateSpecs.get(scope)?.grammar ?? null })
const vscodeRegistry = new vsctm.Registry({ theme: rawTheme, onigLib, loadGrammar: async scope => vscodeSpecs.get(scope)?.grammar ?? null })

const entries = []
for (const language of languages) {
  const fixtures = []
  const fixtureDir = path.join(root, 'tests/fixtures/textmate', language.language)
  let fixtureNames = []
  try {
    fixtureNames = (await fs.readdir(fixtureDir))
      .filter(name => name.endsWith('.golden.jsonl') && !name.endsWith('.theme.golden.jsonl'))
      .sort()
  } catch {}
  for (const goldenName of fixtureNames) {
    const goldenPath = path.join(fixtureDir, goldenName)
    const first = JSON.parse((await fs.readFile(goldenPath, 'utf8')).split('\n')[0])
    fixtures.push({ name: goldenName.replace(/\.golden\.jsonl$/, ''), source: path.join(root, first.file) })
  }
  const result = {
    language: language.language,
    scopeName: language.scopeName,
    fixtures: [],
    scopeEquivalent: true,
    styleEquivalent: true,
  }
  try {
    const syntaxmateGrammar = await syntaxmateRegistry.loadGrammar(language.scopeName)
    const vscodeGrammar = await vscodeRegistry.loadGrammar(language.scopeName)
    if (!syntaxmateGrammar || !vscodeGrammar) throw new Error('root grammar failed to load')
    for (const fixture of fixtures) {
      const source = await fs.readFile(fixture.source, 'utf8')
      const compared = compareTokenization(
        syntaxmateGrammar,
        vscodeGrammar,
        syntaxmateRegistry.getColorMap(),
        vscodeRegistry.getColorMap(),
        source,
      )
      result.fixtures.push({ name: fixture.name, ...compared })
      result.scopeEquivalent &&= compared.scopeMismatchLines === 0
      result.styleEquivalent &&= compared.styleMismatchLines === 0
    }
  } catch (error) {
    result.scopeEquivalent = false
    result.styleEquivalent = false
    result.error = error.message
  }
  entries.push(result)
  console.log(`${result.language}: scopes=${result.scopeEquivalent ? 'equal' : 'different'} styles=${result.styleEquivalent ? 'equal' : 'different'}`)
}

const report = {
  schemaVersion: 2,
  vscodeCommit: expectedRevision,
  oracle: { vscodeTextmate: '9.2.0', vscodeOniguruma: '1.7.0' },
  sharedRootGrammars: entries.length,
  scopeEquivalent: entries.filter(entry => entry.scopeEquivalent).length,
  scopeDivergent: entries.filter(entry => !entry.scopeEquivalent).length,
  styleEquivalent: entries.filter(entry => entry.styleEquivalent).length,
  styleDivergent: entries.filter(entry => !entry.styleEquivalent).length,
  entries,
}
const serialized = `${JSON.stringify(report, null, 2)}\n`
if (check) {
  if (await fs.readFile(output, 'utf8') !== serialized) throw new Error('vscode-grammar-behavior.json is stale')
} else {
  await fs.writeFile(output, serialized)
}
if (report.scopeDivergent || report.styleDivergent) {
  throw new Error(
    `VS Code grammar behavior diverged: ${report.scopeDivergent} scope, ${report.styleDivergent} style`,
  )
}
console.log(`ok: scopes ${report.scopeEquivalent}/${entries.length}, styles ${report.styleEquivalent}/${entries.length}`)

function compareTokenization(syntaxmateGrammar, vscodeGrammar, syntaxmateColors, vscodeColors, source) {
  let syntaxmateState = vsctm.INITIAL
  let vscodeState = vsctm.INITIAL
  let scopeMismatchLines = 0
  let styleMismatchLines = 0
  let comparedTokens = 0
  let firstMismatch = null
  const lines = source.split('\n')
  for (let lineNumber = 0; lineNumber < lines.length; lineNumber++) {
    const syntaxmate = syntaxmateGrammar.tokenizeLine(lines[lineNumber], syntaxmateState, 0)
    const vscode = vscodeGrammar.tokenizeLine(lines[lineNumber], vscodeState, 0)
    const syntaxmateStyled = syntaxmateGrammar.tokenizeLine2(lines[lineNumber], syntaxmateState, 0)
    const vscodeStyled = vscodeGrammar.tokenizeLine2(lines[lineNumber], vscodeState, 0)
    syntaxmateState = syntaxmate.ruleStack
    vscodeState = vscode.ruleStack
    const left = syntaxmate.tokens.map(token => [token.startIndex, token.endIndex, token.scopes])
    const right = vscode.tokens.map(token => [token.startIndex, token.endIndex, token.scopes])
    comparedTokens += Math.max(left.length, right.length)
    if (JSON.stringify(left) !== JSON.stringify(right)) {
      scopeMismatchLines++
      firstMismatch ??= { lineNumber, line: lines[lineNumber], syntaxmate: left, vscode: right }
    }
    const leftStyles = styleRuns(syntaxmateStyled.tokens, syntaxmateColors, lines[lineNumber].length)
    const rightStyles = styleRuns(vscodeStyled.tokens, vscodeColors, lines[lineNumber].length)
    if (JSON.stringify(leftStyles) !== JSON.stringify(rightStyles)) styleMismatchLines++
  }
  return { lines: lines.length, comparedTokens, scopeMismatchLines, styleMismatchLines, firstMismatch }
}

function styleRuns(tokens, colorMap, lineLength) {
  const runs = []
  for (let index = 0; index < tokens.length; index += 2) {
    const start = tokens[index]
    const end = index + 2 < tokens.length ? tokens[index + 2] : lineLength
    const metadata = tokens[index + 1] >>> 0
    const fontStyle = (metadata & 0x00007800) >>> 11
    const style = [
      colorMap[(metadata & 0x00ff8000) >>> 15]?.toLowerCase() ?? null,
      colorMap[(metadata & 0xff000000) >>> 24]?.toLowerCase() ?? null,
      fontStyle,
    ]
    const previous = runs.at(-1)
    if (previous && previous[1] === start && JSON.stringify(previous[2]) === JSON.stringify(style)) previous[1] = end
    else runs.push([start, end, style])
  }
  return runs
}
