#!/usr/bin/env node
/**
 * Generate the checked-in TextMate golden case manifest from fixture naming
 * conventions and the vendored grammar bundle inputs.
 */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'

const TOOL_DIR = path.dirname(fileURLToPath(import.meta.url))
const DEFAULT_ROOT = path.resolve(TOOL_DIR, '..')
const DEFAULT_FIXTURES = 'tests/fixtures/textmate'
const DEFAULT_GRAMMARS = 'assets/grammars/languages'
const DEFAULT_CONFIG = `${DEFAULT_FIXTURES}/cases.config.json`
const DEFAULT_OUTPUT = `${DEFAULT_FIXTURES}/cases.toml`
const DEFAULT_COVERAGE = 'assets/grammars/coverage.toml'
const DEFAULT_DIVERGENCES = `${DEFAULT_FIXTURES}/divergences.toml`
const DEFAULT_POLICY = 'benchmarks/textmate/validation-policy.json'
const CONVENTIONAL_NAMES = /^(basic|stress|smoke)\.(.+)$/

function usage() {
  console.log(`usage: generate-textmate-cases.mjs [--check] [options]

Options:
  --check              Fail if cases.toml differs instead of writing it.
  --root <path>        Repository root (default: parent of tools/).
  --fixtures <path>    Fixture directory, relative to root.
  --grammars <path>    Vendored grammar directory, relative to root.
  --config <path>      Override config, relative to root.
  --output <path>      Generated TOML path, relative to root.
  --coverage <path>    Public-language coverage manifest, relative to root.
  --divergences <path> Exact-parity divergence manifest, relative to root.
  --policy <path>      Locked validation policy, relative to root.
  -h, --help           Show this help.`)
}

