// Exercises the C++ wrapper and checks every conformance case against
// bindings/conformance/expected.json. Usage: test_cpp <conformance-dir>
#include "syntaxmate.hpp"

#include <algorithm>
#include <cstdint>
#include <cstdlib>
#include <fstream>
#include <iostream>
#include <optional>
#include <sstream>
#include <string>
#include <thread>
#include <type_traits>
#include <vector>

namespace sm = syntaxmate;

static int failures = 0;

#define CHECK(cond)                                                                       \
    do {                                                                                  \
        if (!(cond)) {                                                                    \
            std::cerr << __FILE__ << ":" << __LINE__ << ": CHECK failed: " #cond "\n";    \
            ++failures;                                                                   \
        }                                                                                 \
    } while (0)

// ---------------------------------------------------------------------------
// Minimal JSON reader: enough for the conformance fixtures (objects, arrays,
// strings with escapes, integers, booleans, null). Keeps each value's raw text.

struct json {
    enum kind_t { null, boolean, integer, string, array, object } kind = null;
    bool b = false;
    std::int64_t n = 0;
    std::string s;
    std::vector<json> items;
    std::vector<std::pair<std::string, json>> members;
    std::string raw;

    const json& operator[](const std::string& key) const {
        for (const auto& [name, value] : members) {
            if (name == key) return value;
        }
        throw std::runtime_error("missing JSON key " + key);
    }
    const json& operator[](std::size_t i) const { return items.at(i); }
};

class json_parser {
public:
    explicit json_parser(const std::string& text) : text_(text) {}

    json parse() {
        json value = parse_value();
        skip_ws();
        if (pos_ != text_.size()) fail("trailing data");
        return value;
    }

private:
    [[noreturn]] void fail(const std::string& what) {
        throw std::runtime_error("JSON parse error at " + std::to_string(pos_) + ": " + what);
    }
    void skip_ws() {
        while (pos_ < text_.size() && std::string(" \t\r\n").find(text_[pos_]) != std::string::npos) ++pos_;
    }
    bool eat(char c) {
        skip_ws();
        if (pos_ < text_.size() && text_[pos_] == c) {
            ++pos_;
            return true;
        }
        return false;
    }
    void expect(char c) {
        if (!eat(c)) fail(std::string("expected ") + c);
    }
    bool literal(const char* word) {
        std::string w(word);
        if (text_.compare(pos_, w.size(), w) == 0) {
            pos_ += w.size();
            return true;
        }
        return false;
    }

    static void append_utf8(std::string& out, std::uint32_t cp) {
        if (cp < 0x80) {
            out += static_cast<char>(cp);
        } else if (cp < 0x800) {
            out += static_cast<char>(0xC0 | (cp >> 6));
            out += static_cast<char>(0x80 | (cp & 0x3F));
        } else if (cp < 0x10000) {
            out += static_cast<char>(0xE0 | (cp >> 12));
            out += static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
            out += static_cast<char>(0x80 | (cp & 0x3F));
        } else {
            out += static_cast<char>(0xF0 | (cp >> 18));
            out += static_cast<char>(0x80 | ((cp >> 12) & 0x3F));
            out += static_cast<char>(0x80 | ((cp >> 6) & 0x3F));
            out += static_cast<char>(0x80 | (cp & 0x3F));
        }
    }

    std::uint32_t hex4() {
        if (pos_ + 4 > text_.size()) fail("short \\u escape");
        std::uint32_t value = std::stoul(text_.substr(pos_, 4), nullptr, 16);
        pos_ += 4;
        return value;
    }

    std::string parse_string() {
        expect('"');
        std::string out;
        while (true) {
            if (pos_ >= text_.size()) fail("unterminated string");
            char c = text_[pos_++];
            if (c == '"') return out;
            if (c != '\\') {
                out += c;
                continue;
            }
            char e = text_[pos_++];
            switch (e) {
                case '"': out += '"'; break;
                case '\\': out += '\\'; break;
                case '/': out += '/'; break;
                case 'b': out += '\b'; break;
                case 'f': out += '\f'; break;
                case 'n': out += '\n'; break;
                case 'r': out += '\r'; break;
                case 't': out += '\t'; break;
                case 'u': {
                    std::uint32_t cp = hex4();
                    if (cp >= 0xD800 && cp < 0xDC00 && literal("\\u")) {
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (hex4() - 0xDC00);
                    }
                    append_utf8(out, cp);
                    break;
                }
                default: fail("bad escape");
            }
        }
    }

