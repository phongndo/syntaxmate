#!/usr/bin/env node
/** Regenerate the committed LaTeX scope and resolved-style oracle goldens. */
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import process from 'node:process'
import { spawnSync } from 'node:child_process'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const check = process.argv.includes('--check')
const fixture = 'tests/fixtures/textmate/latex/hw2-theme.tex'
const outputs = [
  ['tests/fixtures/textmate/latex/hw2-theme.golden.jsonl', null],
  ['tests/fixtures/textmate/latex/hw2-theme.theme.golden.jsonl', 'assets/themes/github-dark-high-contrast.json'],
]
const temporary = await fs.mkdtemp(path.join(os.tmpdir(), 'mark-theme-golden-'))
try {
  for (const [output, theme] of outputs) {
    const generated = path.join(temporary, path.basename(output))
    const args = [
      'tools/golden-dump.mjs',
      '--assets', 'assets/grammars/languages',
      '--scope', 'text.tex.latex',
      '--language', 'latex',
      '--file', fixture,
      '--out', generated,
      ...(theme ? ['--theme', theme] : []),
    ]
    const result = spawnSync(process.execPath, args, { cwd: root, encoding: 'utf8' })
    if (result.status !== 0) throw new Error(result.stderr || `golden dump exited ${result.status}`)
    const generatedText = await fs.readFile(generated, 'utf8')
    const destination = path.join(root, output)
    if (check) {
      const committed = await fs.readFile(destination, 'utf8')
      if (committed !== generatedText) throw new Error(`${output} is stale; run node tools/generate-theme-goldens.mjs`)
    } else {
      await fs.writeFile(destination, generatedText)
      console.log(`wrote ${output}`)
    }
  }
  if (check) console.log('theme goldens: up to date')
} finally {
  await fs.rm(temporary, { recursive: true, force: true })
}
