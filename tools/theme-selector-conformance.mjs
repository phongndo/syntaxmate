#!/usr/bin/env node
/** Differential TextMate theme resolver check against pinned vscode-textmate. */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import os from 'node:os'
import { spawnSync } from 'node:child_process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const themeDir = path.join(root, 'assets/themes')
const themeFiles = (await fs.readdir(themeDir))
  .filter(name => name.endsWith('.json') && name !== 'licenses.json')
  .sort()

const require = createRequire(import.meta.url)
const resolvePaths = [path.join(root, 'tools/golden-oracle')]
const resolvePackage = name => require.resolve(name, { paths: resolvePaths })
const importPackage = async name => import(pathToFileURL(resolvePackage(name)).href)
const vsctmModule = await importPackage('vscode-textmate')
const vsctm = vsctmModule.default ?? vsctmModule
const onigModule = await importPackage('vscode-oniguruma')
const onig = onigModule.default ?? onigModule
const onigMain = resolvePackage('vscode-oniguruma')
const wasm = await fs.readFile(path.join(path.dirname(onigMain), 'onig.wasm'))
await onig.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength))

let checked = 0
for (const file of themeFiles) {
  const theme = JSON.parse(await fs.readFile(path.join(themeDir, file), 'utf8'))
  const themeName = file.replace(/\.json$/, '').replace(/-medium$/, '')
  const stacks = conformanceStacks(theme)
  await checkTheme(themeName, theme, stacks, themeName)
  checked += stacks.length
  console.log(`${themeName}: ${stacks.length} selector cases match vscode-textmate`)
}

const randomized = randomizedConformance(0x53594e54, 512, 4000)
const randomThemePath = path.join(os.tmpdir(), `syntaxmate-random-theme-${process.pid}.json`)
await fs.writeFile(randomThemePath, JSON.stringify(randomized.theme))
try {
  await checkTheme('seeded-random', randomized.theme, randomized.stacks, randomThemePath)
} finally {
  await fs.rm(randomThemePath, { force: true })
}
checked += randomized.stacks.length
console.log(`seeded-random: ${randomized.stacks.length} generated selector cases match vscode-textmate`)
console.log(`ok: ${checked} resolved style cases across ${themeFiles.length} themes plus seeded random rules`)

async function checkTheme(themeName, theme, stacks, rustThemeName) {
  const grammarSource = {
    scopeName: `source.syntaxmate-theme-oracle.${themeName}`,
    patterns: stacks.map((scopes, index) => ({
      match: `^T${String(index).padStart(5, '0')}$`,
      name: scopes.join(' '),
    })),
  }
  const defaults = {
    foreground: theme.colors?.['editor.foreground'],
    background: theme.colors?.['editor.background'],
  }
  const registry = new vsctm.Registry({
    theme: {
      settings: [
        { settings: Object.fromEntries(Object.entries(defaults).filter(([, value]) => typeof value === 'string')) },
        ...(theme.tokenColors ?? []),
      ],
    },
    onigLib: Promise.resolve({
      createOnigScanner(patterns) { return new onig.OnigScanner(patterns) },
      createOnigString(source) { return new onig.OnigString(source) },
    }),
    loadGrammar: async scope => scope === grammarSource.scopeName ? grammarSource : null,
  })
  const grammar = await registry.loadGrammar(grammarSource.scopeName)
  const colorMap = registry.getColorMap()
  const editorBackground = parseHexColor(theme.colors?.['editor.background'])
  const expected = stacks.map((_, index) => {
    const source = `T${String(index).padStart(5, '0')}`
    const result = grammar.tokenizeLine2(source, vsctm.INITIAL)
    return decode(result.tokens[1] >>> 0, colorMap, editorBackground)
  })
  const input = stacks
    .map(scopes => JSON.stringify([grammarSource.scopeName, ...scopes]))
    .join('\n') + '\n'
  const rust = spawnSync(
    'cargo',
    ['run', '--quiet', '-p', 'syntaxmate', '--example', 'theme-resolve', '--', rustThemeName],
    { cwd: root, input, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 },
  )
  if (rust.status !== 0) throw new Error(rust.stderr || `Rust resolver exited ${rust.status}`)
  const actual = rust.stdout.trimEnd().split('\n').map(JSON.parse)
  for (let index = 0; index < stacks.length; index++) {
    if (!sameStyle(actual[index], expected[index])) {
      throw new Error(
        `${themeName} mismatch for ${JSON.stringify(stacks[index])}\n` +
        `expected ${JSON.stringify(expected[index])}\n` +
        `actual   ${JSON.stringify(actual[index])}`,
      )
    }
  }
}

