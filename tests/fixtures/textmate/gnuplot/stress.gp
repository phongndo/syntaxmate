#!/usr/bin/env gnuplot
# Observatory telemetry report: café, naïve façade, 東京, λ, and 🛰️ 🚀 𝌆.
# This hand-written script intentionally includes legacy diagnostic probes.

reset session
set encoding utf8
set terminal svg size 1280,720 enhanced font "Noto Sans,11"
set output "telemetry-東京.svg"
set title "Aurora telemetry 🛰️"
set timestamp "%Y-%m-%d %H:%M UTC"
set datafile separator comma
set decimalsign locale
set key outside right top
set border 3
set grid xtics ytics mxtics mytics
set tics out
set mxtics 2
set mytics 2
set xlabel "elapsed time (s)"
set ylabel 'temperature (°C)'
set y2label "signal λ"
set y2tics
set xrange [0:*]
set yrange [*:*]
set y2range [-1:1]
set format x "%.1f"
set samples 240
set isosamples 40,40
set pointsize 0.8
set style line 1 lc rgb "#3366cc" lw 2 pt 7
set style line 2 lc rgb '#dc3912' lw 2 dt 2 pt 5
set style line 3 lc rgb "0x109618ff" lw 1 pt 9
set palette defined (0 "#081d58", 0.5 "#41b6c4", 1 "#ffffd9")
set colorbox vertical
set pm3d map
set view map

earth_radius = 6371.0
station_altitude = .125e1
sample_period = 5.
gain = 1E+3
mask = 0x2A
permissions = 0755
bad_octal_probe = 089
zero_probe = 000
width_cm = 12.5cm
height_in = 4in
missing = NaN
tau = 2*pi
ARGUMENT_COPY = ARG1
current_terminal = GPVAL_TERM
mouse_probe = MOUSE_X
legacy_limit = FIT_LIMIT
GPVAL_TERM = "forbidden assignment probe"

array channel_names[4] = ["north", "east", "zenith", "東京"]
array channel_color[4]
channel_color[1] = "#3366cc"
channel_color[2] = '#dc3912'
channel_color[3] = "#109618"
channel_color[4] = "#990099"

clamp(value, low, high) = value < low ? low : value > high ? high : value
wave(t, phase) = sin(t + phase) + 0.25*cos(3*t)
energy(x, y) = sqrt(abs(x*x + y*y))
label_for(index, name) = sprintf("%02d · %s", index, name)
legacy_defined(x) = defined(x)

report_header = "Telemetry: café λ 🚀\n"
quoted = 'pilot''s console'
interpolated = `printf "orbit-%02d" 7`
rgb_double = "#aabbcc"
rgb_alpha = "0x11223344"
old_arg = "legacy $1 and $#"
escaped = "tab\tquote\"slash\\"

$telemetry << TELEMETRY
# seconds,temp,signal,label
0,18.2,0.00,"東京"
5,18.8,0.48,"café"
10,19.4,0.84,"naïve"
15,20.1,1.00,"🛰️"
20,19.7,0.91,"𝌆"
25,19.0,0.60,"façade"
TELEMETRY

$calibration << CALDATA
0 0.00 "zero"
1 0.98 "nominal"
2 1.97 "high"
CALDATA trailing-diagnostic

stats $telemetry using 2 name "TEMP" nooutput
print report_header, TEMP_mean, TEMP_stddev
print gprintf("%8.3f", gamma(2.5)), strlen(quoted), words("a b c")
print columnhead(2), exists("tau"), value("earth_radius"), time(0)
print real({2,3}), imag({2,3}), int(3.9), floor(3.9), ceil(3.1)
print log10(100), exp(1), norm(1), rand(0), sgn(-2), lambertw(0,1)
print besj0(1), besy1(2), EllipticK(.5), ibeta(2,3,.5)
print hsv2rgb(0.5), strftime("%Y", time(0)), substr("orbit",2,3)

total = sum [i=1:4] i**2
average = sum [i=1:4] channel_color[i] ne "" ? i/4.0 : 0
flags = (mask << 2) | 0x03 & 0xff
logic = (total >= 20 && gain != 0) || !exists("never_defined")
comparison = total == 30 ? 1 : 0
bitwise = (~mask ^ 0x0f) >> 1
signed_value = -gain + +sample_period
remainder = total % 7
string_order = "alpha" eq "alpha" && "left" ne "right"
concatenated = "station" . "-07"

name_list = "north east zenith 東京"
do for [name in name_list] {
    print name, strlen(name)
}
do for [index=1:4:1] {
    channel_color[index] = index == 4 ? "#990099" : channel_color[index]
}

if (logic) {
    print "logic accepted", flags
} else {
    printerr "logic rejected"
}
countdown = 2
while (countdown > 0) {
    print "countdown", countdown
    countdown = countdown - 1
}

set macros
plot_style = "with linespoints ls 1"
@plot_style
set for [axis in "x y y2"] format axis "%g"
show all
show bind
show colornames
show functions
show plot
show variables
show version

plot for [channel=2:3] $telemetry using 1:(column(channel)) \
    with linespoints ls channel-1 title columnhead(channel), \
    $telemetry index 0 every 2 using 1:2 via gain with points title "sampled", \
    $calibration thru wave using 1:2 newhistogram "legacy" with lines

splot [x=-pi:pi] [y=-pi:pi] sin(sqrt(x*x+y*y)) with pm3d
fit wave(x, phase) $calibration using 1:2 via phase

import plugin_metric(x) from 'libtelemetry.so'
call "postprocess.gp" "telemetry-東京.svg"
eval "print sprintf('eval: %g', tau)"
save set "telemetry-settings.gp"
history 5
help plot
pwd

undefine TEMP_*
undefine GPVAL_TERM
reset bind
reset errorstate
reset session

# Deprecated compatibility probes are executable so their grammar scopes fire.
set ticslevel 0
set style increment userstyles
set pm3d hidden3d transparent solid
unset clabel
plot $calibration thru wave
update "legacy.dat"
clear unexpected-token
break stray

pause 0 "Rendering complete — résumé 🚀"
system "printf 'system function path\n'"
!printf "shell command path: 東京\n"
; print "leading separator"
print "continued"; print "second statement"
\orphaned_backslash_probe

set output
quit
