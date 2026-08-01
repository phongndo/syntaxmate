do { \
    /* Replacement-list fragment with continued C++ states. */ \
    constexpr auto label = u8"café λ 東京 🚀 𝌆"; \
    const int values[] = {1, 2, 3}; \
    auto twice = [](int value) noexcept { \
        return value * 2; \
    }; \
    int total = 0; \
    for (const int value : values) { \
        if (value % 2 == 0) { \
            total += twice(value); \
        } else { \
            total += value; \
        } \
    } \
    const char *message = total > 0 ? "ready\n" : "idle"; \
    emit_event(label, message, total); \
} while (false)
