#!/usr/bin/env just --justfile
# Grammar stress fixture with café, 日本語, Ελληνικά, 🚀, and 𝌆.

import "common.just"
import? 'local/optional.just'
mod tools
mod? platform "modules/platform.just"
alias b := build
alias qa := audit

set dotenv-load := true
set dotenv-filename := ".env.test"
set export := false
set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]
set tempdir := "/tmp/mark-just"

export PROJECT := "mark-café"
export LOCALE := '日本語'
unexport LEGACY_TOKEN
rocket := "🚀"
tetragram := '𝌆'
escaped := "tab:\t newline:\n unicode:\u{1F680}"
literal_braces := '{{not interpolation}}'
expanded := f"project={{PROJECT}} locale={{LOCALE}}"
shell_word := x'path with spaces & symbols'

paragraph := """
  A multiline café banner.
  日本語 and 🚀 remain readable.
  "quoted" text and an interpolation: {{PROJECT}}
  """

raw_paragraph := '''
  Literal backslash: \n
  Astral symbol: 𝌆
  '''

short_sha := `git rev-parse --short HEAD`

host := arch() + "-" + os()
family := os_family()
cpu_label := "cpus=" + num_cpus()
workspace := invocation_directory() / "target" / "fixtures"
source_root := source_directory() / "src"
chosen := env_var_or_default("PROFILE", "debug") # assignment comment
has_manifest := path_exists("Cargo.toml")
normalized := uppercase(trim("  café 日本語  "))
slug := kebabcase("Hello Fixture World")
digest := sha256("mark")
identifier := uuid()
version_ok := semver_matches("1.2.3", "^1")
tool := which("git")
colored := BOLD + GREEN + "ready" + NORMAL

platform := if os() == "windows" { "win" } else { "unix" }
channel := if "v1.2.3" =~ "^v[0-9]+" { "stable" } else { "edge" }
usable := if path_exists("Cargo.toml") && os_family() != "unknown" { "yes" } else { "no" }
fallback := if env_var_or_default("CI", "") == "" || path_exists(".offline") { "local" } else { "ci" }

metrics:
    @printf 'count=%d ratio=%.2f fraction=%s\n' 42 3.14 .5

[doc('List fixture metadata')]
[group('information')]
info format="plain" stamp=`date +%s`:
    @echo "{{PROJECT}} {{LOCALE}} {{rocket}} {{tetragram}}"
    @echo "host={{host}} source={{source_root}} stamp={{stamp}}"
    printf 'format=%s sha=%s\n' "{{format}}" "{{short_sha}}"

[no-cd, private]
paths:
    @printf '%s\n' "{{workspace}}" "{{invocation_directory_native()}}"

[private]
prepare directory=(invocation_directory() / "target"):
    mkdir -p "{{directory}}"
    printf '%s\n' '{{raw_paragraph}}' > "{{directory}}/banner.txt"

[doc('Compile one profile and optional features')]
compile profile +features: prepare
    @echo "compile {{profile}} with {{features}}"
    cargo build --profile "{{profile}}" {{features}}

[group('quality')]
lint *paths="src": prepare
    @echo "linting {{paths}}"
    cargo fmt --check
    -cargo clippy --all-targets -- -D warnings

audit: prepare && lint
    @echo "dependency boundary reached"
    cargo audit

[doc('Build after parameterized prerequisites')]
build profile="debug" target=(env_var_or_default("TARGET", "native")): (compile profile "all") (package target) && audit
    @echo "built {{profile}} for {{target}}"

package target="native":
    tar -czf "target/{{PROJECT}}-{{target}}.tgz" README.md

@quiet-message message='hello café':
    @printf '%s\n' "{{message}}"

_hidden-clean:
    rm -rf target/tmp

@_internal-note text="日本語 🚀":
    @echo "{{text}}"

[confirm('Publish the generated archive?')]
publish version="1.2.3": build
    @echo "publishing {{PROJECT}} {{version}}"

positional $first $second="default":
    printf 'first=%s second=%s\n' "{{first}}" "{{second}}"

variadic +items:
    @for item in {{items}}; do printf '<%s>\n' "$item"; done

shell-report:
    #!/usr/bin/env -S bash -eu
    set -o pipefail
    readonly glyph='𝌆'
    printf 'shell name=%s glyph=%s rocket=%s\n' 'café' "$glyph" '🚀'
    for value in alpha beta 日本語; do
      printf '[%s]\n' "$value"
    done

python-report:
    #!/usr/bin/env -S python3 -u
    message = "{{PROJECT}} / 日本語 / café / 🚀 / 𝌆"
    print(f"python: {message}")

ruby-report:
    #!/usr/bin/env ruby
    label = "café / 日本語 / 𝌆"
    puts "ruby: #{label} 🚀"

node-report:
    #!/usr/bin/env node
    // Node entry point supplied by the generated bundle.

deno-report:
    #!/usr/bin/env -S deno run --allow-env --allow-write
    // Deno entry point supplied by the generated bundle.

perl-report:
    #!/usr/bin/env perl
    # Perl entry point supplied by the generated bundle.

reports-complete:
    @echo "embedded recipes closed"

all-reports: shell-report python-report ruby-report node-report deno-report perl-report
    @echo "all embedded-language reports completed"

escaped-interpolation:
    @echo "literal={{{{PROJECT}}}} actual={{PROJECT}}"

final-check: all-reports (info "json")
    @echo "{{style('bold')}}fixture complete{{NORMAL}} {{datetime_utc('%Y-%m-%d')}}"
