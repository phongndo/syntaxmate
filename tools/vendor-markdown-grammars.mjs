#!/usr/bin/env node
/** Regenerate private Markdown dependency grammars from pinned Shiki assets. */

import fs from 'node:fs/promises'
import path from 'node:path'
import { execFileSync } from 'node:child_process'
import { pathToFileURL } from 'node:url'

const root = path.resolve(import.meta.dirname, '..')
const source = path.resolve(root, '../tau/node_modules/.pnpm/@shikijs+langs@3.23.0/node_modules/@shikijs/langs/dist')
const output = path.join(root, 'assets/grammars/languages')

const assets = [
  ['abap', 'source.abap'], ['vb', 'source.asp.vb.net'],
  ['bat', 'source.batchfile'], ['bibtex', 'text.bibtex'],
  ['clojure', 'source.clojure'], ['coffee', 'source.coffee'],
  ['less', 'source.css.less'], ['dart', 'source.dart'],
  ['diff', 'source.diff'], ['elixir', 'source.elixir'],
  ['erlang', 'source.erlang'], ['fsharp', 'source.fsharp'],
  ['git-commit', 'text.git-commit'], ['git-rebase', 'text.git-rebase'],
  ['groovy', 'source.groovy'], ['html-derivative', 'text.html.derivative'],
  ['handlebars', 'text.html.handlebars'], ['ini', 'source.ini'],
  ['jsonc', 'source.json.comments'], ['jsonl', 'source.json.lines'],
  ['julia', 'source.julia'], ['latex', 'text.tex.latex'],
  ['log', 'text.log'], ['objective-c', 'source.objc'],
  ['perl', 'source.perl'], ['raku', 'source.perl.6'],
  ['pug', 'text.pug'], ['r', 'source.r'],
  ['regexp', 'source.regexp.python'], ['rst', 'source.rst'],
  ['scala', 'source.scala'], ['xml', 'text.xml'], ['xsl', 'text.xml.xsl'],
]

await fs.access(source).catch(() => {
  throw new Error(`pinned @shikijs/langs source not found at ${source}`)
})

const imported = []
for (const [name, scopeName] of assets) {
  const moduleName = `${name}.mjs`
  const exported = (await import(pathToFileURL(path.join(source, moduleName)).href)).default
  const grammars = Array.isArray(exported) ? exported : [exported]
  const grammar = grammars.find(candidate => candidate.scopeName === scopeName)
  if (!grammar) throw new Error(`${moduleName} does not export ${scopeName}`)
  const relativePath = `assets/grammars/languages/${name}.tmLanguage.json`
  await fs.writeFile(path.join(root, relativePath), `${JSON.stringify(grammar, null, 2)}\n`)
  imported.push({
    language: name,
    grammarName: grammar.name ?? name,
    scopeName,
    module: moduleName,
    path: relativePath,
    source: '@shikijs/langs dist',
    package: '@shikijs/langs',
    version: '3.23.0',
    license: 'MIT',
    privateDependency: true,
  })
}

const vscodeRoot = '/Applications/Visual Studio Code.app/Contents/Resources/app/extensions'
const vscodeRepository = 'https://github.com/microsoft/vscode'
const vscodeAssets = [
  ['ignore', 'source.ignore', 'git-base/syntaxes/ignore.tmLanguage.json'],
  ['js-regexp', 'source.js.regexp', 'javascript/syntaxes/Regular Expressions (JavaScript).tmLanguage'],
]
for (const [name, scopeName, relativeSource] of vscodeAssets) {
  const sourcePath = path.join(vscodeRoot, relativeSource)
  let grammar
  if (sourcePath.endsWith('.json')) {
    grammar = JSON.parse(await fs.readFile(sourcePath, 'utf8'))
  } else {
    grammar = JSON.parse(execFileSync('plutil', ['-convert', 'json', '-o', '-', sourcePath], { encoding: 'utf8' }))
  }
  if (grammar.scopeName !== scopeName) {
    throw new Error(`${relativeSource} has scope ${grammar.scopeName}, expected ${scopeName}`)
  }
  const relativePath = `assets/grammars/languages/${name}.tmLanguage.json`
  await fs.writeFile(path.join(root, relativePath), `${JSON.stringify(grammar, null, 2)}\n`)
  imported.push({
    language: name,
    grammarName: grammar.name ?? name,
    scopeName,
    module: relativeSource,
    path: relativePath,
    source: 'Visual Studio Code built-in extension',
    package: 'microsoft/vscode',
    repository: vscodeRepository,
    version: '1.128.0',
    license: 'MIT',
    privateDependency: true,
  })
}

// Markdown's grammar asks for source.twig, while Shiki exposes the same Twig
// grammar under text.html.twig. Register a private root-scope alias without
// changing any matching rules.
{
  const exported = (await import(pathToFileURL(path.join(source, 'twig.mjs')).href)).default
  const grammar = structuredClone(exported.find(candidate => candidate.scopeName === 'text.html.twig'))
  grammar.scopeName = 'source.twig'
  const relativePath = 'assets/grammars/languages/twig-source.tmLanguage.json'
  await fs.writeFile(path.join(root, relativePath), `${JSON.stringify(grammar, null, 2)}\n`)
  imported.push({
    language: 'twig-source', grammarName: grammar.name ?? 'twig', scopeName: 'source.twig',
    module: 'twig.mjs', path: relativePath, source: '@shikijs/langs dist (root-scope alias)',
    package: '@shikijs/langs', version: '3.23.0', license: 'MIT', privateDependency: true,
  })
}

// The pinned Shiki package has no YANG grammar. Vendor the MIT grammar used by
// the marko2276.yang VS Code extension and convert its plist deterministically.
{
  const url = 'https://raw.githubusercontent.com/marko2276/yang-vscode-syntax/master/syntaxes/yang.tmLanguage'
  const response = await fetch(url)
  if (!response.ok) throw new Error(`failed to download YANG grammar: ${response.status}`)
  const temporary = path.join(root, 'target', 'yang.tmLanguage')
  await fs.mkdir(path.dirname(temporary), { recursive: true })
  await fs.writeFile(temporary, await response.text())
  const grammar = JSON.parse(execFileSync('plutil', ['-convert', 'json', '-o', '-', temporary], { encoding: 'utf8' }))
  if (grammar.scopeName !== 'source.yang') throw new Error('YANG grammar scope changed')
  const relativePath = 'assets/grammars/languages/yang.tmLanguage.json'
  await fs.writeFile(path.join(root, relativePath), `${JSON.stringify(grammar, null, 2)}\n`)
  imported.push({
    language: 'yang', grammarName: grammar.name ?? 'yang', scopeName: 'source.yang',
    module: url, path: relativePath, source: 'marko2276/yang-vscode-syntax',
    package: 'marko2276.yang', repository: 'https://github.com/marko2276/yang-vscode-syntax',
    version: '0.1.3', license: 'MIT', privateDependency: true,
  })
}

const licensesPath = path.join(root, 'assets/grammars/licenses.json')
const licenses = JSON.parse(await fs.readFile(licensesPath, 'utf8'))
const importedNames = new Set(imported.map(asset => asset.language))
licenses.assets = licenses.assets
  .filter(asset => !importedNames.has(asset.language))
  .concat(imported)
  .sort((left, right) => left.language.localeCompare(right.language))
await fs.writeFile(licensesPath, `${JSON.stringify(licenses, null, 2)}\n`)
console.log(`vendored ${imported.length} private Markdown dependency grammars`)
