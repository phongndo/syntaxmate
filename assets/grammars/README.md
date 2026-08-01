# TextMate grammar assets

Vendored TextMate grammar source assets for the in-house engine migration.

The active public catalog is the full pinned Shiki language set plus audited
external grammars recorded in `SOURCE.toml`, including MLIR, YANG, Starlark,
Rego, CUDA, SPIR-V, OpenCL, Metal, WebIDL, and AssemblyScript. Dart,
Handlebars, PHP, Pug, R, reStructuredText, and YAML use the pinned VS Code
1.128 built-in assets after the cross-source
behavior audit found observable differences. Pug preserves VS Code's unresolved
Sass/Stylus includes and reStructuredText preserves VS Code's unresolved CMake
include despite Mark bundling those languages independently. The YAML root is
bundled with its private YAML 1.2 and embedded dependency grammars.
`coverage.toml` is the source of truth for the public language ids (264
languages) and the private dependency blobs embedded alongside them.

Private dependency grammars are also embedded without becoming public catalog
languages. The non-Shiki compatibility assets are recorded in `SOURCE.toml`.

Asset filename remaps:

- `bash` → `shellscript.tmLanguage.json`
- `dockerfile` → `docker.tmLanguage.json`

The files under `languages/` are real grammar JSON objects, primarily imported
from the pinned `@shikijs/langs` package recorded in `SOURCE.toml`; every
additional source is pinned there with its version, revision, license, and any
recorded transformation. Assets are committed as text so diffs remain
reviewable. The generated runtime
bundle (`bundle.bin`) is not committed; it is produced by `build.rs` /
`grammar-compile`.

`licenses.json` records source package, version, license, scope name, module, and
path for every vendored grammar. Required per-asset notices are stored under
`licenses/`, referenced by `licenseTextPath`, embedded in the runtime license
table, and copied into release attribution. `coverage.toml` records the active
public keep/remap list and private dependency blobs; `coverage.full-shiki.toml` is the
generated Shiki-only baseline used to check the import set. Regenerate/check the
Shiki assets with `node tools/vendor-shiki-grammars.mjs` /
`node tools/vendor-shiki-grammars.mjs --check`.
