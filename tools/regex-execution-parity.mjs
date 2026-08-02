#!/usr/bin/env node
/** Replay real vscode-textmate scanner executions through Syntaxmate's matcher. */
import { createHash } from 'node:crypto'
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

import { generateTextMateGolden } from './textmate-oracle.mjs'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const assetsDir = path.join(root, 'assets/grammars/languages')
const differencesPath = path.join(root, 'benchmarks/textmate/regex-execution-differences.json')
const oraclePackagePath = path.join(root, 'tools/golden-oracle/package.json')
const provingCases = [
  ['cpp', 'source.cpp', 'tests/fixtures/textmate/cpp/stress.cpp'],
  ['markdown', 'text.html.markdown', 'tests/fixtures/textmate/markdown/stress.md'],
  ['typescript', 'source.ts', 'tests/fixtures/textmate/typescript/stress.ts'],
  ['yaml', 'source.yaml', 'tests/fixtures/textmate/yaml/stress.yaml'],
]

function parseArgs(argv) {
  const args = { maxExecutions: 512, out: 'target/regex-execution-parity.json' }
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--max-executions') args.maxExecutions = positiveInteger(argv[++index], value)
    else if (value === '--out') args.out = argv[++index]
    else if (value === '--help' || value === '-h') args.help = true
    else throw new Error(`unknown option: ${value}`)
  }
  if (!args.out) throw new Error('--out requires a path')
  return args
}

function positiveInteger(value, option) {
  const parsed = Number(value)
  if (!Number.isSafeInteger(parsed) || parsed < 1) throw new Error(`${option} requires a positive integer`)
  return parsed
}

function usage() {
  console.log(`usage: node tools/regex-execution-parity.mjs [options]

Options:
  --max-executions N  deterministic replay sample size (default: 512)
  --out PATH          summary report path
  --help              show this help`)
}

async function main() {
  const args = parseArgs(process.argv.slice(2))
  if (args.help) return usage()
  const unique = new Map()
  let observed = 0
  for (const [language, scopeName, fixture] of provingCases) {
    await generateTextMateGolden({
      assetsDir,
      language,
      scopeName,
      sourcePath: path.join(root, fixture),
      sourceLabel: fixture,
      onigObserver(execution) {
        observed += 1
        const record = normalizeExecution(execution)
        const serialized = JSON.stringify(record.input)
        const hash = createHash('sha256').update(serialized).digest('hex')
        unique.set(hash, { hash, language, fixture, ...record })
      },
    })
  }

  const records = [...unique.values()]
    .sort((left, right) => left.hash.localeCompare(right.hash))
    .slice(0, args.maxExecutions)
  const executable = buildReplayExecutable()
  const replay = spawnSync(executable, [], {
    cwd: root,
    input: `${records.map(record => JSON.stringify(record.input)).join('\n')}\n`,
    encoding: 'utf8',
    maxBuffer: 128 * 1024 * 1024,
  })
  if (replay.status !== 0) throw new Error(replay.stderr || `regex-replay exited ${replay.status}`)
  const actual = replay.stdout.trimEnd().split('\n').filter(Boolean).map(JSON.parse)
  if (actual.length !== records.length) {
    throw new Error(`regex-replay returned ${actual.length} records for ${records.length} inputs`)
  }

  const observedDifferences = []
  for (let index = 0; index < records.length; index += 1) {
    const record = records[index]
    const result = actual[index]
    if (result.errors.length || JSON.stringify(result.winner) !== JSON.stringify(record.winner)) {
      observedDifferences.push({
        hash: record.hash,
        fingerprint: differenceFingerprint(record.winner, result),
        kind: classifyDifference(record, result),
        language: record.language,
        fixture: record.fixture,
        input: record.input,
        expected: record.winner,
        actual: result,
      })
    }
  }
  const oraclePackage = JSON.parse(await fs.readFile(oraclePackagePath, 'utf8'))
  const oracle = [
    `vscode-textmate@${oraclePackage.dependencies['vscode-textmate']}`,
    `vscode-oniguruma@${oraclePackage.dependencies['vscode-oniguruma']}`,
  ].join(' + ')
  const policy = JSON.parse(await fs.readFile(differencesPath, 'utf8'))
  if (
    policy.schemaVersion !== 2 || policy.sampleSize !== args.maxExecutions ||
    policy.oracle !== oracle
  ) {
    throw new Error(`${path.relative(root, differencesPath)} does not match the pinned ${args.maxExecutions}-execution oracle sample`)
  }
  const known = new Map(Object.entries(policy.differences ?? {}))
  const observedHashes = new Set(observedDifferences.map(difference => difference.hash))
  const failures = observedDifferences.filter(difference => !known.has(difference.hash))
  const staleDifferences = [...known.keys()].filter(hash => !observedHashes.has(hash)).sort()
  for (const difference of observedDifferences) {
    const expected = known.get(difference.hash)
    if (
      expected &&
      (difference.kind !== expected.kind || difference.fingerprint !== expected.fingerprint)
    ) {
      failures.push(difference)
    }
  }
  const report = {
    schemaVersion: 1,
    oracle,
    provingCases: provingCases.map(([language, scope, fixture]) => ({ language, scope, fixture })),
    observedExecutions: observed,
    uniqueExecutions: unique.size,
    sampledExecutions: records.length,
    exactMatches: records.length - observedDifferences.length,
    allowedDifferences: observedDifferences.length - failures.length,
    failed: failures.length + staleDifferences.length,
    staleDifferences,
    failures,
  }
  const output = path.resolve(root, args.out)
  await fs.mkdir(path.dirname(output), { recursive: true })
  await fs.writeFile(output, `${JSON.stringify(report, null, 2)}\n`)
  console.log(JSON.stringify({
    out: args.out,
    observed,
    unique: unique.size,
    sampled: records.length,
    exact: report.exactMatches,
    allowedDifferences: report.allowedDifferences,
    failed: report.failed,
  }))
  if (report.failed) process.exitCode = 1
}

