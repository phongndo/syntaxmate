#!/usr/bin/env node
// Builds the npm package in place: dist/ (wasm-bindgen output), grammars.bundle,
// LICENSE, and third-party/ asset notices. Needs cargo with the
// wasm32-unknown-unknown target, wasm-bindgen-cli matching Cargo.lock, and
// optionally binaryen's wasm-opt.
//   node scripts/build.mjs [--no-opt]

import { execFileSync, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const pkg = path.dirname(path.dirname(fileURLToPath(import.meta.url)))
const bindings = path.dirname(pkg)
const root = path.dirname(bindings)
const dist = path.join(pkg, 'dist')
const optimize = !process.argv.includes('--no-opt')

function run(command, args, cwd = pkg) {
  console.error(`$ ${command} ${args.join(' ')}`)
  execFileSync(command, args, { cwd, stdio: 'inherit' })
}

function has(command) {
  return spawnSync(command, ['--version'], { stdio: 'ignore' }).status === 0
}

const lock = fs.readFileSync(path.join(bindings, 'Cargo.lock'), 'utf8')
const pinned = /name = "wasm-bindgen"\nversion = "([^"]+)"/.exec(lock)?.[1]
const cli = execFileSync('wasm-bindgen', ['--version'], { encoding: 'utf8' }).trim().split(' ')[1]
if (pinned !== cli) {
  throw new Error(`wasm-bindgen-cli ${cli} does not match the wasm-bindgen crate ${pinned}`)
}

run('cargo', [
  'build', '--locked', '-p', 'syntaxmate-wasm',
  '--target', 'wasm32-unknown-unknown', '--profile', 'wasm-release',
], bindings)

fs.rmSync(dist, { recursive: true, force: true })
run('wasm-bindgen', [
  '--target', 'web', '--no-typescript', '--out-dir', dist, '--out-name', 'syntaxmate',
  path.join(bindings, 'target/wasm32-unknown-unknown/wasm-release/syntaxmate_wasm.wasm'),
])

const wasm = path.join(dist, 'syntaxmate_bg.wasm')
if (optimize && has('wasm-opt')) {
  run('wasm-opt', ['-O3', '--strip-debug', '--strip-producers', wasm, '-o', wasm])
} else if (optimize) {
  console.error('warning: wasm-opt not found; skipping post-link optimization')
}

fs.copyFileSync(path.join(root, 'assets/grammars.bundle'), path.join(pkg, 'grammars.bundle'))
fs.copyFileSync(path.join(root, 'LICENSE'), path.join(pkg, 'LICENSE'))
// Grammar and theme license records travel with the bundle that embeds them.
const notices = path.join(pkg, 'third-party')
fs.rmSync(notices, { recursive: true, force: true })
for (const kind of ['grammars', 'themes']) {
  for (const entry of ['SOURCE.toml', 'licenses.json', 'licenses']) {
    fs.cpSync(path.join(root, 'assets', kind, entry), path.join(notices, kind, entry), { recursive: true })
  }
}
console.error(`built ${path.relative(root, wasm)} (${fs.statSync(wasm).size} bytes)`)
