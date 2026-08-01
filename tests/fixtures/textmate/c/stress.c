#include <stdio.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#define JOIN(a, b) a ## b
#define MESSAGE "café λ"
#if defined(DEBUG) && DEBUG
#  define TRACE(fmt, ...) fprintf(stderr, fmt "\n", __VA_ARGS__)
#else
#  define TRACE(fmt, ...) ((void)0)
#endif

#define STRINGIFY_(value) #value
#define STRINGIFY(value) STRINGIFY_(value)
#define ARRAY_COUNT(array) (sizeof(array) / sizeof((array)[0]))
#define CLAMP(value, low, high) \
    ((value) < (low) ? (low) : ((value) > (high) ? (high) : (value)))
#define TYPE_NAME(value) _Generic((value), \
    int: "int", unsigned long: "unsigned long", \
    double: "double", default: "other")
#if !defined(__STDC_VERSION__) || __STDC_VERSION__ < 201112L
#  error "This stress fixture expects C11 or newer"
#elif defined(__STDC_HOSTED__) && __STDC_HOSTED__
#  define HOST_KIND "hosted"
#else
#  define HOST_KIND "freestanding"
#endif

/* C stress fixture.
 * Multi-line comment for begin/end continuation with non-ASCII text: λ🚀.
 */
/* Safe Unicode samples: Ελληνικά, हिन्दी, 中文, café, snowman ☃, music 𝄞, planet 🪐. */
static const char *text = "line one\nline two with \"quotes\"";
static const char unicode_text[] = "BMP: Ж中☕; astral: 😀🚀";
static const char escaped_text[] = "tab\t octal:\101 hex:\x42 slash:\\ question:\?";
typedef unsigned long object_id;
typedef int (*binary_fn)(int left, int right);
typedef void (*visitor_fn)(const void *item, size_t index);
enum color {
    COLOR_RED = 1,
    COLOR_GREEN = 1 << 1,
    COLOR_BLUE = 1 << 2,
    COLOR_ALL = COLOR_RED | COLOR_GREEN | COLOR_BLUE
};
typedef enum status {
    STATUS_OK,
    STATUS_RETRY = 10,
    STATUS_FAILED
} status_t;
struct point {
    double x;
    double y;
};
struct record {
    object_id id;
    const char *name;
    struct point position;
    unsigned visible : 1;
    unsigned priority : 3;
    enum color color;
};
union scalar {
    int signed_value;
    unsigned unsigned_value;
    float real_value;
    unsigned char bytes[sizeof(float)];
};
struct packet {
    uint16_t kind;
    uint16_t length;
    unsigned char payload[];
};
_Static_assert(sizeof(uint32_t) == 4, "uint32_t must be four bytes");
_Static_assert(_Alignof(struct record) >= _Alignof(char), "record alignment");
static const struct point origin = { .x = 0.0, .y = -0.0 };
static struct record records[3] = {
    [0] = { .id = 0x2aUL, .name = "alpha", .position = { 1.0, 2.0 },
            .visible = 1, .priority = 3, .color = COLOR_RED },
    [2] = { .id = 0755UL, .name = "omega", .position = { .y = 4.5 },
            .visible = 1, .priority = 7, .color = COLOR_BLUE }
};
static union scalar scalar_value = { .unsigned_value = UINT32_C(0xdecafbad) };
static const int sparse_values[] = { [0] = 1, [3] = 8, [7] = 21 };
static volatile unsigned signal_count;
static int add(int left, int right)
{
    return left + right;
}
static int multiply(int left, int right)
{
    return left * right;
}
static int apply(binary_fn operation, int left, int right)
{
    return operation != NULL ? (*operation)(left, right) : 0;
}
static void scale_points(struct point *restrict points,
                         size_t count, const double factor)
{
    for (size_t i = 0; i < count; ++i) {
        points[i].x *= factor;
        points[i].y *= factor;
    }
}
static status_t classify(int value)
{
    switch (value) {
    case 0:
        return STATUS_OK;
    case 1:
    case 2:
        return STATUS_RETRY;
    default:
        return value < 0 ? STATUS_FAILED : STATUS_OK;
    }
}
static unsigned rotate_left(unsigned value, unsigned distance)
{
    const unsigned width = (unsigned)(sizeof(value) * 8U);
    distance %= width;
    return distance == 0U ? value
                          : (value << distance) | (value >> (width - distance));
}
static size_t copy_trimmed(char *restrict destination, size_t capacity,
                           const char *restrict source)
{
    size_t used = 0;
    while (*source != '\0' && used + 1U < capacity) {
        if (*source != ' ' && *source != '\t' && *source != '\n') {
            destination[used++] = *source;
        }
        ++source;
    }
    if (capacity > 0U) {
        destination[used] = '\0';
    }
    return used;
}
static int sum_matrix(size_t rows, size_t columns,
                      const int matrix[rows][columns])
{
    int sum = 0;
    for (size_t row = 0; row < rows; ++row) {
        for (size_t column = 0; column < columns; ++column) {
            sum += matrix[row][column];
        }
    }
    return sum;
}
static void visit_ids(const struct record *begin, const struct record *end,
                      visitor_fn visitor)
{
    for (const struct record *cursor = begin; cursor != end; ++cursor) {
        visitor(&cursor->id, (size_t)(cursor - begin));
    }
}
static void print_id(const void *item, size_t index)
{
    const object_id *id = item;
    printf("id[%zu]=%lu\n", index, *id);
}
int main(void) {
    int total = 42;
    int count = 6;
    printf("%s %s %d\n", MESSAGE, text, total / count);
    binary_fn operations[] = { add, multiply };
    struct point path[] = { origin, (struct point){ .x = 3.0, .y = 4.0 } };
    const int matrix[2][3] = { { 1, 2, 3 }, { 4, 5, 6 } };
    char buffer[32];
    unsigned flags = COLOR_RED | COLOR_BLUE;
    int result = apply(operations[0], total, count);
    scale_points(path, ARRAY_COUNT(path), 0x1.8p+1);
    result += sum_matrix(2, 3, matrix) + sparse_values[3];
    result += (int)copy_trimmed(buffer, sizeof buffer, " spaced text ");
    flags ^= COLOR_GREEN;
    flags &= ~COLOR_RED;
    signal_count++;
    if ((flags & COLOR_BLUE) != 0U && classify(result) != STATUS_FAILED) {
        printf("%s/%s %s: %d, %.2f, %zu\n", HOST_KIND, STRINGIFY(COLOR_BLUE),
               TYPE_NAME(result), CLAMP(result, 0, 100), path[1].x,
               ARRAY_COUNT(sparse_values));
    } else {
        goto cleanup;
    }
    do {
        result--;
    } while (result > 90);
    for (int digit = '0'; digit <= '3'; digit++) {
        putchar(digit);
    }
    putchar('\n');
    visit_ids(records, records + ARRAY_COUNT(records), print_id);
cleanup:
    printf("%s | %s | 0x%08x | %g\n", unicode_text, escaped_text,
           rotate_left(scalar_value.unsigned_value, 5U), 6.02214076e23);
    return 0;
}