function differenceFingerprint(expected, actual) {
  return createHash('sha256')
    .update(JSON.stringify({ expected, actual }))
    .digest('hex')
}

function classifyDifference(record, result) {
  const expected = record.winner
  const actual = result.winner
  const end = Buffer.byteLength(record.input.line)
  if (
    result.errors.length === 0 && expected != null && actual != null &&
    expected.index === actual.index && expected.captures.length === actual.captures.length &&
    JSON.stringify(expected.captures[0]) === JSON.stringify(actual.captures[0]) &&
    expected.captures.slice(1).every((capture, captureIndex) => {
      const actualCapture = actual.captures[captureIndex + 1]
      return JSON.stringify(capture) === JSON.stringify(actualCapture) ||
        (actualCapture == null && capture?.start === end && capture?.end === end)
    })
  ) {
    return 'dormant-capture-end-sentinel'
  }
  return 'semantic-mismatch'
}

function normalizeExecution(execution) {
  const line = execution.line
  if (typeof line !== 'string') throw new Error('oracle scanner did not expose its source string')
  const findOptions = Number(execution.findOptions) || 0
  if ((findOptions & 6) !== 0) {
    throw new Error(`unsupported vscode-oniguruma find options: ${findOptions}`)
  }
  const from = utf16ToUtf8(line, execution.startPosition)
  const winner = execution.result == null
    ? null
    : {
        index: execution.result.index,
        captures: execution.result.captureIndices.map(span => normalizeSpan(line, span)),
      }
  return {
    input: {
      patterns: execution.patterns,
      line,
      from,
      allowStartOfFile: (findOptions & 1) === 0,
    },
    winner,
  }
}

function normalizeSpan(line, span) {
  if (span == null || span.start === 0xffffffff || span.end === 0xffffffff) return null
  return { start: utf16ToUtf8(line, span.start), end: utf16ToUtf8(line, span.end) }
}

function utf16ToUtf8(line, offset) {
  return Buffer.byteLength(line.slice(0, offset))
}

function buildReplayExecutable() {
  const build = spawnSync(
    'cargo',
    ['build', '--quiet', '--example', 'regex-replay', '--features', 'diagnostics', '--message-format=json'],
    { cwd: root, encoding: 'utf8' },
  )
  if (build.status !== 0) throw new Error(build.stderr || build.stdout || `cargo build exited ${build.status}`)
  for (const line of build.stdout.split(/\r?\n/)) {
    if (!line) continue
    let message
    try { message = JSON.parse(line) } catch { continue }
    if (message.reason === 'compiler-artifact' && message.target?.name === 'regex-replay' && message.executable) {
      return message.executable
    }
  }
  throw new Error('cargo did not report the regex-replay executable')
}

main().catch(error => {
  console.error(`regex-execution-parity: ${error.stack ?? error.message}`)
  process.exitCode = 1
})
