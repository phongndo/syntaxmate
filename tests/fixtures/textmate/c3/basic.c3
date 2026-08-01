module fixture::basic;

import std::io;

struct Greeting {
    String label;
    int count;
}

fn int doubled(int value) @pure {
    return value * 2;
}

fn void main() {
    // Unicode text includes café, 東京, and the astral rocket 🚀.
    Greeting greeting = { "C3 café 🚀", 3 };
    int total = 0;
    for (int i = 0; i < greeting.count; i++) {
        total += doubled(i);
    }
    if (total > 0) {
        io::printfn("%s: %d", greeting.label, total);
    }
}
