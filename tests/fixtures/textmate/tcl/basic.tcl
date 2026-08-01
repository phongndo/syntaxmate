#!/usr/bin/env tclsh
# Compact Tcl grammar coverage with BMP λ/東京 and astral 🚀/𝌆.
namespace eval ::fixture {
    variable greeting "Hello"
    variable labels [list λ 東京 🚀 𝌆]
}

proc ::fixture::welcome {name {punctuation "!"}} {
    set decorated "${::fixture::greeting}, $name$punctuation"
    append decorated " — [join $::fixture::labels { / }]"
    return $decorated
}

set score(ada) 42
set ratio [expr {($score(ada) + 6) / 2.0}]
if {$ratio >= 20 && $score(ada) != 0} {
    puts [::fixture::welcome "Ada λ 🚀"]
} elseif {$ratio < 0} {
    error "unexpected negative value"
} else {
    puts stderr "ratio=$ratio"
}

set normalized [string map [list "  " - " " -] {Tcl  8.6  𝌆}]
puts [format "%s | %#x" $normalized 255]
set regexpCommand regexp
$regexpCommand -nocase -- {^(hello|東京)\s+(.+)$} {Hello Tcl} whole first rest
puts "regex=$whole first=$first rest=$rest"
