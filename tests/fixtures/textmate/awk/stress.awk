#!/usr/bin/env -S gawk -f
# AWK grammar stress fixture: naïve café, Ελληνικά, 日本語, 🚀, and 𝌆.
# It is a small report program covering patterns, actions, regexps, and built-ins.

BEGIN {
    FS = "[[:space:]]*,[[:space:]]*"
    OFS = "\t"
    ORS = "\n"
    SUBSEP = "::"
    CONVFMT = "%.6g"
    OFMT = "%.2f"
    IGNORECASE = 1
    report_title = "Mission ledger 🚀"
    output_file = ENVIRON["AWK_REPORT"]
    if (output_file == "") output_file = "/tmp/awk-fixture-report.txt"
    srand(17)
    print report_title, strftime("%Y-%m-%d")
    print "arguments", ARGC, ARGV[0]
}

BEGINFILE {
    file_rows = 0
    file_total = 0
    if (ERRNO != "") {
        print "cannot read", FILENAME, ERRNO > "/dev/stderr"
        nextfile
    }
}

function trim(text,    result) {
    result = text
    gsub(/^[[:space:]]+/, "", result)
    gsub(/[[:space:]]+$/, "", result)
    return result
}

function normalize(text) {
    text = trim(text)
    sub(/[[:space:]]+/, " ", text)
    return tolower(text)
}

function clamp(value, low, high) {
    if (value < low) return low
    else if (value > high) return high
    return value
}

function classify(score) {
    return score >= 90 ? "excellent" : score >= 70 ? "ready" : "review"
}

function remember(owner, state, amount,    key) {
    key = owner SUBSEP state
    totals[key] += amount
    counts[key]++
    owners[owner] = 1
    states[state] = 1
    return totals[key]
}

function quoted(text) {
    gsub(/\\/, "\\\\", text)
    gsub(/"/, "\\\"", text)
    return "\"" text "\""
}

/^#/ { next }
/^[[:space:]]*$/ { next }

FNR == 1 && $1 ~ /^(id|mission)$/ {
    for (column = 1; column <= NF; column++) header[normalize($column)] = column
    next
}

$0 !~ /,[[:space:]]*(queued|active|complete)[[:space:]]*,/ {
    warnings[FILENAME]++
    print "unexpected state at", FNR > "/dev/stderr"
    next
}

{
    file_rows++
    id = trim($1)
    owner = trim($2)
    state = normalize($3)
    amount = $4 + 0
    note = trim($5)

    if (owner == "") owner = "anonymous"
    if (amount < 0) {
        negative[id] = amount
        amount = -amount
    }

    score = clamp(amount, 0, 100)
    bucket = classify(score)
    remember(owner, state, amount)
    file_total += amount
    grand_total += amount
    seen[id]++

    words = split(note, parts, /[[:space:]]+/)
    for (word = 1; word <= words; word++) {
        token = normalize(parts[word])
        if (token != "") vocabulary[token]++
    }

    if (match(note, /(café|rocket|日本語)/, found)) {
        highlights[id] = found[0]
    }

    decorated = gensub(/([[:alpha:]]+)/, "[&]", 1, note)
    initials = substr(owner, 1, 1)
    position = index(tolower(note), "urgent")
    logarithm = amount > 0 ? log(amount) : 0
    magnitude = sqrt(amount * amount)
    wave = sin(NR) + cos(FNR) + atan2(amount, NR)
    random_sample = int(rand() * 10)
    scientific = exp(logarithm)

    if (state == "complete") completed++
    else if (state == "active") active++
    else queued++

    printf "%-12s %-9s %8.2f %s\n", owner, state, amount, decorated
    printf "  id=%s bucket=%s words=%d first=%s pos=%d sample=%d\n", \
        id, bucket, length(parts), initials, position, random_sample
}

state == "active" && amount >= 50 {
    active_ids[id] = sprintf("%s:%.1f", owner, amount)
}

id in seen && seen[id] > 1 {
    duplicates[id]++
}

ENDFILE {
    printf "file %s: rows=%d total=%.2f\n", FILENAME, file_rows, file_total
    if (warnings[FILENAME]) printf "file warnings: %d\n", warnings[FILENAME]
    fflush()
}

END {
    print "\n== owner/state totals =="
    for (owner in owners) {
        owner_total = 0
        for (state in states) {
            key = owner SUBSEP state
            if (key in totals) {
                average = totals[key] / counts[key]
                printf "%s %-9s total=%8.2f avg=%6.2f\n", owner, state, totals[key], average
                owner_total += totals[key]
            }
        }
        printf "%s combined=%8.2f\n", owner, owner_total
    }

    print "\n== highlights =="
    for (id in highlights) print id, quoted(highlights[id])
    for (id in active_ids) print "active", id, active_ids[id]
    for (id in duplicates) print "duplicate", id, duplicates[id]

    common = 0
    for (token in vocabulary) {
        if (vocabulary[token] > common) {
            common = vocabulary[token]
            common_word = token
        }
    }

    summary = sprintf("rows=%d total=%.2f complete=%d", NR, grand_total, completed)
    print summary
    print "states", queued, active, completed
    print "most common", common_word, common
    print "numeric formats", 42, 3.1415, 6.02e+23, 7.5e-3
    print "escapes", "quote=\" slash=\\ tab=\t newline=\n octal=\101 hex=\x42"
    print "unicode", "café", "λ", "雪", "🛰️", "𝌆"

    command = "printf '%s' " quoted(summary)
    command | getline command_echo
    close(command)
    print "command", command_echo

    do {
        attempts++
        if (attempts == 1) continue
        break
    } while (attempts < 3)

    for (index = 1; index <= external_count; index++) {
        if (index % 2 == 0) delete external[index]
    }

    printf "%s\n", summary >> output_file
    status = system("true")
    if (status != 0) exit status
}