export function parseArgs(argv) {
  const args = {
    root: DEFAULT_ROOT,
    fixtures: DEFAULT_FIXTURES,
    grammars: DEFAULT_GRAMMARS,
    config: DEFAULT_CONFIG,
    output: DEFAULT_OUTPUT,
    coverage: DEFAULT_COVERAGE,
    divergences: DEFAULT_DIVERGENCES,
    policy: DEFAULT_POLICY,
    check: false,
  }
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
    if (['--root', '--fixtures', '--grammars', '--config', '--output', '--coverage', '--divergences', '--policy'].includes(name)) {
      if (value === undefined) value = argv[++index]
      if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value`)
      args[name.slice(2)] = value
      continue
    }
    throw new Error(`unknown argument: ${item}`)
  }
  return args
}

export async function generateManifest(options = {}) {
  const resolved = resolveOptions(options)
  const [catalog, config, discovered] = await Promise.all([
    loadGrammarCatalog(resolved.grammars, resolved.root),
    loadConfig(resolved.config),
    discoverConventionCases(resolved.fixtures, resolved.root),
  ])
  const specs = mergeCaseSpecs(discovered, config.cases, resolved.root)
  const cases = []
  const goldenOwners = new Map()

  for (const spec of specs) {
    const testCase = await resolveCase(spec, config, catalog, resolved.root)
    const previous = goldenOwners.get(testCase.golden)
    if (previous) {
      throw new Error(`fixtures ${previous} and ${testCase.fixture} resolve to the same golden ${testCase.golden}`)
    }
    goldenOwners.set(testCase.golden, testCase.fixture)
    cases.push(testCase)
  }

  cases.sort((left, right) =>
    compareText(left.language, right.language) ||
    compareText(left.fixture, right.fixture))
  return renderManifest(cases)
}

export async function runGenerator(options = {}) {
  const resolved = resolveOptions(options)
  const generated = await generateManifest(resolved)
  if (resolved.check) {
    await validateLockedContract(generated, resolved)
    let checkedIn
    try {
      checkedIn = await fs.readFile(resolved.output, 'utf8')
    } catch (error) {
      if (error.code === 'ENOENT') {
        throw new Error(`${repoPath(resolved.root, resolved.output)} is missing; run node tools/generate-textmate-cases.mjs`)
      }
      throw error
    }
    if (checkedIn !== generated) {
      throw new Error(`${repoPath(resolved.root, resolved.output)} is out of date; run node tools/generate-textmate-cases.mjs`)
    }
    console.log(`${repoPath(resolved.root, resolved.output)}: up to date`)
    return
  }

  await fs.mkdir(path.dirname(resolved.output), { recursive: true })
  await fs.writeFile(resolved.output, generated)
  console.log(`wrote ${repoPath(resolved.root, resolved.output)}`)
}

function resolveOptions(options) {
  const root = path.resolve(options.root ?? DEFAULT_ROOT)
  return {
    root,
    fixtures: resolveFromRoot(root, options.fixtures ?? DEFAULT_FIXTURES),
    grammars: resolveFromRoot(root, options.grammars ?? DEFAULT_GRAMMARS),
    config: resolveFromRoot(root, options.config ?? DEFAULT_CONFIG),
    output: resolveFromRoot(root, options.output ?? DEFAULT_OUTPUT),
    coverage: resolveFromRoot(root, options.coverage ?? DEFAULT_COVERAGE),
    divergences: resolveFromRoot(root, options.divergences ?? DEFAULT_DIVERGENCES),
    policy: resolveFromRoot(root, options.policy ?? DEFAULT_POLICY),
    lockedCount: options.lockedCount ?? 264,
    check: options.check ?? false,
  }
}

async function validateLockedContract(manifest, options) {
  const [coverage, policy, divergences] = await Promise.all([
    fs.readFile(options.coverage, 'utf8'),
    fs.readFile(options.policy, 'utf8').then(JSON.parse),
    fs.readFile(options.divergences, 'utf8'),
  ])
  if (policy.schemaVersion !== 1) throw new Error('validation policy schemaVersion must be 1')
  const expectedKeys = [
    'publicLanguages',
    'validatedLanguages',
    'oracleLanguages',
    'stressCorpusLanguages',
  ]
  const expected = policy.expectedCounts
  if (!expected || typeof expected !== 'object' || Array.isArray(expected) ||
      Object.keys(expected).sort().join('\0') !== [...expectedKeys].sort().join('\0')) {
    throw new Error(`validation policy expectedCounts must contain exactly ${expectedKeys.join(', ')}`)
  }
  for (const key of expectedKeys) {
    if (!Number.isInteger(expected[key]) || expected[key] < 0) {
      throw new Error(`validation policy expectedCounts.${key} must be a nonnegative integer`)
    }
    if (expected[key] !== options.lockedCount) {
      throw new Error(`validation policy expectedCounts.${key} must remain locked at ${options.lockedCount}`)
    }
  }

  const countMatch = coverage.match(/^public_language_count\s*=\s*(\d+)$/m)
  const keptMatch = coverage.match(/^kept\s*=\s*\[([\s\S]*?)^\]/m)
  if (!countMatch || !keptMatch) throw new Error('coverage manifest lacks public_language_count or kept')
  const publicIds = new Set([...keptMatch[1].matchAll(/"([^"]+)"/g)].map(match => match[1]))
  const declaredPublic = Number(countMatch[1])
  if (publicIds.size !== declaredPublic) {
    throw new Error(`coverage manifest declares ${declaredPublic} public IDs but lists ${publicIds.size}`)
  }

  const divergentFixtures = new Set(
    [...divergences.matchAll(/^fixture\s*=\s*"([^"]+)"$/gm)].map(match => match[1]),
  )
  const byLanguage = new Map()
  for (const block of manifest.split(/^\[\[case\]\]\s*$/m).slice(1)) {
    const language = tomlValue(block, 'language')
    const grammar = tomlValue(block, 'grammar')
    const fixture = tomlValue(block, 'fixture')
    const golden = tomlValue(block, 'golden')
    if (!language || !grammar || !fixture || !golden) {
      throw new Error('generated case is missing language, grammar, fixture, or golden')
    }
    const grammarId = path.posix.basename(grammar).replace(/\.tmLanguage\.json$/, '')
    const publicId = publicIds.has(language) ? language : grammarId
    if (!publicIds.has(publicId)) {
      throw new Error(`cannot map case language ${JSON.stringify(language)} to a public ID`)
    }
    const kind = path.posix.basename(fixture).split('.', 1)[0]
    const records = byLanguage.get(publicId) ?? []
    records.push({ kind, fixture, golden })
    byLanguage.set(publicId, records)
  }

  const oracleIds = new Set(byLanguage.keys())
  const stressIds = new Set()
  const validatedIds = new Set()
  for (const [language, records] of byLanguage) {
    const required = records.filter(record => record.kind === 'basic' || record.kind === 'stress')
    const kinds = new Set(required.map(record => record.kind))
    if (kinds.has('stress')) stressIds.add(language)
    if (kinds.has('basic') && kinds.has('stress') &&
        (await Promise.all(required.map(record => exactContractCase(record, divergentFixtures, options.root)))).every(Boolean)) {
      validatedIds.add(language)
    }
  }

  for (const [label, ids] of [['oracle', oracleIds], ['validated', validatedIds], ['stress', stressIds]]) {
    const missing = [...publicIds].filter(language => !ids.has(language)).sort()
    if (missing.length > 0) {
      throw new Error(`${label} membership differs from the public catalog: missing=${missing.join(', ')}`)
    }
  }
  assertLockedCount(expected, 'publicLanguages', publicIds.size)
  assertLockedCount(expected, 'validatedLanguages', validatedIds.size)
  assertLockedCount(expected, 'oracleLanguages', oracleIds.size)
  assertLockedCount(expected, 'stressCorpusLanguages', stressIds.size)
}

async function exactContractCase(record, divergentFixtures, root) {
  if (divergentFixtures.has(record.fixture)) return false
  const fixturePath = resolveFromRoot(root, record.fixture)
  const goldenPath = resolveFromRoot(root, record.golden)
  let fixture
  let golden
  try {
    [fixture, golden] = await Promise.all([
      fs.readFile(fixturePath, 'utf8'),
      fs.readFile(goldenPath, 'utf8'),
    ])
  } catch (error) {
    if (error.code === 'ENOENT') return false
    throw error
  }
  const lineCount = fixture.split(/\r?\n/).length - Number(fixture.endsWith('\n'))
  if (record.kind === 'basic' && (lineCount < 10 || lineCount > 30)) return false
  if (record.kind === 'stress' && (lineCount < 140 || lineCount > 260)) return false
  const lines = golden.split(/\r?\n/).filter(line => line.trim())
  if (lines.length === 0) return false
  return lines.every(line => JSON.parse(line).stoppedEarly === false)
}

function tomlValue(block, key) {
  return block.match(new RegExp(`^${key}\\s*=\\s*"([^"]+)"`, 'm'))?.[1]
}

