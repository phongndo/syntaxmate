use <threads.scad>
include <dimensions.scad>
/** Compact OpenSCAD fixture.
 * Unicode BMP: café, λ, 東京; astral: 🚀, 𝌆.
 */
/* A regular block comment with { [ ( punctuation. */
$fn = 48;
label = "bracket café \"λ\" 🚀\n";
function clamp_value(value, low, high) = min(max(value, low), high);
module rounded_box(size = [30, 20, 8], radius = 2) {
    color("steelblue") minkowski() {
        cube(size - [2 * radius, 2 * radius, 2 * radius], center = true);
        sphere(r = radius);
    }
}
// Boolean body with transforms, numbers, operators, and delimiters.
difference() {
    translate([0, 0, 4]) rounded_box();
    for (x = [-10, 0, 10]) rotate([90, 0, 0]) cylinder(h = 30, r = 1.5, center = true);
}
if ($preview && true) echo(str(label, ": ", clamp_value(12.5, 0, 10)));
else render(convexity = 4) mirror([1, 0, 0]) scale([1, 1, 0.5]) rounded_box();
