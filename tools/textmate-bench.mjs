#!/usr/bin/env node
/** Standalone pinned vscode-textmate benchmark with no token serialization. */

import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'

function usage() {
  console.log(`usage: textmate-bench.mjs --scope <scope> --file <source> [options]

Options:
  --assets <directory>    Register every *.json grammar in the directory.
  --grammar <json>        Register the root grammar directly.
  --iterations <count>    Full-file passes in this process (default: 1).
  --mode <name>           process-cold or same-driver (default: process-cold).
  --time-limit <ms>       tokenizeLine limit (default: 0, disabled).
  --json                  Emit one JSON result (default output format).

Process-cold means this command is started once per sample with one iteration.
Same-driver loads source/grammars once and repeats only full-file tokenization.`)
}

function parseArgs(argv) {
  const args = { iterations: 1, mode: 'process-cold', timeLimit: 0 }
  for (let index = 2; index < argv.length; index++) {
    const raw = argv[index]
    if (raw === '-h' || raw === '--help') return { help: true }
    if (raw === '--json') continue
    const equals = raw.indexOf('=')
    const name = equals > 0 ? raw.slice(0, equals) : raw
    const value = equals > 0 ? raw.slice(equals + 1) : argv[++index]
    if (!value || value.startsWith('--')) throw new Error(`${name} requires a value`)
    if (name === '--iterations') args.iterations = Number(value)
    else if (name === '--time-limit') args.timeLimit = Number(value)
    else if (name === '--scope') args.scope = value
    else if (name === '--file') args.file = value
    else if (name === '--assets') args.assets = value
    else if (name === '--grammar') args.grammar = value
    else if (name === '--mode') args.mode = value
    else throw new Error(`unknown argument: ${name}`)
  }
  if (!Number.isInteger(args.iterations) || args.iterations < 1) {
    throw new Error('--iterations must be a positive integer')
  }
  if (!Number.isFinite(args.timeLimit) || args.timeLimit < 0) {
    throw new Error('--time-limit must be non-negative')
  }
  if (!['process-cold', 'same-driver'].includes(args.mode)) {
    throw new Error('--mode must be process-cold or same-driver')
  }
  if (args.mode === 'process-cold' && args.iterations !== 1) {
    throw new Error('process-cold requires --iterations 1; launch a fresh process per sample')
  }
  if (!args.scope || !args.file || (!args.assets && !args.grammar)) {
    throw new Error('--scope, --file, and either --assets or --grammar are required')
  }
  return args
}

let args
try {
  args = parseArgs(process.argv)
} catch (error) {
  console.error(error.message)
  usage()
  process.exit(2)
}
if (args.help) {
  usage()
  process.exit(0)
}

const require = createRequire(import.meta.url)
const toolDir = path.dirname(fileURLToPath(import.meta.url))
const resolvePaths = [path.join(toolDir, 'golden-oracle')]
function resolvePackage(name) {
  try {
    return require.resolve(name, { paths: resolvePaths })
  } catch (error) {
    throw new Error(
      `failed to resolve ${name}; run npm install --prefix tools/golden-oracle\n${error.message}`,
    )
  }
}
async function importPackage(name) {
  return import(pathToFileURL(resolvePackage(name)).href)
}

const setupStarted = process.hrtime.bigint()
const vsctmModule = await importPackage('vscode-textmate')
const vsctm = vsctmModule.default ?? vsctmModule
const onigModule = await importPackage('vscode-oniguruma')
const onig = onigModule.default ?? onigModule
const onigMain = resolvePackage('vscode-oniguruma')
let wasmPath = path.join(path.dirname(onigMain), 'release', 'onig.wasm')
try { await fs.access(wasmPath) } catch { wasmPath = path.join(path.dirname(onigMain), 'onig.wasm') }
const wasm = await fs.readFile(wasmPath)
await onig.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength))

const grammars = new Map()
async function registerGrammar(grammarPath) {
  const parsed = JSON.parse(await fs.readFile(grammarPath, 'utf8'))
  if (typeof parsed.scopeName === 'string') grammars.set(parsed.scopeName, parsed)
}
if (args.assets) {
  const names = (await fs.readdir(args.assets)).filter(name => name.endsWith('.json')).sort()
  for (const name of names) await registerGrammar(path.join(args.assets, name))
}
if (args.grammar) await registerGrammar(args.grammar)

const registry = new vsctm.Registry({
  onigLib: Promise.resolve({
    createOnigScanner(patterns) { return new onig.OnigScanner(patterns) },
    createOnigString(source) { return new onig.OnigString(source) },
  }),
  loadGrammar: async scope => grammars.get(scope) ?? null,
})
const grammar = await registry.loadGrammar(args.scope)
if (!grammar) throw new Error(`failed to load grammar ${args.scope}`)
const source = await fs.readFile(args.file, 'utf8')
const lines = source.split('\n')
const setupNanos = process.hrtime.bigint() - setupStarted

let tokenCount = 0
let stoppedEarly = false
const highlightStarted = process.hrtime.bigint()
for (let iteration = 0; iteration < args.iterations; iteration++) {
  let stack = vsctm.INITIAL
  for (const line of lines) {
    const result = grammar.tokenizeLine(line, stack, args.timeLimit)
    stack = result.ruleStack
    tokenCount += result.tokens.length
    stoppedEarly ||= Boolean(result.stoppedEarly)
  }
}
const highlightNanos = process.hrtime.bigint() - highlightStarted
const bytes = Buffer.byteLength(source) * args.iterations
const seconds = Number(highlightNanos) / 1e9
const packageJson = JSON.parse(
  await fs.readFile(path.join(toolDir, 'golden-oracle', 'package.json'), 'utf8'),
)
console.log(JSON.stringify({
  engine: 'vscode-textmate',
  mode: args.mode,
  scope: args.scope,
  file: args.file,
  iterations: args.iterations,
  sourceBytes: Buffer.byteLength(source),
  bytes,
  lines: lines.length * args.iterations,
  setupMicros: Math.round(Number(setupNanos) / 1e3),
  highlightMicros: Math.round(Number(highlightNanos) / 1e3),
  bytesPerSecond: Math.round(bytes / seconds),
  megabytesPerSecond: bytes / seconds / 1e6,
  tokens: tokenCount,
  stoppedEarly,
  versions: {
    vscodeTextmate: packageJson.dependencies['vscode-textmate'],
    vscodeOniguruma: packageJson.dependencies['vscode-oniguruma'],
    node: process.version,
  },
}, null, 2))
