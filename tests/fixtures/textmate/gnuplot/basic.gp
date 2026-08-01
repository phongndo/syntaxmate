# Compact Gnuplot fixture: café, 東京, and rocket 🚀.
set encoding utf8
set title "Orbital café 🚀"
set xlabel 'time (s)'
set ylabel "signal λ"
set xrange [0:2*pi]
samples = 80
wave(x, phase) = sin(x + phase) + 0.2*cos(3*x)
array colors[3] = ["#3366cc", "#dc3912", "#109618"]
$points << EOD
0.0 0.0 "東京"
1.0 0.84 "café"
2.0 0.91 "🚀"
EOD
set key outside
plot for [phase=0:2] $points using 1:(wave($1, phase)) \
    with linespoints title sprintf("phase %d", phase)
if (GPVAL_TERM eq "unknown") print "terminal?"
print sum [i=1:3] i**2, " done"
!printf "rendered basic plot\n"