function randomizedConformance(seed, ruleCount, stackCount) {
  let state = seed >>> 0
  const random = () => {
    state ^= state << 13; state ^= state >>> 17; state ^= state << 5
    return (state >>> 0) / 0x100000000
  }
  const pick = values => values[Math.floor(random() * values.length)]
  const heads = ['entity', 'support', 'variable', 'constant', 'markup', 'meta', 'keyword', 'string']
  const tails = ['name', 'function', 'character', 'quoted', 'bold', 'control', 'member', 'definition']
  const scope = () => `${pick(heads)}.${pick(tails)}.${Math.floor(random() * 12)}`
  const colors = ['#123456', '#abcdef', '#ff9492', '#91cbff', '#dbb7ff', '#00aa44']
  const styles = ['', 'bold', 'italic', 'underline', 'bold italic', 'strikethrough']
  const tokenColors = []
  for (let index = 0; index < ruleCount; index++) {
    const target = scope()
    const parentCount = Math.floor(random() * 3)
    const parents = Array.from({ length: parentCount }, scope)
    const selector = [...parents, target].join(' ')
    const settings = {}
    if (random() < 0.8) settings.foreground = pick(colors)
    if (random() < 0.25) settings.background = pick(colors)
    if (random() < 0.45) settings.fontStyle = pick(styles)
    if (!Object.keys(settings).length) settings.foreground = pick(colors)
    tokenColors.push({ scope: random() < 0.2 ? [selector, scope()] : selector, settings })
  }
  const theme = {
    name: 'Syntaxmate seeded random conformance',
    colors: { 'editor.foreground': '#eeeeee', 'editor.background': '#101010' },
    tokenColors,
  }
  const selectorScopes = tokenColors.flatMap(rule => Array.isArray(rule.scope) ? rule.scope : [rule.scope])
  const stacks = []
  for (let index = 0; index < stackCount; index++) {
    const parts = pick(selectorScopes).split(/\s+/)
    if (random() < 0.5 && parts.length > 1) parts.splice(1, 0, scope())
    if (random() < 0.6) parts[parts.length - 1] += `.child${Math.floor(random() * 4)}`
    if (random() < 0.2) parts.push(scope())
    stacks.push(parts)
  }
  return { theme, stacks }
}

function conformanceStacks(theme) {
  const unique = new Map()
  const add = scopes => unique.set(JSON.stringify(scopes), scopes)
  add(['meta.unmatched.syntaxmate-oracle'])
  for (const rule of theme.tokenColors ?? []) {
    let selectors = typeof rule.scope === 'string'
      ? rule.scope.replace(/^,+|,+$/g, '').split(',')
      : Array.isArray(rule.scope) ? rule.scope.flatMap(scope => scope.split(',')) : []
    for (const raw of selectors) {
      const parts = raw.trim().split(/\s+/).filter(Boolean)
      if (!parts.length) continue
      const target = parts.at(-1)
      const parents = parts.slice(0, -1).filter(part => part !== '>')
      add([...parents, target])
      add([...parents, `${target}.syntaxmate-oracle-child`])
      if (parents.length) add([parents[0], 'meta.intermediate.syntaxmate-oracle', ...parents.slice(1), target])
      add([`not-${target}`])
    }
  }
  return [...unique.values()]
}

function decode(metadata, colorMap, editorBackground) {
  const fontStyle = (metadata & 0x00007800) >>> 11
  const foregroundId = (metadata & 0x00ff8000) >>> 15
  const backgroundId = (metadata & 0xff000000) >>> 24
  const modifiers = []
  if (fontStyle & 1) modifiers.push('italic')
  if (fontStyle & 2) modifiers.push('bold')
  if (fontStyle & 4) modifiers.push('underline')
  if (fontStyle & 8) modifiers.push('strikethrough')
  return {
    foreground: normalizeColor(colorMap[foregroundId], editorBackground),
    background: normalizeColor(colorMap[backgroundId], editorBackground),
    modifiers,
  }
}

function normalizeColor(value, editorBackground) {
  if (!value) return null
  const color = parseHexColor(value)
  if (!color) return value.toLowerCase()
  if (color.alpha === 0xff) return formatRgb(color)
  if (!editorBackground || editorBackground.alpha !== 0xff) {
    throw new Error(`translucent color ${JSON.stringify(value)} requires an opaque editor.background`)
  }
  const composite = (foreground, background) => Math.floor(
    (foreground * color.alpha + background * (0xff - color.alpha) + 127) / 0xff,
  )
  return formatRgb({
    red: composite(color.red, editorBackground.red),
    green: composite(color.green, editorBackground.green),
    blue: composite(color.blue, editorBackground.blue),
  })
}

function parseHexColor(value) {
  if (typeof value !== 'string') return null
  const hex = value.toLowerCase().match(/^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/)?.[1]
  if (!hex) return null
  const channel = index => parseInt(
    hex.length <= 4 ? hex[index] + hex[index] : hex.slice(index * 2, index * 2 + 2),
    16,
  )
  return {
    red: channel(0),
    green: channel(1),
    blue: channel(2),
    alpha: hex.length === 4 || hex.length === 8 ? channel(3) : 0xff,
  }
}

function formatRgb(color) {
  const channel = value => value.toString(16).padStart(2, '0')
  return `#${channel(color.red)}${channel(color.green)}${channel(color.blue)}`
}

function sameStyle(left, right) {
  return left?.foreground === right?.foreground &&
    left?.background === right?.background &&
    JSON.stringify(left?.modifiers) === JSON.stringify(right?.modifiers)
}
