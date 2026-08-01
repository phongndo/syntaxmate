#!/usr/bin/env rdmd
module fixtures.stress;

import std.algorithm : map, filter;
import std.array : array;
static import std.math;
import io = std.stdio : writeln, writefln;
alias Size = size_t;
alias Callback = int delegate(int value);

// D grammar stress fixture: café λ 東京 雪 🚀 𝌆
/* Ordinary block comment with operators ++ -- /= ~= and delimiters {}[]. */
/+ Nesting comment level one.
   /+ Nested level two contains "quotes", café, and 🚀. +/
   Back at level one. +/

version = SyntaxFixture;
debug = TraceFixture;
version (SyntaxFixture) enum configured = true;
else enum configured = false;
debug (TraceFixture) enum traced = true;
static if (Size.sizeof >= 4) enum wideSize = true;

extern(C) int c_entry(int code);
extern(C++, fixture) void cpp_entry();
align(16) struct AlignedRecord {
    long first;
    long second;
}

deprecated("legacy API") alias LegacySize = uint;
pragma(msg, "fixture compile message")
@safe @nogc nothrow pure int attributed(int value) => value + 1;
@("route", 7) struct UserAttributed {
    int payload;
}

enum Color : ubyte { red = 1, green, blue = 0xFF }
enum Answer = 42;
class Base {
    protected string label;
    this(string label) { this.label = label; }
    invariant() { assert(label.length > 0); }
}

interface Printable {
    string render() const;
}

final class Document : Base, Printable {
    private immutable int id;
    this(int id, string label) { super(label); this.id = id; }
    this(this) { label = label.dup; }
    ~this() { }
    override string render() const { return label; }
}

struct Point(T) {
    T x;
    T y;
}

union Bits {
    uint integer;
    float decimal;
}

template Pair(A, B) {
    alias Pair = A[B];
}

mixin template AddReset() {
    void reset() { value = 0; }
}

class Counter {
    mixin AddReset;
    int value;
    shared static this() { }
    shared static ~this() { }
}

bool flag = true;
bool absent = false;
Object nothing = null;
byte signedByte = -12;
ubyte unsignedByte = 250U;
short small = 32_000;
ushort usmall = 65_000U;
int decimal = 1_234_567;
uint hexadecimal = 0xDEAD_BEEFu;
long binary = 0b1010_0110L;
ulong huge = 18_446_744UL;
float fraction = .625f;
double exponent = 6.022e+23;
real precise = 0x1.Fp+4L;
ifloat imaginaryFloat = 2.5fi;
cdouble complexValue = 1.0 + 2.0i;

char newline = '\n';
wchar lambda = '\u03BB';
dchar rocket = '\U0001F680';
string escaped = "quote=\" slash=\\ tab=\t octal=\101 hex=\x42 unicode=\u6771";
wstring wide = "café λ 東京 雪 🚀 𝌆"w;
dstring decoded = "astral 🚀 and tetragram 𝌆"d;
auto raw = r"C:\fixture\new\path\東京";
auto backtick = `raw "quotes" and backslashes \ stay literal`;
auto bytes = x"00 01 7f DE AD BE EF";
auto bracketed = q"[bracket [text] café]";
auto parenthesized = q"(parenthesized (text) 東京)";
auto angled = q"<angled <text> 🚀>";
auto braced = q"{braced {text} 𝌆}";
auto tagged = q"END
Tagged multiline text keeps punctuation {[(<>)]}.
The selected terminator must agree with the begin capture.
END";
auto tokens = q{
    immutable int nested = (1 + 2) * 3;
    if (nested > 0) { writeln("token string"); }
};
auto interpolation = i"value=$(decimal), city=$(raw), sum=$(1 + 2)";
auto interpolationRaw = i`flag=$(flag), glyph=雪`;
auto interpolationTokens = iq{writeln($(decimal + 1));};

int arithmetic(int left, int right) {
    int result = left + right * 2 - 3 / 1 % 2;
    result += 4;
    result -= 1;
    result *= 2;
    result /= 3;
    result %= 5;
    result <<= 1;
    result >>= 1;
    result >>>= 1;
    result &= 0xFF;
    result |= 0x10;
    result ^= 0x01;
    result ^^= 2;
    return result;
}

bool comparisons(int a, int b, int[] values) {
    auto order = a < b && b >= 0 || a == b;
    auto identity = values is null || values !is null;
    auto membership = a in [1: true, 2: false];
    auto relation = a !<> b;
    return order ? !identity : membership !is null;
}

auto square = (int value) => value * value;
Callback increment = delegate int(int value) { return value + 1; };
auto inferred = (value) { return value + 2; };

int inspect(T)(T input) {
    alias Input = typeof(input);
    enum integral = is(T : long);
    auto info = typeid(T);
    auto memberNames = __traits(allMembers, T);
    static assert(__traits(compiles, input));
    assert(memberNames.length >= 0, "traits");
    return cast(int) input;
}

void controlFlow(int[] values) {
outer:
    for (int i = 0; i < values.length; ++i) {
        if (values[i] < 0) continue;
        else if (values[i] == 99) break outer;
        values[i] = arithmetic(values[i], i);
    }

    foreach (index, value; values) writeln(index, value);
    foreach_reverse (value; values) writeln(value);
    static foreach (index; 0 .. 3) writeln(index);
    static foreach_reverse (index; 0 .. 3) writeln(index);

    int cursor = 0;
    while (cursor < values.length) ++cursor;
    do --cursor; while (cursor > 0);

    final switch (values.length) {
        case 0: goto case;
        case 1: writeln("short"); break;
        default: goto done;
    }

done:
    with (values) writeln(length);
    synchronized (values) values ~= 1;
}

void guarded() {
    scope(exit) writeln("exit");
    scope(success) writeln("success");
    scope(failure) writeln("failure");
    try {
        throw new Exception("boom 🚀");
    } catch (Exception error) {
        writeln(error.msg);
    } finally {
        writeln("finally");
    }
}

int compileTime(string source) {
    mixin("int generated = 40 + 2;");
    auto imported = import("fixture.txt");
    return generated + cast(int) imported.length;
}

unittest {
    assert(square(6) == 36);
    assert(arithmetic(4, 2) >= 0);
}

version (X86_64) {
    void assemblySample() {
        asm {
            naked;
            MOV RAX, RBX;
            ADD EAX, 4;
            XOR ECX, ECX;
        }
    }
}

void main() {
    auto points = [Point!double(1.0, 2.0), Point!double(3.0, 4.0)];
    auto transformed = points.map!(point => point.x + point.y).filter!(sum => sum > 0).array;
    auto café = new Document(7, "東京 café 🚀 𝌆");
    io.writefln("%s %s", café.render(), transformed);
    guarded();
}

# line 240 "virtual_fixture.d"
__EOF__
