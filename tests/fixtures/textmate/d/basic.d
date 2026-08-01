#!/usr/bin/env rdmd
module fixtures.basic;

import std.stdio : writeln;
alias Count = ulong;

// Compact D coverage: café 東京 🚀 𝌆
@safe struct Greeting {
    string name;
    Count count;
}

pure nothrow string render(const Greeting value) {
    auto escaped = "hello\nλ: " ~ value.name;
    auto raw = r"C:\fixtures\東京";
    auto token = q{count + 0x2A + 0b1010};
    double ratio = 6.25e-1;
    return escaped ~ raw ~ token;
}

void main() {
    auto café = Greeting("Ada 🚀 𝌆", 3UL);
    foreach (index; 0 .. café.count) writeln(index, render(café));
}
