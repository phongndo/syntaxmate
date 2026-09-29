// Real C ABI and C++ wrapper consumer timings. Build with `make profile`.
// Each invocation emits one JSON sample. Digest/validation and source mutation
// are outside API timers; result destruction is measured separately.
#include "syntaxmate.hpp"

#include <chrono>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <iterator>
#include <memory>
#include <stdexcept>
#include <string>

namespace sm = syntaxmate;
using Clock = std::chrono::steady_clock;
using Duration = std::chrono::duration<double, std::milli>;
static double elapsed(Clock::time_point start) { return Duration(Clock::now() - start).count(); }
static void require(bool value, const char* message) {
    if (!value) throw std::runtime_error(message);
}
static void check(sm_status value) {
    if (value != SM_OK) throw std::runtime_error(sm_last_error_message());
}

struct Digest {
    std::uint64_t value = 14695981039346656037ull;
    std::size_t count = 0;
    void number(std::uint64_t n) {
        for (unsigned i = 0; i < 8; ++i) {
            value = (value ^ (n & 255)) * 1099511628211ull;
            n >>= 8;
        }
    }
    void text(std::string_view s) {
        number(s.size());
        count += s.size();
        for (unsigned char c : s) value = (value ^ c) * 1099511628211ull;
    }
    void style(sm_style s) { number(s.foreground); number(s.background); number(s.modifiers); }
    void tokens(const sm_tokens* tokens) {
        sm_token_view v{};
        check(sm_tokens_view(tokens, &v));
        require(v.complete, "degraded token output");
        number(v.unit); number(v.line_count); number(v.token_count);
        number(v.style_count); number(v.scope_stack_count); style(v.default_style);
        count += v.token_count;
        require(v.line_token_ranges[v.line_count] == v.token_count, "incomplete token arrays");
        for (std::size_t i = 0; i < v.line_count; ++i) number(v.line_starts[i]);
        for (std::size_t i = 0; i <= v.line_count; ++i) number(v.line_token_ranges[i]);
        for (std::size_t i = 0; i < v.token_count; ++i) {
            require(v.token_styles[i] < v.style_count, "invalid style index");
            number(v.token_starts[i]); number(v.token_lengths[i]); number(v.token_styles[i]);
            if (v.token_scopes) number(v.token_scopes[i]);
        }
        for (std::size_t i = 0; i < v.style_count; ++i) style(v.styles[i]);
        for (std::size_t i = 0; i < v.scope_stack_count; ++i) {
            const sm_str* names = nullptr;
            std::size_t depth = 0;
            check(sm_tokens_scope_stack(tokens, i, &names, &depth));
            number(depth);
            for (std::size_t j = 0; j < depth; ++j) text({names[j].ptr, names[j].len});
        }
    }
};

// Every line gets a different key on every call, even for tiny snippets.
// A fixed-width whitespace suffix avoids unbounded input growth. This mode is
// appropriate for grammars for which trailing horizontal whitespace is benign.
static std::string variant(const std::string& source, std::uint64_t iteration) {
    std::string result;
    std::uint64_t line = 0;
    auto append_suffix = [&] {
        require(line <= UINT32_MAX, "too many lines for unique suffixes");
        std::uint64_t key = (iteration << 32) | line++;
        for (unsigned bit = 0; bit < 64; ++bit) result += key & (1ull << bit) ? '\t' : ' ';
    };
    for (char c : source) {
        if (c == '\n') append_suffix();
        result += c;
    }
    append_suffix();
    return result;
}

struct Timings {
    double construct = 0, prepare = 0, api = 0, destroy = 0;
    std::size_t calls = 0, input_bytes = 0;
    Digest digest;
};

struct CConsumer {
    sm_engine* engine = nullptr;
    sm_theme* theme = nullptr;
    sm_session* session = nullptr;
    CConsumer() {
        check(sm_engine_bundled(&engine));
        try { check(sm_theme_bundled("github-dark", 11, &theme)); }
        catch (...) { sm_engine_free(engine); throw; }
    }
    ~CConsumer() { sm_session_free(session); sm_theme_free(theme); sm_engine_free(engine); }
    void prepare(std::string_view lang, bool scopes) {
        sm_token_options options{SM_OFFSET_UTF8, scopes};
        check(sm_engine_session(engine, lang.data(), lang.size(), theme, &options, &session));
    }
    void reset() { check(sm_session_reset(session)); }
    void run(const std::string& mode, std::string_view lang, std::string_view source, Timings& out) {
        auto begin = Clock::now();
        if (mode == "html" || mode == "ansi") {
            sm_string* raw = nullptr;
            bool complete = false;
            if (mode == "html") check(sm_engine_html(engine, lang.data(), lang.size(), source.data(), source.size(), theme, nullptr, &raw, &complete));
            else check(sm_engine_ansi(engine, lang.data(), lang.size(), source.data(), source.size(), theme, nullptr, &raw, &complete));
            out.api += elapsed(begin);
            std::unique_ptr<sm_string, decltype(&sm_string_free)> owner(raw, sm_string_free);
            require(complete, "degraded rendered output");
            out.digest.text({sm_string_data(raw), sm_string_len(raw)});
            begin = Clock::now(); owner.reset(); out.destroy += elapsed(begin);
        } else {
            sm_tokens* raw = nullptr;
            sm_token_options options{SM_OFFSET_UTF8, mode == "scopes"};
            if (session) check(sm_session_line(session, source.data(), source.size(), &raw));
            else check(sm_engine_tokens(engine, lang.data(), lang.size(), source.data(), source.size(), theme, &options, &raw));
            // C++ tokens construction calls sm_tokens_view once too.
            sm_token_view view{};
            check(sm_tokens_view(raw, &view));
            out.api += elapsed(begin);
            std::unique_ptr<sm_tokens, decltype(&sm_tokens_free)> owner(raw, sm_tokens_free);
            out.digest.tokens(raw);
            begin = Clock::now(); owner.reset(); out.destroy += elapsed(begin);
        }
        ++out.calls; out.input_bytes += source.size();
    }
};

