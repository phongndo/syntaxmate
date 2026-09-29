// Header-only C++17 wrapper over the Syntaxmate C API.
// SPDX-License-Identifier: MIT
//
// Handles are move-only RAII types. Failures throw syntaxmate::error.
// Thread safety follows the C API: engine and theme may be shared across
// threads; session may not; tokens may be read concurrently.
#ifndef SYNTAXMATE_HPP
#define SYNTAXMATE_HPP

#include "syntaxmate.h"

#include <cstddef>
#include <cstdint>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace syntaxmate {

/// Stable error categories; values match `sm_status`.
enum class error_kind : std::uint32_t {
    unknown_language = SM_UNKNOWN_LANGUAGE,
    unknown_theme = SM_UNKNOWN_THEME,
    invalid_grammar = SM_INVALID_GRAMMAR,
    invalid_theme = SM_INVALID_THEME,
    invalid_bundle = SM_INVALID_BUNDLE,
    invalid_input = SM_INVALID_INPUT,
    render = SM_RENDER,
    internal = SM_INTERNAL,
};

/// Thrown by every failing operation.
class error : public std::runtime_error {
public:
    error(error_kind kind, const std::string& message)
        : std::runtime_error(message), kind_(kind) {}

    error_kind kind() const noexcept { return kind_; }

private:
    error_kind kind_;
};

/// Unit of token offsets and lengths.
enum class offset_unit : std::uint32_t {
    utf8 = SM_OFFSET_UTF8,
    utf16 = SM_OFFSET_UTF16,
    code_point = SM_OFFSET_CODE_POINT,
};

/// Style colors are 0xRRGGBB or `no_color`; modifiers are a bitset.
using style = sm_style;
inline constexpr std::uint32_t no_color = SM_NO_COLOR;
inline constexpr std::uint32_t bold = SM_BOLD;
inline constexpr std::uint32_t italic = SM_ITALIC;
inline constexpr std::uint32_t underline = SM_UNDERLINE;
inline constexpr std::uint32_t strikethrough = SM_STRIKETHROUGH;

/// A read-only view of `size()` contiguous elements (a minimal C++17 span).
template <class T>
class span {
public:
    constexpr span() noexcept = default;
    constexpr span(const T* data, std::size_t size) noexcept : data_(data), size_(size) {}

    constexpr const T* data() const noexcept { return data_; }
    constexpr std::size_t size() const noexcept { return size_; }
    constexpr bool empty() const noexcept { return size_ == 0; }
    constexpr const T* begin() const noexcept { return data_; }
    constexpr const T* end() const noexcept { return data_ + size_; }
    constexpr const T& operator[](std::size_t i) const noexcept { return data_[i]; }

private:
    const T* data_ = nullptr;
    std::size_t size_ = 0;
};

struct token_options {
    offset_unit unit = offset_unit::utf8;
    bool include_scopes = false;
};

struct html_options {
    bool include_wrapper = true;
    bool include_scopes = false;
    /// Class on the `<pre>` wrapper; `std::nullopt` for none.
    std::optional<std::string> wrapper_class = std::string("syntaxmate");
    /// Scope-class prefix; `std::nullopt` for inline styles.
    std::optional<std::string> class_prefix;
};

struct ansi_options {
    bool colors = true;
    bool sanitize_control_characters = true;
    bool include_default_background = false;
};

namespace detail {

inline void check(sm_status status) {
    if (status == SM_OK) return;
    const char* message = sm_last_error_message();
    throw error(static_cast<error_kind>(status), message ? message : "syntaxmate error");
}

inline std::string_view view(sm_str s) { return s.ptr ? std::string_view(s.ptr, s.len) : std::string_view(); }

// Owners that free a C result even if copying it out throws.
using string_owner = std::unique_ptr<sm_string, decltype(&sm_string_free)>;
using string_list_owner = std::unique_ptr<sm_string_list, decltype(&sm_string_list_free)>;

/// Takes ownership of an `sm_string` and copies it out.
inline std::string take(sm_string* s) {
    string_owner owner(s, sm_string_free);
    const char* data = sm_string_data(s);
    return std::string(data ? data : "", sm_string_len(s));
}

inline std::optional<std::string> take_optional(sm_string* s) {
    if (!s) return std::nullopt;
    return take(s);
}

inline std::vector<std::string> take(sm_string_list* list) {
    string_list_owner owner(list, sm_string_list_free);
    std::vector<std::string> result;
    result.reserve(sm_string_list_len(list));
    for (std::size_t i = 0; i < sm_string_list_len(list); ++i) {
        result.emplace_back(view(sm_string_list_get(list, i)));
    }
    return result;
}

inline sm_token_options convert(const token_options& o) {
    return sm_token_options{static_cast<std::uint32_t>(o.unit), o.include_scopes};
}

/// Move-only owner of a C handle.
template <class T, void (*Free)(T*)>
class handle {
public:
    handle() noexcept = default;
    explicit handle(T* raw) noexcept : raw_(raw) {}
    handle(handle&& other) noexcept : raw_(std::exchange(other.raw_, nullptr)) {}
    handle& operator=(handle&& other) noexcept {
        if (this != &other) {
            Free(raw_);
            raw_ = std::exchange(other.raw_, nullptr);
        }
        return *this;
    }
    handle(const handle&) = delete;
    handle& operator=(const handle&) = delete;
    ~handle() { Free(raw_); }

