#!/usr/bin/env node
// Oracle-backed edits of every B/C language's basic fixture. Keep sources and
// exact UTF-8 spans together; scope-table indexes only deduplicate repeated stacks.
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import { generateTextMateGolden } from './textmate-oracle.mjs'

const args = process.argv.slice(2)
if (args.some(arg => arg !== '--check')) throw new Error('usage: generate-language-edge-goldens.mjs [--check]')
const tiers = JSON.parse(await fs.readFile('benchmarks/textmate/promotion-tiers.json', 'utf8')).tiers
const languages = [...tiers.B, ...tiers.C].sort()
const destination = 'tests/fixtures/textmate/edge-inputs.golden.jsonl'
const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'syntaxmate-language-edges-'))
const output = []
try {
  for (const language of languages) {
    const directory = `tests/fixtures/textmate/${language}`
    const names = (await fs.readdir(directory)).filter(name => name.startsWith('basic.') && !name.endsWith('.golden.jsonl'))
    if (names.length !== 1) throw new Error(`${language}: expected one basic source fixture`)
    const basic = (await fs.readFile(path.join(directory, names[0]), 'utf8')).replaceAll('\r\n', '\n').replace(/\n+$/, '')
    const lines = basic.split('\n')
    const variants = {
      'line-edges': `\n${lines.join('\r\n')}\r\n\n`,
      unicode: lines.map(line => line ? `${line}\t café e\u0301 東京 🚀` : line).join('\n') + '\n',
      'truncated-recovery': lines.map(line => Array.from(line).slice(0, Math.ceil(Array.from(line).length / 2)).join('')).join('\n') + `\n\n${basic}\n`,
    }
    const grammarPath = `assets/grammars/languages/${language}.tmLanguage.json`
    const grammar = JSON.parse(await fs.readFile(grammarPath, 'utf8'))
    for (const [variant, source] of Object.entries(variants)) {
      const sourcePath = path.join(temporary, 'source')
      await fs.writeFile(sourcePath, source)
      const golden = await generateTextMateGolden({
        assetsDir: 'assets/grammars/languages', scopeName: grammar.scopeName,
        language, sourcePath, sourceLabel: `${language}/${variant}`,
      })
      const scopes = []
      const scopeIds = new Map()
      const tokenLines = golden.trimEnd().split('\n').map(record => {
        const { line, tokens, stoppedEarly } = JSON.parse(record)
        if (stoppedEarly) throw new Error(`${language}/${variant}: oracle stopped early`)
        const spans = []
        for (const token of tokens) {
          const start = Buffer.byteLength(line.slice(0, token.startIndex))
          const end = Buffer.byteLength(line.slice(0, token.endIndex))
          if (start >= end) continue
          const key = JSON.stringify(token.scopes)
          if (!scopeIds.has(key)) {
            scopeIds.set(key, scopes.length)
            scopes.push(token.scopes)
          }
          const id = scopeIds.get(key)
          const previous = spans.at(-1)
          if (previous && previous[1] === start && previous[2] === id) previous[1] = end
          else spans.push([start, end, id])
        }
        return spans
      })
      output.push(JSON.stringify({ language, variant, source, scopes, lines: tokenLines }))
    }
  }
  const generated = output.join('\n') + '\n'
  if (args.includes('--check')) {
    if (await fs.readFile(destination, 'utf8') !== generated) throw new Error(`stale ${destination}`)
  } else await fs.writeFile(destination, generated)
} finally {
  await fs.rm(temporary, { recursive: true, force: true })
}
console.log(`language edges: ${output.length} oracle documents across ${languages.length} languages${args.includes('--check') ? ' checked' : ' generated'}`)
