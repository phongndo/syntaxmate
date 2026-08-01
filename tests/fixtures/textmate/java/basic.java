package fixtures.java;

import java.util.List;

@Deprecated
final class Basic<T extends Number> {
    private final List<T> values;

    Basic(List<T> values) {
        this.values = List.copyOf(values);
    }

    /* Multiline state with café and λ;
       astral glyphs 🚀 and 𝌆 are included and closed. */
    String render(int count) {
        String document = """
                café λ launches 🚀
                rare symbol: 𝌆
                """;
        return switch (count) {
            case 0 -> "empty";
            default -> "%s (%d): %s".formatted(document.strip(), count, values);
        };
    }
}
