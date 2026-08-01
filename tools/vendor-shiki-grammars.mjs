#!/usr/bin/env node
/**
 * Regenerate vendored TextMate grammars from the pinned @shikijs/langs package.
 *
 * This imports every dist/*.mjs module from the pinned package recorded in
 * assets/grammars/SOURCE.toml, deduplicates embedded dependency copies by
 * grammar `name`, and writes compact JSON grammar assets. Non-Shiki assets that
 * are already present under assets/grammars/languages are preserved; their
 * existing licenses.json entries are carried forward.
 *
 * Usage:
 *   node tools/vendor-shiki-grammars.mjs          # rewrite generated files
 *   node tools/vendor-shiki-grammars.mjs --check  # verify generated files
 *   node tools/vendor-shiki-grammars.mjs --dry-run
 */

import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const args = new Set(process.argv.slice(2))
const check = args.has('--check')
const dryRun = args.has('--dry-run')
for (const arg of args) {
  if (!['--check', '--dry-run'].includes(arg)) {
    throw new Error(`unknown argument: ${arg}`)
  }
}

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const assetsRoot = path.join(root, 'assets/grammars')
const languagesDir = path.join(assetsRoot, 'languages')
const sourcePath = path.join(assetsRoot, 'SOURCE.toml')
const coveragePath = path.join(assetsRoot, 'coverage.toml')
const fullCoveragePath = path.join(assetsRoot, 'coverage.full-shiki.toml')
const licensesPath = path.join(assetsRoot, 'licenses.json')
const languageMetadataPath = path.join(assetsRoot, 'language-metadata.json')

const sourceToml = await fs.readFile(sourcePath, 'utf8')
const sourcePathValue = tomlString(sourceToml, 'source_path')
const sourceRoot = path.resolve(root, sourcePathValue)
const distDir = path.join(sourceRoot, 'dist')

await assertDirectory(sourceRoot, `pinned @shikijs/langs source not found at ${sourceRoot}`)
await assertDirectory(distDir, `pinned @shikijs/langs dist not found at ${distDir}`)

const packageJson = JSON.parse(await fs.readFile(path.join(sourceRoot, 'package.json'), 'utf8'))
if (tomlString(sourceToml, 'package') !== packageJson.name) {
  throw new Error(`SOURCE.toml package does not match ${packageJson.name}`)
}
if (tomlString(sourceToml, 'package_version') !== packageJson.version) {
  throw new Error(`SOURCE.toml package_version does not match ${packageJson.version}`)
}

const { languageNames } = await import(pathToFileURL(path.join(distDir, 'index.mjs')).href)
const moduleFiles = (await fs.readdir(distDir))
  .filter(file => file.endsWith('.mjs') && file !== 'index.mjs')
  .sort((left, right) => left.localeCompare(right))

const byName = new Map()
let importedGrammarCopies = 0
for (const moduleFile of moduleFiles) {
  const modulePath = path.join(distDir, moduleFile)
  const exported = (await import(pathToFileURL(modulePath).href)).default
  const grammars = Array.isArray(exported) ? exported : [exported]
  for (const grammar of grammars) {
    if (!grammar || typeof grammar !== 'object') {
      throw new Error(`${moduleFile} default export contains a non-object grammar`)
    }
    if (typeof grammar.name !== 'string' || grammar.name.length === 0) {
      throw new Error(`${moduleFile} exports a grammar without string name`)
    }
    if (typeof grammar.scopeName !== 'string' || grammar.scopeName.length === 0) {
      throw new Error(`${moduleFile} exports ${grammar.name} without string scopeName`)
    }
    importedGrammarCopies += 1
    const candidates = byName.get(grammar.name) ?? []
    candidates.push({
      grammar,
      module: moduleFile,
      moduleStem: path.basename(moduleFile, '.mjs'),
      json: `${canonicalJson(grammar)}\n`,
    })
    byName.set(grammar.name, candidates)
  }
}

const shikiNames = [...byName.keys()].sort((left, right) => left.localeCompare(right))
assertSameSet(shikiNames, [...languageNames].sort((left, right) => left.localeCompare(right)), 'dist grammar names', 'index languageNames')

const shikiAssets = []
for (const name of shikiNames) {
  const candidates = byName.get(name).sort(compareCandidate)
  const [selected, ...rest] = candidates
  for (const duplicate of rest) {
    if (duplicate.json !== selected.json) {
      throw new Error(`grammar ${name} differs between ${selected.module} and ${duplicate.module}`)
    }
  }
  shikiAssets.push({
    language: name,
    grammar: selected.grammar,
    json: selected.json,
    module: selected.module,
    path: languageAssetPath(name),
    source: '@shikijs/langs dist',
  })
}

