#!/usr/bin/env node
/**
 * Dev-only regex conformance helper.
 *
 * Compares Oniguruma patterns against vscode-oniguruma (oracle) and the
 * syntaxmate regex-parse example (in-house hybrid matcher). The proving cases
 * are exported and can be listed without loading the oracle:
 *
 *   node tools/regex-conformance.mjs --list
 *
 * Requires for an actual run:
 *   npm install --prefix tools/golden-oracle
 *   cargo (for the syntaxmate example)
 */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { spawnSync } from 'node:child_process'

const unicodePositiveVariants = {
  alnum: 'λ7', alpha: 'λ', blank: ' ', Cc: '\u0001', Cf: '\u200D', Cntrl: '\u0001', Greek: 'Ω',
  Ll: 'a', Lm: 'ʰ', Lo: '文', lower: 'a', Lt: 'ǅ', Lu: 'A', M: '\u0301',
  Mc: 'ा', Mn: '\u0301', Nl: 'Ⅻ', P: '!', Pc: '_', print: 'A', S: '+',
  Sc: '$', Sm: '+', So: '🚀', upper: 'A', word: 'λ_7',
}
const unicodeNegativeVariants = ['Mc', 'Me', 'Mn', 'No', 'Pc', 'Sc', 'Sk', 'So', 'word']
const posixPositiveVariants = {
  alnum: 'λ7', blank: ' ', lower: 'a', upper: 'A', word: 'λ_7',
}

function inventoryVariantCases() {
  const cases = []
  for (const [property, line] of Object.entries(unicodePositiveVariants)) {
    cases.push({
      name: `unicode-property-${property.toLowerCase()}`,
      pattern: `\\p{${property}}+`,
      line,
      engine: 'fallback',
      constructs: ['unicode-property.positive'],
    })
  }
  for (const property of unicodeNegativeVariants) {
    cases.push({
      name: `unicode-property-negative-${property.toLowerCase()}`,
      pattern: `\\P{${property}}+`,
      line: '.',
      engine: 'fallback',
      constructs: ['unicode-property.negative'],
    })
  }
  for (const [property, line] of Object.entries(posixPositiveVariants)) {
    cases.push({
      name: `posix-property-${property}`,
      pattern: `[[:${property}:]]+`,
      line,
      engine: 'fallback',
      constructs: ['posix-class.positive'],
    })
  }
  // UTF-8 cannot encode surrogate code points, but the property must still
  // compile and reject ordinary Unicode scalar values like the oracle does.
  cases.push({
    name: 'unicode-property-surrogate',
    pattern: '\\p{Surrogate}+',
    line: 'A',
    engine: 'fallback',
    constructs: ['unicode-property.positive'],
    expectMiss: true,
  })
  cases.push({
    name: 'posix-property-negative-ascii',
    pattern: '[[:^ascii:]]+',
    line: 'λ',
    engine: 'fallback',
    constructs: ['posix-class.negative'],
  })
  return cases
}

