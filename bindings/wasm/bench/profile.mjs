// Public-package phase profiler, shared by Node and real browsers. All digest,
// completeness, and source construction work stays outside measured intervals.
// A unique trailing-whitespace suffix on every line prevents result-cache hits
// in the matching phases, including repeated lines within the same document.

function check(condition, message) {
  if (!condition) throw new Error(message)
}

function digest(value) {
  const text = typeof value === 'string' ? value : JSON.stringify(value)
  let hash = 2166136261
  for (let index = 0; index < text.length; index++) {
    hash = Math.imul(hash ^ text.charCodeAt(index), 16777619)
  }
  return (hash >>> 0).toString(16).padStart(8, '0')
}

function tokenRecord(tokens) {
  check(tokens.complete, 'incomplete tokenization')
  check(tokens.lineTokenRanges[tokens.lineCount] === tokens.tokenCount, 'truncated tokens')
  return Object.fromEntries(Object.entries(tokens).map(([key, value]) => [
    key, ArrayBuffer.isView(value) ? Array.from(value) : value,
  ]))
}

function summarize(samples) {
  const sorted = samples.toSorted((a, b) => a - b)
  const middle = sorted.length >> 1
  return {
    medianMs: sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2,
    meanMs: samples.reduce((sum, value) => sum + value, 0) / samples.length,
    minMs: sorted[0], maxMs: sorted.at(-1), samplesMs: samples,
  }
}

/**
 * Profile one already-imported, initialized package. Call in a fresh process or
 * browser worker per outer sample. Outer samples measure startup separately.
 */
export async function profile(api, raw, bundle, source, { language = 'rust', iterations = 30 } = {}) {
  check(Number.isSafeInteger(iterations) && iterations > 0, 'iterations must be positive')
  let start = performance.now()
  const highlighter = api.Highlighter.fromBundle(bundle)
  const constructionMs = performance.now() - start
  let theme
  let session
  try {
    start = performance.now()
    theme = api.Theme.bundled('github-dark')
    const themeMs = performance.now() - start
    const options = { lang: language, theme }
    start = performance.now()
    const first = highlighter.tokens(source, options)
    const firstTokensMs = performance.now() - start
    const firstTokensFinishedAtMs = performance.timeOrigin + performance.now()
    const outputs = { tokens: digest(tokenRecord(first)) }
    const html = highlighter.html(source, options)
    const ansi = highlighter.ansi(source, options)
    outputs.html = digest(html)
    outputs.ansi = digest(ansi)
    outputs.scopes = digest(tokenRecord(highlighter.tokens(source, { ...options, includeScopes: true })))
    const phases = {}
    let sequence = 0
    function uniqueSource() {
      return source.split('\n').map(line => {
        const suffix = (++sequence).toString(2).padStart(32, '0').replaceAll('0', ' ').replaceAll('1', '\t')
        return line + suffix
      }).join('\n')
    }
    function measure(name, operation, { matching = false, validate = () => {} } = {}) {
      const samples = []
      const digests = []
      for (let index = 0; index < iterations; index++) {
        const input = matching ? uniqueSource() : source
        // Validate only after the API interval, keeping checks and serialization
        // out of measured matching and conversion costs.
        const begin = performance.now()
        const output = operation(input)
        samples.push(performance.now() - begin)
        validate(output, input)
        digests.push(digest(typeof output === 'string' ? output : tokenRecord(output)))
      }
      phases[name] = { ...summarize(samples), outputDigest: digest(digests) }
    }
    for (const [name, operation] of [
      ['tokens', input => highlighter.tokens(input, options)],
      ['scopes', input => highlighter.tokens(input, { ...options, includeScopes: true })],
      ['html', input => highlighter.html(input, options)],
      ['ansi', input => highlighter.ansi(input, options)],
    ]) {
      const validate = typeof operation(source) === 'string'
        ? (_output, input) => check(highlighter.tokens(input, options).complete, 'incomplete rendering input')
        : () => {}
      measure(`${name}Matching`, operation, { matching: true, validate })
      operation(source)
      measure(`${name}Replay`, operation, { validate })
    }
    // Measure only the TokenBuffer constructor. Packed raw calls already copy
    // data to JS; legacy constructors perform that transfer inside this phase.
    // Old packages ignore the extra names arguments, allowing either raw ABI.
    let rawTokenFormat
    for (const includeScopes of [false, true]) {
      const samples = []
      const digests = []
      for (let index = 0; index < iterations; index++) {
        const names = []
        const rawTokens = highlighter._raw.tokens(language, source, theme._raw, includeScopes, names)
        rawTokenFormat ??= ArrayBuffer.isView(rawTokens) ? 'packed' : 'legacy-handle'
        const begin = performance.now()
        const output = new api.TokenBuffer(rawTokens, names)
        samples.push(performance.now() - begin)
        digests.push(digest(tokenRecord(output)))
      }
      phases[includeScopes ? 'scopesConversion' : 'tokensConversion'] = {
        ...summarize(samples), outputDigest: digest(digests),
      }
    }
    session = highlighter.session(options)
    const lines = source.split('\n')
    const sessionSamples = []
    const sessionDigests = []
    for (const line of lines) tokenRecord(session.line(line))
    for (let index = 0; index < iterations; index++) {
      session.reset()
      let elapsed = 0
      const lineDigests = []
      for (const line of lines) {
        const begin = performance.now()
        const output = session.line(line)
        elapsed += performance.now() - begin
        lineDigests.push(digest(tokenRecord(output)))
      }
      sessionSamples.push(elapsed)
      sessionDigests.push(digest(lineDigests))
    }
    phases.sessionReplay = { ...summarize(sessionSamples), callsPerSample: lines.length,
      outputDigest: digest(sessionDigests) }
    const wasm = raw.initSync()
    const memoryBeforeFree = wasm.memory.buffer.byteLength
    session.free()
    session = undefined
    theme.free()
    theme = undefined
    highlighter.free()
    let clockResolutionMs = Infinity
    for (let index = 0; index < 10; index++) {
      const before = performance.now()
      let after
      do { after = performance.now() } while (after === before)
      clockResolutionMs = Math.min(clockResolutionMs, after - before)
    }
    return {
      sourceUtf16Length: source.length, matchingSourceUtf16Length: source.length + lines.length * 32,
      sourceDigest: digest(source), iterations,
      constructionMs, themeMs, firstTokensMs, firstTokensFinishedAtMs, outputs, phases,
      rawTokenFormat,
      conversionMeaning: rawTokenFormat === 'packed'
        ? 'TokenBuffer constructor only: decodes JS-owned packed data; WASM transfer precedes this interval'
        : 'TokenBuffer constructor only: copies data from WASM and constructs JS style/scope tables',
      memoryBeforeFree, memoryAfterFree: wasm.memory.buffer.byteLength,
      clockResolutionMs,
      memoryMeaning: 'WASM linear-memory high-water mark, not live allocation or retained-cache bytes',
    }
  } finally {
    session?.free()
    theme?.free()
    highlighter.free()
  }
}
