#!/usr/bin/env node
/**
 * Deterministic differential fuzzing against vscode-oniguruma.
 *
 * This mutates the proving corpus rather than relying on ambient randomness, so
 * every CI failure is reproducible from its seed and case index.
 */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'

import { conformanceCases, runConformance } from './regex-conformance.mjs'

const defaultOutput = 'target/regex-differential-fuzz.json'
const alphabet = [...'abAB019 _-+[](){}.!', 'λ', 'Ω', '文', 'é', '🚀', '\u0301']

export function generateMutationCases({ seed, count }) {
  const random = createRandom(seed)
  const bases = conformanceCases.filter(testCase =>
    testCase.expectedDegradation == null &&
    !testCase.constructs.includes('anchor.text-start') &&
    !testCase.constructs.includes('anchor.search-start'))
  const cases = []
  for (let index = 0; index < count; index += 1) {
    const base = bases[random.int(bases.length)]
    cases.push({
      name: `seed-${seed}-case-${index}-${base.name}`,
      pattern: mutatePattern(base.pattern, random),
      line: mutateLine(base.line, random),
      engine: base.engine,
      constructs: base.constructs,
      parityOnly: true,
    })
  }
  return cases
}

function mutatePattern(pattern, random) {
  switch (random.int(4)) {
    case 0: return pattern
    case 1: return `(?:${pattern})`
    case 2: return `(?:${pattern})(?:)`
    default: return `(?:)(?:${pattern})`
  }
}

function mutateLine(original, random) {
  const characters = [...original]
  const operations = 1 + random.int(4)
  for (let operation = 0; operation < operations; operation += 1) {
    const position = random.int(characters.length + 1)
    switch (random.int(3)) {
      case 0:
        characters.splice(position, 0, alphabet[random.int(alphabet.length)])
        break
      case 1:
        if (characters.length > 0) characters.splice(Math.min(position, characters.length - 1), 1)
        break
      default:
        if (characters.length > 0) {
          characters[Math.min(position, characters.length - 1)] = alphabet[random.int(alphabet.length)]
        }
        break
    }
  }
  return characters.join('').slice(0, 512)
}

function createRandom(seed) {
  let state = Number(seed) >>> 0
  if (state === 0) state = 0x9e3779b9
  return {
    int(limit) {
      state ^= state << 13
      state ^= state >>> 17
      state ^= state << 5
      state >>>= 0
      return state % limit
    },
  }
}

function parseArgs(argv) {
  const options = { seed: 1, count: 256, out: defaultOutput }
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--seed') options.seed = positiveInteger(argv[++index], '--seed')
    else if (value === '--cases') options.count = positiveInteger(argv[++index], '--cases')
    else if (value === '--out') options.out = argv[++index]
    else if (value === '--help' || value === '-h') options.help = true
    else throw new Error(`unknown option: ${value}`)
  }
  if (!options.out) throw new Error('--out requires a path')
  return options
}

function positiveInteger(value, option) {
  const parsed = Number(value)
  if (!Number.isSafeInteger(parsed) || parsed < 1) throw new Error(`${option} requires a positive integer`)
  return parsed
}

function usage() {
  console.log(`usage: node tools/fuzz-regex-conformance.mjs [options]

Options:
  --seed N       deterministic seed (default: 1)
  --cases N      generated cases (default: 256)
  --out PATH     report path (default: ${defaultOutput})
  --help         show this help`)
}

async function main() {
  const options = parseArgs(process.argv.slice(2))
  if (options.help) return usage()
  const cases = generateMutationCases(options)
  const report = await runConformance({ cases })
  const output = path.resolve(options.out)
  await fs.mkdir(path.dirname(output), { recursive: true })
  await fs.writeFile(output, `${JSON.stringify({
    schemaVersion: 1,
    seed: options.seed,
    generatedCases: options.count,
    ...report,
  }, null, 2)}\n`)
  console.log(JSON.stringify({ out: options.out, seed: options.seed, passed: report.passed, failed: report.failed }))
  if (report.failed) process.exitCode = 1
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(error => {
    console.error(`regex-differential-fuzz: ${error.stack ?? error.message}`)
    process.exitCode = 1
  })
}
