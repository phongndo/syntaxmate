# Extraction provenance

Syntaxmate's initial engine, grammar catalog, theme resolver, oracle corpus, and
performance fixtures were extracted from [Mark](https://github.com/phongndo/mark)
at commit `38e3a560b6e5e8c50548e0a2a5ae22b8fdc9a7a6`.

The extracted implementation was MIT licensed. Vendored grammar and theme
assets retain independent source, revision, transformation, and license records
under [assets/grammars](assets/grammars/) and [assets/themes](assets/themes/).
Syntaxmate owns subsequent engine, catalog, theme, and public API changes;
applications integrate through the published public API.
