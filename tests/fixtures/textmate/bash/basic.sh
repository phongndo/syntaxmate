#!/usr/bin/env bash
set -euo pipefail

readonly greeting="Hello 🚀"
name="${1:-world}"

format() {
  local subject="$1"
  printf '%s, %s!\n' "$greeting" "$subject"
}

for item in alpha beta gamma; do
  if [[ "$item" == b* ]]; then
    format "${item^}"
  fi
done
format "$name"
