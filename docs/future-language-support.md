# Future language support batches

This roadmap tracks proposed additions to Syntaxmate's TextMate catalog. The
current shipped and validated set is authoritative in
[`language-status.md`](language-status.md); this document only describes future
work and does not count a language as supported before it passes every gate.

## Promotion contract

Every public language added by a batch must meet the same contract as the
existing catalog:

- a pinned, reviewable grammar source with version, revision, and license;
- correct canonical ID, aliases, extensions, basenames, and collision policy;
- `basic` and `stress` fixtures containing 10–30 and 140–260 lines;
- exact scope-stack and coarse-class parity with the `vscode-textmate` oracle;
- no `divergences.toml` exception and `stoppedEarly: false` on every line;
- no native tokenizer degradation or unresolved required dependency;
- a process-cold catalog result of at least 2 MB/s;
- provenance, theme parity, corpus, promotion, catalog-identity, and generated
  documentation checks;
- compiler, parser, or reference-tool validation of fixtures when practical.

A batch is complete only when all of its languages pass. Placeholder,
smoke-only, or unlicensed grammars are not promoted.

## Completed expansion batches

| Batch | Public IDs | Result |
| --- | --- | --- |
| Private-asset promotion | `ignore`, `yang` | Completed |
| Developer/GPU/Web platform | `assemblyscript`, `cuda`, `metal`, `opencl`, `rego`, `spirv`, `starlark`, `webidl` | Completed |

These completed additions are part of the generated catalog ledger, not future
work.

## Batch 3: academic and formal methods

This is the next planned batch.

| Proposed ID | Language | Detection targets | Required coverage |
| --- | --- | --- | --- |
| `standard-ml` | Standard ML | `.sml`, `.sig`, `.fun` | modules, signatures, functors, patterns, records |
| `agda` | Agda | `.agda`, literate Agda forms | modules, dependent types, records, mixfix operators |
| `idris` | Idris 2 | `.idr`, `.lidr` | dependent types, interfaces, elaborator and totality syntax |
| `tlaplus` | TLA+ and PlusCal | `.tla` | modules, temporal operators, proofs, embedded PlusCal |
| `smtlib` | SMT-LIB 2 | `.smt2`, `.smt` | declarations, terms, theories, commands, quoted symbols |
| `dafny` | Dafny | `.dfy` | specifications, methods, datatypes, traits, proofs |

Idris support targets Idris 2. Literate formats may require host-language
injection grammars rather than treating every file as plain source.

## Batch 4: technical documents and diagrams

| Proposed ID | Language | Detection targets | Required coverage |
| --- | --- | --- | --- |
| `org` | Org mode | `.org` | headings, drawers, properties, links, tables, source blocks |
| `quarto` | Quarto | `.qmd` | YAML front matter, Markdown, directives, executable fences |
| `dot` | Graphviz DOT | `.dot`, `.gv` | directed/undirected graphs, attributes, HTML labels |
| `plantuml` | PlantUML | `.puml`, `.plantuml`, `.iuml` | diagrams, preprocessing, themes, embedded blocks |

Quarto must reuse existing Markdown/YAML and fenced-language dependencies rather
than implementing those syntaxes again.

## Batch 5: developer tooling and configuration

| Proposed ID | Language or format | Likely detection targets |
| --- | --- | --- |
| `meson` | Meson build language | `meson.build`, `meson_options.txt` |
| `caddyfile` | Caddy configuration | `Caddyfile` |
| `dhall` | Dhall | `.dhall` |
| `nickel` | Nickel | `.ncl` |
| `jq` | jq filters and programs | `.jq` |
| `jsonata` | JSONata | `.jsonata` |
| `promql` | Prometheus Query Language | explicit selection and reviewed query-file suffixes |
| `logql` | Grafana Loki LogQL | explicit selection and reviewed query-file suffixes |

PromQL and LogQL should remain explicit-selection-only unless an unambiguous,
widely adopted filename convention is identified.

## Batch 6: web templates and platform formats

- `ejs`
- `nunjucks`
- `mustache`
- `smarty`
- `go-template`, including a reviewed Helm injection profile
- `webvtt`
- `vbscript`

Framework names that use ordinary JSX, TSX, or HTML syntax are not automatically
new language IDs. React, Solid, Qwik, HTMX, and Alpine should receive separate
IDs only if a maintained injection grammar provides materially different
highlighting.

## Batch 7: additional research languages

- `isabelle`
- `alloy`
- `fstar`
- `whyml`
- `mercury`
- `koka`
- `ats`
- `proverif`

This batch follows the core academic batch because authoritative editor grammars
and reference-tool fixtures vary substantially across these projects.

## Batch 8: scientific and modeling languages

- `stan`
- `modelica`
- `chapel`
- `octave`
- `scilab`
- `gap`
- `maple`
- `spss`
- `gams`

An Octave grammar must demonstrate meaningful behavior beyond the existing
MATLAB grammar before receiving a separate public ID.

## Batch 9: publishing and diagram extensions

- `context`
- `rmarkdown`
- `lilypond`
- `texinfo`
- `latex3`

Package-specific LaTeX commands do not by themselves justify new public IDs.
A separate grammar must have distinct detection and tested scope behavior.

## Platform profiles that are already covered

These are not pending language additions:

- WebGPU source uses `wgsl`.
- Textual WebAssembly uses `wasm` with `wat`/`wast` aliases.
- WebAssembly component interfaces use `wit`.
- Zig and ZON use `zig`.
- Typst uses `typst`; TeX, LaTeX, and BibTeX are also public.

For evolving specifications, add current-syntax regression fixtures to the
existing public ID instead of creating version-numbered language IDs.

## Batch workflow

For each batch:

1. Audit candidate upstream grammars and licenses before vendoring.
2. Record sources and transformations in `assets/grammars/SOURCE.toml` and
   `licenses.json`.
3. Add metadata, collision rules, and path-detection tests.
4. Add basic/stress fixtures and generate oracle goldens.
5. Run native exact/coarse, budget, theme, provenance, and catalog tests.
6. Rebuild the corpus and persist a passing performance sweep.
7. Update the locked count and catalog identity only after all gates pass.
8. Regenerate [`language-status.md`](language-status.md) and managed count
   snippets.
