#!/usr/bin/env node
/**
 * Dev-only batch regenerator for TextMate goldens.
 *
 * Reads tests/fixtures/textmate/cases.toml and drives
 * tools/golden-dump.mjs for each case. Requires:
 *   npm install --prefix tools/golden-oracle
 *
 * Not used by release builds.
 */
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { generateTextMateGolden } from './textmate-oracle.mjs'

const DEFAULT_MANIFEST = 'tests/fixtures/textmate/cases.toml'

function usage() {
  console.log(`usage: generate-goldens.mjs [--manifest <path>] [--case <language>] [--check]

Options:
  --manifest <path>   TOML manifest to read (default: ${DEFAULT_MANIFEST}).
  --case <language>   Only regenerate/check cases with this language id.
  --check             Write each oracle to a temp file and fail if it differs.
  -h, --help          Show this help.`)
}

function parseArgs(argv) {
  const args = { manifest: DEFAULT_MANIFEST, cases: [], check: false }
  for (let index = 2; index < argv.length; index++) {
    const item = argv[index]
    if (item === '-h' || item === '--help') {
      args.help = true
      continue
    }
    if (item === '--check') {
      args.check = true
      continue
    }
    const equals = item.indexOf('=')
    const name = equals > 0 ? item.slice(0, equals) : item
    let value = equals > 0 ? item.slice(equals + 1) : undefined
    if (name === '--manifest' || name === '--case') {
      if (value === undefined) value = argv[++index]
      if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value`)
      if (name === '--manifest') args.manifest = value
      else args.cases.push(value)
      continue
    }
    throw new Error(`unknown argument: ${item}`)
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

const manifestPath = path.resolve(args.manifest)
const manifestDir = path.dirname(manifestPath)
const manifestText = await fs.readFile(manifestPath, 'utf8')
const manifestCases = parseManifest(manifestText, manifestPath)
const selectedLanguages = new Set(args.cases)
const selectedCases = selectedLanguages.size === 0
  ? manifestCases
  : manifestCases.filter(testCase => selectedLanguages.has(testCase.language))

if (selectedCases.length === 0) {
  console.error(`no cases matched${selectedLanguages.size ? `: ${[...selectedLanguages].join(', ')}` : ''}`)
  process.exit(1)
}

const tempRoot = args.check ? await fs.mkdtemp(path.join(os.tmpdir(), 'syntaxmate-goldens-')) : null
let failures = 0

try {
  for (let index = 0; index < selectedCases.length; index++) {
    const testCase = selectedCases[index]
    const ok = await runCase(testCase, index)
    if (!ok) failures++
  }
} finally {
  if (tempRoot) await fs.rm(tempRoot, { recursive: true, force: true })
}

if (failures > 0) process.exit(1)

async function runCase(testCase, index) {
  const fixtureCwd = await cwdForFixture(testCase.fixture)
  const grammarPath = await resolveManifestPath(testCase.grammar)
  const goldenPath = await resolveManifestPath(testCase.golden)
  const outPath = args.check
    ? path.join(tempRoot, `${String(index).padStart(4, '0')}-${safeName(testCase.language)}.jsonl`)
    : goldenPath

  await fs.mkdir(path.dirname(outPath), { recursive: true })

  try {
    const output = await generateTextMateGolden({
      grammarPath,
      scopeName: testCase.scope,
      language: testCase.language,
      sourcePath: path.resolve(fixtureCwd, testCase.fixture),
      sourceLabel: testCase.fixture,
      embedded: await Promise.all(testCase.embedded.map(async item => ({
        scope: item.scope,
        grammarPath: await resolveManifestPath(item.grammar),
      }))),
    })
    await fs.writeFile(outPath, output)
  } catch (error) {
    console.error(`${testCase.language}: oracle failed: ${error.stack ?? error.message}`)
    return false
  }

  if (args.check) {
    let actual
    let expected
    try {
      actual = await fs.readFile(outPath)
      expected = await fs.readFile(goldenPath)
    } catch (error) {
      console.error(`${testCase.language}: failed to read golden output: ${error.message}`)
      return false
    }
    if (!actual.equals(expected)) {
      console.error(`${testCase.language}: golden differs: ${path.relative(process.cwd(), goldenPath)}`)
      return false
    }
    console.log(`${testCase.language}: ok`)
  } else {
    console.log(`${testCase.language}: wrote ${path.relative(process.cwd(), goldenPath)}`)
  }
  return true
}

function parseManifest(text, filename) {
  const cases = []
  let currentCase = null
  let currentTarget = null
  let currentKind = null
  const lines = text.split(/\r?\n/)
  for (let index = 0; index < lines.length; index++) {
    const lineNumber = index + 1
    const line = stripComment(lines[index]).trim()
    if (!line) continue

    const table = line.match(/^\[\[\s*([A-Za-z0-9_.-]+)\s*\]\]$/)
    if (table) {
      if (table[1] === 'case') {
        currentCase = { embedded: [], __line: lineNumber }
        cases.push(currentCase)
        currentTarget = currentCase
        currentKind = 'case'
      } else if (table[1] === 'case.embedded') {
        if (!currentCase) throw manifestError(filename, lineNumber, '[[case.embedded]] must follow a [[case]]')
        const embedded = { __line: lineNumber }
        currentCase.embedded.push(embedded)
        currentTarget = embedded
        currentKind = 'embedded'
      } else {
        throw manifestError(filename, lineNumber, `unsupported table [[${table[1]}]]`)
      }
      continue
    }

    const assignment = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/)
    if (!assignment) throw manifestError(filename, lineNumber, 'expected [[case]], [[case.embedded]], or key = "value"')
    if (!currentTarget) throw manifestError(filename, lineNumber, 'key/value must appear inside [[case]]')

    const key = assignment[1]
    const allowed = currentKind === 'case'
      ? new Set(['language', 'scope', 'grammar', 'fixture', 'golden'])
      : new Set(['scope', 'grammar'])
    if (!allowed.has(key)) throw manifestError(filename, lineNumber, `unsupported ${currentKind} key ${key}`)
    if (Object.prototype.hasOwnProperty.call(currentTarget, key)) {
      throw manifestError(filename, lineNumber, `duplicate ${currentKind} key ${key}`)
    }
    currentTarget[key] = parseTomlString(assignment[2].trim(), filename, lineNumber)
  }

  const requiredCaseKeys = ['language', 'scope', 'grammar', 'fixture', 'golden']
  for (const testCase of cases) {
    for (const key of requiredCaseKeys) {
      if (testCase[key] === undefined) throw manifestError(filename, testCase.__line, `missing case key ${key}`)
    }
    for (const embedded of testCase.embedded) {
      for (const key of ['scope', 'grammar']) {
        if (embedded[key] === undefined) throw manifestError(filename, embedded.__line, `missing embedded key ${key}`)
      }
      delete embedded.__line
    }
    delete testCase.__line
  }
  return cases
}

function stripComment(line) {
  let quote = null
  let escaped = false
  let result = ''
  for (const char of line) {
    if (quote) {
      result += char
      if (quote === '"' && escaped) escaped = false
      else if (quote === '"' && char === '\\') escaped = true
      else if (char === quote) quote = null
      continue
    }
    if (char === '"' || char === "'") {
      quote = char
      result += char
      continue
    }
    if (char === '#') break
    result += char
  }
  return result
}

function parseTomlString(value, filename, lineNumber) {
  if (value.startsWith('"') && value.endsWith('"')) {
    try {
      const parsed = JSON.parse(value)
      if (typeof parsed === 'string') return parsed
    } catch {}
  }
  if (value.startsWith("'") && value.endsWith("'")) return value.slice(1, -1)
  throw manifestError(filename, lineNumber, 'only single-line string values are supported')
}

function manifestError(filename, lineNumber, message) {
  return new Error(`${filename}:${lineNumber}: ${message}`)
}

async function resolveManifestPath(filePath) {
  if (path.isAbsolute(filePath)) return filePath
  const cwdCandidate = path.resolve(process.cwd(), filePath)
  const manifestCandidate = path.resolve(manifestDir, filePath)
  if (await exists(cwdCandidate)) return cwdCandidate
  if (await exists(manifestCandidate)) return manifestCandidate
  if (await exists(path.dirname(cwdCandidate))) return cwdCandidate
  return manifestCandidate
}

async function cwdForFixture(filePath) {
  if (path.isAbsolute(filePath)) return process.cwd()
  const cwdCandidate = path.resolve(process.cwd(), filePath)
  const manifestCandidate = path.resolve(manifestDir, filePath)
  if (await exists(cwdCandidate)) return process.cwd()
  if (await exists(manifestCandidate)) return manifestDir
  if (await exists(path.dirname(cwdCandidate))) return process.cwd()
  return manifestDir
}

async function exists(filePath) {
  try {
    await fs.access(filePath)
    return true
  } catch {
    return false
  }
}

function safeName(name) {
  return name.replace(/[^A-Za-z0-9_.-]+/g, '_')
}
