#!/usr/bin/env node

import fs from 'node:fs/promises'
import process from 'node:process'
import { createHighlighter } from 'shiki'

function parseArgs(argv) {
  const args = { minimumTimeMs: 100 }
  for (let index = 2; index < argv.length; index += 2) {
    const name = argv[index]
    const value = argv[index + 1]
    if (!value) throw new Error(`${name} requires a value`)
    if (name === '--language') args.language = value
    else if (name === '--file') args.file = value
    else if (name === '--phase') args.phase = value
    else if (name === '--minimum-time-ms') args.minimumTimeMs = Number(value)
    else throw new Error(`unknown option ${JSON.stringify(name)}`)
  }
  if (!args.language || !args.file || !args.phase) {
    throw new Error('--language, --file, and --phase are required')
  }
  if (!Number.isInteger(args.minimumTimeMs) || args.minimumTimeMs < 1) {
    throw new Error('--minimum-time-ms must be a positive integer')
  }
  if (!['cold', 'steady', 'replay'].includes(args.phase)) {
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

function calibrate(minimumTimeMs, operation) {
  const target = BigInt(minimumTimeMs) * 1_000_000n
  let iterations = 1
  while (true) {
    const started = process.hrtime.bigint()
    let output = ''
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
const packageJson = JSON.parse(
  await fs.readFile(new URL('package.json', import.meta.url), 'utf8'),
)

const setupStarted = process.hrtime.bigint()
const highlighter = await createHighlighter({
  langs: [args.language],
  themes: ['github-dark'],
})
const setupNanos = process.hrtime.bigint() - setupStarted
const operation = () => highlighter.codeToHtml(source, {
  lang: args.language,
  theme: 'github-dark',
})

let measured
if (args.phase === 'cold') {
  const started = process.hrtime.bigint()
  const output = operation()
  measured = { iterations: 1, elapsed: process.hrtime.bigint() - started, output }
} else {
  operation()
  measured = calibrate(args.minimumTimeMs, operation)
}

console.log(JSON.stringify({
  schemaVersion: 1,
  track: 'end-to-end',
  engine: 'shiki',
  version: packageJson.dependencies.shiki,
  regexEngine: `@shikijs/engine-oniguruma@${packageJson.dependencies.shiki} (WebAssembly)`,
  phase: args.phase,
  iterations: measured.iterations,
  sourceBytes: Buffer.byteLength(source),
  processedBytes: Buffer.byteLength(source) * measured.iterations,
  setupNanoseconds: Number(setupNanos),
  elapsedNanoseconds: Number(measured.elapsed),
  outputBytes: Buffer.byteLength(measured.output),
  outputDigest: fnv1a(measured.output),
  complete: true,
}))

highlighter.dispose()
