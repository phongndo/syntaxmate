#!/usr/bin/env tclsh
# Broad, hand-written Tcl TextMate stress fixture.
# Unicode includes BMP café, λ, 東京 and astral 🚀, 𝌆, 😀.
package require Tcl 8.6
namespace eval ::mission {
    variable version 3
    variable title "λ launch from 東京 🚀"
    variable glyphs [list café λ 東京 🚀 𝌆 😀]
    variable config
    array set config {
        retries 3
        timeout 1250
        enabled 1
    }
}

proc ::mission::identity {value} {
    return $value
}
proc ::mission::banner {{prefix "Mission"}} {
    variable title
    set suffix [join $::mission::glyphs { · }]
    return "$prefix: $title — $suffix"
}

proc ::mission::numericExamples {} {
    set decimal 42
    set signed -17
    set leading +8
    set fraction 3.14159
    set exponent 6.02e23
    set small .125
    set flags [expr {($decimal << 2) | 3}]
    set mask [expr {$flags & 255}]
    set logical [expr {$decimal >= 40 && $signed != 0}]
    set power [expr {2 ** 8}]
    set remainder [expr {$decimal % 5}]
    return [list $decimal $signed $leading $fraction $exponent $small \
        $flags $mask $logical $power $remainder]
}

proc ::mission::escapeExamples {} {
    set escapes "tab=\t newline=\n quote=\" slash=\\"
    set codepoints "lambda=\u03bb rocket=🚀 hex=\x41 octal=\101"
    set braced {literal $value [command] with {nested braces}}
    set joined "first line \
        continued line"
    return [list $escapes $codepoints $braced $joined]
}

proc ::mission::variableExamples {name} {
    variable config
    set local "Ada"
    set values($name) "array item"
    set direct $local
    set explicit ${local}
    set indexed $values($name)
    set qualified $::mission::version
    set dynamic $config(retries)
    return "$direct|$explicit|$indexed|$qualified|$dynamic"
}

proc ::mission::classify {value} {
    if {$value eq ""} {
        return empty
    } elseif {[string is integer -strict $value]} then {
        return integer
    } elseif {[string is double -strict $value]} {
        return real
    } else {
        return text
    }
}

proc ::mission::countdown {start} {
    set events {}
    while {$start > 0} {
        lappend events $start
        incr start -1
        if {$start == 2} {
            continue
        }
    }
    lappend events launch
    return $events
}

proc ::mission::forSquares {limit} {
    set result {}
    for {set i 0} {$i < $limit} {incr i} {
        lappend result [expr {$i * $i}]
    }
    return $result
}

proc ::mission::mapNames {names} {
    set mapped {}
    foreach name $names index [lrange {10 11 12 13 14 15} 0 end] {
        lappend mapped [format "%02d:%s" $index [string totitle $name]]
    }
    return $mapped
}

proc ::mission::chooseRoute {route} {
    switch -exact -- $route {
        ground { return "walk" }
        orbit  { return "fly 🚀" }
        東京   { return "train" }
        default { return "wait" }
    }
}

proc ::mission::safeDivide {numerator denominator} {
    try {
        if {$denominator == 0} {
            throw {MATH DIVZERO} "division by zero"
        }
        return [expr {$numerator / double($denominator)}]
    } trap {MATH DIVZERO} {message options} {
        return -options $options "caught: $message"
    } on error {message options} {
        return [dict create status error message $message options $options]
    } finally {
        set ::mission::lastAttempt [clock milliseconds]
    }
}

proc ::mission::regexExamples {text} {
    set captures {}
    set regexpCommand regexp
    set regsubCommand regsub
    $regexpCommand -indices -nocase -- {\m(λ|東京|tcl)\M} $text span word
    if {[info exists span]} {
        lappend captures $span $word
    }
    $regexpCommand -- {^(Mission):\s+(.+)$} $text all heading detail
    $regsubCommand -all -- {([[:space:]]+)} $text {_} compact
    $regsubCommand -nocase -- {rocket|🚀} $compact {flight} replaced
    return [list $captures $all $heading $detail $compact $replaced]
}

