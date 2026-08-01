#!/usr/bin/env node
/** Deterministically regenerate/check named TextMate themes from pinned packages. */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { createRequire } from 'node:module'
import { fileURLToPath, pathToFileURL } from 'node:url'

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
const shiki = {
  'catppuccin-latte': 'catppuccin-latte',
  'catppuccin-frappe': 'catppuccin-frappe',
  'catppuccin-macchiato': 'catppuccin-macchiato',
  'catppuccin-mocha': 'catppuccin-mocha',
  'gruvbox-dark': 'gruvbox-dark-medium',
  'gruvbox-light': 'gruvbox-light-medium',
  tokyonight: 'tokyo-night',
  nord: 'nord',
  'ayu-dark': 'ayu-dark',
  'ayu-light': 'ayu-light',
  'ayu-mirage': 'ayu-mirage',
  'everforest-dark': 'everforest-dark',
  'everforest-light': 'everforest-light',
  'kanagawa-dragon': 'kanagawa-dragon',
  'kanagawa-lotus': 'kanagawa-lotus',
  'kanagawa-wave': 'kanagawa-wave',
}

for (const [id, source] of Object.entries(github)) {
  const contents = await fs.readFile(path.join(githubRoot, 'dist', source), 'utf8')
  await writeOrCheck(id, contents)
}
for (const [id, source] of Object.entries(shiki)) {
  const module = await import(pathToFileURL(resolve(`@shikijs/themes/${source}`)).href)
  await writeOrCheck(id, `${JSON.stringify(module.default, null, 2)}\n`)
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
