/* C++ macro-fragment stress fixture: café λ 東京 🚀 𝌆. */
#pragma message("tokenizing continued replacement lists")
#line 100 "cpp-macro-stress.cpp"
#if defined(MARK_ENABLED) && !defined(MARK_DISABLED)
#define MARK_MODE 0x2Au
#else
#define MARK_MODE 0b0011u
#endif
#undef MARK_DISABLED

#define MARK_TEXT_VALUES(Name) \
    constexpr const char *Name##_utf8 = u8"café λ 東京 🚀 𝌆"; \
    constexpr const wchar_t *Name##_wide = L"wide 東京"; \
    constexpr char Name##_newline = '\n'; \
    constexpr auto Name##_escaped = "quote:\" slash:\\ tab:\t"; \
    constexpr auto Name##_raw = R"mark({"route":"/東京/🚀","glyph":"𝌆"})mark"; \
    static_assert(sizeof(Name##_newline) == sizeof(char))

#define MARK_NUMERIC_VALUES(Name) \
    constexpr unsigned Name##_binary = 0b1010'0101u; \
    constexpr unsigned Name##_octal = 0755u; \
    constexpr unsigned Name##_decimal = 1'000'000u; \
    constexpr unsigned Name##_hex = 0xDEAD'BEEFu; \
    constexpr double Name##_fraction = 6.022e23; \
    constexpr long double Name##_hexfloat = 0x1.fp+3L; \
    constexpr bool Name##_ok = Name##_decimal > Name##_hexfloat

#define MARK_ATTRIBUTES(Name) \
    struct [[nodiscard, maybe_unused]] Name { \
        alignas(16) unsigned char bytes[16]; \
        __declspec(deprecated) int legacy; \
        int packed __attribute__((aligned(8))); \
    }; \
    static_assert(alignof(Name) >= 8)

#define MARK_RECORD(Name, ValueType) \
    class Name final : public record_base<ValueType> { \
    public: \
        using value_type = ValueType; \
        explicit Name(ValueType value) noexcept \
            : value_{static_cast<ValueType>(value)} {} \
        Name(const Name &) = default; \
        Name(Name &&) noexcept = default; \
        ~Name() override = default; \
        Name &operator=(const Name &) = default; \
        Name &operator+=(ValueType delta) noexcept { \
            value_ += delta; \
            return *this; \
        } \
        [[nodiscard]] ValueType value() const noexcept { \
            return value_; \
        } \
        friend bool operator==(const Name &, const Name &) = default; \
    private: \
        ValueType value_{}; \
    }

#define MARK_AGGREGATES(Prefix) \
    struct Prefix##Point { \
        double x; \
        double y; \
    }; \
    union Prefix##Bits { \
        unsigned word; \
        unsigned char bytes[sizeof(unsigned)]; \
    }; \
    enum class Prefix##State : unsigned char { \
        idle = 0, \
        ready = 1, \
        failed = 2 \
    }; \
    typedef struct Prefix##Legacy { int code; } Prefix##Legacy; \
    typedef void (*Prefix##Callback)(int, const char *)

#define MARK_TEMPLATE(Container) \
    template <typename T, unsigned Extent = 4> \
    struct Container { \
        using element_type = T; \
        T storage[Extent]{}; \
        constexpr T &operator[](unsigned index) noexcept { \
            return storage[index]; \
        } \
        constexpr const T &operator[](unsigned index) const noexcept { \
            return storage[index]; \
        } \
        template <typename Function> \
        constexpr void each(Function &&function) { \
            for (T &item : storage) { \
                function(item); \
            } \
        } \
    }; \
    template struct Container<int, 8>

#define MARK_FUNCTION(Name, Type) \
    [[nodiscard]] inline Type Name( \
        const Type *values, \
        unsigned count, \
        Type fallback = Type{}) noexcept { \
        if (values == nullptr || count == 0) { \
            return fallback; \
        } \
        Type result{}; \
        for (unsigned index = 0; index < count; ++index) { \
            result += values[index]; \
        } \
        return result; \
    }

#define MARK_CONTROL_FLOW(Name, expression) \
    do { \
        auto Name##_value = (expression); \
        switch (Name##_value) { \
        case 0: \
            log_message("zero"); \
            break; \
        case 1: \
            log_message("one"); \
            [[fallthrough]]; \
        default: \
            log_message(Name##_value > 0 ? "positive" : "negative"); \
            break; \
        } \
    } while (false)

#define MARK_LAMBDA(Name, capture) \
    auto Name = [state = (capture)](auto &&value) mutable noexcept \
        -> decltype(auto) { \
        state += 1; \
        if constexpr (requires { value.size(); }) { \
            return value.size() + static_cast<unsigned>(state); \
        } else { \
            return std::forward<decltype(value)>(value); \
        } \
    }

#define MARK_CASTS(Name, pointer) \
    auto Name##_address = reinterpret_cast<std::uintptr_t>(pointer); \
    auto Name##_base = static_cast<base_type *>(pointer); \
    auto Name##_derived = dynamic_cast<derived_type *>(Name##_base); \
    auto Name##_mutable = const_cast<char *>(Name##_derived->data()); \
    auto Name##_type = typeid(*Name##_derived); \
    auto Name##_size = sizeof(*Name##_derived); \
    auto Name##_alignment = alignof(derived_type); \
    static_assert(noexcept(Name##_derived->data()))

#define MARK_EXCEPTION(Name, operation) \
    try { \
        (operation)(); \
    } catch (const parse_error &Name##_error) { \
        report(Name##_error.what()); \
    } catch (...) { \
        throw runtime_error{"unknown macro failure"}; \
    }

#define MARK_NAMESPACE(Name) \
    namespace Name { \
    inline namespace literals { \
        constexpr long double operator""_unit(long double value) { \
            return value * 10.0L; \
        } \
    } \
    using namespace literals; \
    using size_type = unsigned long; \
    namespace alias = Name::literals; \
    extern "C" { \
        void Name##_hook(int); \
    } \
    }

#define MARK_ASSEMBLY(result, input) \
    asm volatile ( \
        "add %1, %0" \
        : "=r"(result) \
        : "r"(input), "0"(result) \
        : "cc" \
    )

#define MARK_NESTED_INITIALIZERS(Name) \
    const config Name { \
        .title = "café 東京", \
        .dimensions = {1920, 1080}, \
        .flags = MARK_MODE | 0x10u, \
        .callback = [](int code) { \
            return code >= 0 && code != 13; \
        }, \
    }

#define MARK_COMMENTED(Name) \
    /* A block comment remains open across a continued physical line: \
       BMP λ 東京 and astral 🚀 𝌆 are still comment content. */ \
    auto Name = object \
        .member() \
        ->template convert<vector<int>>() \
        [0]

#define MARK_CONDITIONAL(Name, left, right) \
    constexpr auto Name = \
        ((left) && (right)) \
            ? ((left) + (right)) \
            : ((left) xor (right)); \
    static_assert(Name >= 0, "closed conditional expression")

MARK_TEXT_VALUES(unicode);
MARK_NUMERIC_VALUES(numbers);
MARK_ATTRIBUTES(AlignedRecord);
MARK_RECORD(Counter, long);
MARK_AGGREGATES(Event);
MARK_TEMPLATE(FixedBuffer);
MARK_FUNCTION(sum_values, long);
MARK_CONTROL_FLOW(dispatch, MARK_MODE);
MARK_LAMBDA(project, 3);
MARK_NAMESPACE(macro_fixture);
