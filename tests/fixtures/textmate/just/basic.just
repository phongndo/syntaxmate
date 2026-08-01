#!/usr/bin/env just --justfile
# Compact Just fixture: café, 日本語, 🚀, and 𝌆.
set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

export GREETING := "café"
audience := '日本語'
label := GREETING + " / " + audience + " 🚀 𝌆"
revision := `git rev-parse --short HEAD`

alias b := build

[doc('Build the sample project')]
build profile="debug": prepare (validate profile)
    @echo "{{label}} [{{profile}}] on {{os()}}"
    printf '%s\n' "escaped braces: {{{{ok}}}}"

prepare:
    mkdir -p "build/{{audience}}"

[private]
validate profile=(env_var_or_default("PROFILE", "debug")):
    #!/usr/bin/env bash
    set -euo pipefail
    printf 'profile=%s glyph=%s\n' "{{profile}}" '𝌆'

finished:
    @echo "done 🚀"
