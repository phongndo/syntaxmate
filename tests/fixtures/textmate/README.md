# TextMate golden fixtures

Source fixtures and checked-in `vscode-textmate` oracle output for the native
engine. The harness lives in `tests/textmate_golden.rs` and is compiled by the
crate's test configuration. The generated status split lives in
[`docs/language-status.md`](../../../docs/language-status.md).

<!-- BEGIN GENERATED: language-counts -->
The generated manifest has **544 cases** covering **264 public language IDs** in the **264-ID supported catalog**. **264 IDs are validated** by the complete generated contract; **264 IDs** are in `catalog-repeated`. The current validated IDs are `abap`, `actionscript-3`, `ada`, `angular-expression`, `angular-html`, `angular-inline-style`, `angular-inline-template`, `angular-let-declaration`, `angular-template`, `angular-template-blocks`, `angular-ts`, `apache`, `apex`, `apl`, `applescript`, `ara`, `asciidoc`, `asm`, `assemblyscript`, `astro`, `awk`, `ballerina`, `bat`, `beancount`, `berry`, `bibtex`, `bicep`, `bird2`, `blade`, `bsl`, `c`, `c3`, `cadence`, `cairo`, `clarity`, `clojure`, `cmake`, `cobol`, `codeowners`, `codeql`, `coffee`, `common-lisp`, `coq`, `cpp`, `cpp-macro`, `crystal`, `csharp`, `css`, `csv`, `cuda`, `cue`, `cypher`, `d`, `dart`, `dax`, `desktop`, `diff`, `docker`, `dotenv`, `dream-maker`, `edge`, `elixir`, `elm`, `emacs-lisp`, `erb`, `erlang`, `es-tag-css`, `es-tag-glsl`, `es-tag-html`, `es-tag-sql`, `es-tag-xml`, `fennel`, `fish`, `fluent`, `fortran-fixed-form`, `fortran-free-form`, `fsharp`, `gdresource`, `gdscript`, `gdshader`, `genie`, `gherkin`, `git-commit`, `git-rebase`, `gleam`, `glimmer-js`, `glimmer-ts`, `glsl`, `gn`, `gnuplot`, `go`, `graphql`, `groovy`, `hack`, `haml`, `handlebars`, `haskell`, `haxe`, `hcl`, `hjson`, `hlsl`, `html`, `html-derivative`, `http`, `hurl`, `hxml`, `hy`, `ignore`, `imba`, `ini`, `java`, `javascript`, `jinja`, `jinja-html`, `jison`, `json`, `json5`, `jsonc`, `jsonl`, `jsonnet`, `jssm`, `jsx`, `julia`, `just`, `kdl`, `kotlin`, `kusto`, `latex`, `lean`, `less`, `liquid`, `llvm`, `log`, `logo`, `lua`, `luau`, `make`, `markdown`, `markdown-nix`, `markdown-vue`, `marko`, `matlab`, `mdc`, `mdx`, `mermaid`, `metal`, `mipsasm`, `mlir`, `mojo`, `moonbit`, `move`, `narrat`, `nextflow`, `nextflow-groovy`, `nginx`, `nim`, `nix`, `nushell`, `objective-c`, `objective-cpp`, `ocaml`, `odin`, `opencl`, `openscad`, `pascal`, `perl`, `php`, `pkl`, `plsql`, `po`, `polar`, `postcss`, `powerquery`, `powershell`, `prisma`, `prolog`, `proto`, `pug`, `puppet`, `purescript`, `python`, `qml`, `qmldir`, `qss`, `r`, `racket`, `raku`, `razor`, `reg`, `regexp`, `rego`, `rel`, `riscv`, `ron`, `rosmsg`, `rst`, `ruby`, `rust`, `sas`, `sass`, `scala`, `scheme`, `scss`, `sdbl`, `shaderlab`, `shellscript`, `shellsession`, `smalltalk`, `solidity`, `soy`, `sparql`, `spirv`, `splunk`, `sql`, `ssh-config`, `starlark`, `stata`, `stylus`, `surrealql`, `svelte`, `swift`, `system-verilog`, `systemd`, `talonscript`, `tasl`, `tcl`, `templ`, `terraform`, `tex`, `toml`, `ts-tags`, `tsv`, `tsx`, `turtle`, `twig`, `typescript`, `typespec`, `typst`, `v`, `vala`, `vb`, `verilog`, `vhdl`, `viml`, `vue`, `vue-directives`, `vue-html`, `vue-interpolations`, `vue-sfc-style-variable-injection`, `vue-vine`, `vyper`, `wasm`, `webidl`, `wenyan`, `wgsl`, `wikitext`, `wit`, `wolfram`, `xml`, `xsl`, `yaml`, `yang`, `zenscript`, `zig`.
<!-- END GENERATED: language-counts -->

