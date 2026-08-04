#!/usr/bin/env node

import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'

function parseArgs(argv) {
  const args = { minimumTimeMs: 100 }
  for (let index = 2; index < argv.length; index += 2) {
    const name = argv[index]
    const value = argv[index + 1]
    if (!value) throw new Error(`${name} requires a value`)
    if (name === '--assets') args.assets = value
    else if (name === '--scope') args.scope = value
    else if (name === '--file') args.file = value
    else if (name === '--phase') args.phase = value
    else if (name === '--minimum-time-ms') args.minimumTimeMs = Number(value)
    else throw new Error(`unknown option ${JSON.stringify(name)}`)
  }
  if (!args.assets || !args.scope || !args.file || !args.phase) {
    throw new Error('--assets, --scope, --file, and --phase are required')
  }
  if (!Number.isInteger(args.minimumTimeMs) || args.minimumTimeMs < 1) {
    throw new Error('--minimum-time-ms must be a positive integer')
  }
  if (!['first', 'steady', 'replay'].includes(args.phase)) {
    throw new Error(`unsupported phase ${JSON.stringify(args.phase)}`)
  }
  return args
}

function fnv1a(value) {
  let hash = 0xcbf29ce484222325n
  for (const byte of Buffer.from(value)) {
    hash ^= BigInt(byte)
    hash = BigInt.asUintN(64, hash * 0x100000001b3n)
  }
  return hash.toString(16).padStart(16, '0')
}

function utf16ToUtf8(line, target) {
  let utf16 = 0
  let bytes = 0
  for (const character of line) {
    if (utf16 === target) return bytes
    utf16 += character.length
    bytes += Buffer.byteLength(character)
    if (utf16 > target) throw new Error(`offset ${target} splits a UTF-16 character`)
  }
  if (target === utf16 || target === utf16 + 1) return bytes
  throw new Error(`offset ${target} exceeds line length ${utf16}`)
}

function scopeDigest(lines, records) {
  let canonical = ''
  for (let lineIndex = 0; lineIndex < records.length; lineIndex++) {
    const line = lines[lineIndex]
    const coalesced = []
    for (const token of records[lineIndex]) {
      const range = {
        start: utf16ToUtf8(line, token.startIndex),
        end: utf16ToUtf8(line, token.endIndex),
      }
      if (range.start >= range.end) continue
      const previous = coalesced.at(-1)
      if (
        previous
        && previous.end === range.start
        && previous.scopes.length === token.scopes.length
        && previous.scopes.every((scope, index) => scope === token.scopes[index])
      ) {
        previous.end = range.end
      } else {
        coalesced.push({ ...range, scopes: token.scopes })
      }
    }
    for (const token of coalesced) {
      canonical += `${lineIndex}:${token.start}:${token.end}:`
      for (const scope of token.scopes) canonical += `${scope}\x1f`
      canonical += '\n'
    }
  }
  return fnv1a(canonical)
}

function calibrate(minimumTimeMs, operation) {
  const target = BigInt(minimumTimeMs) * 1_000_000n
  let iterations = 1
  while (true) {
    const started = process.hrtime.bigint()
    let output
    for (let index = 0; index < iterations; index++) output = operation()
    const elapsed = process.hrtime.bigint() - started
    if (elapsed >= target || iterations >= 16_384) {
      return { iterations, elapsed, output }
    }
    iterations *= 2
  }
}

const args = parseArgs(process.argv)
const source = await fs.readFile(args.file, 'utf8')
const lines = source.split('\n')
const packageJson = JSON.parse(
  await fs.readFile(new URL('package.json', import.meta.url), 'utf8'),
)

const setupStarted = process.hrtime.bigint()
const require = createRequire(import.meta.url)
const root = path.dirname(fileURLToPath(import.meta.url))
const resolvePaths = [root]
const resolvePackage = name => require.resolve(name, { paths: resolvePaths })
const importPackage = async name => import(pathToFileURL(resolvePackage(name)).href)
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
const assetNames = (await fs.readdir(args.assets)).filter(name => name.endsWith('.json')).sort()
for (const name of assetNames) {
  const grammar = JSON.parse(await fs.readFile(path.join(args.assets, name), 'utf8'))
  if (typeof grammar.scopeName === 'string') grammars.set(grammar.scopeName, grammar)
}
const registry = new vsctm.Registry({
  onigLib: Promise.resolve({
    createOnigScanner(patterns) { return new onig.OnigScanner(patterns) },
    createOnigString(value) { return new onig.OnigString(value) },
  }),
  loadGrammar: async scope => grammars.get(scope) ?? null,
})
const grammar = await registry.loadGrammar(args.scope)
if (!grammar) throw new Error(`failed to load grammar ${args.scope}`)
const setupNanos = process.hrtime.bigint() - setupStarted

const operation = () => {
  let stack = vsctm.INITIAL
  const records = []
  let stoppedEarly = false
  for (const line of lines) {
    const result = grammar.tokenizeLine(line, stack, 0)
    stack = result.ruleStack
    records.push(result.tokens)
    stoppedEarly ||= Boolean(result.stoppedEarly)
  }
  return { records, stoppedEarly }
}

let measured
if (args.phase === 'first') {
  const started = process.hrtime.bigint()
  const output = operation()
  measured = { iterations: 1, elapsed: process.hrtime.bigint() - started, output }
} else {
  operation()
  measured = calibrate(args.minimumTimeMs, operation)
}
if (measured.output.stoppedEarly) throw new Error('vscode-textmate stopped early')
const tokens = measured.output.records.reduce((sum, line) => sum + line.length, 0)

console.log(JSON.stringify({
  schemaVersion: 1,
  track: 'engine',
  engine: 'vscode-textmate',
  version: packageJson.dependencies['vscode-textmate'],
  regexEngine: `vscode-oniguruma@${packageJson.dependencies['vscode-oniguruma']}`,
  phase: args.phase,
  iterations: measured.iterations,
  sourceBytes: Buffer.byteLength(source),
  processedBytes: Buffer.byteLength(source) * measured.iterations,
  setupNanoseconds: Number(setupNanos),
  elapsedNanoseconds: Number(measured.elapsed),
  tokens,
  scopeDigest: scopeDigest(lines, measured.output.records),
  complete: true,
}))