function assertLockedCount(expected, key, actual) {
  if (actual !== expected[key]) {
    throw new Error(`validation policy requires ${key}=${expected[key]}, found ${actual}`)
  }
}

function resolveFromRoot(root, value) {
  return path.isAbsolute(value) ? path.normalize(value) : path.resolve(root, value)
}

async function loadGrammarCatalog(grammarDir, root) {
  const names = (await fs.readdir(grammarDir)).filter(name => name.endsWith('.json')).sort()
  const assets = new Map()
  const scopes = new Map()
  for (const name of names) {
    const absolutePath = path.join(grammarDir, name)
    const grammar = JSON.parse(await fs.readFile(absolutePath, 'utf8'))
    if (typeof grammar.scopeName !== 'string' || grammar.scopeName.length === 0) {
      throw new Error(`${repoPath(root, absolutePath)}: missing string scopeName`)
    }
    const asset = name.endsWith('.tmLanguage.json')
      ? name.slice(0, -'.tmLanguage.json'.length)
      : name.slice(0, -'.json'.length)
    if (assets.has(asset)) throw new Error(`duplicate grammar asset ${asset}`)
    if (scopes.has(grammar.scopeName)) {
      throw new Error(`duplicate grammar scope ${grammar.scopeName}: ${scopes.get(grammar.scopeName).path} and ${repoPath(root, absolutePath)}`)
    }
    const record = { asset, scope: grammar.scopeName, path: repoPath(root, absolutePath), grammar }
    assets.set(asset, record)
    scopes.set(record.scope, record)
  }
  return { assets, scopes }
}

async function loadConfig(configPath) {
  const config = JSON.parse(await fs.readFile(configPath, 'utf8'))
  assertObject(config, 'config')
  assertOnlyKeys(config, new Set(['languageAssets', 'cases']), 'config')
  const languageAssets = config.languageAssets ?? {}
  const cases = config.cases ?? []
  assertObject(languageAssets, 'languageAssets')
  if (!Array.isArray(cases)) throw new Error('config.cases must be an array')
  for (const [language, asset] of Object.entries(languageAssets)) {
    if (!language || typeof asset !== 'string' || !asset) {
      throw new Error('config.languageAssets values must be non-empty strings')
    }
  }
  cases.forEach((spec, index) => validateConfigCase(spec, index))
  return { languageAssets, cases }
}