## Layout

| Path | Role |
| --- | --- |
| `cases.toml` | Generated oracle manifest (`[[case]]` + automatic `[[case.embedded]]`) |
| `cases.config.json` | Hand-written language-asset mappings and exceptional case overrides |
| `divergences.toml` | Explicit allowlist for known engine ≠ oracle ranges |
| `<lang>/basic.*` | Small language-specific exact-parity gate |
| `<lang>/stress.*` | Broader language-specific exact-parity gate |
| `<lang>/smoke.*` | Small language-specific exact-parity gate |
| `<lang>/*.golden.jsonl` | Generated oracle output — do not hand-edit |

Only languages with both `basic` and `stress` cases meet the currently
machine-readable validation fixture contract. A `smoke` case alone is oracle
coverage, not full validation. Every committed case is compared in both exact
scope-stack and coarse `SyntaxClass` modes, and `divergences.toml` is empty.

## Fixture language ids

The original core regression set is:

`bash`, `c`, `cpp`, `csharp`, `css`, `docker`, `go`, `html`, `java`,
`javascript`, `json`, `jsx`, `kotlin`, `lua`, `make`, `markdown`, `nix`, `php`,
`powershell`, `python`, `ruby`, `rust`, `scss`, `sql`, `swift`, `terraform`,
`toml`, `tsx`, `typescript`, `yaml`

Extended basic/stress fixtures are tracked in the generated ledger above;
fixture additions do not require hand-updating a second count or ID list here.

Notes:

- Language id `bash` uses the `shellscript.tmLanguage.json` asset (`source.shell`).
- Language id `docker` uses `docker.tmLanguage.json` (`source.dockerfile`).
- Markdown, HTML, SCSS, PHP, and C++ cases load embedded grammars via
  `[[case.embedded]]` so oracle output includes injected/include scopes.
- `zig/stress.zig` deliberately keeps pre-0.11 async constructs (`async`,
  `await`-era `anyframe`, `suspend`/`resume`, `nosuspend`, `@frame()`) that
  Zig 0.15.2 rejects. The vendored grammar still ships dedicated keyword rules
  for them, and fixtures track grammar coverage, not compiler acceptance;
  replacing those lines with modern Zig would silently drop the only coverage
  of those rules. Revisit when the vendored grammar drops the keywords.
- The upstream `bird2` grammar never pops `meta.function-definition.bird` on
  valid syntax: the rule's `end` brace is always consumed by the nested
  `#blocks` rule first, so every `function` leaks its scope to end-of-file
  (`vscode-textmate` agrees; parity holds either way). Function definitions
  therefore live only in `bird2/basic.bird2` — a leaked scope in the
  perf-swept stress fixture changes the tokenizer state of every following
  line, which defeats line caching across repeated-corpus copies and sinks
  process-cold throughput below the floor.

## Regenerate the case manifest

`cases.toml` is checked in, but generated from every `<lang>/basic.*`,
`<lang>/stress.*`, and current `<lang>/smoke.*` source. Generated
`*.golden.jsonl` and files with a `sample` name segment are ignored. The
generator reads each vendored grammar's `scopeName`, follows its external
`include` scopes recursively, and emits only dependencies present in
`assets/grammars/languages/`; unresolved optional includes are omitted just
as they are by `vscode-textmate`.

