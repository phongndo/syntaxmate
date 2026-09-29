// Checks that the C++ wrapper frees C allocations when copying them out
// throws. Replaces global operator new so the Nth allocation throws
// std::bad_alloc; a leak shows up under LeakSanitizer. Valgrind substitutes
// its own operator new, so the test skips itself there.
#include "syntaxmate.hpp"

#include <cstdlib>
#include <iostream>
#include <new>

namespace sm = syntaxmate;

// 0 disables injection; otherwise the allocation that brings it to 0 throws.
static long countdown = 0;
static long injected = 0;

void* operator new(std::size_t size) {
    if (countdown > 0 && --countdown == 0) {
        ++injected;
        throw std::bad_alloc();
    }
    if (void* p = std::malloc(size ? size : 1)) return p;
    throw std::bad_alloc();
}
void operator delete(void* p) noexcept { std::free(p); }
void operator delete(void* p, std::size_t) noexcept { std::free(p); }

// Runs `call` with the 1st, 2nd, ... allocation failing until it succeeds.
// Returns the number of failures injected.
template <class F>
static long exhaust(F call) {
    long before = injected;
    for (long n = 1;; ++n) {
        countdown = n;
        try {
            call();
            countdown = 0;
            return injected - before;
        } catch (const std::bad_alloc&) {
        }
    }
}

static bool injection_works() {
    countdown = 1;
    try {
        ::operator delete(::operator new(1));
    } catch (const std::bad_alloc&) {
        return true;
    }
    countdown = 0;
    return false;
}

int main() {
    if (!injection_works()) {
        std::cout << "C++ allocation-failure tests skipped: operator new is not replaceable here\n";
        return 0;
    }
    int failures = 0;
    auto expect_injected = [&](const char* name, long count) {
        if (count == 0) {
            std::cerr << name << ": no allocation failure was injected\n";
            ++failures;
        }
    };
    try {
        sm::engine engine = sm::engine::bundled();
        sm::theme theme = sm::theme::bundled("github-dark");
        // sm_string -> std::string (long enough to defeat the small-string buffer).
        expect_injected("html", exhaust([&] { engine.html("rust", "fn main() {}", theme); }));
        expect_injected("stylesheet", exhaust([&] { theme.stylesheet("sm-"); }));
        // sm_string_list -> std::vector<std::string>.
        expect_injected("languages", exhaust([&] { engine.languages(); }));
        expect_injected("themes", exhaust([&] { engine.themes(); }));
    } catch (const std::exception& e) {
        std::cerr << "uncaught exception: " << e.what() << "\n";
        return 1;
    }
    if (failures) return 1;
    std::cout << "C++ allocation-failure tests passed\n";
    return 0;
}