// Markdown's grammar still includes `source.twig`; Shiki's Twig grammar is
// rooted at `text.html.twig`. Keep the private compatibility root-scope alias
// that the current bundle already uses.
const shikiAliasAssets = [makeTwigSourceAlias(shikiAssets)]
// These public languages intentionally track VS Code's pinned built-in
// grammars because their Shiki revisions have observable scope/style
// differences in the cross-asset behavior audit.
const vscodeOverrides = new Set(['dart', 'handlebars', 'php', 'pug', 'r', 'rst', 'yaml'])
const generatedAssets = [
  ...shikiAssets.filter(asset => !vscodeOverrides.has(asset.language)),
  ...shikiAliasAssets,
]
// Grammar content may come from an override, but language aliases and file
// types are registration metadata supplied by Shiki. VS Code's raw
// tmLanguage files do not contain its package-level language contributions.
const metadataAssets = [...shikiAssets, ...shikiAliasAssets]
const generatedNames = new Set(generatedAssets.map(asset => asset.language))

const existingLanguageNames = await existingAssetNames(languagesDir)
const preservedNames = existingLanguageNames
  .filter(name => !generatedNames.has(name))
  .sort((left, right) => left.localeCompare(right))
const allAssetNames = [...new Set([...shikiNames, ...shikiAliasAssets.map(asset => asset.language), ...preservedNames])]
  .sort((left, right) => left.localeCompare(right))

const currentCoverage = parseCoverage(await fs.readFile(coveragePath, 'utf8'))
const publicAssetNames = publicAssetsFromCoverage(currentCoverage)
const privateAssetNames = allAssetNames.filter(name => !publicAssetNames.has(name))

const currentLicenses = JSON.parse(await fs.readFile(licensesPath, 'utf8'))
const licenseAssets = await mergedLicenseAssets({
  generatedAssets,
  preservedNames,
  currentLicenses,
  publicAssetNames,
})
const licenses = {
  schemaVersion: 1,
  source: {
    package: packageJson.name,
    version: packageJson.version,
    repository: repositoryUrl(packageJson.repository),
    license: packageJson.license,
    sourcePath: sourcePathValue,
  },
  assets: licenseAssets,
}

const activeCoverage = renderActiveCoverage({
  kept: currentCoverage.kept,
  remapped: currentCoverage.remapped,
  allAssetNames,
  privateAssetNames,
})
const fullCoverage = renderFullCoverage({
  shikiNames,
  allAssetNames,
})
const languageMetadata = await renderLanguageMetadata({
  coverage: currentCoverage,
  metadataAssets,
  languagesDir,
  packageJson,
})

const outputs = new Map()
for (const asset of generatedAssets) {
  outputs.set(path.join(root, asset.path), asset.json)
}
outputs.set(licensesPath, `${JSON.stringify(licenses, null, 2)}\n`)
outputs.set(languageMetadataPath, languageMetadata)
outputs.set(coveragePath, activeCoverage)
outputs.set(fullCoveragePath, fullCoverage)

const changed = []
for (const [file, contents] of outputs) {
  const old = await fs.readFile(file, 'utf8').catch(error => {
    if (error.code === 'ENOENT') return undefined
    throw error
  })
  if (old !== contents) changed.push(path.relative(root, file))
}

if (check) {
  if (changed.length > 0) {
    throw new Error(`vendored Shiki grammars are stale:\n${changed.map(file => `  ${file}`).join('\n')}`)
  }
} else if (!dryRun) {
  await fs.mkdir(languagesDir, { recursive: true })
  for (const [file, contents] of outputs) {
    await fs.mkdir(path.dirname(file), { recursive: true })
    await fs.writeFile(file, contents)
  }
}

const action = check ? 'checked' : dryRun ? 'would vendor' : 'vendored'
console.log(`${action} ${shikiAssets.length} unique Shiki grammars from ${moduleFiles.length} modules (${importedGrammarCopies} exported copies)`)
console.log(`preserved ${preservedNames.length} non-generated assets: ${preservedNames.join(', ') || '(none)'}`)
console.log(`${check || dryRun ? 'would change' : 'changed'} ${changed.length} files`)

function tomlString(text, key) {
  const match = text.match(new RegExp(`(?:^|\\n)\\s*${escapeRegExp(key)}\\s*=\\s*"([^"]+)"`))
  if (!match) throw new Error(`SOURCE.toml missing ${key}`)
  return match[1]
}

async function assertDirectory(directory, message) {
  const stat = await fs.stat(directory).catch(error => {
    if (error.code === 'ENOENT') return undefined
    throw error
  })
  if (!stat?.isDirectory()) throw new Error(message)
}

function compareCandidate(left, right) {
  const leftExact = left.moduleStem === left.grammar.name ? 0 : 1
  const rightExact = right.moduleStem === right.grammar.name ? 0 : 1
  return leftExact - rightExact || left.module.localeCompare(right.module)
}