proc ::mission::listExamples {} {
    set data [list alpha "beta gamma" {delta epsilon} λ 東京 🚀]
    lappend data tail
    set data [linsert $data 1 inserted]
    set data [lreplace $data 2 2 replacement]
    lset data 0 ALPHA
    set middle [lrange $data 1 end-1]
    set found [lsearch -glob $data *place*]
    set sorted [lsort -dictionary $data]
    return [concat $middle [list $found] $sorted]
}

proc ::mission::stringExamples {value} {
    set trimmed [string trim $value]
    set upper [string toupper $trimmed]
    set lower [string tolower $upper]
    set mapped [string map [list λ lambda 東京 Tokyo] $lower]
    set range [string range $mapped 0 12]
    set match [string match -nocase *tcl* $mapped]
    return [format {range=%s match=%d length=%d} \
        $range $match [string length $mapped]]
}

proc ::mission::arrayExamples {} {
    array set inventory [list fuel 90 payload "science 𝌆" crew 4]
    set exists [array exists inventory]
    set names [array names inventory *]
    set count [array size inventory]
    unset inventory(crew)
    return [list $exists $names $count [parray inventory]]
}

proc ::mission::dictionaryExamples {} {
    set record [dict create id 7 city 東京 active true]
    dict set record payload "λ sensor"
    dict lappend record tags science Tcl 🚀
    dict incr record visits
    set city [dict get $record city]
    set hasId [dict exists $record id]
    dict for {key value} $record {
        set summary($key) [string length $value]
    }
    return [list $record $city $hasId [array get summary]]
}

proc ::mission::fileExamples {root} {
    set child [file join $root data "東京.txt"]
    set extension [file extension $child]
    set tail [file tail $child]
    set normalized [file normalize $root]
    set pattern [file join $root *.tcl]
    set matches [glob -nocomplain -- $pattern]
    return [list $child $extension $tail $normalized $matches]
}

proc ::mission::channelExample {path} {
    set channel [open $path w]
    fconfigure $channel -encoding utf-8 -translation lf
    puts $channel [::mission::banner]
    flush $channel
    set offset [tell $channel]
    close $channel
    return $offset
}

proc ::mission::callback {script value} {
    uplevel 1 [list {*}$script $value]
}

proc ::mission::scopedUpdate {variableName} {
    upvar 1 $variableName target
    if {![info exists target]} {
        set target 0
    }
    incr target
    return $target
}

set names {ada grace katherine λ 東京}
set report [dict create \
    banner [::mission::banner "Status"] \
    numbers [::mission::numericExamples] \
    escapes [::mission::escapeExamples] \
    variables [::mission::variableExamples ada] \
    classes [lmap item {{} 42 3.5 Tcl} {::mission::classify $item}] \
    countdown [::mission::countdown 5] \
    squares [::mission::forSquares 6] \
    names [::mission::mapNames $names] \
    route [::mission::chooseRoute orbit] \
    division [::mission::safeDivide 10 0]]

set transformed [subst {title=$::mission::title; version=$::mission::version}]
set lambda [apply {{x y} {expr {$x + $y}}} 20 22]
set code [catch {error "sample failure 𝌆"} message options]
after 1 [list set ::mission::timerFired 1]
update
vwait ::mission::timerFired
puts [::mission::regexExamples "Mission: Tcl λ 東京 rocket 🚀"]
puts [::mission::stringExamples "  Tcl λ 東京 🚀  "]
puts [list $report $transformed $lambda $code $message]

# A command-boundary comment covers punctuation; escaped newline follows. \
# this remains readable continuation text for Tcl's lexical rules
if {[info exists options]} { unset options }
rename ::mission::identity ::mission::echo
puts [::mission::echo "done café λ 東京 🚀 𝌆 😀"]
