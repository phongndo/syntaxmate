#!/usr/bin/env node
import assert from 'node:assert/strict'

import { selectBalancedExecutionSample } from './regex-execution-parity.mjs'

function record(hash, language) {
  return { hash, language }
}

{
  const sample = selectBalancedExecutionSample([
    { language: 'alpha', records: [record('a3', 'alpha'), record('a1', 'alpha'), record('a2', 'alpha')] },
    { language: 'beta', records: [record('b2', 'beta'), record('b1', 'beta')] },
    { language: 'gamma', records: [record('c1', 'gamma')] },
  ], 5)
  assert.deepEqual(
    sample.map(({ hash, language }) => [hash, language]),
    [
      ['a1', 'alpha'],
      ['b1', 'beta'],
      ['c1', 'gamma'],
      ['a2', 'alpha'],
      ['b2', 'beta'],
    ],
  )
}

{
  const sample = selectBalancedExecutionSample([
    { language: 'alpha', records: [record('same', 'alpha'), record('unique-a', 'alpha')] },
    { language: 'beta', records: [record('same', 'beta'), record('unique-b', 'beta')] },
  ], 4)
  assert.deepEqual(sample.map(item => item.hash), ['same', 'unique-b', 'unique-a'])
  assert.equal(new Set(sample.map(item => item.hash)).size, sample.length)
}

{
  const sample = selectBalancedExecutionSample([
    { language: 'empty', records: [] },
    { language: 'full', records: [record('2', 'full'), record('1', 'full')] },
  ], 10)
  assert.deepEqual(sample.map(item => item.hash), ['1', '2'])
}

console.log('regex execution parity sampling: ok')
