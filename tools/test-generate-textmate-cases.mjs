#!/usr/bin/env node
import assert from 'node:assert/strict'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'

import { generateManifest, runGenerator } from './generate-textmate-cases.mjs'

const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mark-textmate-cases-'))
try {
  const fixtures = path.join(root, 'fixtures')
  const grammars = path.join(root, 'grammars')
  await fs.mkdir(path.join(fixtures, 'bash'), { recursive: true })
  await fs.mkdir(path.join(fixtures, 'host'), { recursive: true })
  await fs.mkdir(grammars, { recursive: true })

  await Promise.all([
    write(path.join(fixtures, 'bash/basic.sh'), lines(10, 'echo basic')),
    write(path.join(fixtures, 'bash/basic.golden.jsonl'), golden()),
    write(path.join(fixtures, 'bash/stress.sh'), lines(140, 'echo stress')),
    write(path.join(fixtures, 'bash/stress.golden.jsonl'), golden()),
    write(path.join(fixtures, 'bash/basic.sample.sh'), 'ignored\n'),
    write(path.join(fixtures, 'bash/sample.sh'), 'ignored\n'),
    write(path.join(fixtures, 'host/basic.test'), lines(10, 'host basic')),
    write(path.join(fixtures, 'host/basic.golden.jsonl'), golden()),
    write(path.join(fixtures, 'host/stress.test'), lines(140, 'host stress')),
    write(path.join(fixtures, 'host/stress.golden.jsonl'), golden()),
    write(path.join(fixtures, 'host/odd.fixture'), 'odd\n'),
    write(path.join(fixtures, 'host/odd.golden.jsonl'), golden()),
    grammar(grammars, 'shellscript', {
      scopeName: 'source.shell',
      patterns: [],
    }),
    grammar(grammars, 'host', {
      scopeName: 'source.host',
      patterns: [
        { include: '#local' },
        { include: '$self' },
        { include: 'source.missing#optional' },
        { include: 'source.dep#entry' },
        { include: 'source.yaml' },
      ],
    }),
    grammar(grammars, 'dep', {
      scopeName: 'source.dep',
      repository: {
        entry: { patterns: [{ include: 'source.leaf' }, { include: 'source.host' }] },
      },
    }),
    grammar(grammars, 'leaf', {
      scopeName: 'source.leaf',
      patterns: [],
    }),
    grammar(grammars, 'yaml', {
      scopeName: 'source.yaml',
      patterns: [{ include: 'source.yaml.1.2' }],
    }),
    grammar(grammars, 'yaml-1.2', {
      scopeName: 'source.yaml.1.2',
      patterns: [],
    }),
  ])

  const configPath = path.join(root, 'config.json')
  const outputPath = path.join(root, 'cases.toml')
  const coveragePath = path.join(root, 'coverage.toml')
  const divergencesPath = path.join(root, 'divergences.toml')
  const policyPath = path.join(root, 'policy.json')
  await write(configPath, JSON.stringify({
    languageAssets: { bash: 'shellscript' },
    cases: [{ language: 'host', fixture: 'fixtures/host/odd.fixture' }],
  }))
  await write(coveragePath, `public_language_count = 2
kept = [
  "host",
  "shellscript",
]
`)
  await write(divergencesPath, '# exact contract: no divergences\n')
  await write(policyPath, JSON.stringify({
    schemaVersion: 1,
    expectedCounts: {
      publicLanguages: 2,
      validatedLanguages: 2,
      oracleLanguages: 2,
      stressCorpusLanguages: 2,
    },
  }))
  const options = {
    root,
    fixtures: 'fixtures',
    grammars: 'grammars',
    config: 'config.json',
    output: 'cases.toml',
    coverage: 'coverage.toml',
    divergences: 'divergences.toml',
    policy: 'policy.json',
    lockedCount: 2,
  }

  const first = await generateManifest(options)
  const second = await generateManifest(options)
  assert.equal(first, second, 'generation must be deterministic')
  assert.equal((first.match(/^\[\[case\]\]$/gm) ?? []).length, 5)
  assert.match(first, /language = "bash"\nscope = "source\.shell"\ngrammar = "grammars\/shellscript\.tmLanguage\.json"/)
  assert.match(first, /fixture = "fixtures\/host\/odd\.fixture"\ngolden = "fixtures\/host\/odd\.golden\.jsonl"/)
  assert.doesNotMatch(first, /sample/)
  assert.match(first, /scope = "source\.dep"\ngrammar = "grammars\/dep\.tmLanguage\.json"/)
  assert.match(first, /scope = "source\.leaf"\ngrammar = "grammars\/leaf\.tmLanguage\.json"/)
  assert.match(first, /scope = "source\.yaml"\ngrammar = "grammars\/yaml\.tmLanguage\.json"/)
  assert.doesNotMatch(first, /scope = "source\.yaml\.1\.2"/)
  assert.doesNotMatch(first, /source\.missing/)
  assert.ok(first.indexOf('scope = "source.dep"') < first.indexOf('scope = "source.leaf"'))

  await runGenerator(options)
  await runGenerator({ ...options, check: true })
  await fs.appendFile(outputPath, '# stale\n')
  await assert.rejects(
    runGenerator({ ...options, check: true }),
    /cases\.toml is out of date/,
  )

  // A regenerated manifest must not normalize a contract regression into a
  // green --check result: the independent policy still locks both roles.
  await fs.rm(path.join(fixtures, 'host/stress.test'))
  await fs.rm(path.join(fixtures, 'host/stress.golden.jsonl'))
  await runGenerator(options)
  await assert.rejects(
    runGenerator({ ...options, check: true }),
    /validated membership differs from the public catalog: missing=host/,
  )
  console.log('generate-textmate-cases tests: ok')
} finally {
  await fs.rm(root, { recursive: true, force: true })
}

async function write(filePath, contents) {
  await fs.mkdir(path.dirname(filePath), { recursive: true })
  await fs.writeFile(filePath, contents)
}

async function grammar(directory, asset, value) {
  await write(
    path.join(directory, `${asset}.tmLanguage.json`),
    `${JSON.stringify(value, null, 2)}\n`,
  )
}

function lines(count, text) {
  return `${Array.from({ length: count }, (_, index) => `${text} ${index}`).join('\n')}\n`
}

function golden() {
  return `${JSON.stringify({ stoppedEarly: false })}\n`
}