export const conformanceCases = Object.freeze([
  { name: 'dfa-captures', pattern: String.raw`foo(\d+)`, line: 'xxfoo123', engine: 'auto', constructs: [] },
  { name: 'positive-lookahead', pattern: String.raw`foo(?=bar)`, line: 'xxfoobar', engine: 'fallback', constructs: ['lookahead.positive'] },
  { name: 'negative-lookahead', pattern: String.raw`foo(?!baz)`, line: 'xxfoobar', engine: 'fallback', constructs: ['lookahead.negative'] },
  { name: 'positive-lookbehind', pattern: String.raw`(?<=foo)bar`, line: 'xxfoobar', engine: 'fallback', constructs: ['lookbehind.positive'] },
  { name: 'negative-lookbehind', pattern: String.raw`(?<!baz)bar`, line: 'xxfoobar', engine: 'fallback', constructs: ['lookbehind.negative'] },
  { name: 'positive-lookbehind-capture', pattern: String.raw`(?<=(a))b`, line: 'ab', engine: 'fallback', constructs: ['lookbehind.positive'] },
  { name: 'lookbehind-capture-backref', pattern: String.raw`(?<=(a))\1`, line: 'aa', engine: 'fallback', constructs: ['lookbehind.positive', 'backreference.numbered'] },
  { name: 'lookbehind-scoped-flags', pattern: String.raw`(?<=(?i:foo))bar`, line: 'FOObar', engine: 'fallback', constructs: ['lookbehind.positive', 'inline-flags.scoped-set'] },

  { name: 'numbered-backref', pattern: String.raw`(foo)\1`, line: 'xxfoofoo', engine: 'fallback', constructs: ['backreference.numbered'] },
  { name: 'named-backref-angle', pattern: String.raw`(?<word>foo)\k<word>`, line: 'xxfoofoo', engine: 'fallback', constructs: ['named-group.angle', 'backreference.named-angle'] },
  { name: 'duplicate-named-backref', pattern: String.raw`(?<x>a)(?<x>b)\k<x>`, line: 'abb', engine: 'fallback', constructs: ['named-group.angle', 'named-group.duplicate', 'backreference.named-angle'] },

  { name: 'global-ignore-case', pattern: String.raw`(?i)foo`, line: 'xxFOO', engine: 'fallback', constructs: ['inline-flags.global-set'] },
  { name: 'global-ignore-case-extended', pattern: '(?ix) f o o', line: 'xxFOO', engine: 'fallback', constructs: ['inline-flags.global-set', 'inline-flags.extended-set'] },
  { name: 'global-extended-ignore-case', pattern: '(?xi) f o o', line: 'xxFOO', engine: 'fallback', constructs: ['inline-flags.global-set', 'inline-flags.extended-set'] },
  { name: 'scoped-ignore-case', pattern: String.raw`(?i:foo)bar`, line: 'xxFOObar', engine: 'fallback', constructs: ['inline-flags.scoped-set'] },
  { name: 'scoped-flag-clearing', pattern: String.raw`(?i:foo(?-i:bar))`, line: 'xxFOObar', engine: 'fallback', constructs: ['inline-flags.scoped-set', 'inline-flags.scoped-clear'] },
  { name: 'global-extended-mode', pattern: '(?x) f o o  # ignored comment\n b a r', line: 'xxfoobar', engine: 'fallback', constructs: ['inline-flags.global-set', 'inline-flags.extended-set'] },
  { name: 'scoped-extended-mode', pattern: '(?x: f o o  # ignored comment\n b a r)', line: 'xxfoobar', engine: 'fallback', constructs: ['inline-flags.scoped-set', 'inline-flags.extended-set'] },
  { name: 'extended-mode-clearing', pattern: String.raw`(?x:a b(?-x: c d))`, line: 'ab c d', engine: 'fallback', constructs: ['inline-flags.scoped-set', 'inline-flags.scoped-clear', 'inline-flags.extended-set', 'inline-flags.extended-clear'] },
  { name: 'scoped-multiline-flag', pattern: '(?m:bar)', line: 'xxbar', engine: 'fallback', constructs: ['inline-flags.scoped-set'] },
  { name: 'scoped-im-flags-cleared', pattern: '(?im:^foo(?-im:$))', line: 'FOO', engine: 'fallback', constructs: ['inline-flags.scoped-set', 'inline-flags.scoped-clear', 'anchor.line-start'] },
  { name: 'global-flag-clearing', pattern: '(?i)foo(?-i)BAR', line: 'FOOBAR', engine: 'fallback', constructs: ['inline-flags.global-set', 'inline-flags.global-clear'] },
  { name: 'bare-flag-remainder-alternation', pattern: 'a(?i)b|c', line: 'aC', engine: 'fallback', constructs: ['inline-flags.global-set'] },
  { name: 'unicode-ignore-case-cyrillic', pattern: '(?i)Выбрать', line: 'ВЫБРАТЬ', engine: 'fallback', constructs: ['inline-flags.global-set'] },

  { name: 'unicode-property-letter', pattern: String.raw`\p{L}+`, line: '12é文', engine: 'fallback', constructs: ['unicode-property.positive'] },
  { name: 'unicode-property-alphabetic', pattern: String.raw`\p{Alphabetic}+`, line: '12é文', engine: 'fallback', constructs: ['unicode-property.positive'] },
  { name: 'unicode-property-number', pattern: String.raw`\p{N}+`, line: 'abc٣7', engine: 'fallback', constructs: ['unicode-property.positive'] },
  { name: 'unicode-property-negated', pattern: String.raw`\P{L}+`, line: 'é文12!', engine: 'fallback', constructs: ['unicode-property.negative'] },
  { name: 'posix-alpha-unicode', pattern: String.raw`[[:alpha:]]+`, line: '12é文', engine: 'fallback', constructs: ['posix-class.positive'] },
  { name: 'posix-negated-digit', pattern: String.raw`[[:^digit:]]+`, line: '12é文', engine: 'fallback', constructs: ['posix-class.negative'] },
  { name: 'nested-class-intersection', pattern: String.raw`[a-z&&[^aeiou]]+`, line: 'ae-bcdf', engine: 'auto', constructs: ['character-class.nested', 'character-class.intersection'] },
  { name: 'nested-class-intersection-rhs-union', pattern: String.raw`[a-w&&[^c-g]z]+`, line: 'cg-bh', engine: 'auto', constructs: ['character-class.nested', 'character-class.intersection'] },
  { name: 'nested-class-purescript-operators', pattern: String.raw`[[\p{S}\p{P}]&&[^]"'(),;\[_${'`'}{}]]+`, line: 'name + →🚀', engine: 'auto', constructs: ['character-class.nested', 'character-class.intersection', 'unicode-property.positive'] },

  { name: 'text-start-anchor', pattern: String.raw`\Afoo`, line: 'foo', engine: 'auto', allowA: true, constructs: ['anchor.text-start'] },
  { name: 'search-start-anchor', pattern: String.raw`\Gfoo`, line: 'xxfoo', engine: 'auto', from: 2, gPos: 2, constructs: ['anchor.search-start'] },
  { name: 'search-start-anchor-miss', pattern: String.raw`\Gfoo`, line: 'xxfoo', engine: 'auto', gPos: 0, expectMiss: true, constructs: ['anchor.search-start'] },
  { name: 'line-anchor-resume-miss', pattern: String.raw`^foo`, line: 'foo', engine: 'auto', from: 1, expectMiss: true, constructs: ['anchor.line-start'] },

  { name: 'atomic-ordered-failure', pattern: String.raw`(?>a|ab)c`, line: 'abc', engine: 'fallback', expectMiss: true, constructs: ['atomic-group'] },
  { name: 'atomic-ordered-match', pattern: String.raw`(?>ab|a)c`, line: 'abc', engine: 'fallback', constructs: ['atomic-group'] },
  { name: 'star-possessive-failure', pattern: String.raw`a*+a`, line: 'aaa', engine: 'fallback', expectMiss: true, constructs: ['possessive.star'] },
  { name: 'question-possessive-failure', pattern: String.raw`a?+a`, line: 'a', engine: 'fallback', expectMiss: true, constructs: ['possessive.question'] },
  { name: 'compound-possessive-failure', pattern: String.raw`(a|ab)++c`, line: 'abc', engine: 'fallback', expectMiss: true, constructs: ['possessive.plus'] },
  { name: 'bounded-possessive-inner-backtrack', pattern: String.raw`(a|ab){1}+c`, line: 'abc', engine: 'fallback', constructs: ['possessive.bounded'] },
  { name: 'bounded-possessive-zero-width', pattern: String.raw`(a?){2}+a`, line: 'a', engine: 'fallback', constructs: ['possessive.bounded'] },

  { name: 'bounded-empty-repeat', pattern: String.raw`(?:){2}a`, line: 'a', engine: 'fallback', constructs: [] },
  { name: 'variable-lookbehind-capture', pattern: String.raw`(?<=(a|aa))b`, line: 'aab', engine: 'fallback', constructs: ['lookbehind.positive'] },
  { name: 'numbered-conditional-matched', pattern: String.raw`(a)?(?(1)b|c)d`, line: 'abd', engine: 'fallback', constructs: ['conditional.numbered'] },
  { name: 'numbered-conditional-unmatched', pattern: String.raw`(a)?(?(1)b|c)d`, line: 'cd', engine: 'fallback', constructs: ['conditional.numbered'] },
  { name: 'named-conditional-matched', pattern: String.raw`(?<x>a)?(?(<x>)b|c)d`, line: 'abd', engine: 'fallback', constructs: ['named-group.angle', 'conditional.named-angle'] },
  { name: 'named-conditional-unmatched', pattern: String.raw`(?<x>a)?(?(<x>)b|c)d`, line: 'cd', engine: 'fallback', constructs: ['named-group.angle', 'conditional.named-angle'] },
  { name: 'named-subroutine-call', pattern: String.raw`(?<word>ab)\g<word>`, line: 'xxabab', engine: 'fallback', constructs: ['named-group.angle', 'subroutine.angle'] },
  { name: 'absent-group-documented-degradation', pattern: '(?~a)', line: 'bbb', engine: 'fallback', constructs: ['absent-group'], expectedDegradation: 'unsupported-no-match' },
  ...inventoryVariantCases(),
])

// A short alias makes the cases convenient for ad-hoc ESM consumers while the
// descriptive export is what grammar-stats looks for.
export const cases = conformanceCases

export async function runConformance(options = {}) {
  const selectedNames = new Set(options.caseNames ?? [])
  const candidateCases = options.cases ?? conformanceCases
  const selectedCases = candidateCases.filter(testCase => selectedNames.size === 0 || selectedNames.has(testCase.name))
  if (selectedNames.size) {
    const found = new Set(selectedCases.map(testCase => testCase.name))
    const unknown = [...selectedNames].filter(name => !found.has(name))
    if (unknown.length) throw new Error(`unknown conformance case(s): ${unknown.join(', ')}`)
  }

  const onig = await loadOniguruma()
  const syntaxmateExecutable = options.syntaxmateExecutable ?? buildSyntaxmateExecutable()
  const records = []
  for (const testCase of selectedCases) {
    const scanner = new onig.OnigScanner([testCase.pattern])
    let onigMatch
    let onigError
    try {
      onigMatch = scanner.findNextMatchSync(testCase.line, testCase.from ?? 0)
    } catch (error) {
      onigError = error.message
      onigMatch = null
    }
    const syntaxmate = runSyntaxmate(testCase, syntaxmateExecutable)
    const documentedDegradation = testCase.expectedDegradation === 'unsupported-no-match'
    const expectedResult = testCase.parityOnly
      ? capturesEqual(syntaxmate, onigMatch, testCase.line)
      : documentedDegradation
        ? onigMatch != null && syntaxmate.match == null
        : testCase.expectMiss
          ? onigMatch == null && syntaxmate.match == null
          : onigMatch != null && syntaxmate.match != null
    const pass = !onigError && !syntaxmate.error && expectedResult &&
      (documentedDegradation || capturesEqual(syntaxmate, onigMatch, testCase.line))
    records.push({
      ...testCase,
      onig: simplifyOnig(onigMatch, testCase.line),
      ...(onigError ? { onigError } : {}),
      syntaxmate,
      pass,
    })
  }
  return {
    version: 2,
    oracle: 'vscode-oniguruma',
    oraclePackage: 'tools/golden-oracle (vscode-oniguruma@1.7.0)',
    cases: records,
    passed: records.filter(record => record.pass).length,
    failed: records.filter(record => !record.pass).length,
  }
}

async function loadOniguruma() {
  const require = createRequire(import.meta.url)
  const toolDir = path.dirname(fileURLToPath(import.meta.url))
  const resolvePaths = [path.join(toolDir, 'golden-oracle'), path.resolve(process.cwd(), 'tools/golden-oracle')]
  const resolvePackage = name => {
    try {
      return require.resolve(name, { paths: resolvePaths })
    } catch (error) {
      throw new Error(
        `failed to resolve ${name}. Install the pinned oracle with:\n` +
          `  npm install --prefix tools/golden-oracle\n` +
          `(${error.message})`,
      )
    }
  }
  const onigModule = await import(pathToFileURL(resolvePackage('vscode-oniguruma')).href)
  const onig = onigModule.default ?? onigModule
  const onigMain = resolvePackage('vscode-oniguruma')
  let wasmPath = path.join(path.dirname(onigMain), 'release', 'onig.wasm')
  try { await fs.access(wasmPath) } catch { wasmPath = path.join(path.dirname(onigMain), 'onig.wasm') }
  const wasm = await fs.readFile(wasmPath)
  await onig.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength))
  return onig
}