```sh
node tools/generate-textmate-cases.mjs
node tools/generate-textmate-cases.mjs --check
node tools/test-generate-textmate-cases.mjs
```

After fixture and corpus promotion work, regenerate the status and its public
count snippets in that order:

```sh
python3 tools/generate-language-status.py
python3 tools/check-language-docs.py --write
python3 tools/check-language-docs.py --check
```

The final command is static and CI-safe: it reads checked-in manifests, status,
and scale policy, but does not run the golden suite or performance sweep.

## Run the golden harness

The normal command remains the complete local/release gate:

```sh
cargo test -p syntaxmate --test textmate_golden --locked
```

CI runs the two catalog-wide loops as four deterministic, zero-based shards:

```sh
SYNTAXMATE_SHARD_INDEX=0 SYNTAXMATE_SHARD_TOTAL=4 \
  cargo test -p syntaxmate --test textmate_golden --locked
```

Both variables must be set together, `TOTAL` must be positive, and `INDEX`
must be less than `TOTAL`. Cases are assigned by unsigned-big-endian
`SHA-256(public_language_id) mod TOTAL`, using the root grammar asset ID so
all cases stay together (`bash` is assigned under public ID `shellscript`).
Language-specific regression tests still use their complete case sets; only
the two manifest-wide parity and budget loops are filtered.

Use `cases.config.json` when a fixture language id does not match its grammar
asset (for example `bash` → `shellscript`) or when a non-convention fixture
must remain a golden case (currently `cpp/libcxx_vector.cpp`). An explicit
case may also set `asset`, `scope`, `grammar`, or `golden`; matching a
discovered fixture path overrides its inferred fields.

## Regenerate oracle goldens

Install the pinned Node oracle (dev-only):

```sh
npm install --prefix tools/golden-oracle
```

From the repository root:

```sh
node tools/generate-goldens.mjs
node tools/generate-goldens.mjs --check
node tools/generate-goldens.mjs --case rust
```

## Triage one language

Run every committed `basic`/`stress`/`smoke` case for one language in a single
loop:

```sh
node tools/triage-language.mjs rust
node tools/triage-language.mjs rust --json
```

By default this writes fresh oracle output to a temporary directory, compares
the native full scope stream with `compare-textmate-scopes.py`, checks native
degradation/budget counters, and measures repeated `stress` input against the
2 MB/s process-cold floor. It exits nonzero for `scope-mismatch`, `degraded`,
`budget-kill`, `oracle-stopped-early`, or `perf-floor` blockers. Use `--golden`
for an offline/read-only comparison with the checked-in golden, `--kind stress`
to focus one fixture kind, or `--keep-temp` to retain diagnostic artifacts.

Ad-hoc single dump:

```sh
node tools/golden-dump.mjs \
  --language rust \
  --scope source.rust \
  --grammar assets/grammars/languages/rust.tmLanguage.json \
  --file tests/fixtures/textmate/rust/basic.rs \
  --out tests/fixtures/textmate/rust/basic.golden.jsonl
```

## Comparison modes

The Rust harness compares:

1. **Exact** scope stacks (UTF-16 oracle offsets converted to UTF-8 byte ranges).
2. **Coarse** `SyntaxClass` segments after scope classification.

`divergences.toml` can skip `exact`, `coarse`, or `any` for a 1-based line
range. Unused allowlist entries fail the harness (stale-divergence detection).

Today:

- All basic, stress, smoke, and additional C++ cases are exact + coarse gates.
- `divergences.toml` has no entries. Any future exception must be explicit and
  causes the affected language to remain supported rather than validated in
  the generated ledger.

## Property tests

`textmate_golden.rs` also covers non-oracle properties:

- no panic on generated UTF-8 inputs
- zero-width matches advance / do not loop
- checkpoint replay matches full replay
- fallback step-budget kills pathological patterns without hanging the line
