ops@luna:~/aurora$ date -u '+%Y-%m-%dT%H:%M:%SZ'
2026-07-12T08:15:00Z
ops@luna:~/aurora$ printf 'Starting %s\n' "orbital checkout"
Starting orbital checkout
ops@luna:~/aurora$ export LC_ALL=C.UTF-8
ops@luna:~/aurora$ export VEHICLE='Asteria-7' SITE=λ-station
ops@luna:~/aurora$ printf 'vehicle=%q site=%q\n' "$VEHICLE" "$SITE"
vehicle=Asteria-7 site=λ-station
ops@luna:~/aurora$ pwd
/home/ops/aurora
ops@luna:~/aurora$ printf '%s\n' config/{flight,radio,thermal}.toml
config/flight.toml
config/radio.toml
config/thermal.toml
ops@luna:~/aurora$ ls -1 logs/*.log 2>/dev/null | head -n 3
logs/attitude.log
logs/power.log
logs/thermal.log

[ops@luna ~/aurora]$ status=${STATUS:-nominal}; echo "status=${status^^}"
status=NOMINAL
[ops@luna ~/aurora]$ serial='AST-0042'; echo "unit=${serial#AST-}"
unit=0042
[ops@luna ~/aurora]$ path=/srv/aurora/archive.tar.gz; echo "${path##*/}"
archive.tar.gz
[ops@luna ~/aurora]$ printf 'home=%s literal=%s\n' "$HOME" '$HOME'
home=/home/ops literal=$HOME
[ops@luna ~/aurora]$ printf 'quoted: %q\n' "one two;three"
quoted: one\ two\;three

sh-5.2$ n=7; printf '%d squared is %d\n' "$n" "$((n * n))"
7 squared is 49
sh-5.2$ ((n++)); echo "$n"
8
sh-5.2$ [[ $n -ge 8 && $VEHICLE == Asteria-* ]]; echo $?
0
sh-5.2$ [[ 'orbit-42' =~ ^orbit-([0-9]+)$ ]]; echo "${BASH_REMATCH[1]}"
42
sh-5.2$ test -r config/flight.toml && echo readable || echo missing
readable

ops@luna:~/aurora$ if [[ -f config/flight.toml ]]; then
> printf '%s\n' 'flight configuration found'
> elif [[ -d config ]]; then
> printf '%s\n' 'configuration directory only'
> else
> printf '%s\n' 'configuration missing' >&2
> fi
flight configuration found
ops@luna:~/aurora$ for subsystem in guidance power thermal; do
> printf 'checking %-8s ... ' "$subsystem"
> echo ok
> done
checking guidance ... ok
checking power    ... ok
checking thermal  ... ok
ops@luna:~/aurora$ case "$SITE" in
> λ-station) echo 'ground link: Europe' ;;
> 雪-base) echo 'ground link: north' ;;
> *) echo 'ground link: unknown' ;;
> esac
ground link: Europe

ops@luna:~/aurora$ attempt=1
ops@luna:~/aurora$ while (( attempt <= 3 )); do
> echo "link attempt $attempt"
> ((attempt++))
> done
link attempt 1
link attempt 2
link attempt 3
ops@luna:~/aurora$ until [[ -e run/ready ]]; do echo waiting; break; done
waiting

ops@luna:~/aurora$ check_channel() {
> local channel=$1
> printf 'channel=%s state=%s\n' "$channel" "${2:-up}"
> }
ops@luna:~/aurora$ check_channel S-band
channel=S-band state=up
ops@luna:~/aurora$ check_channel Ka-band degraded
channel=Ka-band state=degraded
ops@luna:~/aurora$ declare -a crew=(Ada Lin 'Mina Park')
ops@luna:~/aurora$ printf 'crew[2]=%s count=%d\n' "${crew[2]}" "${#crew[@]}"
crew[2]=Mina Park count=3
ops@luna:~/aurora$ declare -A limits=([temp]=80 [voltage]=32)
ops@luna:~/aurora$ printf 'thermal limit=%d°C\n' "${limits[temp]}"
thermal limit=80°C

ops@luna:~/aurora$ mapfile -t phases < <(printf '%s\n' launch cruise return)
ops@luna:~/aurora$ printf '<%s> ' "${phases[@]}"; printf '\n'
<launch> <cruise> <return>
ops@luna:~/aurora$ joined=$(IFS=,; echo "${phases[*]}"); echo "$joined"
launch,cruise,return
ops@luna:~/aurora$ printf -v stamp '%(%FT%TZ)T' -1; echo "${stamp%%T*}"
2026-07-12

