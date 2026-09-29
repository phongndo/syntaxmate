// Exercise the real profiling loop with deterministic cold-work accounting.
#define main profile_cli_main
#include "../bench/profile.cpp"
#undef main

#include <set>
#include <vector>

struct RecordingConsumer {
    static inline std::vector<std::vector<std::string>> documents;
    std::set<std::string> cached;
    bool session = false;

    RecordingConsumer() { documents.clear(); }
    void prepare(std::string_view, bool) { session = true; }
    void reset() { documents.emplace_back(); }
    void run(const std::string&, std::string_view, std::string_view source, Timings& out) {
        if (session) {
            require(!documents.empty(), "session must reset before its first line");
            documents.back().emplace_back(source);
        }
        // Reset preserves the cache. A nonzero returned API total therefore
        // detects cold work leaking into a purportedly prewarmed sample.
        if (cached.emplace(source).second) out.api += 1;
        out.destroy += 1;
        ++out.calls;
        out.input_bytes += source.size();
        out.digest.text(source);
    }
};

int main() {
    try {
        const struct {
            const char* source;
            std::vector<std::string> lines;
        } inputs[] = {{"alpha\n\nbeta\n", {"alpha", "", "beta", ""}},
                      {"alpha", {"alpha"}}, {"", {""}}};
        for (const auto* mode : {"tokens", "scopes"}) {
            for (std::size_t iterations : {1, 3}) {
                for (const auto& input : inputs) {
                    const auto result = run<RecordingConsumer>(mode, "session", "rust", input.source, iterations);
                    require(result.api == 0, "cold session work entered the measured sample");
                    require(RecordingConsumer::documents.size() == iterations + 1,
                            "expected one discarded document before measured documents");
                    require(RecordingConsumer::documents.front() == input.lines,
                            "session line splitting changed");
                    Timings expected;
                    for (std::size_t i = 0; i < iterations; ++i) {
                        for (const auto& line : RecordingConsumer::documents.front()) {
                            ++expected.calls;
                            expected.input_bytes += line.size();
                            expected.digest.text(line);
                        }
                        require(RecordingConsumer::documents[i + 1] == RecordingConsumer::documents.front(),
                                "warm and measured documents must contain identical lines");
                    }
                    require(result.calls == expected.calls && result.destroy == expected.calls,
                            "warm-up leaked into measured calls or destruction");
                    require(result.input_bytes == expected.input_bytes &&
                            result.digest.value == expected.digest.value &&
                            result.digest.count == expected.digest.count,
                            "warm-up leaked into input/output accounting");
                }
            }
        }
        for (const auto* phase : {"first", "replay", "steady"}) {
            const std::size_t iterations = std::string_view(phase) == "first" ? 1 : 3;
            const auto result = run<RecordingConsumer>("tokens", phase, "rust", "alpha", iterations);
            const double cold_calls = std::string_view(phase) == "replay" ? 0 : iterations;
            require(result.api == cold_calls && result.calls == iterations && result.destroy == iterations,
                    "document phase timing/accounting changed");
        }
        std::cout << "profile warm-up and accounting checks passed\n";
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
        return 1;
    }
}