    T* get() const noexcept { return raw_; }
    explicit operator bool() const noexcept { return raw_ != nullptr; }

private:
    T* raw_ = nullptr;
};

}  // namespace detail

/// A bundled or custom theme.
class theme {
public:
    static theme bundled(std::string_view name) {
        sm_theme* raw = nullptr;
        detail::check(sm_theme_bundled(name.data(), name.size(), &raw));
        return theme(raw);
    }

    static theme from_json(std::string_view json) {
        sm_theme* raw = nullptr;
        detail::check(sm_theme_from_json(json.data(), json.size(), &raw));
        return theme(raw);
    }

    /// Borrowed until the theme is destroyed.
    std::string_view name() const { return detail::view(sm_theme_name(get())); }

    style default_style() const {
        style out{};
        detail::check(sm_theme_default_style(get(), &out));
        return out;
    }

    /// CSS for HTML rendered with `html_options::class_prefix` = `class_prefix`.
    std::string stylesheet(std::string_view class_prefix) const {
        sm_string* out = nullptr;
        detail::check(sm_theme_stylesheet(get(), class_prefix.data(), class_prefix.size(), &out));
        return detail::take(out);
    }

    const sm_theme* get() const noexcept { return handle_.get(); }

private:
    explicit theme(sm_theme* raw) noexcept : handle_(raw) {}
    detail::handle<sm_theme, sm_theme_free> handle_;
};

/// An owned token buffer with zero-copy views over its arrays.
class tokens {
public:
    tokens(tokens&& other) noexcept
        : handle_(std::move(other.handle_)), view_(std::exchange(other.view_, sm_token_view{})) {}
    tokens& operator=(tokens&& other) noexcept {
        if (this != &other) {
            handle_ = std::move(other.handle_);
            view_ = std::exchange(other.view_, sm_token_view{});
        }
        return *this;
    }

    offset_unit unit() const noexcept { return static_cast<offset_unit>(view_.unit); }
    bool complete() const noexcept { return view_.complete; }
    std::size_t line_count() const noexcept { return view_.line_count; }
    std::size_t token_count() const noexcept { return view_.token_count; }

    span<std::uint32_t> line_starts() const noexcept { return {view_.line_starts, view_.line_count}; }
    /// `line_count() + 1` boundaries (empty once moved from); tokens of line
    /// `l` are `[line_token_ranges()[l], line_token_ranges()[l + 1])`.
    span<std::uint32_t> line_token_ranges() const noexcept {
        return view_.line_token_ranges
                   ? span<std::uint32_t>(view_.line_token_ranges, view_.line_count + 1)
                   : span<std::uint32_t>();
    }
    span<std::uint32_t> token_starts() const noexcept { return {view_.token_starts, view_.token_count}; }
    span<std::uint32_t> token_lengths() const noexcept { return {view_.token_lengths, view_.token_count}; }
    span<std::uint32_t> token_styles() const noexcept { return {view_.token_styles, view_.token_count}; }
    /// Empty unless `token_options::include_scopes` was set.
    span<std::uint32_t> token_scopes() const noexcept {
        return view_.token_scopes ? span<std::uint32_t>(view_.token_scopes, view_.token_count)
                                  : span<std::uint32_t>();
    }
    span<style> styles() const noexcept { return {view_.styles, view_.style_count}; }
    style default_style() const noexcept { return view_.default_style; }

    std::size_t scope_stack_count() const noexcept { return view_.scope_stack_count; }
    /// Scope stack `index`, outermost first; views are borrowed from this buffer.
    std::vector<std::string_view> scope_stack(std::size_t index) const {
        const sm_str* names = nullptr;
        std::size_t depth = 0;
        detail::check(sm_tokens_scope_stack(get(), index, &names, &depth));
        std::vector<std::string_view> result;
        result.reserve(depth);
        for (std::size_t i = 0; i < depth; ++i) result.push_back(detail::view(names[i]));
        return result;
    }

    const sm_tokens* get() const noexcept { return handle_.get(); }

private:
    friend class engine;
    friend class session;

    explicit tokens(sm_tokens* raw) : handle_(raw) { detail::check(sm_tokens_view(raw, &view_)); }

    detail::handle<sm_tokens, sm_tokens_free> handle_;
    sm_token_view view_{};
};

