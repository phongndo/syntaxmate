import { createHash } from 'node:crypto'
import fs from 'node:fs/promises'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const require = createRequire(import.meta.url)
const toolDir = path.dirname(fileURLToPath(import.meta.url))
const resolvePaths = [
  path.join(toolDir, 'golden-oracle'),
  path.resolve(process.cwd(), 'tools/golden-oracle'),
]
const jsonCache = new Map()
let runtimePromise

function resolvePackage(name) {
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

async function loadRuntime() {
  if (!runtimePromise) {
    runtimePromise = (async () => {
      const vsctmModule = await import(pathToFileURL(resolvePackage('vscode-textmate')).href)
      const vsctm = vsctmModule.default ?? vsctmModule
      const onigModule = await import(pathToFileURL(resolvePackage('vscode-oniguruma')).href)
      const onig = onigModule.default ?? onigModule
      const onigMain = resolvePackage('vscode-oniguruma')
      let wasmPath = path.join(path.dirname(onigMain), 'release', 'onig.wasm')
      try {
        await fs.access(wasmPath)
      } catch {
        wasmPath = path.join(path.dirname(onigMain), 'onig.wasm')
      }
      const wasm = await fs.readFile(wasmPath)
      await onig.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength))
      return { vsctm, onig }
    })()
  }
  return runtimePromise
}

async function readJson(filePath) {
  const absolutePath = path.resolve(filePath)
  let value = jsonCache.get(absolutePath)
  if (!value) {
    value = fs.readFile(absolutePath, 'utf8').then(JSON.parse)
    jsonCache.set(absolutePath, value)
  }
  return value
}

export async function generateTextMateGolden(options) {
  const {
    grammarPath,
    assetsDir,
    scopeName,
    language = scopeName,
    sourcePath,
    sourceLabel = sourcePath,
    embedded = [],
    themePath,
    timeLimit = 0,
  } = options
  if ((!grammarPath && !assetsDir) || !scopeName || !sourcePath) {
    throw new Error('grammarPath or assetsDir, scopeName, and sourcePath are required')
  }

  const { vsctm, onig } = await loadRuntime()
  const grammarSpecs = [...embedded]
  if (grammarPath) grammarSpecs.unshift({ scope: scopeName, grammarPath })
  if (assetsDir) {
    const names = (await fs.readdir(assetsDir)).filter(name => name.endsWith('.json')).sort()
    for (const name of names) {
      const assetGrammarPath = path.join(assetsDir, name)
      const parsed = await readJson(assetGrammarPath)
      if (typeof parsed.scopeName === 'string') {
        grammarSpecs.push({ scope: parsed.scopeName, grammarPath: assetGrammarPath, parsed })
      }
    }
  }

  const grammars = new Map()
  for (const spec of grammarSpecs) {
    if (grammars.has(spec.scope)) throw new Error(`duplicate grammar scope ${spec.scope}`)
    grammars.set(spec.scope, spec.parsed ?? await readJson(spec.grammarPath))
  }

  let rawTheme
  if (themePath) {
    const theme = await readJson(themePath)
    const foreground = theme.colors?.['editor.foreground']
    const background = theme.colors?.['editor.background']
    rawTheme = {
      settings: [
        {
          settings: {
            ...(typeof foreground === 'string' ? { foreground } : {}),
            ...(typeof background === 'string' ? { background } : {}),
          },
        },
        ...(Array.isArray(theme.tokenColors) ? theme.tokenColors : []),
      ],
    }
  }

  const registry = new vsctm.Registry({
    ...(rawTheme ? { theme: rawTheme } : {}),
    onigLib: Promise.resolve({
      createOnigScanner(patterns) { return new onig.OnigScanner(patterns) },
      createOnigString(source) { return new onig.OnigString(source) },
    }),
    loadGrammar: async scope => grammars.get(scope) ?? null,
  })
  try {
    const grammar = await registry.loadGrammar(scopeName)
    if (!grammar) throw new Error(`failed to load grammar ${scopeName}`)

    const source = await fs.readFile(sourcePath, 'utf8')
    const lines = source.split('\n')
    let ruleStack = vsctm.INITIAL
    const records = []
    const colorMap = registry.getColorMap()
    for (let lineNumber = 0; lineNumber < lines.length; lineNumber++) {
      const line = lines[lineNumber]
      const entryRuleStack = ruleStack
      const result = grammar.tokenizeLine(line, entryRuleStack, timeLimit)
      const styled = rawTheme
        ? grammar.tokenizeLine2(line, entryRuleStack, timeLimit)
        : null
      ruleStack = result.ruleStack
      const ruleStackText = String(ruleStack)
      records.push(JSON.stringify({
        language,
        scopeName,
        file: sourceLabel,
        lineNumber,
        line,
        tokens: result.tokens.map(token => ({
          startIndex: token.startIndex,
          endIndex: token.endIndex,
          scopes: token.scopes,
          ...(styled ? { style: decodedStyleAt(styled.tokens, token.startIndex, colorMap) } : {}),
        })),
        ruleStack: ruleStackText,
        ruleStackHash: createHash('sha256').update(ruleStackText).digest('hex'),
        stoppedEarly: Boolean(result.stoppedEarly),
      }))
    }

    return `${records.join('\n')}\n`
  } finally {
    registry.dispose()
  }
}

function decodedStyleAt(tokens, offset, colorMap) {
  let metadata = 0
  for (let index = 0; index < tokens.length; index += 2) {
    if (tokens[index] > offset) break
    metadata = tokens[index + 1] >>> 0
  }
  const fontStyle = (metadata & 0x00007800) >>> 11
  const foregroundId = (metadata & 0x00ff8000) >>> 15
  const backgroundId = (metadata & 0xff000000) >>> 24
  const modifiers = []
  if (fontStyle & 1) modifiers.push('italic')
  if (fontStyle & 2) modifiers.push('bold')
  if (fontStyle & 4) modifiers.push('underline')
  if (fontStyle & 8) modifiers.push('strikethrough')
  return {
    foreground: colorMap[foregroundId]?.toLowerCase() ?? null,
    background: colorMap[backgroundId]?.toLowerCase() ?? null,
    modifiers,
  }
}