ops@luna:~/aurora$ cat <<'MANIFEST'
vehicle=Asteria-7
payload=ion-imager
label=測試 🚀
MANIFEST
vehicle=Asteria-7
payload=ion-imager
label=測試 🚀
ops@luna:~/aurora$ read -r key value <<< 'mode science'
ops@luna:~/aurora$ printf '%s=%s\n' "$key" "$value"
mode=science
ops@luna:~/aurora$ printf '%b' 'line-one\nline-two\tΩ\n'
line-one
line-two	Ω
ops@luna:~/aurora$ printf '%s\n' $'escape:\u03bb' $'astral:\U0001F6F0'
escape:λ
astral:🛰

ops@luna:~/aurora$ sed -n '1,3p' config/flight.toml
[mission]
name = "Asteria"
orbit = 410
ops@luna:~/aurora$ awk -F= '/orbit/ { gsub(/ /, "", $2); print $2 + 10 }' config/flight.toml
420
ops@luna:~/aurora$ grep -E 'WARN|ERROR' logs/*.log | tail -n 2
logs/power.log:WARN battery reserve 31%
logs/thermal.log:WARN radiator Δ=4.2°C
ops@luna:~/aurora$ find telemetry -type f -name '*.csv' -print0 | xargs -0 -n1 basename
attitude.csv
power.csv
ops@luna:~/aurora$ cut -d, -f1 telemetry/power.csv | sort -u
battery
bus
solar

ops@luna:~/aurora$ command -v jq >/dev/null && jq -r '.state' state.json
nominal
ops@luna:~/aurora$ jq -n --arg craft "$VEHICLE" '{vehicle:$craft,ready:true,count:3}'
{
  "vehicle": "Asteria-7",
  "ready": true,
  "count": 3
}
ops@luna:~/aurora$ curl -fsS 'https://status.example.test/v1/health?site=luna' | jq -r '.message'
uplink nominal
ops@luna:~/aurora$ printf '%s\n' 'sha256  telemetry.bin' | read -r algorithm file
ops@luna:~/aurora$ echo "pipeline variables stay isolated: ${algorithm:-unset}"
pipeline variables stay isolated: unset

(tools) sh-5.2$ git status --short
 M config/flight.toml
?? telemetry/new-pass.csv
(tools) sh-5.2$ git diff --stat
 config/flight.toml | 4 ++--
 1 file changed, 2 insertions(+), 2 deletions(-)
(tools) sh-5.2$ git log -1 --pretty='format:%h %s'
7ac91e2 Tune guidance window
(tools) sh-5.2$ printf 'tag=%s\n' "$(git describe --tags --always)"
tag=v2.4.1-3-g7ac91e2

❯ coproc LINK { printf '%s\n' SYN ACK; }
❯ read -r first <&"${LINK[0]}"; echo "packet=$first"
packet=SYN
❯ wait "$LINK_PID"
❯ trap 'echo cleanup-complete' EXIT
❯ umask
0022
❯ ulimit -n
1024
❯ type -a printf | head -n 1
printf is a shell builtin

➜ printf 'BMP: %s %s %s\n' 'λ' 'Ж' '雪'
BMP: λ Ж 雪
➜ printf 'astral: %s %s\n' '🚀' '𐀀'
astral: 🚀 𐀀
➜ echo "nested=$(printf '%s' "$(echo orbit)")"
nested=orbit
➜ diff <(printf 'a\nb\n') <(printf 'a\nc\n') || true
2c2
< b
---
> c
➜ { printf '%s ' north east south west; echo; } | tr 'a-z' 'A-Z'
NORTH EAST SOUTH WEST
➜ false || printf '%s\n' 'recovered from expected probe failure'
recovered from expected probe failure

α printf 'Greek prompt separator reached: %s\n' 'ναι'
Greek prompt separator reached: ναι
% printf 'percent prompt and glob: %s\n' config/*.toml
percent prompt and glob: config/flight.toml
# id -u
0
# printf 'root check complete\n' >>/var/tmp/aurora-check.log
# tail -n 1 /var/tmp/aurora-check.log
root check complete

ops@luna:~/aurora$ jobs -l
ops@luna:~/aurora$ history 5 | sed 's/^ *[0-9]* *//'
printf 'BMP: %s %s %s\n' 'λ' 'Ж' '雪'
printf 'astral: %s %s\n' '🚀' '𐀀'
jobs -l
history 5 | sed 's/^ *[0-9]* *//'
ops@luna:~/aurora$ unset VEHICLE SITE status serial path crew limits phases joined
ops@luna:~/aurora$ printf 'session complete at %s\n' "$(date -u +%H:%M:%S)"
session complete at 08:21:43
ops@luna:~/aurora$ exit
cleanup-complete
logout
