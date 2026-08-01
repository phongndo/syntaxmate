#!/usr/bin/env node
/**
 * Dev-only TextMate oracle dumper.
 *
 * Runs pinned vscode-textmate + vscode-oniguruma over a source file and writes
 * one JSON object per line (JSONL) with UTF-16 token offsets, full scope stacks,
 * final rule-stack debug text, ruleStackHash, and stoppedEarly.
 */
import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'
import { pathToFileURL } from 'node:url'
import { generateTextMateGolden } from './textmate-oracle.mjs'

function usage() {
  console.log(`usage: golden-dump.mjs --scope <scopeName> --file <source> [options]

Options:
  --assets <directory>         Register every *.json grammar in a directory.
  --grammar <tmLanguage.json>  Register the root grammar directly.
  --language <id>              Language id to write in each record (defaults to --scope).
  --out <jsonl>                Write JSONL to this file instead of stdout.
  --embedded <scope=grammar>   Register an embedded grammar; may be repeated.
  --theme <theme.json>         Also emit resolved TextMate foreground/background/fontStyle.
  --time-limit <ms>            vscode-textmate tokenizeLine time limit (default: 0 = none).
  -h, --help                   Show this help.

Oracle packages resolve only from tools/golden-oracle (dev-only, pinned).`)
}

function parseArgs(argv) {
  const args = { embedded: [], timeLimit: 0 }
  for (let index = 2; index < argv.length; index++) {
    const item = argv[index]
    if (item === '-h' || item === '--help') {
      args.help = true
      continue
    }
    const equals = item.indexOf('=')
    const name = equals > 0 ? item.slice(0, equals) : item
    let value = equals > 0 ? item.slice(equals + 1) : undefined
    if (['--assets', '--grammar', '--scope', '--language', '--file', '--out', '--embedded', '--theme', '--time-limit'].includes(name)) {
      if (value === undefined) value = argv[++index]
      if (value === undefined || value.startsWith('--')) throw new Error(`${name} requires a value`)
      if (name === '--embedded') args.embedded.push(value)
      else if (name === '--time-limit') {
        const parsed = Number(value)
        if (!Number.isFinite(parsed) || parsed < 0) {
          throw new Error(`--time-limit must be a non-negative number, got ${JSON.stringify(value)}`)
        }
        args.timeLimit = parsed
      } else args[name.slice(2)] = value
      continue
    }
    throw new Error(`unknown argument: ${item}`)
  }
  return args
}

function parseEmbedded(spec) {
  const equals = spec.indexOf('=')
  if (equals <= 0 || equals === spec.length - 1) {
    throw new Error(`invalid --embedded value ${JSON.stringify(spec)}; expected <scope=grammar>`)
  }
  return { scope: spec.slice(0, equals), grammarPath: spec.slice(equals + 1) }
}

async function main() {
  let args
  try {
    args = parseArgs(process.argv)
    if (args.help) return usage()
    if ((!args.grammar && !args.assets) || !args.scope || !args.file) {
      usage()
      process.exitCode = 2
      return
    }
    const output = await generateTextMateGolden({
      grammarPath: args.grammar,
      assetsDir: args.assets,
      scopeName: args.scope,
      language: args.language ?? args.scope,
      sourcePath: args.file,
      sourceLabel: args.file,
      embedded: args.embedded.map(parseEmbedded),
      themePath: args.theme,
      timeLimit: args.timeLimit,
    })
    if (args.out) await fs.writeFile(args.out, output)
    else process.stdout.write(output)
  } catch (error) {
    console.error(error.message)
    usage()
    process.exitCode = 2
  }
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  await main()
}
