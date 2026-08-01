module fixture::atlas;

import std::io;
import std::math;

alias console = module std::io;
typedef uint ObjectId;
alias CompareFn = fn int(int left, int right);

faultdef MISSING_POINT, INVALID_SCALE, OUT_OF_RANGE;

const int MAX_POINTS = 16;
const double GOLDEN_RATIO = 1.61803398875;
const uint PERMISSION_MASK = 0x01_FF_u32;
const char NEWLINE = '\n';

enum Direction : uint {
    NORTH,
    EAST,
    SOUTH,
    WEST,
}

struct Point {
    double x;
    double y;
    String label;
}

struct Route {
    ObjectId id;
    Point[16] points;
    usz count;
    bool active;
}

union Number {
    long integer;
    double real;
}

bitstruct Flags : uint {
    bool visible : 0;
    bool selected : 1;
    uint priority : 2..4;
}

<*
 Returns the squared distance from the origin.
 @param point: point to inspect
 @return? point.x * point.x + point.y * point.y
*>
fn double magnitude_squared(Point point) @pure @inline {
    return point.x * point.x + point.y * point.y;
}

fn int compare_ints(int left, int right) @pure {
    if (left < right) return -1;
    if (left > right) return 1;
    return 0;
}

macro int squared(int value) {
    return value * value;
}

fn Direction rotate(Direction direction) {
    switch (direction) {
        case NORTH: return EAST;
        case EAST: return SOUTH;
        case SOUTH: return WEST;
        default: return NORTH;
    }
}

fn void describe(Direction direction) {
    switch (direction) {
        case NORTH:
        case SOUTH:
            console::printn("vertical");
        case EAST, WEST:
            console::printn("horizontal");
    }
}

fn int sum_slice(int[] values) {
    int total = 0;
    foreach (index, value : values) {
        total += value + (int)index;
    }
    return total;
}

fn void scale_slice(double[] values, double factor) {
    foreach (&value : values) {
        *value *= factor;
    }
}

fn int classify(int value) {
    int result = 0;
    for (int bit = 0; bit < 8; bit++) {
        if ((value & (1 << bit)) == 0) continue;
        result += bit;
        if (result > 12) break;
    }
    return result;
}

fn int countdown(int start) {
    int value = start;
    do {
        value--;
    } while (value > 3);
    while (value > 0) {
        --value;
    }
    return value;
}

fn void log_route(Route* route) {
    defer console::printn("route complete");
    if (route == null || !route.active) {
        console::printn("inactive");
        return;
    }
    foreach (index, point : route.points[0..route.count]) {
        console::printfn("%d: %s %.2f", index, point.label,
            magnitude_squared(point));
    }
}

fn int apply(CompareFn operation, int left, int right) {
    return operation(left, right);
}

fn void literal_examples() {
    // Safe Unicode samples: café, λ, 東京, music 𝄞, rocket 🚀.
    String escaped = "line one\nline two\t\"quoted\"";
    String raw = `C:\atlas\routes and ``quoted`` text`;
    char marker = '\u03bb';
    uint binary = 0b1010_0110;
    uint octal = 0o755;
    ulong large = 9_223_372_036_854_775_000UL;
    double decimal = 6.022_140_76e23;
    double hexadecimal = 0x1.ffp+8;
    char[] bytes = x"CA FE 00 42";
    char[] encoded = b64"QzMgZml4dHVyZQ==";
    console::printfn("%s %s %c %x %o %lu %e %a %s %s",
        escaped, raw, marker, binary, octal, large, decimal,
        hexadecimal, bytes, encoded);
}

fn void comment_examples() {
    /* A conventional block comment covers punctuation: { [ ( ) ] }. */
    /* Outer comment begins.
       /* C3 permits a nested block comment. */
       Outer comment resumes with Ελληνικά and हिन्दी.
    */
    console::printn("comments closed");
}

$if $defined(ENABLE_ATLAS):
const String BUILD_MODE = "enabled";
$else
const String BUILD_MODE = "review";
$endif

fn void main() @export {
    Point origin = { 0.0, 0.0, "origin" };
    Point beacon = { 3.0, 4.0, "東京 🚀" };
    Route route = {
        .id = 0x2A,
        .points = { origin, beacon },
        .count = 2,
        .active = true,
    };
    int[5] samples = { 1, 2, 3, 5, 8 };
    double[3] weights = { 0.5, 1.25, 2.0 };
    CompareFn comparator = &compare_ints;
    int order = apply(comparator, samples[0], samples[4]);
    scale_slice(weights, GOLDEN_RATIO);
    log_route(&route);
    describe(rotate(NORTH));
    console::printfn("%s: sum=%d order=%d square=%d",
        BUILD_MODE, sum_slice(samples), order, squared(6));
    literal_examples();
    comment_examples();
}