    json parse_value() {
        skip_ws();
        std::size_t start = pos_;
        json value;
        if (pos_ >= text_.size()) fail("unexpected end");
        char c = text_[pos_];
        if (c == '{') {
            value.kind = json::object;
            ++pos_;
            if (!eat('}')) {
                do {
                    skip_ws();
                    std::string key = parse_string();
                    expect(':');
                    value.members.emplace_back(std::move(key), parse_value());
                } while (eat(','));
                expect('}');
            }
        } else if (c == '[') {
            value.kind = json::array;
            ++pos_;
            if (!eat(']')) {
                do value.items.push_back(parse_value());
                while (eat(','));
                expect(']');
            }
        } else if (c == '"') {
            value.kind = json::string;
            value.s = parse_string();
        } else if (literal("true")) {
            value.kind = json::boolean;
            value.b = true;
        } else if (literal("false")) {
            value.kind = json::boolean;
        } else if (literal("null")) {
            value.kind = json::null;
        } else {
            value.kind = json::integer;
            std::size_t used = 0;
            value.n = std::stoll(text_.substr(pos_, 24), &used);
            pos_ += used;
        }
        value.raw = text_.substr(start, pos_ - start);
        return value;
    }

    const std::string& text_;
    std::size_t pos_ = 0;
};

static json read_json(const std::string& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) throw std::runtime_error("cannot open " + path);
    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string text = buffer.str();
    return json_parser(text).parse();
}

// ---------------------------------------------------------------------------
// Conformance

static std::string where;

static void mismatch(const std::string& field) {
    std::cerr << "conformance mismatch: " << where << " " << field << "\n";
    ++failures;
}

static void compare_array(const json& expected, sm::span<std::uint32_t> actual, const std::string& field) {
    bool same = expected.items.size() == actual.size();
    for (std::size_t i = 0; same && i < actual.size(); ++i) {
        same = expected[i].n == static_cast<std::int64_t>(actual[i]);
    }
    if (!same) mismatch(field);
}

static bool same_style(const json& expected, const sm::style& actual) {
    return expected.items.size() == 3 && expected[0].n == actual.foreground &&
           expected[1].n == actual.background && expected[2].n == actual.modifiers;
}

static void compare_buffer(const json& expected, const sm::tokens& actual, const std::string& field) {
    if (expected["complete"].b != actual.complete()) mismatch(field + ".complete");
    compare_array(expected["lineStarts"], actual.line_starts(), field + ".lineStarts");
    compare_array(expected["lineTokenRanges"], actual.line_token_ranges(), field + ".lineTokenRanges");
    compare_array(expected["tokenStarts"], actual.token_starts(), field + ".tokenStarts");
    compare_array(expected["tokenLengths"], actual.token_lengths(), field + ".tokenLengths");
    compare_array(expected["tokenStyles"], actual.token_styles(), field + ".tokenStyles");
    const json& styles = expected["styles"];
    bool same = styles.items.size() == actual.styles().size();
    for (std::size_t i = 0; same && i < styles.items.size(); ++i) same = same_style(styles[i], actual.styles()[i]);
    if (!same) mismatch(field + ".styles");
    if (!same_style(expected["defaultStyle"], actual.default_style())) mismatch(field + ".defaultStyle");
    if (!actual.token_scopes().empty()) mismatch(field + " has unrequested scopes");
}