function validateConfigCase(spec, index) {
  const label = `config.cases[${index}]`
  assertObject(spec, label)
  assertOnlyKeys(spec, new Set(['language', 'fixture', 'asset', 'scope', 'grammar', 'golden']), label)
  for (const required of ['language', 'fixture']) {
    if (typeof spec[required] !== 'string' || !spec[required]) {
      throw new Error(`${label}.${required} must be a non-empty string`)
    }
  }
  for (const optional of ['asset', 'scope', 'grammar', 'golden']) {
    if (spec[optional] !== undefined && (typeof spec[optional] !== 'string' || !spec[optional])) {
      throw new Error(`${label}.${optional} must be a non-empty string`)
    }
  }
}

async function discoverConventionCases(fixturesDir, root) {
  const cases = []
  const languages = (await fs.readdir(fixturesDir, { withFileTypes: true }))
    .filter(entry => entry.isDirectory())
    .sort((left, right) => compareText(left.name, right.name))
  for (const language of languages) {
    const directory = path.join(fixturesDir, language.name)
    const entries = (await fs.readdir(directory, { withFileTypes: true }))
      .filter(entry => entry.isFile())
      .sort((left, right) => compareText(left.name, right.name))
    for (const entry of entries) {
      const match = entry.name.match(CONVENTIONAL_NAMES)
      if (!match || isGeneratedOrSample(entry.name)) continue
      cases.push({
        language: language.name,
        fixture: repoPath(root, path.join(directory, entry.name)),
        role: match[1],
      })
    }
  }
  return cases
}

function isGeneratedOrSample(name) {
  if (name.endsWith('.golden.jsonl')) return true
  const segments = name.toLowerCase().split('.')
  return segments.includes('sample')
}

function mergeCaseSpecs(discovered, configured, root) {
  const byFixture = new Map(discovered.map(spec => [spec.fixture, spec]))
  for (const override of configured) {
    const fixture = repoPath(root, resolveFromRoot(root, override.fixture))
    const existing = byFixture.get(fixture)
    byFixture.set(fixture, { ...existing, ...override, fixture })
  }
  return [...byFixture.values()]
}

async function resolveCase(spec, config, catalog, root) {
  const fixturePath = resolveFromRoot(root, spec.fixture)
  await requireFile(fixturePath, root, 'fixture')
  const asset = spec.asset ?? config.languageAssets[spec.language] ?? spec.language
  let grammarRecord
  if (spec.grammar) {
    const grammarPath = resolveFromRoot(root, spec.grammar)
    await requireFile(grammarPath, root, 'grammar')
    const grammar = JSON.parse(await fs.readFile(grammarPath, 'utf8'))
    if (typeof grammar.scopeName !== 'string' || !grammar.scopeName) {
      throw new Error(`${repoPath(root, grammarPath)}: missing string scopeName`)
    }
    grammarRecord = {
      asset,
      scope: grammar.scopeName,
      path: repoPath(root, grammarPath),
      grammar,
    }
  } else {
    grammarRecord = catalog.assets.get(asset)
    if (!grammarRecord) {
      throw new Error(`no vendored grammar asset ${JSON.stringify(asset)} for fixture language ${JSON.stringify(spec.language)}`)
    }
  }

  const fixture = repoPath(root, fixturePath)
  const golden = spec.golden
    ? repoPath(root, resolveFromRoot(root, spec.golden))
    : defaultGolden(fixture, spec.role)
  return {
    language: spec.language,
    scope: spec.scope ?? grammarRecord.scope,
    grammar: grammarRecord.path,
    fixture,
    golden,
    embedded: externalGrammarClosure(grammarRecord, catalog),
  }
}

function externalGrammarClosure(rootGrammar, catalog) {
  const pending = [{ grammar: rootGrammar, repository: null }]
  const visited = new Set()
  const embedded = new Map()
  while (pending.length > 0) {
    const { grammar, repository } = pending.pop()
    const visitKey = `${grammar.scope}#${repository ?? ''}`
    if (visited.has(visitKey)) continue
    visited.add(visitKey)
    const includes = [...collectExternalIncludes(grammar.grammar, repository)].sort().reverse()
    for (const include of includes) {
      const [scope, dependencyRepository = null] = include.split('#', 2)
      const dependency = catalog.scopes.get(scope)
      // vscode-textmate's Registry treats a null loadGrammar result as an
      // unavailable include. Do not manufacture manifest entries for it.
      if (!dependency) continue
      if (scope !== rootGrammar.scope) embedded.set(scope, dependency)
      // The vendored YAML root is a dispatcher whose private version grammars
      // are validated by the direct YAML cases. Keep those override-only
      // assets out of unrelated host cases: recursively expanding them here
      // silently changes every Markdown-like oracle's dependency surface.
      if (scope === 'source.yaml' && rootGrammar.scope !== 'source.yaml') continue
      pending.push({ grammar: dependency, repository: dependencyRepository })
    }
  }
  return [...embedded.values()]
    .sort((left, right) => compareText(left.scope, right.scope))
    .map(grammar => ({ scope: grammar.scope, grammar: grammar.path }))
}

