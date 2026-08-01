#!/usr/bin/env bash
set -euo pipefail

# Bash stress fixture with non-ASCII text: café λ🚀.
name="${1:-café}"
json_payload="$(printf '%s\n' '{' \
  '  "message": "hello λ🚀",' '  "regex": "^/api/[[:alpha:]]+$"' '}')"

cat <<EOF
user=${name}
now=$(date +%s)
payload=$(printf '%s' "$json_payload" | sed 's/"/\\"/g')
EOF

result=$((42 / 2))
echo "result=${result} upper=$(printf '%s' "$name" | tr '[:lower:]' '[:upper:]')"
# Configuration exercises defaults, assignment, alternatives, and substring syntax.
: "${MARK_ENV:=development}"
config_file="${CONFIG_FILE:-/etc/mark/config.ini}"
cache_dir="${XDG_CACHE_HOME:-${HOME}/.cache}/mark"
display_name="${name:+user:$name}"
missing_hint="${OPTIONAL_VALUE-<unset>}"
trimmed_path="${config_file%/*}"
config_leaf="${config_file##*/}"
slug="${name// /-}"
initials="${name:0:2}"
readonly program_name="${0##*/}"
# Indexed and associative arrays model a small deployment inventory.
declare -a regions=("us-east-1" "eu-west-1" "ap-south-1")
regions+=("local-λ")
declare -A endpoints=(
  [api]="https://example.invalid/api"
  [metrics]="https://example.invalid/metrics"
  [health]="/readyz"
)
declare -i retries=3
declare -r build_tag="v2.4.0-β"
first_region="${regions[0]}"
last_region="${regions[-1]}"
endpoint_names=("${!endpoints[@]}")
# Arithmetic includes bases, increments, bit operations, and a ternary.
hex_mask=$((16#ff & 2#10101010))
(( retries += 2 ))
(( retries > 4 )) && retry_class=high || retry_class=low
next_window=$((result * 2))
(( next_window += retries ** 2 ))
parity=$((next_window % 2 == 0 ? 0 : 1))
log() { local level=${1:?log level required}; shift; printf '[%s] %s\n' "$level" "$*" >&2; }
join_by() { local delimiter=$1 first=${2-}; shift 2 || return 0; printf '%s' "$first" "${@/#/$delimiter}"; }
function normalize_name { local value=${1,,}; value=${value//[^[:alnum:]_-]/_}; printf '%s\n' "${value^^}"; }
usage() { printf '%s\n' 'Usage: stress.sh [NAME] [--dry-run] [--format=text|json]' 'Literal: $HOME $(date) — Ελληνικά 日本語 हिन्दी 🎼 🧪 𐐷.'; }
dry_run=false
format=text
while (($# > 1)); do
  case $2 in
    --dry-run) dry_run=true ;;
    --format=*) format=${2#*=} ;;
    -h|--help) usage; break ;;
    --) shift; break ;;
    *) printf 'warning: unknown option %q\n' "$2" >&2 ;;
  esac
  shift
done
# Conditional operators, glob matching, regex captures, and legacy test syntax.
if [[ -n $name && $name == c* ]]; then
  category=customer
elif [[ $name =~ ^([[:alpha:]_]+)(-[0-9]+)?$ ]]; then
  category="word:${BASH_REMATCH[1]}"
else
  category=other
fi
if [[ -n ${endpoints[api]+set} && -f $config_file ]]; then
  config_state=present
elif [ -d "$cache_dir" ] || test "$MARK_ENV" = development; then
  config_state=optional
else
  config_state=missing
fi
case ${format,,} in
  json|ndjson)
    content_type=application/json
    ;;
  text)
    content_type='text/plain; charset=utf-8'
    ;;
  t*)
    formatter=plain
    ;;
  *)
    content_type=application/octet-stream
    ;;
esac
# Iteration covers words, array indices, C-style arithmetic, and command output.
for region in "${regions[@]}"; do
  printf 'region=%q endpoint=%q\n' "$region" "${endpoints[health]}"
done
for ((i = 0; i < ${#regions[@]}; i++)); do
  printf -v "region_${i}" '%s' "${regions[i]}"
done
for key in "${!endpoints[@]}"; do
  printf '%-8s -> %s\n' "$key" "${endpoints[$key]}"
done | LC_ALL=C sort
count=0
while ((count < 2)); do
  ((++count))
  [[ $count -eq 1 ]] && continue
  break
done
until ((retries == 0)); do
  ((--retries))
done
while IFS=: read -r label value; do
  printf 'pair[%s]=%s\n' "$label" "$value"
done < <(printf '%s\n' 'alpha:1' 'βeta:2' 'rocket:🚀')
# Pipelines, grouped commands, a subshell, redirections, and status negation.
tmpdir=${TMPDIR:-/tmp}/mark-stress-$$
mkdir -p "$tmpdir"
cleanup() { local status=$?; rm -rf -- "$tmpdir"; return "$status"; }
trap cleanup EXIT
trap 'printf "interrupted by signal\n" >&2; exit 130' INT TERM

{ printf '%s\n' "name=$name" "category=$category"; printf '%s\n' "regions=$(join_by , "${regions[@]}")"; } >"$tmpdir/summary.txt"

(cd "$tmpdir" || exit; umask 077; sed -n '1,2p' summary.txt) | tee "$tmpdir/preview.txt" >/dev/null

if ! grep -Eq '^name=' "$tmpdir/summary.txt"; then
  log ERROR 'summary did not contain a name'
fi

mapfile -t summary_lines < "$tmpdir/summary.txt"
filtered=$(printf '%s\n' "${summary_lines[@]}" | awk -F= 'NF == 2 { print $2 }')
normalized="$(normalize_name "$display_name")"
printf '%s' "$filtered" >"$tmpdir/filtered.txt"

# An unquoted here-document expands variables and command substitutions.
cat >"$tmpdir/report.txt" <<REPORT
program=$program_name
environment=$MARK_ENV
generated=$(printf '%(%s)T' -1)
name=${name@Q}
REPORT

# Quoted delimiters preserve shell-looking examples and backslashes literally.
cat >>"$tmpdir/report.txt" <<'LITERAL_REPORT'
literal=${parameter:-default}
command=$(printf 'not executed')
path=C:\temp\mark and quote="unchanged"
LITERAL_REPORT

if read -r -d '' release_notes <<'NOTES'
Release notes — Καλημέρα, naïve façade, 東京, 🐚, and astral 𝄞.
* Preserve `backticks`, ${braces}, and glob characters [a-z]*?.
NOTES
then
  : # A NUL-delimited read would report success.
else
  : # EOF is expected for this text here-document.
fi

# Here-strings and explicit file descriptors add more redirection forms.
read -r report_first_line <<<"$(<"$tmpdir/report.txt")"
exec 3>"$tmpdir/events.log"
printf 'event=%q state=%q\n' "$normalized" "$config_state" >&3
exec 3>&-

command printf 'content-type: %s\n' "$content_type"
builtin printf 'mask=%d window=%d parity=%d class=%s\n' \
  "$hex_mask" "$next_window" "$parity" "$retry_class"
# Unicode remains safely quoted: snowman ☃, chess ♞, emoji 👩🏽‍💻, Gothic 𐍈.
printf 'done name=%q first=%q last=%q note=%s\n' \
  "$name" "$first_region" "$last_region" "$release_notes"
