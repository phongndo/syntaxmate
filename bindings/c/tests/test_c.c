/* Exercises the C API, including error paths and ownership. */
#include "syntaxmate.h"

#include <pthread.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int failures = 0;

#define CHECK(cond)                                                         \
    do {                                                                    \
        if (!(cond)) {                                                      \
            fprintf(stderr, "%s:%d: CHECK failed: %s\n", __FILE__, __LINE__, \
                    #cond);                                                 \
            failures++;                                                     \
        }                                                                   \
    } while (0)

#define CHECK_OK(expr) CHECK_STATUS(expr, SM_OK)
#define CHECK_STATUS(expr, want)                                               \
    do {                                                                       \
        sm_status got_ = (expr);                                               \
        if (got_ != (want)) {                                                  \
            const char *message_ = sm_last_error_message();                    \
            fprintf(stderr, "%s:%d: %s returned %u, want %u (%s)\n", __FILE__, \
                    __LINE__, #expr, (unsigned)got_, (unsigned)(want),         \
                    message_ ? message_ : "no message");                       \
            failures++;                                                        \
        }                                                                      \
    } while (0)

#define S(literal) literal, sizeof(literal) - 1

static int str_eq(sm_str s, const char *want) {
    return s.ptr != NULL && s.len == strlen(want) && memcmp(s.ptr, want, s.len) == 0 &&
           s.ptr[s.len] == '\0';
}

static int string_eq(const sm_string *s, const char *want) {
    return s != NULL && sm_string_len(s) == strlen(want) &&
           memcmp(sm_string_data(s), want, strlen(want)) == 0;
}

static int list_contains(const sm_string_list *list, const char *want) {
    size_t i;
    for (i = 0; i < sm_string_list_len(list); i++) {
        if (str_eq(sm_string_list_get(list, i), want)) return 1;
    }
    return 0;
}

static void test_catalog(const sm_engine *engine) {
    sm_string_list *list = NULL;
    sm_string *id = NULL;
    sm_str missing;

    CHECK(strlen(sm_version()) > 0);

    CHECK_OK(sm_engine_languages(engine, &list));
    CHECK(sm_string_list_len(list) > 10);
    CHECK(list_contains(list, "rust"));
    missing = sm_string_list_get(list, sm_string_list_len(list));
    CHECK(missing.ptr == NULL && missing.len == 0);
    sm_string_list_free(list);

    CHECK_OK(sm_engine_themes(engine, &list));
    CHECK(list_contains(list, "github-dark"));
    sm_string_list_free(list);

    CHECK_OK(sm_engine_canonical_language(engine, S("rs"), &id));
    CHECK(string_eq(id, "rust"));
    sm_string_free(id);

    id = (sm_string *)1; /* must be cleared */
    CHECK_OK(sm_engine_canonical_language(engine, S("no-such-language"), &id));
    CHECK(id == NULL);

    CHECK_OK(sm_engine_detect(engine, S("src/main.py"), S(""), &id));
    CHECK(string_eq(id, "python"));
    sm_string_free(id);

    CHECK_OK(sm_engine_detect(engine, NULL, 0, S("#!/bin/sh\necho hi\n"), &id));
    CHECK(string_eq(id, "shellscript"));
    sm_string_free(id);
}

static void test_render(const sm_engine *engine, const sm_theme *theme) {
    /* Inputs are (pointer, length): only the first 11 bytes are source. */
    const char buffer[] = "let x = 1;\nTRAILING GARBAGE";
    sm_string *out = NULL;
    sm_html_options html = sm_html_options_default();
    sm_ansi_options ansi = sm_ansi_options_default();
    bool complete = false;

    CHECK_OK(sm_engine_html(engine, S("rust"), buffer, 11, theme, NULL, &out, &complete));
    CHECK(complete);
    CHECK(strncmp(sm_string_data(out), "<pre class=\"syntaxmate\"", 23) == 0);
    CHECK(strstr(sm_string_data(out), "TRAILING") == NULL);
    CHECK(sm_string_data(out)[sm_string_len(out)] == '\0');
    sm_string_free(out);

    html.class_prefix = "sm";
    html.class_prefix_len = 2;
    html.wrapper_class = NULL;
    html.wrapper_class_len = 0;
    CHECK_OK(sm_engine_html(engine, S("rust"), buffer, 11, theme, &html, &out, NULL));
    CHECK(strstr(sm_string_data(out), "sm-sm-") != NULL);
    CHECK(strstr(sm_string_data(out), "syntaxmate") == NULL);
    sm_string_free(out);

    CHECK_OK(sm_theme_stylesheet(theme, S("sm"), &out));
    CHECK(strstr(sm_string_data(out), ".sm-") != NULL);
    sm_string_free(out);

    CHECK_OK(sm_engine_ansi(engine, S("rust"), buffer, 11, theme, NULL, &out, NULL));
    CHECK(strstr(sm_string_data(out), "\x1b[") != NULL);
    sm_string_free(out);

    ansi.colors = false;
    CHECK_OK(sm_engine_ansi(engine, S("rust"), buffer, 11, theme, &ansi, &out, NULL));
    CHECK(string_eq(out, "let x = 1;\n"));
    sm_string_free(out);
}

static void test_tokens(const sm_engine *engine, const sm_theme *theme) {
    const char source[] = "// é\nlet s = \"x\";";
    sm_tokens *tokens = NULL;
    sm_token_view view;
    sm_token_options options = sm_token_options_default();
    const sm_str *names = NULL;
    size_t depth = 0, i, covered = 0;
    sm_style default_style;

    CHECK_OK(sm_engine_tokens(engine, S("rust"), S(source), theme, NULL, &tokens));
    CHECK_OK(sm_tokens_view(tokens, &view));
    CHECK(view.unit == SM_OFFSET_UTF8);
    CHECK(view.complete);
    CHECK(view.line_count == 2);
    CHECK(view.line_starts[0] == 0 && view.line_starts[1] == 6); /* "é" is 2 bytes */
    CHECK(view.line_token_ranges[0] == 0);
    CHECK(view.line_token_ranges[2] == view.token_count);
    CHECK(view.token_count > 0);
    CHECK(view.token_scopes == NULL && view.scope_stack_count == 0);
    for (i = 0; i < view.token_count; i++) {
        CHECK(view.token_styles[i] < view.style_count);
        covered += view.token_lengths[i];
    }
    CHECK(covered == strlen(source) - 1); /* every byte except the "\n" */
    CHECK_OK(sm_theme_default_style(theme, &default_style));
    CHECK(memcmp(&default_style, &view.default_style, sizeof default_style) == 0);
    CHECK(view.default_style.foreground != SM_NO_COLOR);
    CHECK_STATUS(sm_tokens_scope_stack(tokens, 0, &names, &depth), SM_INVALID_INPUT);
    sm_tokens_free(tokens);

    options.unit = SM_OFFSET_CODE_POINT;
    options.include_scopes = true;
    CHECK_OK(sm_engine_tokens(engine, S("rust"), S(source), theme, &options, &tokens));
    CHECK_OK(sm_tokens_view(tokens, &view));
    CHECK(view.unit == SM_OFFSET_CODE_POINT);
    CHECK(view.line_starts[1] == 5);
    CHECK(view.token_scopes != NULL && view.scope_stack_count > 0);
    for (i = 0; i < view.token_count; i++) CHECK(view.token_scopes[i] < view.scope_stack_count);
    CHECK_OK(sm_tokens_scope_stack(tokens, view.token_scopes[0], &names, &depth));
    CHECK(depth >= 1 && str_eq(names[0], "source.rust"));
    CHECK_STATUS(sm_tokens_scope_stack(tokens, view.scope_stack_count, &names, &depth),
                 SM_INVALID_INPUT);
    sm_tokens_free(tokens);

    /* Empty source still has one (empty) line. */
    CHECK_OK(sm_engine_tokens(engine, S("rust"), NULL, 0, theme, NULL, &tokens));
    CHECK_OK(sm_tokens_view(tokens, &view));
    CHECK(view.line_count == 1 && view.token_count == 0);
    sm_tokens_free(tokens);
}

static void test_session(const sm_engine *engine) {
    sm_theme *theme = NULL;
    sm_session *session = NULL;
    sm_tokens *first = NULL, *again = NULL;
    sm_token_view a, b;
    sm_token_options options = sm_token_options_default();

    options.unit = SM_OFFSET_UTF16;
    CHECK_OK(sm_theme_bundled(S("github-light"), &theme));
    CHECK(str_eq(sm_theme_name(theme), "GitHub Light Default"));
    CHECK_OK(sm_engine_session(engine, S("rust"), theme, &options, &session));
    /* The session owns what it needs. */
    sm_theme_free(theme);

    CHECK_OK(sm_session_line(session, S("/* open"), &first));
    CHECK_OK(sm_session_line(session, S("still comment */ let"), &again));
    CHECK_OK(sm_tokens_view(again, &b));
    CHECK(b.line_count == 1 && b.line_starts[0] == 0 && b.unit == SM_OFFSET_UTF16);
    sm_tokens_free(again);

    CHECK_STATUS(sm_session_line(session, S("a\nb"), &again), SM_INVALID_INPUT);
    CHECK(again == NULL);

    CHECK_OK(sm_session_reset(session));
    CHECK_OK(sm_session_line(session, S("/* open"), &again));
    CHECK_OK(sm_tokens_view(first, &a));
    CHECK_OK(sm_tokens_view(again, &b));
    CHECK(a.token_count == b.token_count);
    CHECK(memcmp(a.token_lengths, b.token_lengths, a.token_count * sizeof(uint32_t)) == 0);
    sm_tokens_free(first);
    sm_tokens_free(again);
    sm_session_free(session);
}

static void test_errors(const sm_engine *engine, const sm_theme *theme) {
    sm_theme *bad_theme = (sm_theme *)1;
    sm_engine *bad_engine = (sm_engine *)1;
    sm_string *out = NULL;
    sm_tokens *tokens = NULL;
    sm_session *session = NULL;
    sm_token_options options = sm_token_options_default();
    const char invalid_utf8[] = "\xff\xfe";
    const unsigned char garbage[] = {1, 2, 3, 4};

    CHECK_STATUS(sm_theme_bundled(S("no-such-theme"), &bad_theme), SM_UNKNOWN_THEME);
    CHECK(bad_theme == NULL);
    CHECK(sm_last_error_message() != NULL && strlen(sm_last_error_message()) > 0);

    /* A later success clears the thread's error. */
    CHECK_OK(sm_theme_default_style(theme, &(sm_style){0, 0, 0}));
    CHECK(sm_last_error_message() == NULL);

    CHECK_STATUS(sm_theme_from_json(S("{not json"), &bad_theme), SM_INVALID_THEME);
    CHECK(bad_theme == NULL);
    CHECK_STATUS(sm_engine_from_bundle(garbage, sizeof garbage, &bad_engine), SM_INVALID_BUNDLE);
    CHECK(bad_engine == NULL);

    CHECK_STATUS(sm_engine_html(engine, S("no-such-language"), S("x"), theme, NULL, &out, NULL),
                 SM_UNKNOWN_LANGUAGE);
    CHECK(out == NULL);
    CHECK_STATUS(sm_engine_session(engine, S("no-such-language"), theme, NULL, &session),
                 SM_UNKNOWN_LANGUAGE);

    /* Null pointers. */
    CHECK_STATUS(sm_engine_html(NULL, S("rust"), S("x"), theme, NULL, &out, NULL),
                 SM_INVALID_INPUT);
    CHECK_STATUS(sm_engine_html(engine, S("rust"), S("x"), NULL, NULL, &out, NULL),
                 SM_INVALID_INPUT);
    CHECK_STATUS(sm_engine_html(engine, S("rust"), S("x"), theme, NULL, NULL, NULL),
                 SM_INVALID_INPUT);
    CHECK_STATUS(sm_engine_tokens(engine, S("rust"), NULL, 3, theme, NULL, &tokens),
                 SM_INVALID_INPUT);
    CHECK_STATUS(sm_engine_languages(engine, NULL), SM_INVALID_INPUT);
    CHECK_STATUS(sm_tokens_view(NULL, &(sm_token_view){0}), SM_INVALID_INPUT);
    CHECK_STATUS(sm_session_reset(NULL), SM_INVALID_INPUT);
    CHECK_STATUS(sm_session_line(NULL, S("x"), &tokens), SM_INVALID_INPUT);
    CHECK_STATUS(sm_theme_default_style(NULL, &(sm_style){0, 0, 0}), SM_INVALID_INPUT);
    CHECK(sm_string_data(NULL) == NULL && sm_string_len(NULL) == 0);
    CHECK(sm_string_list_len(NULL) == 0 && sm_theme_name(NULL).ptr == NULL);

    /* Invalid UTF-8 in every kind of text input. */
    CHECK_STATUS(sm_engine_tokens(engine, S("rust"), S(invalid_utf8), theme, NULL, &tokens),
                 SM_INVALID_INPUT);
    CHECK(strstr(sm_last_error_message(), "UTF-8") != NULL);
    CHECK_STATUS(sm_engine_tokens(engine, S(invalid_utf8), S("x"), theme, NULL, &tokens),
                 SM_INVALID_INPUT);
    CHECK_STATUS(sm_theme_bundled(S(invalid_utf8), &bad_theme), SM_INVALID_INPUT);
    CHECK_STATUS(sm_engine_detect(engine, S(invalid_utf8), S("x"), &out), SM_INVALID_INPUT);

    /* Lengths past PTRDIFF_MAX are rejected before the pointer is read. */
    CHECK_STATUS(sm_engine_tokens(engine, S("rust"), "x", SIZE_MAX, theme, NULL, &tokens),
                 SM_INVALID_INPUT);
    CHECK_STATUS(sm_engine_html(engine, "rust", (size_t)PTRDIFF_MAX + 1, S("x"), theme, NULL,
                                &out, NULL),
                 SM_INVALID_INPUT);
    CHECK(out == NULL);

    /* The message survives calls that do not return a status. */
    {
        const char *message;
        CHECK_STATUS(sm_theme_bundled(S("nope"), &bad_theme), SM_UNKNOWN_THEME);
        message = sm_last_error_message();
        CHECK(message != NULL && strstr(message, "nope") != NULL);
        CHECK(sm_string_len(NULL) == 0 && sm_version() != NULL);
        sm_string_free(NULL);
        CHECK(sm_last_error_message() == message);
    }

    options.unit = 7;
    CHECK_STATUS(sm_engine_tokens(engine, S("rust"), S("x"), theme, &options, &tokens),
                 SM_INVALID_INPUT);
    CHECK(tokens == NULL);

    /* Freeing NULL is a no-op. */
    sm_engine_free(NULL);
    sm_theme_free(NULL);
    sm_session_free(NULL);
    sm_tokens_free(NULL);
    sm_string_free(NULL);
    sm_string_list_free(NULL);
}

typedef struct {
    const sm_engine *engine;
    const sm_theme *theme;
    int ok;
} worker_args;

static void *worker(void *raw) {
    worker_args *args = raw;
    int round;
    args->ok = 1;
    for (round = 0; round < 20; round++) {
        sm_tokens *tokens = NULL;
        sm_theme *missing = NULL;
        if (sm_engine_tokens(args->engine, S("rust"), S("fn main() {}\n"), args->theme, NULL,
                             &tokens) != SM_OK)
            args->ok = 0;
        sm_tokens_free(tokens);
        /* Errors are per-thread. */
        if (sm_theme_bundled(S("nope"), &missing) != SM_UNKNOWN_THEME ||
            sm_last_error_message() == NULL)
            args->ok = 0;
    }
    return NULL;
}

static void test_threads(const sm_engine *engine, const sm_theme *theme) {
    enum { N = 4 };
    pthread_t threads[N];
    worker_args args[N];
    int i;
    for (i = 0; i < N; i++) {
        args[i].engine = engine;
        args[i].theme = theme;
        args[i].ok = 0;
        CHECK(pthread_create(&threads[i], NULL, worker, &args[i]) == 0);
    }
    for (i = 0; i < N; i++) {
        CHECK(pthread_join(threads[i], NULL) == 0);
        CHECK(args[i].ok);
    }
}

int main(void) {
    sm_engine *engine = NULL;
    sm_theme *theme = NULL;

    CHECK_OK(sm_engine_bundled(&engine));
    CHECK_OK(sm_theme_bundled(S("github-dark"), &theme));
    if (engine == NULL || theme == NULL) return 1;

    test_catalog(engine);
    test_render(engine, theme);
    test_tokens(engine, theme);
    test_session(engine);
    test_errors(engine, theme);
    test_threads(engine, theme);

    sm_theme_free(theme);
    sm_engine_free(engine);
    if (failures) {
        fprintf(stderr, "%d check(s) failed\n", failures);
        return 1;
    }
    puts("C API tests passed");
    return 0;
}