/// Incremental highlighting, one logical line per call. Not thread-safe.
class session {
public:
    /// Highlights the next line (without its `\n`); offsets are line-relative.
    tokens line(std::string_view text) {
        sm_tokens* out = nullptr;
        detail::check(sm_session_line(handle_.get(), text.data(), text.size(), &out));
        return tokens(out);
    }

    /// Returns to the start-of-document state.
    void reset() { detail::check(sm_session_reset(handle_.get())); }

    sm_session* get() const noexcept { return handle_.get(); }

private:
    friend class engine;
    explicit session(sm_session* raw) noexcept : handle_(raw) {}
    detail::handle<sm_session, sm_session_free> handle_;
};

/// A grammar catalog and highlighter; safe to share across threads.
class engine {
public:
    /// The embedded grammar catalog.
    static engine bundled() {
        sm_engine* raw = nullptr;
        detail::check(sm_engine_bundled(&raw));
        return engine(raw);
    }

    /// A grammar bundle (the bytes are copied).
    static engine from_bundle(const void* data, std::size_t size) {
        sm_engine* raw = nullptr;
        detail::check(sm_engine_from_bundle(static_cast<const std::uint8_t*>(data), size, &raw));
        return engine(raw);
    }

    std::vector<std::string> languages() const {
        sm_string_list* out = nullptr;
        detail::check(sm_engine_languages(get(), &out));
        return detail::take(out);
    }

    std::vector<std::string> themes() const {
        sm_string_list* out = nullptr;
        detail::check(sm_engine_themes(get(), &out));
        return detail::take(out);
    }

    /// The canonical ID for an ID or alias, or `std::nullopt` if unknown.
    std::optional<std::string> canonical_language(std::string_view language) const {
        sm_string* out = nullptr;
        detail::check(sm_engine_canonical_language(get(), language.data(), language.size(), &out));
        return detail::take_optional(out);
    }

    /// Detects a language from an optional path and the source's first line.
    std::optional<std::string> detect(std::optional<std::string_view> path,
                                      std::string_view source) const {
        sm_string* out = nullptr;
        detail::check(sm_engine_detect(get(), path ? path->data() : nullptr, path ? path->size() : 0,
                                       source.data(), source.size(), &out));
        return detail::take_optional(out);
    }

    /// Escaped HTML. `complete`, if given, is set false when tokenization
    /// stopped at a resource limit.
    std::string html(std::string_view language, std::string_view source, const theme& theme,
                     const html_options& options = {}, bool* complete = nullptr) const {
        auto optional = [](const std::optional<std::string>& s) {
            return s ? std::make_pair(s->data(), s->size())
                     : std::make_pair(static_cast<const char*>(nullptr), std::size_t{0});
        };
        auto [wrapper, wrapper_len] = optional(options.wrapper_class);
        auto [prefix, prefix_len] = optional(options.class_prefix);
        sm_html_options raw{options.include_wrapper, options.include_scopes, wrapper, wrapper_len,
                            prefix, prefix_len};
        sm_string* out = nullptr;
        detail::check(sm_engine_html(get(), language.data(), language.size(), source.data(),
                                     source.size(), theme.get(), &raw, &out, complete));
        return detail::take(out);
    }

    /// 24-bit ANSI text. `complete` is as for `html`.
    std::string ansi(std::string_view language, std::string_view source, const theme& theme,
                     const ansi_options& options = {}, bool* complete = nullptr) const {
        sm_ansi_options raw{options.colors, options.sanitize_control_characters,
                            options.include_default_background};
        sm_string* out = nullptr;
        detail::check(sm_engine_ansi(get(), language.data(), language.size(), source.data(),
                                     source.size(), theme.get(), &raw, &out, complete));
        return detail::take(out);
    }

    /// Highlights a whole document into a token buffer.
    syntaxmate::tokens tokens(std::string_view language, std::string_view source,
                              const theme& theme, const token_options& options = {}) const {
        sm_token_options raw = detail::convert(options);
        sm_tokens* out = nullptr;
        detail::check(sm_engine_tokens(get(), language.data(), language.size(), source.data(),
                                       source.size(), theme.get(), &raw, &out));
        return syntaxmate::tokens(out);
    }

    /// Starts a session; it does not borrow the engine or theme.
    syntaxmate::session session(std::string_view language, const theme& theme,
                                const token_options& options = {}) const {
        sm_token_options raw = detail::convert(options);
        sm_session* out = nullptr;
        detail::check(
            sm_engine_session(get(), language.data(), language.size(), theme.get(), &raw, &out));
        return syntaxmate::session(out);
    }

    const sm_engine* get() const noexcept { return handle_.get(); }

private:
    explicit engine(sm_engine* raw) noexcept : handle_(raw) {}
    detail::handle<sm_engine, sm_engine_free> handle_;
};

/// The library version.
inline std::string_view version() { return sm_version(); }

}  // namespace syntaxmate

#endif  // SYNTAXMATE_HPP
