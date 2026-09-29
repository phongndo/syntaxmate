#!/usr/bin/env node
// Phase-separated package profiling. Each sample is a fresh Node process.
// Alternate --packages baseline,candidate order. JSON includes every sample;
// compare its output digests before interpreting latency changes.
// node bench/profile-node.mjs --packages ../baseline,. --file input.rs --samples 7
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { readFileSync, statSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { profile } from './profile.mjs'

const args = Object.fromEntries(Array.from({ length: (process.argv.length - 2) / 2 }, (_, index) =>
  process.argv.slice(2 + index * 2, 4 + index * 2)))
const packages = (args['--packages'] ?? '.').split(',').map(value => path.resolve(value))
const file = path.resolve(args['--file'] ?? '../../tests/fixtures/textmate/rust/stress.rs')
const language = args['--language'] ?? 'rust'
const iterations = Number(args['--iterations'] ?? 30)
const samples = Number(args['--samples'] ?? 7)
if (args['--child']) {
  const source = readFileSync(file, 'utf8')
  const pkg = path.resolve(args['--child'])
  let started = performance.now()
  const importStartedAtMs = performance.timeOrigin + started
  const api = await import(pathToFileURL(path.join(pkg, 'lib/node.js')))
  const importMs = performance.now() - started
  started = performance.now()
  const bundle = await api.loadBundle()
  const bundleReadMs = performance.now() - started
  const raw = await import(pathToFileURL(path.join(pkg, 'dist/syntaxmate.js')))
  console.log(JSON.stringify({ importStartedAtMs, importMs, bundleReadMs,
    ...(await profile(api, raw, bundle, source, { language, iterations })) }))
} else {
  if (!Number.isSafeInteger(samples) || samples <= 0) throw new Error('samples must be positive')
  const results = []
  for (let index = 0; index < samples; index++) {
    for (const pkg of index % 2 ? packages.toReversed() : packages) {
      const started = performance.now()
      const startedAtMs = performance.timeOrigin + started
      const child = spawnSync(process.execPath, [fileURLToPath(import.meta.url),
        '--child', pkg, '--file', file, '--language', language, '--iterations', String(iterations)],
      { encoding: 'utf8' })
      const processTotalMs = performance.now() - started
      if (child.status !== 0) throw new Error(child.stderr || `child failed: ${child.status}`)
      const result = JSON.parse(child.stdout)
      results.push({ package: pkg, sample: index, processTotalMs,
        processBeforeImportMs: result.importStartedAtMs - startedAtMs,
        processColdMs: result.firstTokensFinishedAtMs - startedAtMs, ...result })
    }
  }
  for (const result of results) {
    const reference = results[0]
    for (const key of ['sourceDigest', 'outputs']) {
      if (JSON.stringify(result[key]) !== JSON.stringify(reference[key])) throw new Error(`${key} differs`)
    }
    for (const [phase, value] of Object.entries(result.phases)) {
      if (value.outputDigest !== reference.phases[phase].outputDigest) throw new Error(`${phase} output differs`)
    }
  }
  console.log(JSON.stringify({ environment: { node: process.version, platform: os.platform(),
    release: os.release(), cpu: os.cpus()[0]?.model, file, language,
    sourceSha256: createHash('sha256').update(readFileSync(file)).digest('hex'),
    packageSizes: packages.map(pkg => ({ package: pkg,
      wasmBytes: statSync(path.join(pkg, 'dist/syntaxmate_bg.wasm')).size,
      bundleBytes: statSync(path.join(pkg, 'grammars.bundle')).size })) }, results }, null, 2))
}
