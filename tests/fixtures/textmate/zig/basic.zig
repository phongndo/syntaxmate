const std = @import("std");

const Reading = struct {
    label: []const u8,
    value: f64,
};

/// Return null rather than divide an empty slice; Unicode: café 🚀 𝌆.
fn average(values: []const f64) ?f64 {
    if (values.len == 0) return null;
    var total: f64 = 0;
    for (values) |value| {
        total += value;
    }
    return total / @as(f64, @floatFromInt(values.len));
}

pub fn main() !void {
    const readings = [_]Reading{
        .{ .label = "café 🚀", .value = 0x1.5p+3 },
        .{ .label = "orbit 𝌆", .value = 2.5e1 },
    };
    const values = [_]f64{ readings[0].value, readings[1].value };
    const mean = average(&values) orelse return error.EmptyReadings;
    std.debug.print("{s}: {d:.2}\n", .{ readings[0].label, mean });
}
