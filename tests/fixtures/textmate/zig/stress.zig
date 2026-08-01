//! Orbital telemetry decoder used as a broad Zig grammar fixture.
//! It models a small ground-station pipeline, not a list of token samples.
//! Unicode station note: Tromsø, 東京, and the relay 🛰️ are online.

const std = @import("std");
const builtin = @import("builtin");
const Allocator = std.mem.Allocator;
pub const protocol_version: u16 = 0x02_1a;
const sync_word: [4:0]u8 = .{ 0x53, 0x41, 0x54, 0x01 };
const default_port = 0o17_520;
const feature_mask = 0b1010_0110;
const light_speed_km_s: f64 = 2.997_924_58e5;
const circular_ratio: f64 = 0x1.921f_b6p+1;
const no_data: u8 = 255;
threadlocal var packets_seen: usize = 0;
comptime {
    @setEvalBranchQuota(20_000);
    if (@sizeOf(Header) != 8) @compileError("wire header must remain eight bytes");
}
const usage =
    \\orbit-monitor --device PATH [--verbose]
    \\  Decodes café-station telemetry and prints a compact report.
    \\  Escape examples in ordinary strings: \"quote\", tab=\t, lambda=\u{03bb}.
;
/// Errors that can cross the packet-decoding boundary.
pub const DecodeError = error{
    BadSync,
    Truncated,
    InvalidKind,
    InvalidUtf8,
    ChecksumMismatch,
};
pub const LinkState = enum(u8) {
    acquiring = 1,
    locked = 2,
    degraded = 0xff,

    pub fn isUsable(self: LinkState) bool {
        return self == .locked or self == .degraded;
    }
};
const Flags = packed struct(u8) {
    encrypted: bool,
    compressed: bool,
    priority: u2,
    reserved: u4 = 0,
};
const Header = extern struct {
    kind: u8,
    flags: Flags,
    sequence: u16,
    payload_len: u32 align(4),
};
const Sample = union(enum) {
    temperature: f32,
    voltage_mv: u16,
    state: LinkState,
    message: []const u8,
};
const DriverToken = opaque {};
const CDescriptor = extern struct {
    name: [*:0]const u8,
    context: ?*anyopaque,
    registers: [*c]volatile u32,
};
extern fn station_open(desc: *const CDescriptor) callconv(.C) ?*DriverToken;
extern fn station_close(token: *DriverToken) callconv(.C) void;
fn RingBuffer(comptime T: type, comptime capacity: usize) type {
    return struct {
        const Self = @This();
        items: [capacity]T = undefined,
        read_index: usize = 0,
        write_index: usize = 0,
        full: bool = false,
        fn push(self: *Self, value: T) ?T {
            const displaced = if (self.full) self.items[self.write_index] else null;
            self.items[self.write_index] = value;
            self.write_index = (self.write_index + 1) % capacity;
            if (self.full) self.read_index = self.write_index;
            self.full = self.write_index == self.read_index;
            return displaced;
        }
        fn pop(self: *Self) ?T {
            if (!self.full and self.read_index == self.write_index) return null;
            const value = self.items[self.read_index];
            self.read_index = (self.read_index + 1) % capacity;
            self.full = false;
            return value;
        }
    };
}
const SampleQueue = RingBuffer(Sample, 16);
fn readInteger(comptime T: type, bytes: []const u8) DecodeError!T {
    if (bytes.len < @sizeOf(T)) return error.Truncated;
    return std.mem.readInt(T, bytes[0..@sizeOf(T)], .little);
}
fn checksum(bytes: []const u8) u16 {
    var sum: u16 = 0;
    for (bytes, 0..) |byte, index| {
        sum +%= @as(u16, byte) ^ @as(u16, @truncate(index));
    }
    return sum;
}
fn parseHeader(packet: []const u8) DecodeError!Header {
    if (packet.len < sync_word.len + @sizeOf(Header)) return error.Truncated;
    if (!std.mem.eql(u8, packet[0..sync_word.len], &sync_word)) return error.BadSync;
    const wire = packet[sync_word.len..][0..@sizeOf(Header)];
    return @bitCast(wire.*);
}
fn decodeSample(allocator: Allocator, header: Header, payload: []const u8) !Sample {
    errdefer std.log.err("discarding sequence {d}", .{header.sequence});
    return switch (header.kind) {
        1 => .{ .temperature = @bitCast(try readInteger(u32, payload)) },
        2 => .{ .voltage_mv = try readInteger(u16, payload) },
        3 => .{ .state = std.meta.intToEnum(LinkState, payload[0]) catch error.InvalidKind },
        4 => message: {
            if (!std.unicode.utf8ValidateSlice(payload)) return error.InvalidUtf8;
            break :message .{ .message = try allocator.dupe(u8, payload) };
        },
        else => error.InvalidKind,
    };
}
fn describe(sample: Sample, writer: anytype) !void {
    switch (sample) {
        .temperature => |celsius| try writer.print("temperature={d:.2}°C", .{celsius}),
        .voltage_mv => |mv| try writer.print("supply={d}mV", .{mv}),
        .state => |state| try writer.print("link={s}", .{@tagName(state)}),
        .message => |text| try writer.print("message=\"{s}\"", .{text}),
    }
}
fn consumeFields(line: []const u8) usize {
    var fields = std.mem.splitScalar(u8, line, ',');
    var accepted: usize = 0;
    while (fields.next()) |field| : (accepted += 1) {
        if (field.len == 0) continue;
        if (field[0] == '#') break;
    }
    return accepted;
}
fn calibrated(raw: ?f64, gain: f64) f64 {
    const value = raw orelse return -0.0;
    return if (value >= 0.0 and gain != 0.0) value * gain else 0.0;
}
noinline fn copyPayload(noalias dest: []u8, source: []const u8) void {
    const count = @min(dest.len, source.len);
    @memcpy(dest[0..count], source[0..count]);
    if (count < dest.len) @memset(dest[count..], no_data);
}
inline fn clamp(comptime T: type, value: T, low: T, high: T) T {
    return @max(low, @min(value, high));
}
fn processPacket(allocator: Allocator, queue: *SampleQueue, packet: []const u8) !void {
    const header = try parseHeader(packet);
    const start = sync_word.len + @sizeOf(Header);
    const end = start + header.payload_len;
    if (end + 2 > packet.len) return error.Truncated;
    const expected = try readInteger(u16, packet[end..]);
    if (checksum(packet[0..end]) != expected) return error.ChecksumMismatch;
    const sample = try decodeSample(allocator, header, packet[start..end]);
    if (queue.push(sample)) |old| {
        if (old == .message) allocator.free(old.message);
    }
    packets_seen += 1;
}
fn drain(queue: *SampleQueue, writer: anytype) !void {
    while (queue.pop()) |sample| {
        defer if (sample == .message) std.heap.page_allocator.free(sample.message);
        try describe(sample, writer);
        try writer.writeByte('\n');
    } else {
        std.log.debug("queue drained", .{});
    }
}
const Reactor = struct {
    waiter: ?anyframe->void = null,
    fn wait(self: *Reactor) void {
        suspend {
            self.waiter = @frame();
        }
    }
    fn wake(self: *Reactor) void {
        if (self.waiter) |frame| resume frame;
        self.waiter = null;
    }
};
fn exerciseAsync(reactor: *Reactor) void {
    const frame = async reactor.wait();
    reactor.wake();
    nosuspend resume frame;
}
export fn telemetry_register(base: usize) callconv(.C) *allowzero volatile u32 {
    return @ptrFromInt(base);
}
pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    var queue = SampleQueue{};
    const stdout = std.io.getStdOut().writer();
    const greeting: [:0]const u8 = "λ station 🛰️ ready";
    const markers = [_]u21{ 'A', '\n', '\x7f', '\u{03bb}', '界' };
    _ = .{ builtin.mode, usage, greeting, markers, circular_ratio, light_speed_km_s };
    const demo = [_]u8{ 0x53, 0x41, 0x54, 0x01, 2, 0, 7, 0, 2, 0, 0, 0, 0xe4, 0x0c };
    processPacket(arena.allocator(), &queue, &demo) catch |err| switch (err) {
        error.BadSync, error.ChecksumMismatch => std.log.warn("radio noise: {s}", .{@errorName(err)}),
        else => return err,
    };
    try drain(&queue, stdout);
}
test "ring buffer preserves insertion order" {
    var queue = RingBuffer(u8, 3){};
    try std.testing.expect(queue.push(10) == null);
    try std.testing.expect(queue.push(20) == null);
    try std.testing.expectEqual(@as(?u8, 10), queue.pop());
}
test "numeric helpers and optional fallback" {
    try std.testing.expectEqual(@as(f64, 6.0), calibrated(3.0, 2.0));
    try std.testing.expectEqual(@as(f64, -0.0), calibrated(null, 4.0));
    try std.testing.expectEqual(@as(u8, 9), clamp(u8, 12, 1, 9));
    try std.testing.expect(consumeFields("azimuth,elevation,#note") == 2);
}
