dev@luna:~/aurora$ export MISSION="Artemis λ"
dev@luna:~/aurora$ printf 'mission=%s\n' "$MISSION"
mission=Artemis λ
dev@luna:~/aurora$ count=$((2 + 3)); echo "count=$count"
count=5
dev@luna:~/aurora$ printf '%s\n' alpha beta | sort -r
beta
alpha
(venv) sh-5.2$ if [[ -n "$MISSION" ]]; then
> printf 'ready: %s 🚀\n' "$MISSION"
> else
> echo 'not ready' >&2
> fi
ready: Artemis λ 🚀
dev@luna:~/aurora$ payload=$(printf '%s' '雪'); echo "payload=$payload"
payload=雪
dev@luna:~/aurora$ printf 'astral=%s\n' '𐀀'
astral=𐀀
dev@luna:~/aurora$ unset payload
dev@luna:~/aurora$ exit
logout
