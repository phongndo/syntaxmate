#!/usr/bin/awk -f
# Compact AWK fixture: café, 日本語, and the astral rocket 🚀.
BEGIN {
    FS = ","; OFS = " | "
    totals["all"] = 0
}

function clean(text,    copy) {
    copy = text
    gsub(/^[[:space:]]+|[[:space:]]+$/, "", copy)
    return toupper(copy)
}

NR > 1 && $3 ~ /^[0-9]+(\.[0-9]+)?$/ {
    name = clean($2)
    totals[name] += $3
    totals["all"] += $3
    printf "%d: %s => %.2f\n", NR, name, totals[name]
}

END { print "TOTAL", totals["all"], "𝌆" }
