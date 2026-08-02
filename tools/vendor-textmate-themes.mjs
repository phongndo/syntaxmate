#!/usr/bin/env node
/** Deterministically regenerate/check named TextMate themes from pinned packages. */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const outputRoot = path.join(root, 'assets/themes')
const check = process.argv.includes('--check')
const require = createRequire(import.meta.url)
const paths = [path.join(root, 'tools/golden-oracle')]
const resolve = name => require.resolve(name, { paths })
const githubRoot = path.dirname(resolve('github-vscode-themes'))

const github = {
  'github-dark': 'dark-default.json',
  'github-dark-high-contrast': 'dark-high-contrast.json',
  'github-light': 'light-default.json',
  'github-light-high-contrast': 'light-high-contrast.json',
}
for (const [id, source] of Object.entries(github)) {
  const contents = await fs.readFile(path.join(githubRoot, 'dist', source), 'utf8')
  await writeOrCheck(id, contents)
}
console.log(`TextMate themes: ${check ? 'up to date' : 'regenerated'}`)

async function writeOrCheck(id, contents) {
  const destination = path.join(outputRoot, `${id}.json`)
  if (check) {
    const current = await fs.readFile(destination, 'utf8')
    if (current !== contents) throw new Error(`${path.relative(root, destination)} is stale`)
  } else {
    await fs.writeFile(destination, contents)
  }
}
