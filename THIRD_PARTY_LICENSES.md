# Third-party assets

Syntaxmate bundles TextMate grammars and themes from pinned upstream sources.
The machine-readable source, version, revision, SPDX identifier, and per-asset
notice records are stored in:

- `assets/grammars/SOURCE.toml`
- `assets/grammars/licenses.json`
- `assets/grammars/licenses/`
- `assets/themes/SOURCE.toml`

The embedded grammar bundle retains per-language license metadata through the
public `Catalog::licenses` API. Generated release checks must verify these
records before publication.
