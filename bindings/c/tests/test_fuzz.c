/* Randomized C API test: drives the text entry points with random byte
 * sequences (including invalid UTF-8, NULs, and CR/LF mixes), random options,
 * and NULL/length combinations, then checks each result's invariants. Run it
 * under the sanitizers (`make check`) or valgrind (`make valgrind`).
 *
 * Usage: test_fuzz [ITERATIONS [SEED]]; SEED is printed so failures replay. */
#include "syntaxmate.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures = 0;
static unsigned long iteration = 0;

#define CHECK(cond)                                                                    \
    do {                                                                               \
        if (!(cond)) {                                                                 \
            fprintf(stderr, "%s:%d: iteration %lu: CHECK failed: %s\n", __FILE__, __LINE__, \
                    iteration, #cond);                                                 \
            if (++failures > 20) exit(1);                                              \
        }                                                                              \
    } while (0)

/* xorshift64*: deterministic across platforms. */
static uint64_t rng_state;
static uint64_t next(void) {
    rng_state ^= rng_state >> 12;
    rng_state ^= rng_state << 25;
    rng_state ^= rng_state >> 27;
    return rng_state * 0x2545F4914F6CDD1DULL;
}
static size_t below(size_t n) { return n ? (size_t)(next() % n) : 0; }

/* Fragments that exercise grammars, multi-byte UTF-8, and line handling. */
static const char *const FRAGMENTS[] = {
    "fn main() {", "}", "let s = \"", "\"", "/*", "*/", "//", "#", "<div a=\"", ">",
    "</div>", "{\"k\": [1, 2.5e3, null]}", "  ", "\t", "\r", "\n", "\r\n", "é", "€",
    "𝄞", "日本", "\\", "'", "`", "${", "<!--", "-->", "- item", "key: value", "```",
    "def f(x):", "    return x", "#include <x>", "class A {}", "0x1F", "@decorator",
};
static const char *const LANGUAGES[] = {
    "rust", "python", "json", "html", "markdown", "cpp", "yaml", "typescript",
    "shellscript", "rs", "no-such-language", "",
};
static const char *const THEMES[] = {"github-dark", "github-light"};

/* Fills `buf` (capacity `cap`) with random input; returns its length. */
static size_t random_text(char *buf, size_t cap) {
    size_t len = 0, pieces = below(24);
    while (pieces-- > 0) {
        size_t kind = below(10);
        if (kind < 7) {
            const char *f = FRAGMENTS[below(sizeof FRAGMENTS / sizeof *FRAGMENTS)];
            size_t n = strlen(f);
            if (len + n > cap) break;
            memcpy(buf + len, f, n);
            len += n;
        } else if (kind < 9) {
            /* Printable ASCII run. */
            size_t n = below(12);
            while (n-- > 0 && len < cap) buf[len++] = (char)(' ' + below(95));
        } else {
            /* Arbitrary bytes, often invalid UTF-8 (lone continuation, overlong, NUL). */
            size_t n = 1 + below(3);
            while (n-- > 0 && len < cap) buf[len++] = (char)below(256);
        }
    }
    return len;
}

static int valid_utf8(const unsigned char *s, size_t len) {
    size_t i = 0;
    while (i < len) {
        unsigned c = s[i];
        size_t n, k;
        uint32_t cp;
        if (c < 0x80) {
            i++;
            continue;
        }
        if (c >= 0xC2 && c <= 0xDF) n = 1, cp = c & 0x1F;
        else if (c >= 0xE0 && c <= 0xEF) n = 2, cp = c & 0x0F;
        else if (c >= 0xF0 && c <= 0xF4) n = 3, cp = c & 0x07;
        else return 0;
        for (k = 1; k <= n; k++) {
            if (i + k >= len || (s[i + k] & 0xC0) != 0x80) return 0;
            cp = (cp << 6) | (s[i + k] & 0x3F);
        }
        /* Overlong forms, surrogates, and values past U+10FFFF. */
        if ((n == 2 && cp < 0x800) || (n == 3 && (cp < 0x10000 || cp > 0x10FFFF)) ||
            (cp >= 0xD800 && cp <= 0xDFFF))
            return 0;
        i += n + 1;
    }
    return 1;
}

/* Length of valid UTF-8 `s[0..len)` in `unit`s. */
static size_t units(const unsigned char *s, size_t len, uint32_t unit) {
    size_t i, count = 0;
    if (unit == SM_OFFSET_UTF8) return len;
    for (i = 0; i < len; i++) {
        if ((s[i] & 0xC0) == 0x80) continue; /* continuation byte */
        count += (unit == SM_OFFSET_UTF16 && s[i] >= 0xF0) ? 2 : 1;
    }
    return count;
}

static int known_language(const char *name) {
    return strcmp(name, "no-such-language") != 0 && strcmp(name, "") != 0;
}

static int known_status(sm_status status) { return status <= SM_INTERNAL; }

/* Checks every documented invariant of a token view over `source`. */
static void check_view(const sm_tokens *tokens, const char *source, size_t len, uint32_t unit,
                       int scopes) {
    sm_token_view v;
    size_t l, i, lines = 1, pos = 0;
    CHECK(sm_tokens_view(tokens, &v) == SM_OK);
    CHECK(v.unit == unit);
    for (i = 0; i < len; i++) lines += source[i] == '\n';
    CHECK(v.line_count == lines);
    CHECK(v.line_starts != NULL && v.line_token_ranges != NULL);
    CHECK(v.token_count == 0 || (v.token_starts && v.token_lengths && v.token_styles));
    CHECK(v.line_token_ranges[0] == 0 && v.line_token_ranges[v.line_count] == v.token_count);
    CHECK((v.token_scopes != NULL) == (scopes && v.token_count > 0));
    CHECK(scopes || v.scope_stack_count == 0);
    for (l = 0; l < v.line_count; l++) {
        const char *nl = memchr(source + pos, '\n', len - pos);
        size_t end = nl ? (size_t)(nl - source) : len;
        size_t start_units = units((const unsigned char *)source, pos, unit);
        size_t end_units = units((const unsigned char *)source, end, unit);
        size_t cursor = start_units;
        CHECK(v.line_starts[l] == start_units);
        CHECK(v.line_token_ranges[l] <= v.line_token_ranges[l + 1]);
        for (i = v.line_token_ranges[l]; i < v.line_token_ranges[l + 1] && i < v.token_count; i++) {
            /* Ordered, non-overlapping, non-empty, and within the line. */
            CHECK(v.token_starts[i] >= cursor);
            CHECK(v.token_lengths[i] > 0);
            CHECK((size_t)v.token_starts[i] + v.token_lengths[i] <= end_units);
            cursor = (size_t)v.token_starts[i] + v.token_lengths[i];
            CHECK(v.token_styles[i] < v.style_count);
            if (v.token_scopes) CHECK(v.token_scopes[i] < v.scope_stack_count);
        }
        pos = end + 1;
    }
    for (i = 0; i < v.scope_stack_count; i++) {
        const sm_str *names = NULL;
        size_t depth = 0, k;
        CHECK(sm_tokens_scope_stack(tokens, i, &names, &depth) == SM_OK);
        for (k = 0; k < depth; k++) {
            CHECK(names[k].ptr != NULL && names[k].ptr[names[k].len] == '\0');
            CHECK(strlen(names[k].ptr) == names[k].len);
        }
    }
}

static void check_string(const sm_string *s) {
    const char *data = sm_string_data(s);
    size_t len = sm_string_len(s);
    CHECK(data != NULL && data[len] == '\0');
    CHECK(valid_utf8((const unsigned char *)data, len));
}

/* On failure the out-handle is NULL and a message is set; on success it is not. */
static void check_outcome(sm_status status, const void *out) {
    CHECK(known_status(status));
    if (status == SM_OK) {
        CHECK(sm_last_error_message() == NULL);
    } else {
        CHECK(out == NULL);
        CHECK(sm_last_error_message() != NULL);
    }
}

int main(int argc, char **argv) {
    unsigned long iterations = argc > 1 ? strtoul(argv[1], NULL, 10) : 300;
    unsigned long long seed = argc > 2 ? strtoull(argv[2], NULL, 10) : 0x5eed5eedULL;
    static char source[4096], extra[64];
    sm_engine *engine = NULL;
    sm_theme *themes[2] = {NULL, NULL};
    size_t t;

    printf("test_fuzz: %lu iterations, seed %llu\n", iterations, seed);
    rng_state = seed ? (uint64_t)seed : 1;
    if (sm_engine_bundled(&engine) != SM_OK) return 1;
    for (t = 0; t < 2; t++) {
        if (sm_theme_bundled(THEMES[t], strlen(THEMES[t]), &themes[t]) != SM_OK) return 1;
    }

    for (iteration = 0; iteration < iterations; iteration++) {
        const char *language = LANGUAGES[below(sizeof LANGUAGES / sizeof *LANGUAGES)];
        size_t language_len = strlen(language);
        const sm_theme *theme = themes[below(2)];
        size_t len = random_text(source, sizeof source);
        int valid = valid_utf8((const unsigned char *)source, len);
        int good = valid && known_language(language);
        /* NULL with length 0 is the empty string. */
        const char *text = (len == 0 && below(2)) ? NULL : source;
        sm_status status;

        switch (below(6)) {
        case 0: { /* HTML with random options, including invalid option strings. */
            sm_html_options options = sm_html_options_default();
            sm_string *out = (sm_string *)1;
            bool complete = false;
            int options_valid = 1;
            options.include_wrapper = below(2);
            options.include_scopes = below(2);
            if (below(3) == 0) {
                size_t n = random_text(extra, sizeof extra);
                options.class_prefix = extra;
                options.class_prefix_len = n;
                options_valid = valid_utf8((const unsigned char *)extra, n);
            }
            if (below(4) == 0) options.wrapper_class = NULL, options.wrapper_class_len = 0;
            status = sm_engine_html(engine, language, language_len, text, len, theme,
                                    below(4) ? &options : NULL, &out, &complete);
            check_outcome(status, out);
            if (good && options_valid) CHECK(status == SM_OK);
            if (!valid) CHECK(status == SM_INVALID_INPUT);
            if (status == SM_OK) {
                check_string(out);
                CHECK(complete);
            }
            sm_string_free(out);
            break;
        }
        case 1: { /* ANSI; with sanitization, the output is still valid UTF-8. */
            sm_ansi_options options = sm_ansi_options_default();
            sm_string *out = (sm_string *)1;
            options.colors = below(2);
            options.include_default_background = below(2);
            status = sm_engine_ansi(engine, language, language_len, text, len, theme, &options,
                                    &out, NULL);
            check_outcome(status, out);
            if (good) CHECK(status == SM_OK);
            if (!valid) CHECK(status == SM_INVALID_INPUT);
            if (status == SM_OK) check_string(out);
            sm_string_free(out);
            break;
        }
        case 2:
        case 3: { /* Whole-document tokens in every unit, plus invalid units. */
            sm_token_options options = sm_token_options_default();
            sm_tokens *tokens = (sm_tokens *)1;
            options.unit = (uint32_t)below(4); /* 3 is invalid */
            options.include_scopes = below(2);
            status = sm_engine_tokens(engine, language, language_len, text, len, theme, &options,
                                      &tokens);
            check_outcome(status, tokens);
            if (good && options.unit <= SM_OFFSET_CODE_POINT) CHECK(status == SM_OK);
            if (!valid || options.unit > SM_OFFSET_CODE_POINT) CHECK(status == SM_INVALID_INPUT);
            if (status == SM_OK)
                check_view(tokens, source, len, options.unit, options.include_scopes);
            sm_tokens_free(tokens);
            break;
        }
        case 4: { /* A session fed the input piece by piece; pieces may hold '\n'. */
            sm_token_options options = sm_token_options_default();
            sm_session *session = (sm_session *)1;
            size_t pos = 0;
            options.unit = (uint32_t)below(3);
            options.include_scopes = below(2);
            status = sm_engine_session(engine, language, language_len, theme, &options, &session);
            check_outcome(status, session);
            if (known_language(language)) CHECK(status == SM_OK);
            if (status != SM_OK) break;
            while (pos <= len) {
                size_t n = below(len - pos + 1);
                sm_tokens *tokens = (sm_tokens *)1;
                int piece_valid = valid_utf8((const unsigned char *)source + pos, n);
                int newline = memchr(source + pos, '\n', n) != NULL;
                status = sm_session_line(session, source + pos, n, &tokens);
                check_outcome(status, tokens);
                if (piece_valid && !newline) CHECK(status == SM_OK);
                if (!piece_valid || newline) CHECK(status == SM_INVALID_INPUT);
                if (status == SM_OK)
                    check_view(tokens, source + pos, n, options.unit, options.include_scopes);
                sm_tokens_free(tokens);
                if (below(8) == 0) CHECK(sm_session_reset(session) == SM_OK);
                pos += n + 1;
            }
            sm_session_free(session);
            break;
        }
        default: { /* Catalog lookups on random names and paths. */
            sm_string *out = (sm_string *)1;
            size_t n = random_text(extra, sizeof extra);
            int extra_valid = valid_utf8((const unsigned char *)extra, n);
            status = sm_engine_canonical_language(engine, extra, n, &out);
            CHECK(known_status(status));
            CHECK(extra_valid ? status == SM_OK : status == SM_INVALID_INPUT);
            if (out) check_string(out);
            sm_string_free(out);
            out = (sm_string *)1;
            status = sm_engine_detect(engine, below(2) ? extra : NULL, below(2) ? n : 0, text, len,
                                      &out);
            CHECK(known_status(status));
            if (out) check_string(out);
            sm_string_free(out);
            break;
        }
        }
    }

    for (t = 0; t < 2; t++) sm_theme_free(themes[t]);
    sm_engine_free(engine);
    if (failures) {
        fprintf(stderr, "%d check(s) failed (seed %llu)\n", failures, seed);
        return 1;
    }
    puts("C API randomized tests passed");
    return 0;
}
