#!/bin/sh
set -eu

cd "$(dirname "$0")/.."

python3 tools/check-textmate-theme-assets.py
python3 tools/check-grammar-provenance.py
python3 tools/check-theme-performance.py
node tools/vendor-textmate-themes.mjs --check
node tools/vendor-shiki-grammars.mjs --check
node tools/generate-theme-goldens.mjs --check
node tools/theme-selector-conformance.mjs
node tools/theme-catalog-parity.mjs --check

if [ -n "${VSCODE_SOURCE:-}" ]; then
  python3 tools/check-grammar-provenance.py --verify-vscode
  python3 tools/audit-vscode-grammars.py --vscode-checkout "$VSCODE_SOURCE" --check
  node tools/vscode-grammar-behavior-audit.mjs --vscode-checkout "$VSCODE_SOURCE" --check
fi

echo "TextMate parity: all local checks passed"
