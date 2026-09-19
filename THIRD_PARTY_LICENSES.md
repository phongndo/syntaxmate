# Third-party assets

Syntaxmate bundles TextMate grammars and themes from pinned upstream sources.
The machine-readable source, version, revision, SPDX identifier, and per-asset
notice records are stored in:

- [Grammar source pins](assets/grammars/SOURCE.toml)
- [Grammar license records](assets/grammars/licenses.json) and [notices](assets/grammars/licenses/)
- [Theme source pins](assets/themes/SOURCE.toml) and [license records](assets/themes/licenses.json)

The embedded grammar bundle retains per-language license metadata through the
public `Catalog::licenses` API. Generated release checks must verify these
records before publication.