function buildSyntaxmateExecutable() {
  const result = spawnSync(
    'cargo',
    ['build', '-q', '-p', 'syntaxmate', '--example', 'regex-parse', '--features', 'diagnostics', '--message-format=json'],
    { encoding: 'utf8' },
  )
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `cargo build exited ${result.status}`)
  }
  for (const line of result.stdout.split(/\r?\n/)) {
    if (!line) continue
    let message
    try { message = JSON.parse(line) } catch { continue }
    if (message.reason === 'compiler-artifact' && message.target?.name === 'regex-parse' && message.executable) {
      return message.executable
    }
  }
  throw new Error('cargo did not report the regex-parse executable')
}

function runSyntaxmate(testCase, executable) {
  const args = ['--match', '--engine', testCase.engine]
  if (testCase.from != null) args.push('--from', String(testCase.from))
  if (testCase.allowA) args.push('--allow-a')
  if (testCase.gPos != null) args.push('--allow-g', String(testCase.gPos))
  args.push(testCase.pattern, testCase.line)
  const result = spawnSync(executable, args, { encoding: 'utf8' })
  if (result.status !== 0) return { error: result.stderr || result.stdout, status: result.status }
  const lines = result.stdout.split(/\r?\n/)
  const matchLine = lines.find(line => line.startsWith('match: '))
  const match = parseSpan(matchLine)
  const captures = lines.filter(line => line.startsWith('capture[')).map(line => parseSpan(line))
  return { match, captures }
}

