#!/usr/bin/env fish
# Small Fish sample: café, λ, 水, and the astral swimmer 🐟.
set -l name (string join ' ' -- $argv)
set -l colors red green blue
set -l banner "Hello, 世界 🚀
from a multiline string"

function greet --description 'print a friendly greeting' --argument-names who
    set -l loud (string upper -- $who)
    if string match -rq '^[[:alpha:] _-]+$' -- $who
        printf '%s: %s (%s)\n' "$banner" $loud "$colors[1..2]"
    else
        echo 'single-quoted path: C:\temp\fish' 2>errors.log
    end
end

for subject in $name[1] guest
    greet "$subject"
end

set -l joined (string join ,
    $colors)
echo "joined=$joined; pid=$fish_pid" | string replace -a blue azure
printf '%s\n' escaped\ space \x46\u03bb\U0001F41F
