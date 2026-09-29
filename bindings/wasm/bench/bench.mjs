#!/usr/bin/env node
// Compares the syntaxmate npm package with Shiki on HTML highlighting in Node,
// using the stress fixtures and pinned Shiki from benchmarks/competitors.
// Every sample runs in a fresh process; engine order alternates per sample.
//
//   npm ci --prefix ../../benchmarks/competitors --ignore-scripts   # once
//   node bench/bench.mjs [--samples 5] [--minimum-time-ms 200]
//                        [--languages rust,python] [--out results.json]
//
// Phases, all with theme github-dark and default HTML options:
// - cold:   import + WebAssembly/grammar setup + first codeToHtml/html call.
// - steady: warm throughput over 16 variants of the fixture that differ only
//           by trailing spaces, so Syntaxmate's per-line result cache cannot hit.
// - replay: warm throughput re-highlighting the identical document, which
//           Syntaxmate's line cache serves; Shiki has no equivalent cache.

import { execFileSync, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../../..')
const competitors = path.join(root, 'benchmarks/competitors')
const shikiEntry = path.join(competitors, 'node_modules/shiki/dist/index.mjs')
const syntaxmateEntry = path.resolve(here, '../lib/node.js')
const LANGUAGES = [
  'bash', 'cpp', 'html', 'java', 'json', 'markdown', 'python', 'rust', 'typescript', 'yaml',
]
const THEME = 'github-dark'
const VARIANTS = 16

function parseArgs(argv) {
  const args = { samples: 5, minimumTimeMs: 200, languages: LANGUAGES }
  for (let index = 2; index < argv.length; index++) {
    const name = argv[index]
    if (name === '--driver') {
      args.driver = true
      continue
    }
    const value = argv[++index]
    if (value === undefined) throw new Error(`${name} requires a value`)
    if (name === '--samples') args.samples = Number(value)
    else if (name === '--minimum-time-ms') args.minimumTimeMs = Number(value)
    else if (name === '--languages') args.languages = value.split(',').filter(Boolean)
    else if (name === '--out') args.out = value
    else if (name === '--engine') args.engine = value
    else if (name === '--phase') args.phase = value
    else if (name === '--language') args.language = value
    else if (name === '--file') args.file = value
    else throw new Error(`unknown option ${JSON.stringify(name)}`)
  }
  return args
}

function fixture(language) {
  const dir = path.join(root, 'tests/fixtures/textmate', language)
  const name = fs.readdirSync(dir).find((entry) => /^stress\.[^.]+$/.test(entry))
  if (!name) throw new Error(`no stress fixture for ${language}`)
  return path.join(dir, name)
}

function variants(source) {
  return Array.from({ length: VARIANTS }, (_, variant) => {
    const pad = ' '.repeat(variant + 1)
    return source.split('\n').map((line) => line + pad).join('\n')
  })
}

function now() {
  return process.hrtime.bigint()
}

function ms(nanos) {
  return Number(nanos) / 1e6
}

async function load(engine, language) {
  if (engine === 'syntaxmate') {
    const { createHighlighter } = await import(pathToFileURL(syntaxmateEntry))
    const imported = now()
    const highlighter = await createHighlighter()
    return {
      imported,
      html: (code) => highlighter.html(code, { lang: language, theme: THEME }),
    }
  }
  const { createHighlighter } = await import(pathToFileURL(shikiEntry))
  const imported = now()
  const highlighter = await createHighlighter({ langs: [language], themes: [THEME] })
  return {
    imported,
    html: (code) => highlighter.codeToHtml(code, { lang: language, theme: THEME }),
  }
}

async function driver(args) {
  const source = fs.readFileSync(args.file, 'utf8')
  const started = now()
  const { imported, html } = await load(args.engine, args.language)
  const ready = now()
  if (args.phase === 'cold') {
    const output = html(source)
    const done = now()
    return {
      importMs: ms(imported - started),
      setupMs: ms(ready - imported),
      firstMs: ms(done - ready),
      totalMs: ms(done - started),
      outputBytes: Buffer.byteLength(output),
    }
  }
  const inputs = args.phase === 'steady' ? variants(source) : [source]
  for (const input of inputs) html(input)
  let iterations = inputs.length
  for (;;) {
    let bytes = 0
    const begin = now()
    for (let index = 0; index < iterations; index++) {
      const input = inputs[index % inputs.length]
      bytes += Buffer.byteLength(input)
      html(input)
    }
    const elapsed = now() - begin
    if (ms(elapsed) >= args.minimumTimeMs || iterations >= 1 << 20) {
      return {
        iterations,
        elapsedMs: ms(elapsed),
        mibPerSecond: bytes / (1 << 20) / (Number(elapsed) / 1e9),
        msPerDocument: ms(elapsed) / iterations,
      }
    }
    iterations *= 2
  }
}

function median(values) {
  const sorted = [...values].sort((a, b) => a - b)
  const middle = sorted.length >> 1
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2
}

function environment() {
  const version = (dir) => JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8')).version
  return {
    date: new Date().toISOString(),
    node: process.version,
    platform: `${os.type()} ${os.release()} ${os.arch()}`,
    cpu: os.cpus()[0]?.model,
    cpus: os.cpus().length,
    memoryGiB: Math.round(os.totalmem() / 2 ** 30),
    syntaxmate: version(path.resolve(here, '..')),
    shiki: version(path.join(competitors, 'node_modules/shiki')),
    commit: execFileSync('git', ['rev-parse', '--short', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
    wasmBytes: fs.statSync(path.resolve(here, '../dist/syntaxmate_bg.wasm')).size,
  }
}

async function main() {
  const args = parseArgs(process.argv)
  if (args.driver) {
    console.log(JSON.stringify(await driver(args)))
    return
  }
  if (!fs.existsSync(shikiEntry)) {
    throw new Error(`missing ${shikiEntry}; run npm ci --prefix benchmarks/competitors --ignore-scripts`)
  }
  const engines = ['syntaxmate', 'shiki']
  const results = []
  for (const phase of ['cold', 'steady', 'replay']) {
    for (const language of args.languages) {
      const file = fixture(language)
      const samples = { syntaxmate: [], shiki: [] }
      for (let sample = 0; sample < args.samples; sample++) {
        const order = sample % 2 ? [...engines].reverse() : engines
        for (const engine of order) {
          const child = spawnSync(process.execPath, [
            fileURLToPath(import.meta.url), '--driver', '--engine', engine, '--phase', phase,
            '--language', language, '--file', file, '--minimum-time-ms', String(args.minimumTimeMs),
          ], { encoding: 'utf8' })
          if (child.status !== 0) throw new Error(`${engine} ${phase} ${language} failed:\n${child.stderr}`)
          samples[engine].push(JSON.parse(child.stdout))
        }
      }
      for (const engine of engines) {
        const runs = samples[engine]
        const summary = phase === 'cold'
          ? Object.fromEntries(['importMs', 'setupMs', 'firstMs', 'totalMs'].map((key) => [key, median(runs.map((run) => run[key]))]))
          : {
              mibPerSecond: median(runs.map((run) => run.mibPerSecond)),
              msPerDocument: median(runs.map((run) => run.msPerDocument)),
            }
        results.push({ phase, language, engine, sourceBytes: fs.statSync(file).size, ...summary, samples: runs })
      }
      console.error(`${phase} ${language} done`)
    }
  }
  report(results)
  if (args.out) {
    fs.writeFileSync(args.out, JSON.stringify({ environment: environment(), results }, null, 2) + '\n')
  }
}

function report(results) {
  const find = (phase, language, engine) =>
    results.find((r) => r.phase === phase && r.language === language && r.engine === engine)
  const languages = [...new Set(results.map((r) => r.language))]
  const env = environment()
  console.log(`node ${env.node}, ${env.cpu} (${env.cpus} CPUs), ${env.platform}`)
  console.log(`syntaxmate ${env.syntaxmate} @ ${env.commit} (wasm ${env.wasmBytes} B), shiki ${env.shiki}; medians\n`)
  console.log('| language | cold syntaxmate ms | cold shiki ms | steady syntaxmate MiB/s | steady shiki MiB/s | steady ratio | replay syntaxmate MiB/s | replay shiki MiB/s |')
  console.log('| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |')
  const ratios = []
  for (const language of languages) {
    const cold = (engine) => find('cold', language, engine)?.totalMs.toFixed(1)
    const rate = (phase, engine) => find(phase, language, engine)?.mibPerSecond
    const ratio = rate('steady', 'syntaxmate') / rate('steady', 'shiki')
    ratios.push(ratio)
    console.log(`| ${language} | ${cold('syntaxmate')} | ${cold('shiki')} | ${rate('steady', 'syntaxmate').toFixed(2)} | ${rate('steady', 'shiki').toFixed(2)} | ${ratio.toFixed(2)}x | ${rate('replay', 'syntaxmate').toFixed(2)} | ${rate('replay', 'shiki').toFixed(2)} |`)
  }
  const geomean = Math.exp(ratios.reduce((sum, ratio) => sum + Math.log(ratio), 0) / ratios.length)
  console.log(`\nsteady geometric-mean speedup (syntaxmate / shiki): ${geomean.toFixed(2)}x`)
}

await main()
