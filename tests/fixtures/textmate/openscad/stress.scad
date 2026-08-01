use <gears/involute.scad>
include <hardware/fasteners.scad>
/**
 * Parametric observatory assembly stress fixture.
 * BMP text: naïve façade, Ω, Ж, 雪, 東京.
 * Astral text: 🛰️, 🚀, 🔭, 𝌆.
 */
/*
   This ordinary block comment deliberately spans lines.
   Braces { }, brackets [ ], parentheses ( ), and "quotes" stay comments.
*/

$fn = 72;
$fa = 4;
$fs = 0.4;
epsilon = 0.01;
golden_ratio = 1.618;
hex_probe = 0x2A;
enabled = true;
maintenance_mode = false;
title = "Observatory 雪原 \"Aurora\" 🚀\nDeck";
single_quoted_probe = 'legacy\x41\101\n';
palette = ["midnightblue", "silver", "orange", "white"];
radii = [18, 24.5, 31, 42];
heights = [3, 6, 9, 12];
identity4 = [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
points2d = [[-8, -5], [8, -5], [11, 0], [8, 5], [-8, 5], [-11, 0]];
faces3d = [[0, 1, 2], [0, 2, 3], [0, 3, 4], [0, 4, 1]];

function clamp_value(value, low = 0, high = 1) = min(max(value, low), high);
function lerp(a, b, t) = a + (b - a) * clamp_value(t);
function polar(radius, angle) = [radius * cos(angle), radius * sin(angle)];
function ring_points(radius, count) = [for (index = [0 : count - 1]) polar(radius, 360 * index / count)];
function factorial(n) = n <= 1 ? 1 : n * factorial(n - 1);
function safe_log(value) = value > 0 ? log(value) : 0;
function metric(value) = abs(value) + ceil(value) + floor(value) + round(value);
function trig(angle) = acos(cos(angle)) + asun(sin(angle)) + atan2(sin(angle), cos(angle));
function powers(value) = exp(value) + ln(max(value, 1)) + pow(value, 2) + sqrt(abs(value));
function sampled(index) = lookup(index, [[0, 1], [1, 4], [2, 9]]);
function bounded_randoms(seed) = rands(0, 1, 4, seed);
function signed_wave(angle) = sign(sin(angle)) * tan(angle / 4);
function caption(index) = str(title, " #", index);

module washer(outer = 8, inner = 4, thickness = 1) {
    difference() {
        cylinder(h = thickness, r = outer / 2, center = true);
        cylinder(h = thickness + 2 * epsilon, r = inner / 2, center = true);
    }
}

module rounded_plate(size = [60, 42, 3], corner = 4) {
    minkowski() {
        cube([size[0] - 2 * corner, size[1] - 2 * corner, size[2]], center = true);
        cylinder(h = epsilon, r = corner, center = true);
    }
}

module vent_slot(length = 18, width = 3, depth = 5) {
    hull() {
        translate([-length / 2, 0, 0]) cylinder(h = depth, r = width / 2, center = true);
        translate([length / 2, 0, 0]) cylinder(h = depth, r = width / 2, center = true);
    }
}

module radial_vents(count = 12, radius = 22) {
    for (angle = [0 : 360 / count : 359]) {
        rotate([0, 0, angle])
            translate([radius, 0, 0])
                rotate([0, 0, 90]) vent_slot();
    }
}

module lattice_panel(width = 48, depth = 30, bars = 6) {
    intersection() {
        cube([width, depth, 2], center = true);
        union() {
            for (offset = [-bars : bars]) {
                translate([offset * 5, 0, 0]) rotate([0, 0, 35]) cube([2, 80, 4], center = true);
                translate([offset * 5, 0, 0]) rotate([0, 0, -35]) cube([2, 80, 4], center = true);
            }
        }
    }
}

module telescope_tube(length = 70, radius = 9) {
    color(palette[1]) difference() {
        rotate([0, 90, 0]) cylinder(h = length, r = radius, center = true);
        rotate([0, 90, 0]) cylinder(h = length + 1, r = radius - 1.2, center = true);
    }
    color(palette[2]) translate([length / 2, 0, 0]) rotate([0, 90, 0]) washer(22, 17, 3);
    color(palette[0]) translate([-length / 2, 0, 0]) rotate([0, 90, 0]) cylinder(h = 4, r1 = 11, r2 = 8);
}

module fork_arm(side = 1) {
    translate([0, side * 18, 22]) {
        hull() {
            cube([12, 6, 34], center = true);
            translate([0, 0, 18]) rotate([90, 0, 0]) cylinder(h = 6, r = 8, center = true);
        }
    }
}

module pier(height = 38) {
    color("gray") union() {
        cylinder(h = height, r1 = 13, r2 = 10);
        translate([0, 0, height]) cylinder(h = 4, r = 15);
        for (angle = [0 : 120 : 359])
            rotate([0, 0, angle]) translate([10, 0, 2]) cube([18, 4, 4], center = true);
    }
}

module dome_shell(radius = 42, wall = 1.5) {
    difference() {
        intersection() {
            sphere(r = radius);
            translate([0, 0, radius / 2]) cube([2 * radius, 2 * radius, radius], center = true);
        }
        sphere(r = radius - wall);
        translate([0, 0, radius]) cube([12, 2 * radius, radius], center = true);
    }
}

module poly_marker(scale_factor = 1) {
    scale([scale_factor, scale_factor, scale_factor])
        polyhedron(
            points = [[0, 0, 5], [-4, -4, 0], [4, -4, 0], [4, 4, 0], [-4, 4, 0]],
            faces = faces3d,
            convexity = 6
        );
}

module transformed_marker(position = [0, 0, 0]) {
    translate(position)
        mirror([1, 0, 0])
            multimatrix(identity4)
                color([0.9, 0.6, 0.1, 0.8]) poly_marker(0.7);
}

module bolt_circle(count = 8, radius = 25) {
    intersection_for(angle = [0 : 360 / count : 359]) {
        rotate([0, 0, angle]) translate([radius, 0, 0]) cylinder(h = 8, r = 2, center = true);
    }
}

module equipment_deck(show_lattice = true) {
    difference() {
        rounded_plate();
        radial_vents(10, 18);
        for (x = [-22, 22], y = [-14, 14])
            translate([x, y, 0]) cylinder(h = 8, r = 1.8, center = true);
    }
    if (show_lattice) translate([0, 0, 4]) lattice_panel();
    else translate([0, 0, 4]) cube([48, 30, 2], center = true);
}

module instrument_mount(tilt = 25) {
    union() {
        fork_arm(-1);
        fork_arm(1);
        translate([0, 0, 42]) rotate([0, tilt, 0]) telescope_tube();
    }
}

module observatory(exploded = 0) {
    color(palette[0]) equipment_deck(!maintenance_mode);
    translate([0, 0, 4 + exploded]) pier();
    translate([0, 0, 42 + 2 * exploded]) instrument_mount(lerp(15, 55, 0.35));
    translate([0, 0, 5 + 3 * exploded]) color([0.7, 0.8, 1, 0.35]) dome_shell();
    for (angle = [45, 135, 225, 315])
        rotate([0, 0, angle]) transformed_marker([34, 0, 4]);
}

// Deprecated assignment remains a useful keyword and delimiter probe.
assign(sample = metric(2.75), wave = signed_wave(30)) {
    echo("metrics", sample, wave, trig(30), powers(2));
}

// Exercise render, nested booleans, comparison, logical, and modulo operators.
if (enabled && !maintenance_mode && (factorial(5) >= 120)) {
    render(convexity = 10) difference() {
        observatory($preview ? 8 : 0);
        if ((hex_probe % 2) == 0) translate([0, -50, 30]) cube([8, 100, 60], center = true);
    }
} else {
    color("red") cube([10, 10, 10], center = true);
}

// Final calls cover echo strings, decimals, periods, commas, and brackets.
echo(caption(7), safe_log(100), sampled(2), bounded_randoms(8675309));
echo("Unicode payload: café Ω 雪 🛰️ 𝌆", golden_ratio, radii[1]);
