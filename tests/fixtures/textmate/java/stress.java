package fixtures.java;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.StringReader;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.function.Function;
import java.util.stream.Collectors;

/**
 * A compact syntax tour with café, Ελληνικά, and a rocket 🚀.
 * {@link Stress#summarize(List, String, Number[])}
 * @author fixture
 * @since 21
 * @param <T> the comparable payload
 */
@Deprecated(since = "test", forRemoval = false)
public final class Stress<T extends Number & Comparable<T>> {
    private static final long MASK = 0xCAFE_BABEL;
    private static final int BITS = 0b1010_0110;
    private static final double HEX_FLOAT = 0x1.fp3;
    private static final float SCIENCE = 6.022_140_76e+23F;
    private static final String BANNER = "Java \"stress\" — λ 🚀";
    private final List<T> values;
    private volatile boolean running = true;
    private transient int cachedHash;
    static {
        assert MASK != 0L : "mask must be initialized";
    }
    {
        cachedHash = -1;
    }
    public Stress(List<? extends T> input) {
        this.values = new ArrayList<>(input);
    }
    /**
     * Formats values and demonstrates a checked declaration.
     * @param source values to format
     * @param separator text placed between values
     * @return the formatted result
     * @throws IOException when the synthetic reader fails
     */
    @SafeVarargs
    public final String summarize(List<? super T> source, String separator, T... extras)
            throws IOException, IllegalArgumentException {
        source.addAll(values);
        source.addAll(Arrays.asList(extras));
        var joined = values.stream()
                .filter(Objects::nonNull)
                .map(Object::toString)
                .collect(Collectors.joining(separator));
        try (var reader = new BufferedReader(new StringReader(joined))) {
            return Optional.ofNullable(reader.readLine()).orElse("");
        } catch (IOException | IllegalStateException problem) {
            throw new IOException("unable to summarize", problem);
        } finally {
            cachedHash = joined.hashCode();
        }
    }
    public int classify(Object candidate) {
        if (candidate instanceof String text && !text.isBlank()) {
            return text.length();
        } else if (candidate == null) {
            return -1;
        }
        int base = switch (candidate) {
            case Integer value -> value < 0 ? -value : value;
            case Long value when value > 100L -> 100;
            default -> {
                yield candidate.hashCode() & 0xff;
            }
        };
        return base >= 10 ? base : base + BITS;
    }
    public synchronized Map<String, Integer> index(List<String> names) {
        Function<String, Integer> length = String::length;
        return names.stream().collect(Collectors.toMap(
                name -> name.strip().toLowerCase(),
                length,
                Integer::max));
    }
    public String document(String owner) {
        String template = """
                {
                  "owner": "%s",
                  "message": "café and 🚀",
                  "escaped": "\\t\\\"quoted\\\""
                }
                """;
        return template.formatted(owner);
    }
    public long arithmetic(int seed) {
        long result = (seed << 2) ^ MASK;
        result |= 0777L;
        result &= ~0b11L;
        result >>>= 1;
        for (int i = 0; i < values.size(); i++) {
            if ((i % 2 == 0) && running) {
                result += values.get(i).longValue();
            } else {
                continue;
            }
        }
        int countdown = 2;
        do {
            result--;
        } while (countdown-- > 0);
        return result;
    }
    public List<String> anonymousComparator() {
        Comparator<String> byLength = new Comparator<>() {
            @Override
            public int compare(String left, String right) {
                return Integer.compare(left.length(), right.length());
            }
        };
        List<String> output = new ArrayList<>(List.of("local: " + BANNER, "short"));
        output.sort(byLength.thenComparing(Comparator.naturalOrder()));
        return output;
    }
    public static int matrix(int[][] grid, int... offsets) {
        int total = 0;
        outer:
        for (int[] row : grid) {
            for (int cell : row) {
                if (cell < 0) break outer;
                total += cell;
            }
        }
        for (int offset : offsets) total += offset;
        return total;
    }
    public char unicodeSample() {
        char greek = 'λ';
        char newline = '\n';
        return greek > newline ? greek : newline;
    }
    public void close() {
        synchronized (this) {
            running = false;
        }
    }
    @Marker(priority = 7, tags = {"syntax", "oracle"})
    public record Snapshot(String name, Instant created, List<Integer> points)
            implements Named {
        public Snapshot {
            name = name.strip();
            points = List.copyOf(points);
        }

        @Override
        public String displayName() {
            return name + "@" + created;
        }
    }
}

@interface Marker {
    int priority() default 0;
    String[] tags() default {};
}

sealed interface Named permits Stress.Snapshot, NamedText {
    String displayName();
}

record NamedText(String displayName) implements Named {}

enum Mode {
    FAST(1),
    SAFE(2) {
        @Override
        boolean audited() { return true; }
    };

    private final int code;

    Mode(int code) { this.code = code; }

    int code() { return code; }

    boolean audited() { return false; }
}