function parseSpan(line) {
  if (!line || line.includes('<none>') || line.includes('None')) return null
  const match = line.match(/(?:Some\()?([0-9]+)\.\.([0-9]+)/)
  return match ? { start: Number(match[1]), end: Number(match[2]) } : null
}

function simplifyOnig(match, line) {
  if (!match) return null
  return { index: match.index, captures: match.captureIndices.map(span => normalizeOnigSpan(span, line)) }
}

function spansEqual(syntaxmate, onig, line) {
  onig = normalizeOnigSpan(onig, line)
  if (!syntaxmate || !onig) return syntaxmate == null && onig == null
  return syntaxmate.start === onig.start && syntaxmate.end === onig.end
}

function normalizeOnigSpan(span, line) {
  if (!span || span.start === 0xffffffff || span.end === 0xffffffff) return null
  // vscode-oniguruma reports UTF-16 string offsets; syntaxmate reports UTF-8
  // byte offsets, as required by the tokenizer. Normalize before comparing.
  return {
    start: Buffer.byteLength(line.slice(0, span.start)),
    end: Buffer.byteLength(line.slice(0, span.end)),
  }
}

function capturesEqual(syntaxmate, onigMatch, line) {
  if (!onigMatch) return syntaxmate.match == null
  const oracle = onigMatch.captureIndices ?? []
  if (!spansEqual(syntaxmate.match, oracle[0], line)) return false
  if (syntaxmate.captures.length !== oracle.length) return false
  return oracle.every((capture, index) =>
    spansEqual(syntaxmate.captures[index], capture, line))
}

function parseArgs(argv) {
  const options = { out: 'target/regex-conformance-phase2.json', caseNames: [] }
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--out') {
      options.out = argv[++index]
      if (!options.out) throw new Error('--out requires a path')
    } else if (value === '--case') {
      const names = argv[++index]
      if (!names) throw new Error('--case requires a case name (or comma-separated names)')
      options.caseNames.push(...names.split(',').filter(Boolean))
    } else if (value === '--list') options.list = true
    else if (value === '--list-constructs') options.listConstructs = true
    else if (value === '--help' || value === '-h') options.help = true
    else throw new Error(`unknown option: ${value}`)
  }
  return options
}