function makeTwigSourceAlias(shikiAssets) {
  const twig = shikiAssets.find(asset => asset.language === 'twig')
  if (!twig) throw new Error('Shiki package did not provide twig grammar for source.twig alias')
  const grammar = structuredClone(twig.grammar)
  if (grammar.scopeName !== 'text.html.twig') {
    throw new Error(`unexpected twig scopeName ${grammar.scopeName}`)
  }
  grammar.scopeName = 'source.twig'
  return {
    language: 'twig-source',
    grammar,
    json: `${canonicalJson(grammar)}\n`,
    module: twig.module,
    path: languageAssetPath('twig-source'),
    source: '@shikijs/langs dist (root-scope alias)',
    privateDependency: true,
  }
}

function languageAssetPath(name) {
  return `assets/grammars/languages/${name}.tmLanguage.json`
}

async function existingAssetNames(directory) {
  const files = await fs.readdir(directory).catch(error => {
    if (error.code === 'ENOENT') return []
    throw error
  })
  return files
    .filter(file => file.endsWith('.tmLanguage.json'))
    .map(file => file.slice(0, -'.tmLanguage.json'.length))
}

async function mergedLicenseAssets({ generatedAssets, preservedNames, currentLicenses, publicAssetNames }) {
  const currentByLanguage = new Map((currentLicenses.assets ?? []).map(asset => [asset.language, asset]))
  const assets = []
  for (const asset of generatedAssets) {
    assets.push(cleanObject({
      language: asset.language,
      grammarName: asset.grammar.name ?? asset.language,
      scopeName: asset.grammar.scopeName,
      module: asset.module,
      path: asset.path,
      source: asset.source,
      package: packageJson.name,
      version: packageJson.version,
      license: packageJson.license,
      privateDependency: asset.privateDependency || !publicAssetNames.has(asset.language) || undefined,
    }))
  }
  for (const name of preservedNames) {
    const current = currentByLanguage.get(name)
    if (!current) {
      throw new Error(`${languageAssetPath(name)} is preserved but has no licenses.json entry`)
    }
    assets.push(cleanObject({
      ...current,
      path: languageAssetPath(name),
      privateDependency: !publicAssetNames.has(name) || undefined,
    }))
  }
  return assets.sort((left, right) => left.language.localeCompare(right.language))
}