function collectExternalIncludes(grammar, repositoryRule = null) {
  const found = new Set()
  const repository = grammar?.repository ?? {}
  // Repository includes are a shared graph. Permanent memoization is both
  // cycle protection and necessary to avoid exponentially revisiting common
  // Markdown repositories.
  const visitedLocal = new Set()

  const visitPatterns = patterns => {
    if (!Array.isArray(patterns)) return
    for (const rule of patterns) visitRule(rule)
  }
  const visitRule = rule => {
    if (!rule || typeof rule !== 'object' || Array.isArray(rule)) return
    if (typeof rule.include === 'string') {
      const include = rule.include
      if (include.startsWith('#')) {
        const name = include.slice(1)
        if (!visitedLocal.has(name) && repository[name]) {
          visitedLocal.add(name)
          visitRule(repository[name])
        }
      } else if (include !== '$self' && include !== '$base') {
        found.add(include)
      }
      return
    }
    // Match vscode-textmate dependency discovery: capture maps are not part
    // of the ordinary pattern graph and do not activate external grammars.
    visitPatterns(rule.patterns)
  }

  if (repositoryRule !== null) visitRule(repository[repositoryRule])
  else {
    visitPatterns(grammar?.patterns)
    for (const injection of Object.values(grammar?.injections ?? {})) visitRule(injection)
  }
  return found
}

function defaultGolden(fixture, role) {
  const directory = path.posix.dirname(fixture)
  const stem = role ?? path.posix.basename(fixture).replace(/\.[^.]+$/, '')
  return path.posix.join(directory, `${stem}.golden.jsonl`)
}

function renderManifest(cases) {
  const lines = [
    '# Generated by tools/generate-textmate-cases.mjs; do not edit.',
    '# Edit cases.config.json for language mappings or exceptional cases.',
    '# Regenerate with: node tools/generate-textmate-cases.mjs',
    '',
  ]
  for (const testCase of cases) {
    lines.push('[[case]]')
    lines.push(`language = ${tomlString(testCase.language)}`)
    lines.push(`scope = ${tomlString(testCase.scope)}`)
    lines.push(`grammar = ${tomlString(testCase.grammar)}`)
    lines.push(`fixture = ${tomlString(testCase.fixture)}`)
    lines.push(`golden = ${tomlString(testCase.golden)}`)
    for (const grammar of testCase.embedded) {
      lines.push('')
      lines.push('[[case.embedded]]')
      lines.push(`scope = ${tomlString(grammar.scope)}`)
      lines.push(`grammar = ${tomlString(grammar.grammar)}`)
    }
    lines.push('')
  }
  return `${lines.join('\n').trimEnd()}\n`
}

function tomlString(value) {
  return JSON.stringify(value)
}

function compareText(left, right) {
  return left < right ? -1 : left > right ? 1 : 0
}

async function requireFile(filePath, root, kind) {
  let stat
  try {
    stat = await fs.stat(filePath)
  } catch (error) {
    if (error.code === 'ENOENT') throw new Error(`${kind} does not exist: ${repoPath(root, filePath)}`)
    throw error
  }
  if (!stat.isFile()) throw new Error(`${kind} is not a file: ${repoPath(root, filePath)}`)
}

function repoPath(root, filePath) {
  const relative = path.relative(root, filePath)
  if (relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
    throw new Error(`path is outside repository root: ${filePath}`)
  }
  return relative.split(path.sep).join('/')
}

function assertObject(value, label) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be an object`)
  }
}

function assertOnlyKeys(value, allowed, label) {
  for (const key of Object.keys(value)) {
    if (!allowed.has(key)) throw new Error(`${label} has unsupported key ${key}`)
  }
}

async function main() {
  let args
  try {
    args = parseArgs(process.argv)
  } catch (error) {
    console.error(error.message)
    usage()
    process.exitCode = 2
    return
  }
  if (args.help) {
    usage()
    return
  }
  try {
    await runGenerator(args)
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  await main()
}