static void run_conformance(const std::string& dir) {
    json cases = read_json(dir + "/cases.json");
    json expected = read_json(dir + "/expected.json");
    sm::engine engine = sm::engine::bundled();
    sm::theme custom = sm::theme::from_json(cases["customTheme"].raw);
    CHECK(cases["cases"].items.size() == expected.members.size());
    CHECK(!cases["cases"].items.empty());

    for (const json& c : cases["cases"].items) {
        where = c["name"].s;
        const json& want = expected[c["name"].s];
        const std::string& language = c["language"].s;
        const std::string& source = c["source"].s;
        std::optional<sm::theme> bundled;
        if (c["theme"].kind == json::string) bundled = sm::theme::bundled(c["theme"].s);
        const sm::theme& theme = bundled ? *bundled : custom;

        if (engine.html(language, source, theme) != want["html"].s) mismatch("html");
        sm::html_options classes;
        classes.class_prefix = "sm";
        if (engine.html(language, source, theme, classes) != want["htmlClasses"].s) mismatch("htmlClasses");
        if (engine.ansi(language, source, theme) != want["ansi"].s) mismatch("ansi");

        const std::pair<const char*, sm::offset_unit> units[] = {
            {"utf8", sm::offset_unit::utf8},
            {"utf16", sm::offset_unit::utf16},
            {"codePoint", sm::offset_unit::code_point},
        };
        for (const auto& [name, unit] : units) {
            sm::tokens buffer = engine.tokens(language, source, theme, {unit, false});
            compare_buffer(want["tokens"][name], buffer, std::string("tokens.") + name);
        }

        sm::tokens scoped = engine.tokens(language, source, theme, {sm::offset_unit::utf8, true});
        compare_array(want["scopes"]["tokenScopes"], scoped.token_scopes(), "scopes.tokenScopes");
        const json& stacks = want["scopes"]["scopeStacks"];
        bool same = stacks.items.size() == scoped.scope_stack_count();
        for (std::size_t i = 0; same && i < stacks.items.size(); ++i) {
            std::vector<std::string_view> stack = scoped.scope_stack(i);
            same = stack.size() == stacks[i].items.size();
            for (std::size_t j = 0; same && j < stack.size(); ++j) same = stack[j] == stacks[i][j].s;
        }
        if (!same) mismatch("scopes.scopeStacks");

        sm::session session = engine.session(language, theme, {sm::offset_unit::utf16, false});
        const json& lines = want["session"];
        std::size_t index = 0, start = 0;
        while (true) {
            std::size_t end = source.find('\n', start);
            std::string_view line(source.data() + start, (end == std::string::npos ? source.size() : end) - start);
            if (index >= lines.items.size()) {
                mismatch("session line count");
                break;
            }
            compare_buffer(lines[index], session.line(line), "session[" + std::to_string(index) + "]");
            ++index;
            if (end == std::string::npos) break;
            start = end + 1;
        }
        if (index != lines.items.size()) mismatch("session line count");
    }
    where.clear();
}

// ---------------------------------------------------------------------------
// Wrapper behavior

static_assert(!std::is_copy_constructible_v<sm::engine>);
static_assert(!std::is_copy_constructible_v<sm::tokens>);
static_assert(std::is_nothrow_move_constructible_v<sm::engine>);
static_assert(std::is_nothrow_move_constructible_v<sm::session>);
static_assert(std::is_nothrow_move_constructible_v<sm::tokens>);

template <class F>
static sm::error_kind thrown(F&& f) {
    try {
        f();
    } catch (const sm::error& e) {
        CHECK(std::string(e.what()).size() > 0);
        return e.kind();
    }
    return static_cast<sm::error_kind>(0);
}

