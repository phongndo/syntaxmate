#!/usr/bin/env node
/**
 * Inventory the regular expressions used by the vendored TextMate grammars.
 *
 * The default invocation remains a JSON stats report:
 *   node tools/grammar-stats.mjs [grammar-directory]
 *
 * To compare an inventory (or a language batch) with the regex proving set:
 *   node tools/grammar-stats.mjs --conformance-diff --languages ini,elixir
 */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'

const featureKeys = ['lookahead', 'lookbehind', 'backreference', 'anchorA', 'anchorG', 'lineAnchor', 'namedGroup', 'possessiveOrAtomic', 'inlineFlags', 'unicodeOrPosixClass']
const patternKeys = ['match', 'begin', 'end', 'while']
const variantSensitiveConstructs = new Set([
  'inline-flags.global-set', 'inline-flags.global-clear',
  'inline-flags.scoped-set', 'inline-flags.scoped-clear',
  'unicode-property.positive', 'unicode-property.negative',
  'posix-class.positive', 'posix-class.negative',
])

export function extractConstructs(pattern) {
  const found = []
  const activeOutsideClass = activeOutsideClassMask(pattern)
  const add = (id, match, detail) => found.push({ id, token: match[0], detail, index: match.index })
  const matches = (regex, id, detail = undefined, requireOutsideClass = false) => {
    for (const match of pattern.matchAll(regex)) {
      if (!isActiveEscapeAware(pattern, match.index)) continue
      if (requireOutsideClass && !activeOutsideClass[match.index]) continue
      add(id, match, typeof detail === 'function' ? detail(match) : detail)
    }
  }

  matches(/\(\?=/g, 'lookahead.positive', undefined, true)
  matches(/\(\?!/g, 'lookahead.negative', undefined, true)
  matches(/\(\?<=/g, 'lookbehind.positive', undefined, true)
  matches(/\(\?<!/g, 'lookbehind.negative', undefined, true)
  matches(/\(\?<([A-Za-z_][A-Za-z0-9_]*)>/g, 'named-group.angle', match => match[1], true)
  matches(/\(\?P<([A-Za-z_][A-Za-z0-9_]*)>/g, 'named-group.python', match => match[1], true)
  const namedGroups = [...pattern.matchAll(/\(\?(?:P)?<([A-Za-z_][A-Za-z0-9_]*)>/g)]
    .filter(match => isActiveEscapeAware(pattern, match.index) && activeOutsideClass[match.index])
  const namedGroupCounts = new Map()
  for (const match of namedGroups) namedGroupCounts.set(match[1], (namedGroupCounts.get(match[1]) ?? 0) + 1)
  for (const match of namedGroups) {
    if (namedGroupCounts.get(match[1]) > 1) add('named-group.duplicate', match, match[1])
  }

  matches(/\\([1-9][0-9]*)/g, 'backreference.numbered', match => match[1], true)
  matches(/\\k<([^>]+)>/g, 'backreference.named-angle', match => match[1], true)
  matches(/\\k'([^']+)'/g, 'backreference.named-quote', match => match[1], true)
  // In Oniguruma, \g is a subexpression call, not a backreference. Keeping it
  // separate prevents a backreference case from falsely closing this gap.
  matches(/\\g<([^>]+)>/g, 'subroutine.angle', match => match[1], true)
  matches(/\\g'([^']+)'/g, 'subroutine.quote', match => match[1], true)

  matches(/\\A/g, 'anchor.text-start', undefined, true)
  matches(/\\G/g, 'anchor.search-start', undefined, true)
  for (let index = 0; index < pattern.length; index += 1) {
    if (pattern[index] === '^' && activeOutsideClass[index]) found.push({ id: 'anchor.line-start', token: '^', index })
  }

  matches(/\(\?>/g, 'atomic-group', undefined, true)
  for (let index = 0; index < pattern.length - 1; index += 1) {
    if (!activeOutsideClass[index] || pattern[index + 1] !== '+') continue
    const quantifier = pattern[index]
    if (quantifier === '*') found.push({ id: 'possessive.star', token: '*+', index })
    else if (quantifier === '+') found.push({ id: 'possessive.plus', token: '++', index })
    else if (quantifier === '?') found.push({ id: 'possessive.question', token: '?+', index })
    else if (quantifier === '}') {
      const start = pattern.lastIndexOf('{', index)
      const body = start >= 0 ? pattern.slice(start + 1, index) : ''
      if (/^\s*\d*\s*(?:,\s*\d*\s*)?$/.test(body) && /\d/.test(body)) {
        found.push({ id: 'possessive.bounded', token: pattern.slice(start, index + 2), detail: body.replace(/\s/g, ''), index: start })
      }
    }
  }

  for (const match of pattern.matchAll(/\(\?([imsx]*)(?:-([imsx]+))?([:)])/g)) {
    if (!isActiveEscapeAware(pattern, match.index) || !activeOutsideClass[match.index]) continue
    const [, enabled, disabled, terminator] = match
    if (!enabled && !disabled) continue
    const scope = terminator === ':' ? 'scoped' : 'global'
    if (enabled) add(`inline-flags.${scope}-set`, match, enabled)
    if (disabled) add(`inline-flags.${scope}-clear`, match, disabled)
    if (enabled?.includes('x')) add('inline-flags.extended-set', match, enabled)
    if (disabled?.includes('x')) add('inline-flags.extended-clear', match, disabled)
  }

  matches(/\\p\{([^}]+)\}/g, 'unicode-property.positive', match => match[1])
  matches(/\\P\{([^}]+)\}/g, 'unicode-property.negative', match => match[1])
  matches(/\[:([A-Za-z][A-Za-z0-9_-]*):\]/g, 'posix-class.positive', match => match[1])
  matches(/\[:\^([A-Za-z][A-Za-z0-9_-]*):\]/g, 'posix-class.negative', match => match[1])

  matches(/\(\?\(([1-9][0-9]*)\)/g, 'conditional.numbered', match => match[1], true)
  matches(/\(\?\(<([^>]+)>\)/g, 'conditional.named-angle', match => match[1], true)
  matches(/\(\?\('([^']+)'\)/g, 'conditional.named-quote', match => match[1], true)
  matches(/\(\?~/g, 'absent-group', undefined, true)
  found.push(...extractCharacterClassConstructs(pattern))

  return found.sort((left, right) => left.index - right.index || left.id.localeCompare(right.id))
}

export async function collectGrammarStats(options = {}) {
  const root = options.root ?? 'assets/grammars/languages'
  const exampleLimit = options.exampleLimit ?? 3
  const requestedLanguages = new Set(options.languages ?? [])
  const allFiles = (await fs.readdir(root)).filter(file => file.endsWith('.json')).sort()
  const files = allFiles.filter(file => requestedLanguages.size === 0 || requestedLanguages.has(languageFromFile(file)))
  if (requestedLanguages.size) {
    const found = new Set(files.map(languageFromFile))
    const unknown = [...requestedLanguages].filter(language => !found.has(language)).sort()
    if (unknown.length) throw new Error(`unknown grammar language id(s): ${unknown.join(', ')}`)
  }

  const totals = Object.fromEntries(featureKeys.map(key => [key, 0]))
  let patternCount = 0
  let fallbackPatternCount = 0
  let dfaPatternCount = 0
  let includeEdges = 0
  let injectionCount = 0
  const grammars = []
  const allConstructs = new Map()

  for (const file of files) {
    const full = path.join(root, file)
    const grammar = JSON.parse(await fs.readFile(full, 'utf8'))
    const localConstructs = new Map()
    const local = {
      language: languageFromFile(file),
      scopeName: grammar.scopeName,
      patterns: 0,
      dfaPatterns: 0,
      fallbackPatterns: 0,
      includes: 0,
      injections: grammar.injections ? Object.keys(grammar.injections).length : 0,
      features: Object.fromEntries(featureKeys.map(key => [key, 0])),
    }
    injectionCount += local.injections
    visit(grammar, node => {
      for (const key of patternKeys) {
        if (typeof node[key] !== 'string') continue
        const pattern = node[key]
        patternCount += 1
        local.patterns += 1
        const features = classify(pattern)
        const fallback = features.lookahead || features.lookbehind || features.backreference || features.anchorG || features.namedGroup || features.possessiveOrAtomic
        if (fallback) { fallbackPatternCount += 1; local.fallbackPatterns += 1 }
        else { dfaPatternCount += 1; local.dfaPatterns += 1 }
        for (const feature of featureKeys) {
          if (features[feature]) {
            totals[feature] += 1
            local.features[feature] += 1
          }
        }
        recordConstructs(localConstructs, extractConstructs(pattern), { field: key, pattern }, exampleLimit)
      }
      if (typeof node.include === 'string') {
        includeEdges += 1
        local.includes += 1
      }
    })
    local.constructs = serializeConstructs(localConstructs)
    local.constructList = Object.keys(local.constructs)
    for (const [id, record] of localConstructs) {
      let aggregate = allConstructs.get(id)
      if (!aggregate) {
        aggregate = { patternCount: 0, occurrenceCount: 0, variants: new Map(), grammars: new Set(), examples: [] }
        allConstructs.set(id, aggregate)
      }
      aggregate.patternCount += record.patternCount
      aggregate.occurrenceCount += record.occurrenceCount
      for (const [variant, count] of record.variants) {
        aggregate.variants.set(variant, (aggregate.variants.get(variant) ?? 0) + count)
      }
      aggregate.grammars.add(local.language)
      for (const example of record.examples) {
        if (aggregate.examples.length >= exampleLimit) break
        aggregate.examples.push({ language: local.language, ...example })
      }
    }
    grammars.push(local)
  }

  return {
    schemaVersion: 2,
    constructTaxonomyVersion: 1,
    grammarCount: grammars.length,
    patternCount,
    dfaPatternCount,
    fallbackPatternCount,
    fallbackPatternPercent: patternCount ? fallbackPatternCount / patternCount : 0,
    includeEdges,
    injectionCount,
    features: totals,
    constructs: serializeConstructs(allConstructs, true),
    grammars,
  }
}

export function diffConformance(stats, cases) {
  const representedBy = new Map()
  const representedVariants = new Map()
  const caseMetadataMismatches = []
  for (const testCase of cases) {
    const declared = new Set(testCase.constructs ?? [])
    const detected = extractConstructs(testCase.pattern)
    const detectedIds = new Set(detected.map(occurrence => occurrence.id))
    const undeclared = [...detectedIds].filter(id => !declared.has(id)).sort()
    const notDetected = [...declared].filter(id => !detectedIds.has(id)).sort()
    if (undeclared.length || notDetected.length) caseMetadataMismatches.push({ case: testCase.name, undeclared, notDetected })
    for (const id of declared) {
      const names = representedBy.get(id) ?? []
      names.push(testCase.name)
      representedBy.set(id, names)
    }
    for (const occurrence of detected) {
      if (!declared.has(occurrence.id) || occurrence.detail == null) continue
      const variants = representedVariants.get(occurrence.id) ?? new Set()
      variants.add(occurrence.detail)
      representedVariants.set(occurrence.id, variants)
    }
  }
  const inventoryIds = Object.keys(stats.constructs).sort()
  const representedIds = [...representedBy.keys()].sort()
  const missingIds = inventoryIds.filter(id => !representedBy.has(id))
  const unusedIds = representedIds.filter(id => !stats.constructs[id])
  const missingSet = new Set(missingIds)
  const missingVariants = {}
  let inventoryVariantCount = 0
  let representedVariantCount = 0
  for (const id of inventoryIds) {
    if (!variantSensitiveConstructs.has(id)) continue
    const variants = Object.keys(stats.constructs[id].variants ?? {})
    const represented = representedVariants.get(id) ?? new Set()
    inventoryVariantCount += variants.length
    representedVariantCount += variants.filter(variant => represented.has(variant)).length
    const missing = variants.filter(variant => !represented.has(variant))
    if (missing.length) missingVariants[id] = Object.fromEntries(missing.map(variant => [variant, stats.constructs[id].variants[variant]]))
  }
  for (const grammar of stats.grammars) {
    grammar.conformanceGaps = grammar.constructList.filter(id => missingSet.has(id))
    grammar.conformanceVariantGaps = Object.fromEntries(Object.entries(missingVariants).flatMap(([id, variants]) => {
      const local = grammar.constructs[id]?.variants ?? {}
      const missing = Object.keys(variants).filter(variant => local[variant] != null)
      return missing.length ? [[id, missing]] : []
    }))
  }
  return {
    summary: {
      inventoryConstructs: inventoryIds.length,
      representedConstructs: inventoryIds.filter(id => representedBy.has(id)).length,
      missingConstructs: missingIds.length,
      inventoryVariants: inventoryVariantCount,
      representedVariants: representedVariantCount,
      missingVariants: inventoryVariantCount - representedVariantCount,
    },
    represented: Object.fromEntries(representedIds.map(id => [id, representedBy.get(id)])),
    missing: Object.fromEntries(missingIds.map(id => [id, stats.constructs[id]])),
    missingVariants,
    representedOutsideInventory: unusedIds,
    caseMetadataMismatches,
    grammarsWithGaps: stats.grammars
      .filter(grammar => grammar.conformanceGaps.length || Object.keys(grammar.conformanceVariantGaps).length)
      .map(grammar => ({ language: grammar.language, missing: grammar.conformanceGaps, missingVariants: grammar.conformanceVariantGaps })),
  }
}

export function hasConformanceGaps(conformance) {
  return Boolean(
    conformance.summary.missingConstructs ||
    conformance.summary.missingVariants ||
    conformance.caseMetadataMismatches.length,
  )
}

function visit(value, callback) {
  if (Array.isArray(value)) {
    for (const item of value) visit(item, callback)
  } else if (value && typeof value === 'object') {
    callback(value)
    for (const item of Object.values(value)) visit(item, callback)
  }
}

function classify(pattern) {
  return {
    lookahead: /\(\?[=!]/.test(pattern),
    lookbehind: /\(\?<([=!])/.test(pattern),
    backreference: /\\[1-9]/.test(pattern),
    anchorA: /\\A/.test(pattern),
    anchorG: /\\G/.test(pattern),
    lineAnchor: /(^|[^\\])\^/.test(pattern),
    namedGroup: /\(\?(P<|<[^=!])/.test(pattern),
    possessiveOrAtomic: /\(\?>|[+*?}]\+/.test(pattern),
    inlineFlags: /\(\?[imsx-]/.test(pattern),
    unicodeOrPosixClass: /\\[pP]\{|\[:\^?[a-z]+:\]/i.test(pattern),
  }
}

function recordConstructs(records, occurrences, example, exampleLimit) {
  const byId = new Map()
  for (const occurrence of occurrences) {
    const list = byId.get(occurrence.id) ?? []
    list.push(occurrence)
    byId.set(occurrence.id, list)
  }
  for (const [id, items] of byId) {
    let record = records.get(id)
    if (!record) {
      record = { patternCount: 0, occurrenceCount: 0, variants: new Map(), examples: [] }
      records.set(id, record)
    }
    record.patternCount += 1
    record.occurrenceCount += items.length
    for (const item of items) {
      if (item.detail != null) record.variants.set(item.detail, (record.variants.get(item.detail) ?? 0) + 1)
    }
    if (record.examples.length < exampleLimit) {
      record.examples.push({
        ...example,
        tokens: [...new Set(items.map(item => item.token))],
        details: [...new Set(items.map(item => item.detail).filter(detail => detail != null))],
      })
    }
  }
}

function serializeConstructs(records, aggregate = false) {
  return Object.fromEntries([...records.entries()].sort(([left], [right]) => left.localeCompare(right)).map(([id, record]) => [id, {
    patternCount: record.patternCount,
    occurrenceCount: record.occurrenceCount,
    ...(aggregate ? { grammarCount: record.grammars.size, grammars: [...record.grammars].sort() } : {}),
    ...(record.variants?.size ? { variants: Object.fromEntries([...record.variants].sort(([left], [right]) => left.localeCompare(right))) } : {}),
    examples: record.examples,
  }]))
}

function languageFromFile(file) {
  return file.replace(/\.tmLanguage\.json$/, '').replace(/\.json$/, '')
}

function activeOutsideClassMask(pattern) {
  const activeOutsideClass = new Uint8Array(pattern.length)
  let inClass = false
  for (let index = 0; index < pattern.length; index += 1) {
    if (!isActiveEscapeAware(pattern, index)) continue
    activeOutsideClass[index] = Number(!inClass)
    if (pattern[index] === '[' && !inClass) inClass = true
    else if (pattern[index] === ']' && inClass) inClass = false
  }
  return activeOutsideClass
}

function extractCharacterClassConstructs(pattern) {
  const found = []
  let depth = 0
  for (let index = 0; index < pattern.length; index += 1) {
    if (!isActiveEscapeAware(pattern, index)) continue
    if (pattern[index] === '[') {
      // POSIX classes such as [[:alpha:]] are atoms within the outer class,
      // not nested character classes.
      if (depth > 0 && pattern[index + 1] === ':') {
        const end = pattern.indexOf(':]', index + 2)
        if (end >= 0) index = end + 1
        continue
      }
      if (depth > 0) {
        found.push({ id: 'character-class.nested', token: '[', index })
      }
      depth += 1
    } else if (pattern[index] === ']' && depth > 0) {
      depth -= 1
    } else if (pattern[index] === '&' && pattern[index + 1] === '&' && depth > 0) {
      found.push({ id: 'character-class.intersection', token: '&&', index })
      index += 1
    }
  }
  return found
}

function isActiveEscapeAware(pattern, index) {
  let slashes = 0
  for (let cursor = index - 1; cursor >= 0 && pattern[cursor] === '\\'; cursor -= 1) slashes += 1
  return slashes % 2 === 0
}

async function loadConformanceCases(file) {
  const url = file ? pathToFileURL(path.resolve(file)).href : new URL('./regex-conformance.mjs', import.meta.url).href
  const module = await import(url)
  const cases = module.conformanceCases ?? module.cases
  if (!Array.isArray(cases)) throw new Error(`${file ?? 'tools/regex-conformance.mjs'} does not export a conformanceCases array`)
  return cases
}

function parseArgs(argv) {
  const options = { root: 'assets/grammars/languages', exampleLimit: 3, languages: [], compact: false, conformanceDiff: false, failOnGaps: false }
  const positional = []
  for (let index = 0; index < argv.length; index += 1) {
    const value = argv[index]
    if (value === '--conformance-diff') options.conformanceDiff = true
    else if (value === '--fail-on-gaps') { options.failOnGaps = true; options.conformanceDiff = true }
    else if (value === '--compact') options.compact = true
    else if (value === '--help' || value === '-h') options.help = true
    else if (value === '--examples') options.exampleLimit = parseNonnegativeInteger(argv[++index], '--examples')
    else if (value === '--languages' || value === '--language') {
      const languages = argv[++index]
      if (!languages) throw new Error(`${value} requires a comma-separated value`)
      options.languages.push(...languages.split(',').map(language => language.trim()).filter(Boolean))
    } else if (value === '--conformance-cases') {
      options.conformanceCases = argv[++index]
      if (!options.conformanceCases) throw new Error('--conformance-cases requires a module path')
      options.conformanceDiff = true
    } else if (value.startsWith('-')) throw new Error(`unknown option: ${value}`)
    else positional.push(value)
  }
  if (positional.length > 1) throw new Error('expected at most one grammar directory')
  if (positional[0]) options.root = positional[0]
  options.languages = [...new Set(options.languages)]
  return options
}

function parseNonnegativeInteger(value, option) {
  if (!/^\d+$/.test(value ?? '')) throw new Error(`${option} requires a non-negative integer`)
  return Number(value)
}

function usage() {
  console.log(`usage: node tools/grammar-stats.mjs [grammar-directory] [options]

Options:
  --languages ID,ID       inventory only these grammar ids (repeatable)
  --examples N            retain N pattern examples per construct (default: 3)
  --conformance-diff      diff construct shapes against regex-conformance.mjs
  --conformance-cases MJS use another module exporting conformanceCases
  --fail-on-gaps          exit 1 for inventory gaps or case metadata mismatches
  --compact               emit compact rather than pretty JSON
  -h, --help              show this help`)
}

async function main() {
  try {
    const options = parseArgs(process.argv.slice(2))
    if (options.help) return usage()
    const stats = await collectGrammarStats(options)
    if (options.conformanceDiff) {
      stats.conformance = diffConformance(stats, await loadConformanceCases(options.conformanceCases))
    }
    console.log(JSON.stringify(stats, null, options.compact ? undefined : 2))
    if (options.failOnGaps && hasConformanceGaps(stats.conformance)) process.exitCode = 1
  } catch (error) {
    console.error(`grammar-stats: ${error.message}`)
    process.exitCode = 2
  }
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) await main()
