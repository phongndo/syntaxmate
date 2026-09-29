// Each browser sample runs in a fresh worker: no shared module instance, grammar
// preparation, or line cache. HTTP and browser compiled-code caches may persist.
import { profile } from './profile.mjs'

self.onmessage = async ({ data: { packageUrl, source, language, iterations } }) => {
  try {
    const started = performance.now()
    const startedAtMs = performance.timeOrigin + started
    const api = await import(new URL('lib/browser.js', packageUrl))
    const importMs = performance.now() - started
    let wasmInitMs
    let bundleFetchMs
    const loadStarted = performance.now()
    const [, bundle] = await Promise.all([
      api.init().then(() => { wasmInitMs = performance.now() - loadStarted }),
      api.loadBundle().then(bytes => {
        bundleFetchMs = performance.now() - loadStarted
        return bytes
      }),
    ])
    const raw = await import(new URL('dist/syntaxmate.js', packageUrl))
    const result = await profile(api, raw, bundle, source, { language, iterations })
    self.postMessage({ importMs, wasmInitMs, bundleFetchMs,
      browserColdMs: result.firstTokensFinishedAtMs - startedAtMs, ...result })
  } catch (error) {
    self.postMessage({ error: String(error.stack ?? error) })
  }
}
