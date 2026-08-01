#include <iostream>
#include <string>
#include <vector>

template <typename T>
T doubled(const T &value) {
    return value + value;
}

struct Greeting {
    std::string name;
    int count;
};

int main() {
    const Greeting greeting{"rocket 🚀", 3};
    std::vector<int> values{1, 2, greeting.count};
    for (const auto value : values) {
        std::cout << greeting.name << ": " << doubled(value) << '\n';
    }
}