function parseCoverage(text) {
  const kept = quotedStrings(arrayBody(text, 'kept'))
  const remapped = []
  for (const match of text.matchAll(/\[\[remapped\]\]([\s\S]*?)(?=\n\[\[|$)/g)) {
    remapped.push({
      language: tomlStringFromBlock(match[1], 'language'),
      asset: tomlStringFromBlock(match[1], 'asset'),
    })
  }
  return {
    kept: kept.sort((left, right) => left.localeCompare(right)),
    remapped: remapped.sort((left, right) => left.language.localeCompare(right.language)),
  }
}

function arrayBody(text, key) {
  const match = text.match(new RegExp(`(?:^|\\n)${escapeRegExp(key)}\\s*=\\s*\\[([\\s\\S]*?)\\]`))
  if (!match) throw new Error(`coverage.toml missing ${key} array`)
  return match[1]
}

function quotedStrings(text) {
  return [...text.matchAll(/"([^"\\]*(?:\\.[^"\\]*)*)"/g)].map(match => JSON.parse(match[0]))
}

function tomlStringFromBlock(block, key) {
  const match = block.match(new RegExp(`(?:^|\\n)\\s*${escapeRegExp(key)}\\s*=\\s*"([^"]+)"`))
  if (!match) throw new Error(`coverage.toml remap missing ${key}`)
  return match[1]
}

function publicAssetsFromCoverage(coverage) {
  const assets = new Set(coverage.kept)
  for (const remap of coverage.remapped) assets.add(remap.asset)
  return assets
}

function renderActiveCoverage({ kept, remapped, allAssetNames, privateAssetNames }) {
  const publicLanguageCount = kept.length + remapped.length
  return `# Generated by tools/vendor-shiki-grammars.mjs from the pinned Shiki package.\n` +
    `# Active public catalog remains curated; coverage.full-shiki.toml is the\n` +
    `# Phase G skeleton for promoting all Shiki language ids.\n` +
    `public_language_count = ${publicLanguageCount}\n` +
    `asset_grammar_count = ${allAssetNames.length}\n` +
    `private_dependency_count = ${privateAssetNames.length}\n` +
    `kept_count = ${kept.length}\n` +
    `remapped_count = ${remapped.length}\n\n` +
    `# Private dependency grammar blobs are present under languages/ and embedded\n` +
    `# in the bundle, but are not public catalog languages yet.\n` +
    `${tomlArray('private_assets', privateAssetNames)}\n` +
    `# Languages whose public id matches the asset filename stem.\n` +
    `${tomlArray('kept', kept)}\n` +
    `# Public language id differs from the Shiki asset filename stem.\n` +
    remapped.map(remap => `[[remapped]]\nlanguage = ${JSON.stringify(remap.language)}\nasset = ${JSON.stringify(remap.asset)}\n`).join('\n')
}

function renderFullCoverage({ shikiNames, allAssetNames }) {
  const shikiNameSet = new Set(shikiNames)
  const privateAssets = allAssetNames.filter(name => !shikiNameSet.has(name))
  return `# Generated by tools/vendor-shiki-grammars.mjs from @shikijs/langs@${packageJson.version}.\n` +
    `# Skeleton only: build.rs reads coverage.toml, not this file. Copy this\n` +
    `# over coverage.toml only when the Phase G public-catalog gates are met.\n` +
    `public_language_count = ${shikiNames.length}\n` +
    `asset_grammar_count = ${allAssetNames.length}\n` +
    `private_dependency_count = ${privateAssets.length}\n` +
    `kept_count = ${shikiNames.length}\n` +
    `remapped_count = 0\n\n` +
    `# Additional non-Shiki or compatibility assets preserved alongside Shiki.\n` +
    `${tomlArray('private_assets', privateAssets)}\n` +
    `# Full Shiki language id list. Each id matches its asset filename stem.\n` +
    `${tomlArray('kept', shikiNames)}`
}

async function renderLanguageMetadata({ coverage, metadataAssets, languagesDir, packageJson }) {
  const metadataByName = new Map(metadataAssets.map(asset => [asset.language, asset.grammar]))
  const entries = []
  const publicEntries = [
    ...coverage.kept.map(id => ({ id, asset: id })),
    ...coverage.remapped.map(({ language: id, asset }) => ({ id, asset })),
  ].sort((left, right) => left.id.localeCompare(right.id))

  for (const { id, asset } of publicEntries) {
    let grammar = metadataByName.get(asset)
    if (!grammar) {
      const file = path.join(languagesDir, `${asset}.tmLanguage.json`)
      grammar = JSON.parse(await fs.readFile(file, 'utf8'))
    }
    const aliases = sortedUnique((grammar.aliases ?? []).map(normalizeToken).filter(Boolean))
    const extensions = []
    const basenames = []
    for (const rawFileType of grammar.fileTypes ?? []) {
      const fileType = String(rawFileType).trim()
      if (!fileType) continue
      if (fileType.includes('.') && !fileType.startsWith('.')) {
        basenames.push(fileType)
      } else {
        extensions.push(normalizeToken(fileType))
      }
    }
    entries.push(cleanObject({
      id,
      asset: asset === id ? undefined : asset,
      aliases,
      extensions: sortedUnique(extensions.filter(Boolean)),
      basenames: sortedUnique(basenames),
    }))
  }

  return `${JSON.stringify({
    schemaVersion: 1,
    source: {
      package: packageJson.name,
      version: packageJson.version,
      derivation: 'aliases and fileTypes from vendored grammar registrations',
    },
    languages: entries,
  }, null, 2)}\n`
}

function tomlArray(name, values) {
  if (values.length === 0) return `${name} = []\n`
  return `${name} = [\n${values.map(value => `  ${JSON.stringify(value)},`).join('\n')}\n]\n`
}

function repositoryUrl(repository) {
  if (typeof repository === 'string') return repository
  if (repository && typeof repository.url === 'string') return repository.url
  return ''
}

function canonicalJson(value) {
  return JSON.stringify(sortJsonValue(value))
}

function sortJsonValue(value) {
  if (Array.isArray(value)) return value.map(sortJsonValue)
  if (!value || typeof value !== 'object') return value
  const sorted = {}
  for (const key of Object.keys(value).sort((left, right) => left.localeCompare(right))) {
    sorted[key] = sortJsonValue(value[key])
  }
  return sorted
}

function cleanObject(object) {
  return Object.fromEntries(Object.entries(object).filter(([, value]) => value !== undefined))
}

function normalizeToken(value) {
  return String(value).trim().replace(/^\.+/, '').toLowerCase()
}

function sortedUnique(values) {
  return [...new Set(values)].sort((left, right) => left.localeCompare(right))
}

function assertSameSet(left, right, leftName, rightName) {
  if (left.length !== right.length || left.some((value, index) => value !== right[index])) {
    const leftSet = new Set(left)
    const rightSet = new Set(right)
    const onlyLeft = left.filter(value => !rightSet.has(value))
    const onlyRight = right.filter(value => !leftSet.has(value))
    throw new Error(`${leftName} do not match ${rightName}; only ${leftName}: ${onlyLeft.join(', ') || '(none)'}; only ${rightName}: ${onlyRight.join(', ') || '(none)'}`)
  }
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}
