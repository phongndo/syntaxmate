#!/usr/bin/env node
import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs/promises'
import os from 'node:os'
import path from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { diffConformance, extractConstructs, hasConformanceGaps } from './grammar-stats.mjs'

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

test('nested and intersected character classes match their case metadata', () => {
  const ids = extractConstructs(String.raw`[a-z&&[^aeiou]]+`).map(item => item.id)
  assert.deepEqual(ids, ['character-class.intersection', 'character-class.nested'])
  assert.deepEqual(
    extractConstructs(String.raw`[[:alpha:]]+`).map(item => item.id),
    ['posix-class.positive'],
  )
})

test('case metadata mismatches are conformance gaps', () => {
  const stats = {
    constructs: {
      'lookahead.positive': { variants: {} },
    },
    grammars: [{
      language: 'demo',
      constructList: ['lookahead.positive'],
      constructs: { 'lookahead.positive': { variants: {} } },
    }],
  }
  const conformance = diffConformance(stats, [{
    name: 'stale-metadata',
    pattern: 'x',
    constructs: ['lookahead.positive'],
  }])

  assert.equal(conformance.summary.missingConstructs, 0)
  assert.equal(conformance.summary.missingVariants, 0)
  assert.deepEqual(conformance.caseMetadataMismatches, [{
    case: 'stale-metadata',
    undeclared: [],
    notDetected: ['lookahead.positive'],
  }])
  assert.equal(hasConformanceGaps(conformance), true)
})

test('--fail-on-gaps exits 1 for case metadata mismatches alone', async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'mark-grammar-stats-'))
  try {
    const grammars = path.join(root, 'grammars')
    const cases = path.join(root, 'cases.mjs')
    await fs.mkdir(grammars)
    await fs.writeFile(path.join(grammars, 'demo.tmLanguage.json'), JSON.stringify({
      scopeName: 'source.demo',
      patterns: [{ match: '(?=x)x' }],
    }))
    await fs.writeFile(cases, `export const conformanceCases = [{
      name: 'stale-metadata',
      pattern: 'x',
      constructs: ['lookahead.positive'],
    }]\n`)

    const result = spawnSync(process.execPath, [
      'tools/grammar-stats.mjs',
      grammars,
      '--conformance-cases', cases,
      '--fail-on-gaps',
      '--compact',
    ], { cwd: repoRoot, encoding: 'utf8' })

    assert.equal(result.status, 1, result.stderr || result.stdout)
    const report = JSON.parse(result.stdout)
    assert.equal(report.conformance.summary.missingConstructs, 0)
    assert.equal(report.conformance.summary.missingVariants, 0)
    assert.equal(report.conformance.caseMetadataMismatches.length, 1)
  } finally {
    await fs.rm(root, { recursive: true, force: true })
  }
})
