#include <stdio.h>

typedef struct {
    const char *label;
    int values[3];
} Sample;

static int sum(const Sample *sample) {
    int total = 0;
    for (int i = 0; i < 3; ++i) {
        total += sample->values[i];
    }
    return total;
}

int main(void) {
    Sample sample = {"rocket 🚀", {2, 3, 5}};
    printf("%s: %d\n", sample.label, sum(&sample));
    return 0;
}
