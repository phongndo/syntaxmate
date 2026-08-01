#include <iostream>
#include <string>
#include <array>
#include <compare>
#include <concepts>
#include <cstddef>
#include <cstdint>
#include <map>
#include <memory>
#include <optional>
#include <ranges>
#include <span>
#include <stdexcept>
#include <string_view>
#include <tuple>
#include <type_traits>
#include <utility>
#include <variant>
#include <vector>

#define MARK_STRESS(name) void name()
#if __cplusplus >= 202002L
#  define HAS_MODERN_CPP 1
#else
#  define HAS_MODERN_CPP 0
#endif
#define MARK_STRINGIZE_INNER(token) #token
#define MARK_STRINGIZE(token) MARK_STRINGIZE_INNER(token)
#define MARK_JOIN_INNER(left, right) left##right
#define MARK_JOIN(left, right) MARK_JOIN_INNER(left, right)
#define MARK_TRACE(expression) \
    do { std::clog << #expression << " = " << (expression) << '\n'; } while (false)
#if defined(__clang__) || defined(__GNUC__)
[[maybe_unused]] constexpr std::string_view compiler_family = "clang-or-gcc";
#elif defined(_MSC_VER)
[[maybe_unused]] constexpr std::string_view compiler_family = "msvc";
#else
[[maybe_unused]] constexpr std::string_view compiler_family = "unknown";
#endif

/* C++ stress fixture with non-ASCII text: café λ🚀.
 * Scenario: tokenize a production-style library translation unit without compiling it.
 * Ownership notes distinguish borrowed spans, unique handles, shared observers, and weak references.
 * Numeric examples cover binary, octal, decimal, hexadecimal, digit separators, and long-double suffixes.
 * Template constraints model integral and floating-point algorithms with explicit ordering requirements.
 * Diagnostics mention source locations such as module/widget.cpp:42 without opening a string literal.
 * International labels include Καλημέρα, 東京, हिन्दी, العربية, naïve façade, and astral 🧭 𝄞 🦀.
 * Raw resources represent regular expressions, HTML fragments, escaped paths, and structured configuration.
 * Value categories exercise lvalues, const references, forwarding references, moves, and copy elision.
 * Error paths use typed exceptions, optional results, visitor dispatch, and bounds-checked access.
 * Compile-time paths use constexpr evaluation, consteval literals, concepts, requires clauses, and assertions.
 * Preprocessor branches retain portable spellings for Clang, GCC, MSVC, and otherwise unidentified toolchains.
 * Public types expose scoped and unscoped enumerations, aggregate coordinates, polymorphic names, and fixed buffers.
 * Operator examples include assignment, arithmetic, bitwise combination, indexing, conversion, comparison, and streaming.
 * Containers include arrays, vectors, maps, spans, tuples, variants, optionals, unique pointers, shared pointers, and weak pointers.
 * Control flow demonstrates range-for traversal, conditional initialization, compile-time branches, catches, and early returns.
 * Lambdas capture initialized state, accept constrained placeholder parameters, specify mutability, and promise noexcept execution.
 * Structured bindings unpack heterogeneous tuples and associative entries while preserving const-reference qualification.
 * Range pipelines filter even integers before transforming values, then pass the lazy view into a constrained accumulator.
 * Character data deliberately combines ordinary, UTF-8, escaped, regex-delimited, HTML-delimited, and document-delimited strings.
 * API names follow ordinary library conventions so declaration, definition, function-call, and scope-resolution rules interact.
 * Attributes cover maybe_unused and nodiscard positions on variables, functions, classes, and return-value declarations.
 * Static storage, inline members, explicit constructors, virtual destruction, defaulted operations, and friends model object lifetime.
 * Cast expressions distinguish checked polymorphic conversion from const, reinterpret, and ordinary static conversions.
 * Exception handling translates standard conversion failures into optional absence while preserving a custom diagnostic type.
 * The final macro-defined entry point preserves the compact legacy stream expression and integer-ratio calculation unchanged.
 */
static const std::string pattern = R"regex(^/api/([\w-]+)/(?:"quoted")$)regex";
static const std::string html = R"HTML(<div data-title="λ🚀">
  <span>{{ value }}</span>
</div>)HTML";
// BMP scripts: Ελληνικά, 日本語, हिन्दी, العربية; astral: 🧭 𝄞 🦀.
[[maybe_unused]] constexpr std::u8string_view greeting = u8"héllo 世界 🌍";
[[maybe_unused]] constexpr auto escaped = "tab:\t quote:\" slash:\\ hex:\x41";
[[maybe_unused]] constexpr auto document = R"doc({"emoji":"🧪","path":"C:\\tmp"})doc";
namespace mark::syntax {
using index_type = std::size_t;
using std::string_view;
inline namespace literals {
struct Distance { long double metres; friend constexpr bool operator==(const Distance&, const Distance&) = default; };
consteval Distance operator""_km(long double value) noexcept { return {value * 1000.0L}; }
} // namespace literals
enum class TokenKind : std::uint8_t { identifier, number = 4, punctuation };
enum Permission : unsigned { read = 1U << 0U, write = 1U << 1U, execute = 1U << 2U };
constexpr Permission operator|(Permission lhs, Permission rhs) noexcept {
    return static_cast<Permission>(static_cast<unsigned>(lhs) |
                                   static_cast<unsigned>(rhs));
}
template <typename T>
concept Numeric = std::integral<T> || std::floating_point<T>;
template <typename T>
concept Printable = requires(std::ostream& output, const T& value) {
    { output << value } -> std::same_as<std::ostream&>;
};
template <Numeric T>
[[nodiscard]] constexpr T clamp_value(T value, T low, T high) noexcept
    requires std::totally_ordered<T> {
    return value < low ? low : (high < value ? high : value);
}
template <typename... Values>
constexpr auto sum(Values&&... values)
    noexcept((std::is_nothrow_constructible_v<std::common_type_t<Values...>, Values> && ...)) {
    return (std::forward<Values>(values) + ... + 0);
}
struct Coordinate {
    double x{};
    double y{};
    constexpr Coordinate() = default;
    constexpr Coordinate(double x_value, double y_value) : x{x_value}, y{y_value} {}
    [[nodiscard]] constexpr double norm_squared() const noexcept { return x * x + y * y; }
    [[nodiscard]] constexpr double operator[](index_type index) const {
        return index == 0 ? x : index == 1 ? y : throw std::out_of_range{"coordinate"};
    }
    explicit constexpr operator bool() const noexcept { return x != 0.0 || y != 0.0; }
    friend constexpr auto operator<=>(const Coordinate&, const Coordinate&) = default;
};
class Named {
public:
    explicit Named(std::string label) : label_{std::move(label)} {}
    virtual ~Named() = default;
    [[nodiscard]] virtual std::string_view name() const noexcept { return label_; }
protected:
    std::string label_;
};
class [[nodiscard]] Widget final : public Named {
public:
    explicit Widget(std::string label, int value = 0) : Named{std::move(label)}, value_{value} {}
    Widget(const Widget&) = default;
    Widget(Widget&&) noexcept = default;
    Widget& operator=(const Widget&) = default;
    Widget& operator=(Widget&&) noexcept = default;
    ~Widget() override = default;
    [[nodiscard]] constexpr int value() const noexcept { return value_; }
    Widget& operator+=(int delta) noexcept { value_ += delta; return *this; }
    friend Widget operator+(Widget widget, int delta) noexcept { return widget += delta; }
    static inline std::size_t instances_observed = 0;
private:
    int value_{};
};
template <typename T, std::size_t Extent>
    requires (Extent > 0)
class Buffer {
public:
    using value_type = T;
    constexpr T& operator[](std::size_t index) noexcept { return storage_[index]; }
    constexpr const T& operator[](std::size_t index) const noexcept { return storage_[index]; }
    [[nodiscard]] constexpr std::span<T, Extent> span() noexcept { return storage_; }
private:
    std::array<T, Extent> storage_{};
};
template <typename T>
struct Description { static constexpr std::string_view value = "object"; };
template <>
struct Description<bool> { static constexpr std::string_view value = "boolean"; };
struct ParseError : std::runtime_error { using std::runtime_error::runtime_error; };
[[nodiscard]] std::optional<int> parse_integer(std::string_view text) {
    try {
        std::size_t consumed = 0;
        const int value = std::stoi(std::string{text}, &consumed, 0);
        if (consumed != text.size()) {
            throw ParseError{"trailing input"};
        }
        return value;
    } catch (const std::invalid_argument&) {
        return std::nullopt;
    } catch (const std::out_of_range&) {
        return std::nullopt;
    }
}
constexpr int fibonacci(unsigned n) noexcept {
    int previous = 0;
    int current = 1;
    for (unsigned i = 0; i < n; ++i) {
        const auto next = previous + current;
        previous = std::exchange(current, next);
    }
    return previous;
}
consteval std::size_t literal_length(std::string_view text) noexcept { return text.size(); }
template <std::ranges::input_range Range>
    requires Numeric<std::ranges::range_value_t<Range>>
auto range_total(Range&& range) {
    std::ranges::range_value_t<Range> result{};
    for (auto&& item : range) {
        result += item;
    }
    return result;
}
void exercise_expressions() {
    constexpr auto route_length = literal_length("/v1/λ");
    constexpr Distance journey = 1.25_km;
    static_assert(route_length == 6 && journey.metres == 1250.0L);
    std::vector<int> values{0b1010, 07, 42, 0x2A, 1'000};
    auto even_squares = values
        | std::views::filter([](int value) { return value % 2 == 0; })
        | std::views::transform([](int value) noexcept { return value * value; });
    [[maybe_unused]] const auto total = range_total(even_squares);
    std::map<std::string, int, std::less<>> scores{{"Ada", 9}, {"Bjarne", 10}};
    for (const auto& [name, score] : scores) {
        std::cout << name << ':' << score << ' ';
    }
    auto [first, second] = std::tuple{Coordinate{1.0, 2.0}, TokenKind::identifier};
    auto scale = [factor = 2](std::integral auto value) mutable noexcept { return value * factor++; };
    [[maybe_unused]] auto result = scale(static_cast<int>(first.x));
    [[maybe_unused]] auto token = second;
}
void exercise_ownership_and_casts(Named& named) {
    int mutable_value = 7;
    int* pointer = &mutable_value;
    int& reference = *pointer;
    const int* pointer_to_const = pointer;
    int* const const_pointer = pointer;
    auto owner = std::make_unique<Widget>("owned", reference);
    std::shared_ptr<Named> shared = std::make_shared<Widget>("shared", *const_pointer);
    std::weak_ptr<Named> weak = shared;
    [[maybe_unused]] auto* widget = dynamic_cast<Widget*>(&named);
    [[maybe_unused]] auto address = reinterpret_cast<std::uintptr_t>(pointer_to_const);
    [[maybe_unused]] int& writable = const_cast<int&>(*pointer_to_const);
    [[maybe_unused]] Named* base = static_cast<Named*>(owner.get());
    if (auto locked = weak.lock(); locked && !locked->name().empty()) {
        ++Widget::instances_observed;
    }
}
void exercise_variants() {
    std::variant<int, std::string, Coordinate> value = std::string{"variant ✨"};
    const auto size = std::visit([](const auto& item) -> std::size_t {
        using Item = std::remove_cvref_t<decltype(item)>;
        if constexpr (requires { item.size(); }) {
            return item.size();
        } else if constexpr (std::same_as<Item, Coordinate>) {
            return 2;
        } else {
            return 1;
        }
    }, value);
    MARK_TRACE(size);
}
} // namespace mark::syntax

MARK_STRESS(run) {
    auto ratio = 42 / 7 / 2;
    std::cout << pattern << html << ratio << '\n';
}