static void run_wrapper_tests() {
    CHECK(!sm::version().empty());
    sm::engine engine = sm::engine::bundled();
    sm::theme theme = sm::theme::bundled("github-dark");
    CHECK(theme.name() == "GitHub Dark Default");
    CHECK(theme.default_style().background != sm::no_color);
    CHECK(theme.stylesheet("sm").find(".sm-") != std::string::npos);

    auto languages = engine.languages();
    CHECK(std::find(languages.begin(), languages.end(), "rust") != languages.end());
    CHECK(!engine.themes().empty());
    CHECK(engine.canonical_language("rs") == std::optional<std::string>("rust"));
    CHECK(!engine.canonical_language("nope"));
    CHECK(engine.detect("x.py", "") == std::optional<std::string>("python"));
    CHECK(!engine.detect(std::nullopt, "plain words"));

    // string_view inputs need not be NUL-terminated.
    std::string_view source = std::string_view("let x = 1;XYZ").substr(0, 10);
    bool complete = false;
    CHECK(engine.html("rust", source, theme, {}, &complete).find("XYZ") == std::string::npos);
    CHECK(complete);
    sm::ansi_options plain;
    plain.colors = false;
    CHECK(engine.ansi("rust", source, theme, plain) == "let x = 1;");

    sm::tokens tokens = engine.tokens("rust", "fn a() {}\nfn b() {}", theme, {sm::offset_unit::utf8, true});
    CHECK(tokens.line_count() == 2 && tokens.line_token_ranges().size() == 3);
    CHECK(tokens.token_scopes().size() == tokens.token_count());
    CHECK(tokens.scope_stack(tokens.token_scopes()[0]).front() == "source.rust");
    std::uint32_t covered = 0;
    for (std::uint32_t length : tokens.token_lengths()) covered += length;
    CHECK(covered == 18);

    // Moves transfer ownership; the moved-from buffer is empty.
    sm::tokens moved = std::move(tokens);
    CHECK(moved.token_count() > 0 && tokens.token_count() == 0 && tokens.token_starts().empty());
    CHECK(tokens.line_count() == 0 && tokens.line_starts().empty() && tokens.line_token_ranges().empty());
    CHECK(tokens.token_lengths().empty() && tokens.token_styles().empty() && tokens.token_scopes().empty());
    CHECK(tokens.styles().empty() && tokens.scope_stack_count() == 0 && tokens.get() == nullptr);
    std::size_t visited = 0;
    for (std::uint32_t range : tokens.line_token_ranges()) visited += range + 1;
    CHECK(visited == 0);

    // Sessions outlive the engine and theme that created them.
    std::optional<sm::session> session;
    {
        sm::engine temporary = sm::engine::bundled();
        sm::theme light = sm::theme::bundled("github-light");
        session = temporary.session("rust", light);
    }
    sm::tokens first = session->line("/* open");
    session->line("still */");
    session->reset();
    CHECK(session->line("/* open").token_count() == first.token_count());

    CHECK(thrown([&] { sm::theme::bundled("nope"); }) == sm::error_kind::unknown_theme);
    CHECK(thrown([&] { sm::theme::from_json("{"); }) == sm::error_kind::invalid_theme);
    CHECK(thrown([&] { engine.html("nope", "", theme); }) == sm::error_kind::unknown_language);
    CHECK(thrown([&] { session->line("a\nb"); }) == sm::error_kind::invalid_input);
    CHECK(thrown([&] { engine.tokens("rust", "\xff", theme); }) == sm::error_kind::invalid_input);
    CHECK(thrown([&] { sm::engine::from_bundle("junk", 4); }) == sm::error_kind::invalid_bundle);
    CHECK(thrown([&] { moved.scope_stack(moved.scope_stack_count()); }) == sm::error_kind::invalid_input);

    // One engine and theme shared by several threads.
    std::vector<std::thread> threads;
    std::vector<int> ok(4, 0);
    for (int& flag : ok) {
        threads.emplace_back([&engine, &theme, &flag] {
            int good = 1;
            for (int i = 0; i < 10; ++i) {
                good &= engine.tokens("javascript", "const x = 1;", theme).token_count() > 0;
            }
            flag = good;
        });
    }
    for (auto& thread : threads) thread.join();
    for (int flag : ok) CHECK(flag);
}

int main(int argc, char** argv) {
    if (argc != 2) {
        std::cerr << "usage: " << argv[0] << " <conformance-dir>\n";
        return 2;
    }
    try {
        run_wrapper_tests();
        run_conformance(argv[1]);
    } catch (const std::exception& e) {
        std::cerr << "uncaught exception" << (where.empty() ? "" : " in " + where) << ": " << e.what() << "\n";
        return 1;
    }
    if (failures) {
        std::cerr << failures << " check(s) failed\n";
        return 1;
    }
    std::cout << "C++ wrapper and conformance tests passed\n";
    return 0;
}
