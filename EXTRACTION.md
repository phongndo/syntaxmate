# Extraction provenance

Syntaxmate's initial engine, grammar catalog, theme resolver, oracle corpus, and
performance fixtures were extracted from Mark at commit:

```text
38e3a560b6e5e8c50548e0a2a5ae22b8fdc9a7a6
```

Source repository: <https://github.com/phongndo/mark>

The extracted implementation was already MIT licensed. Vendored grammar and
theme assets retain their independent source, revision, transformation, and
license records under `assets/grammars` and `assets/themes`.

The first extraction preserves the Mark oracle output before establishing
Syntaxmate-owned public APIs and release metadata. Its release-candidate gate
covers all 264 catalog languages and 544 oracle cases with an empty divergence
allowlist. The one-iteration catalog comparison on the extraction host measured
14.519 MB/s for Syntaxmate versus 14.687 MB/s for the source baseline (a 1.1%
difference); machine-sensitive CI uses three iterations. Future engine,
catalog, and theme changes are owned and versioned by Syntaxmate.
