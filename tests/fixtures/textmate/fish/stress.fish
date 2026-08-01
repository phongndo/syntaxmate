#!/usr/bin/env fish
# Fish grammar stress fixture: BMP café, Ω, 水; astral 🐠, 🚀, and 𝄞.
# Hashes after variables are comments, while an escaped \# stays an argument.

set -g project_name mark
set -l roots src tests assets
set -l numbers 1 2 3 5 8
set -l empty
set -l greeting "hello 世界 🐟"
set -l literal 'single quotes keep $project_name and (commands) literal'
set -l multiline_double "first line with $project_name
second line with a quote: \" and rocket 🚀"
set -l multiline_single 'first literal line
second literal line: café and 𝄞'

# Variable classes, list slices, and interpolation inside strings.
echo $project_name $argv $status $pipestatus $fish_pid $FISH_VERSION
echo $roots[1] $roots[2..-1] $numbers[-1] $numbers[1..3]
echo "root=$roots[1]; tail=$roots[2..-1]; argv=$argv[1]"
set -l copied $roots
set copied[2] integration-tests
set -q empty; or set empty fallback

# Bare and dollar-prefixed command-substitution states from the vendored grammar.
set -l basename (string replace -r '^.*/' '' -- (pwd))
set -l nested (string upper -- (string join '-' alpha beta))
set -l listing (
    printf '%s\n' $roots
    string join ',' one two three
)
set -l alternate $(string join ':' left right)
echo "$basename $nested $listing $alternate"

# Backslash escapes recognized outside quotes and the limited double-quote set.
printf '%s\n' plain\ word quote\" dollar\$ hash\# semi\; star\*
printf '%s\n' \x46 \X4a \101 \u03bb \U0001F680 \cG
echo "double: \"quote\", \\slash, \$dollar"
echo 'single: \'quote\' and \\ slash and \`tick\`'
echo line\
continued

# Function declaration options and locally scoped setup.
function describe_path --description 'classify one filesystem path' --argument-names target
    set -l kind missing
    if test -d $target
        set kind directory
    else if test -f $target
        set kind file
    else if test -L $target
        set kind symlink
    else
        set kind absent
    end

    printf '%-12s %s\n' $kind $target
    return 0
end

function collect_options -d 'exercise argparse-style options'
    argparse 'h/help' 'v/verbose' 'n/name=' 't/tag=+' -- $argv
    or return 2

    if set -q _flag_help
        echo 'usage: collect_options [-v] --name VALUE [PATH...]'
        return
    end

    set -l chosen default
    if set -q _flag_name
        set chosen $_flag_name
    end
    set -l tag_count (count $_flag_tag)
    printf 'name=%s tags=%d rest=%d\n' $chosen $tag_count (count $argv)
end

# A for-loop with continue, break, globs, and command options.
for root in $roots
    if test $root = assets
        echo "skipping $root"
        continue
    end
    describe_path ./$root
    command find ./$root -maxdepth 1 -type f 2>/dev/null | string match -r '\.(fish|rs)$'
    if test $status -ne 0
        break
    end
end

# While conditions can launch commands and combine status operators.
set -l index 1
while test $index -le (count $numbers)
    set -l value $numbers[$index]
    if test (math "$value % 2") -eq 0
        echo "$value is even"
    else
        echo "$value is odd"
    end
    set index (math $index + 1)
end

# Logical keywords and symbolic operators occur at command boundaries.
test -n "$greeting"; and echo present
test -z "$empty"; or echo "empty was replaced: $empty"
not contains -- forbidden $roots; and echo allowed
test -d src && echo source-exists
false || echo recovered
printf '%s\n' alpha beta | string match -q beta; and echo piped

# Switch uses Fish wildcard patterns; string match supplies regex-like arguments.
set -l candidate feature_42
switch $candidate
    case 'feature_*'
        echo 'feature branch'
    case 'release-?' 'hotfix-*'
        echo 'maintenance branch'
    case '*'
        echo 'other branch'
end

if string match -rq '^[[:alpha:]_][[:alnum:]_-]*$' -- $candidate
    string replace -r '^feature_' 'feat/' -- $candidate
else
    printf 'invalid name: %s\n' $candidate >&2
end

# A begin/end block keeps redirections attached to the compound command.
begin
    echo "project=$project_name"
    echo "cwd="(pwd)
    printf 'roots=%s\n' (string join , $roots)
end >summary.txt 2>>errors.log

# Current and legacy redirection forms are both explicit in this grammar.
printf 'overwrite\n' >output.txt
printf 'append\n' >>output.txt
string collect <output.txt
echo diagnostic 2>diagnostic.log
echo merged 2>&1 | string collect
echo old-stderr ^legacy-error.log
echo old-append ^^legacy-error.log

# Builtins and common commands with short and long options.
set --show project_name
set -lx LC_ALL C
string length -- "$multiline_double"
string split -m 1 ':' 'key:value:tail'
string trim --chars=' .' '  dotted... '
string escape --style=var 'fish value'
math --scale=2 '22 / 7'
printf '%04d %s\n' 7 completed
type -q git; and command git --version
builtin contains -i -- src $roots
count $roots $numbers

# read, source, wait, and exit are parsed as commands/keywords by the grammar.
printf 'Ada Lovelace\n' | read -l first last
echo "$last, $first"
if test -r ./optional.fish
    source ./optional.fish
end
sleep 0.01 &
set -l sleeper $last_pid
wait $sleeper

# Wildcard operators and a background pipeline in plausible command positions.
for script in scripts/*.fish scripts/setup?.fish
    test -e $script; or continue
    echo "script: $script"
end
printf '%s\n' background | string collect >/dev/null &

# Nested multiline substitutions keep every parenthesis and quote balanced.
set -l report (string join ' | ' \
    (string upper -- $project_name) \
    (string join ',' \
        $roots[1..2]) \
    "status=$status")
echo $report

# Semicolon-separated commands exercise command restart boundaries.
echo one; echo two; printf '%s\n' three
if true; echo inline-condition; end
begin; echo compact-block; end

# Final function call options remain ordinary arguments after the command name.
collect_options --verbose --name 'fixture user' --tag syntax --tag fish src tests
describe_path ./README.md

# All multiline strings, substitutions, functions, loops, switches, and blocks close.
echo "done: $project_name — λ — 🐡"