function listConstructs() {
  const represented = new Map()
  for (const testCase of conformanceCases) {
    for (const id of testCase.constructs) {
      const names = represented.get(id) ?? []
      names.push(testCase.name)
      represented.set(id, names)
    }
  }
  return Object.fromEntries([...represented].sort(([left], [right]) => left.localeCompare(right)))
}

function usage() {
  console.log(`usage: node tools/regex-conformance.mjs [options]

Options:
  --out PATH             report path (default: target/regex-conformance-phase2.json)
  --case NAME[,NAME]     run only named cases (repeatable)
  --list                 print the exportable proving cases; do not run them
  --list-constructs      print construct shape -> proving case mapping; do not run
  -h, --help             show this help`)
}

async function main() {
  try {
    const options = parseArgs(process.argv.slice(2))
    if (options.help) return usage()
    if (options.list) return console.log(JSON.stringify({ version: 2, cases: conformanceCases }, null, 2))
    if (options.listConstructs) return console.log(JSON.stringify({ version: 1, constructs: listConstructs() }, null, 2))
    const report = await runConformance(options)
    await fs.mkdir(path.dirname(options.out), { recursive: true })
    await fs.writeFile(options.out, JSON.stringify(report, null, 2) + '\n')
    console.log(JSON.stringify({ out: options.out, passed: report.passed, failed: report.failed }))
    if (report.failed) process.exitCode = 1
  } catch (error) {
    console.error(`regex-conformance: ${error.stack ?? error.message}`)
    process.exitCode = 2
  }
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) await main()
