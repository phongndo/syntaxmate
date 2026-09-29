// Throughput of the C ABI and the C++ wrapper on the same workloads as
// native.rs (see there for the variant scheme). Run with
// `make -C bindings/c bench`; see README.md.
// Usage: bench SAMPLES MIN_MS LANGUAGE=PATH...
#include "syntaxmate.h"
#include "syntaxmate.hpp"

#include <algorithm>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

namespace {

const char THEME[] = "github-dark";

void ok(sm_status status) {
    if (status != SM_OK) {
        std::fprintf(stderr, "bench: %s\n", sm_last_error_message());
        std::exit(1);
    }
}

unsigned suffix_bits(std::size_t lines) {
    unsigned bits = 0;
    while (bits < 16 && (lines << bits) < 4096) ++bits;
    return bits;
}

std::vector<std::string> split(const std::string& text) {
    std::vector<std::string> out;
    std::size_t start = 0;
    for (;;) {
        std::size_t end = text.find('\n', start);
        out.push_back(text.substr(start, end == std::string::npos ? std::string::npos : end - start));
        if (end == std::string::npos) return out;
        start = end + 1;
    }
}

std::string variant(const std::string& source, std::size_t number, unsigned bits) {
    std::string suffix;
    for (unsigned bit = 0; bit < bits; ++bit) suffix += (number >> bit & 1) ? '\t' : ' ';
    std::string out;
    bool first = true;
    for (const std::string& line : split(source)) {
        if (!first) out += '\n';
        first = false;
        out += line;
        out += suffix;
    }
    return out;
}

struct test_case {
    std::string language;
    std::vector<std::string> variants;
    std::vector<std::vector<std::string>> lines;
};

// BENCH_MODES, if set, is a comma-separated list of modes to run.
bool selected(const char* mode) {
    const char* only = std::getenv("BENCH_MODES");
    if (!only || !*only) return true;
    std::string list = std::string(",") + only + ",";
    return list.find(std::string(",") + mode + ",") != std::string::npos;
}

template <class F>
double measure(int samples, long min_ms, F call) {
    using clock = std::chrono::steady_clock;
    std::vector<double> results;
    std::size_t index = 0;
    call(index);  // warm-up
    for (int s = 0; s < samples; ++s) {
        auto started = clock::now();
        long calls = 0;
        for (;;) {
            call(++index);
            ++calls;
            if (clock::now() - started >= std::chrono::milliseconds(min_ms)) break;
        }
        std::chrono::duration<double, std::nano> elapsed = clock::now() - started;
        results.push_back(elapsed.count() / static_cast<double>(calls));
    }
    std::sort(results.begin(), results.end());
    return results[results.size() / 2];
}

void print(const std::string& language, const char* mode, double ns, std::size_t bytes) {
    std::printf("%-12s %-17s %12.0f %9.1f\n", language.c_str(), mode, ns,
                static_cast<double>(bytes) / ns * 1e3);
}

template <class F>
void run(int samples, long min_ms, const std::string& language, const char* mode,
         std::size_t bytes, F call) {
    if (selected(mode)) print(language, mode, measure(samples, min_ms, call), bytes);
}

volatile std::size_t sink;

}  // namespace