struct CppConsumer {
    sm::engine engine = sm::engine::bundled();
    sm::theme theme = sm::theme::bundled("github-dark");
    std::optional<sm::session> session;
    void prepare(std::string_view lang, bool scopes) { session = engine.session(lang, theme, {sm::offset_unit::utf8, scopes}); }
    void reset() { session->reset(); }
    void run(const std::string& mode, std::string_view lang, std::string_view source, Timings& out) {
        auto begin = Clock::now();
        if (mode == "html" || mode == "ansi") {
            bool complete = false;
            std::optional<std::string> value;
            if (mode == "html") value = engine.html(lang, source, theme, {}, &complete);
            else value = engine.ansi(lang, source, theme, {}, &complete);
            out.api += elapsed(begin);
            require(complete, "degraded rendered output");
            out.digest.text(*value);
            begin = Clock::now(); value.reset(); out.destroy += elapsed(begin);
        } else {
            std::optional<sm::tokens> value;
            if (session) value = session->line(source);
            else value = engine.tokens(lang, source, theme, {sm::offset_unit::utf8, mode == "scopes"});
            out.api += elapsed(begin);
            out.digest.tokens(value->get());
            begin = Clock::now(); value.reset(); out.destroy += elapsed(begin);
        }
        ++out.calls; out.input_bytes += source.size();
    }
};

template<class Consumer>
static Timings run(const std::string& mode, const std::string& phase, const std::string& lang,
                   const std::string& source, std::size_t iterations) {
    Timings result;
    auto begin = Clock::now();
    Consumer consumer;
    result.construct = elapsed(begin);
    auto replay_session = [&](Timings& sample) {
        consumer.reset();
        std::size_t start = 0;
        do {
            auto end = source.find('\n', start);
            consumer.run(mode, lang, std::string_view(source).substr(start, end == std::string::npos ? end : end - start), sample);
            if (end == std::string::npos) break;
            start = end + 1;
        } while (true);
    };
    if (phase == "session") {
        require(mode == "tokens" || mode == "scopes", "session mode requires tokens or scopes");
        begin = Clock::now(); consumer.prepare(lang, mode == "scopes"); result.prepare = elapsed(begin);
        // Warm matching and session caches without counting the first document.
        Timings discarded;
        replay_session(discarded);
    } else if (phase != "first") {
        Timings discarded;
        consumer.run(mode, lang, source, discarded);
    }
    for (std::size_t i = 0; i < iterations; ++i) {
        if (phase == "session") {
            replay_session(result);
        } else {
            auto input = phase == "steady" ? variant(source, i + 1) : source;
            consumer.run(mode, lang, input, result);
        }
    }
    return result;
}

int main(int argc, char** argv) {
    try {
        require(argc == 7, "usage: profile <c|cpp> <html|ansi|tokens|scopes> <first|steady|replay|session> <language> <source-file> <iterations>");
        const std::string api = argv[1], mode = argv[2], phase = argv[3], lang = argv[4];
        require(api == "c" || api == "cpp", "unknown API");
        require(mode == "html" || mode == "ansi" || mode == "tokens" || mode == "scopes", "unknown output mode");
        require(phase == "first" || phase == "steady" || phase == "replay" || phase == "session", "unknown phase");
        std::size_t iterations = std::stoull(argv[6]);
        require(iterations > 0 && iterations <= UINT32_MAX && (phase != "first" || iterations == 1), "first requires one iteration; other phases require 1..UINT32_MAX iterations");
        std::ifstream file(argv[5], std::ios::binary);
        require(file.good(), "cannot read input");
        std::string source{std::istreambuf_iterator<char>(file), {}};
        Timings result = api == "c" ? run<CConsumer>(mode, phase, lang, source, iterations)
                                    : run<CppConsumer>(mode, phase, lang, source, iterations);
        std::cout << std::setprecision(12) << "{\"constructMs\":" << result.construct
                  << ",\"prepareMs\":" << result.prepare << ",\"apiMs\":" << result.api
                  << ",\"destroyMs\":" << result.destroy << ",\"calls\":" << result.calls
                  << ",\"inputBytes\":" << result.input_bytes << ",\"outputCount\":" << result.digest.count
                  << ",\"digest\":\"" << std::hex << result.digest.value << "\",\"complete\":true}\n";
        return 0;
    } catch (const std::exception& e) { std::cerr << e.what() << '\n'; return 1; }
}
