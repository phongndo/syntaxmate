# Competitive benchmarks

Syntaxmate is measured on two deliberately separate tracks. The tracks answer
different questions and must not be merged into one leaderboard.

- **Engine:** Syntaxmate against VS Code's TextMate tokenizer using identical
  grammar assets and source fixtures. Grammar setup is excluded, and normalized
  UTF-8 scope-stream digests must match.
- **End to end:** Syntaxmate, Shiki, and Syntect using each product's normal
  bundled syntax, theme, highlighting, and HTML API. This measures the product
  experience, not identical grammar behavior.

## Reference result

Measured 2026-08-04 on an Apple M4 Max with 64 GB RAM, macOS 26.5.2, Rust
1.88.0, and Node 26.6.0. Each set uses seven separate-process samples in a
rotating order. Warm samples auto-calibrate to at least 100 ms. Values below
are aggregates of per-corpus p50 latency; per-corpus p50/p95 values and the
complete environment are in [`results-2026-08-04.json`](results-2026-08-04.json).

Pinned versions:

- Syntaxmate 0.1.1
- [Shiki](https://shiki.style/guide/install) 4.4.1 with its default Oniguruma WebAssembly backend
- [Syntect](https://docs.rs/syntect/5.3.0/syntect/) 5.3.0 with its default native Oniguruma backend
- [`vscode-textmate`](https://github.com/microsoft/vscode-textmate) 9.3.2 with `vscode-oniguruma` 2.0.1

### Engine results

Ten common stress fixtures are measured: Bash, C++, HTML, Java, JSON, Markdown,
Python, Rust, TypeScript, and YAML.

| Phase | Syntaxmate | vscode-textmate | Syntaxmate speedup |
| --- | ---: | ---: | ---: |
| First tokenization | 1.261 MB/s | 0.082 MB/s | **15.32×** |
| Steady, line-result cache disabled | 4.707 MB/s | 0.830 MB/s | **5.67×** |
| Unchanged-document replay | 198.085 MB/s | 0.821 MB/s | **241.37×** |

The normalized scope-stream digest matched for every engine corpus, phase, and
sample. The replay result is intentionally not presented as raw regex-engine
speed: it measures Syntaxmate's built-in line-result cache, while the direct
`vscode-textmate` API driver has no equivalent line-result cache. Applications
can layer their own caching above that API.

### End-to-end HTML results

The common bundled set has nine languages. TypeScript is excluded because
Syntect 5.3.0's default syntax set does not provide a `.ts` syntax.

| Phase and metric | Syntaxmate | Shiki | Syntect |
| --- | ---: | ---: | ---: |
| Process-cold mean latency, lower is better | 11.184 ms | 142.457 ms | **9.118 ms** |
| Steady throughput, line-result cache disabled | **3.925 MB/s** | 0.714 MB/s | 2.253 MB/s |
| Unchanged-document replay throughput | **19.300 MB/s** | 0.712 MB/s | 2.246 MB/s |

In aggregate, Syntaxmate is **5.50× faster than Shiki** and **1.74× faster than
Syntect** in steady end-to-end HTML generation. On unchanged-document replay,
it is **27.12× faster than Shiki** and **8.59× faster than Syntect**. Syntect
has the best process-cold aggregate latency, about **1.23× faster than
Syntaxmate**; Syntaxmate starts about **12.74× faster than Shiki**.

Steady per-corpus throughput shows where the aggregate has exceptions:

| Language | Syntaxmate | Shiki | Syntect |
| --- | ---: | ---: | ---: |
| Bash | **3.914** | 1.218 | 2.292 |
| C++ | **2.790** | 0.501 | 1.765 |
| HTML | 2.839 | 0.275 | **3.094** |
| Java | **5.910** | 1.330 | 2.430 |
| JSON | **11.427** | 3.831 | 5.583 |
| Markdown | 3.290 | **4.222** | 3.400 |
| Python | **5.468** | 0.968 | 1.214 |
| Rust | **8.779** | 1.000 | 2.408 |
| YAML | **3.120** | 2.156 | 1.937 |

Values are input MB/s. The products emit different HTML representations and
output sizes, so end-to-end results are performance comparisons rather than
output-equivalence claims.

## Reproduce

The runner installs only the pinned benchmark dependencies, builds optimized
drivers, and writes a summary report:

```sh
python3 tools/run-competitive-benchmarks.py \
  --samples 7 \
  --minimum-time-ms 100 \
  --out target/competitive-benchmarks.json
```

Use `--include-samples` when raw records are needed for an audit. The runner
checks source-byte accounting, output determinism, completion status, sample
counts, and engine scope-digest equality before writing a report.

## Interpretation limits

- Results describe this machine and corpus set, not every workload.
- Process-cold includes process launch and runtime startup; steady/replay timing
  excludes setup.
- The engine track is the only apples-to-apples grammar comparison.
- The end-to-end track uses each product's bundled grammars and themes. Syntect
  uses Sublime syntax definitions; Syntaxmate and Shiki use TextMate grammars.
- Replay measures normal product behavior on identical input. Applications may
  add caching above Shiki, Syntect, or `vscode-textmate`.