int main(int argc, char** argv) {
    if (argc < 3) {
        std::fprintf(stderr, "usage: bench SAMPLES MIN_MS LANGUAGE=PATH...\n");
        return 2;
    }
    int samples = std::atoi(argv[1]);
    long min_ms = std::atol(argv[2]);
    std::vector<test_case> cases;
    for (int a = 3; a < argc; ++a) {
        const char* eq = std::strchr(argv[a], '=');
        if (!eq) return 2;
        std::ifstream file(eq + 1, std::ios::binary);
        std::stringstream buffer;
        buffer << file.rdbuf();
        std::string source = buffer.str();
        test_case c{std::string(argv[a], static_cast<std::size_t>(eq - argv[a])), {}, {}};
        unsigned bits = suffix_bits(split(source).size());
        for (std::size_t n = 0; n < (std::size_t{1} << bits); ++n) {
            c.variants.push_back(variant(source, n, bits));
            c.lines.push_back(split(c.variants.back()));
        }
        cases.push_back(std::move(c));
    }

    sm_engine* engine = nullptr;
    sm_theme* theme = nullptr;
    ok(sm_engine_bundled(&engine));
    ok(sm_theme_bundled(THEME, std::strlen(THEME), &theme));
    namespace sm = syntaxmate;
    sm::engine cpp_engine = sm::engine::bundled();
    sm::theme cpp_theme = sm::theme::bundled(THEME);
    sm_token_options utf8 = sm_token_options_default();
    sm_token_options scopes = utf8;
    scopes.include_scopes = true;

    auto html = [&](const sm_engine* e, const sm_theme* t, const std::string& language,
                    const std::string& source) {
        sm_string* out = nullptr;
        ok(sm_engine_html(e, language.data(), language.size(), source.data(), source.size(), t,
                          nullptr, &out, nullptr));
        sink = sm_string_len(out);
        sm_string_free(out);
    };
    auto tokens = [&](const std::string& language, const std::string& source,
                      const sm_token_options* options) {
        sm_tokens* out = nullptr;
        sm_token_view view;
        ok(sm_engine_tokens(engine, language.data(), language.size(), source.data(),
                            source.size(), theme, options, &out));
        ok(sm_tokens_view(out, &view));
        sink = view.token_count;
        sm_tokens_free(out);
    };

    std::printf("%-12s %-17s %12s %9s\n", "language", "mode", "ns/call", "MB/s");
    for (const test_case& c : cases) {
        const std::string& language = c.language;
        std::size_t n = c.variants.size();
        std::size_t bytes = c.variants[0].size();
        auto bench = [&](const char* mode, auto call) {
            run(samples, min_ms, language, mode, bytes, call);
        };
        bench("html-cold", [&](std::size_t) {
            sm_engine* fresh = nullptr;
            sm_theme* fresh_theme = nullptr;
            ok(sm_engine_bundled(&fresh));
            ok(sm_theme_bundled(THEME, std::strlen(THEME), &fresh_theme));
            html(fresh, fresh_theme, language, c.variants[0]);
            sm_theme_free(fresh_theme);
            sm_engine_free(fresh);
        });
        bench("html-steady", [&](std::size_t i) { html(engine, theme, language, c.variants[i % n]); });
        bench("html-replay", [&](std::size_t) { html(engine, theme, language, c.variants[0]); });
        bench("tokens-steady", [&](std::size_t i) { tokens(language, c.variants[i % n], &utf8); });
        bench("scopes-steady", [&](std::size_t i) { tokens(language, c.variants[i % n], &scopes); });
        sm_session* session = nullptr;
        ok(sm_engine_session(engine, language.data(), language.size(), theme, &utf8, &session));
        bench("session-steady", [&](std::size_t i) {
            ok(sm_session_reset(session));
            for (const std::string& line : c.lines[i % n]) {
                sm_tokens* out = nullptr;
                sm_token_view view;
                ok(sm_session_line(session, line.data(), line.size(), &out));
                ok(sm_tokens_view(out, &view));
                sink = view.token_count;
                sm_tokens_free(out);
            }
        });
        sm_session_free(session);
        bench("html-steady-c++", [&](std::size_t i) {
            sink = cpp_engine.html(language, c.variants[i % n], cpp_theme).size();
        });
        // Replay is the cheapest render, so the std::string copy shows most here.
        bench("html-replay-c++", [&](std::size_t) {
            sink = cpp_engine.html(language, c.variants[0], cpp_theme).size();
        });
        bench("tokens-steady-c++", [&](std::size_t i) {
            sink = cpp_engine.tokens(language, c.variants[i % n], cpp_theme).token_count();
        });
    }
    run(samples, min_ms, "rust", "html-tiny", 10,
        [&](std::size_t) { html(engine, theme, "rust", "let x = 1;"); });
    sm_theme_free(theme);
    sm_engine_free(engine);
    return 0;
}
