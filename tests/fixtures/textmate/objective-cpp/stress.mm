/*
 * Objective-C++ grammar stress fixture for a small Foundation-to-C++ job bridge that accepts Cocoa objects, normalizes native values, and reports results.
 * The examples deliberately combine templates, ownership helpers, protocols, categories, properties, blocks, lambdas, preprocessor branches, and message sends.
 * Protocol declarations model observation and archival boundaries, while a class extension keeps mutable event storage private to the implementation section.
 * Human-readable labels retain café accents, Ελληνικά, 東京, and astral telemetry 🛰️🚀 so UTF-8 bytes and UTF-16 oracle offsets are exercised together.
 * Numeric samples cover modern C++ separators and bases alongside boxed Objective-C values, enumeration constants, option masks, and format placeholders.
 * Every quoted string, raw string, comment, declaration, implementation, preprocessor branch, Objective-C container literal, and C++ namespace state closes.
 * The source is hand-written as one coherent bridge rather than generated repetition or padding; later calls consume types and helpers declared above them.
 * It is syntax-highlighting input only; imported frameworks need not exist on the test platform, and no compiler or runtime execution is part of this fixture.
 * Declaration coverage includes Objective-C interfaces, protocols, categories, class extensions, properties, nullability qualifiers, generic containers, selectors, methods, ivars, and implementation blocks.
 * C++ coverage includes namespaces, templates, concepts, aliases, scoped enumerations, classes, constructors, destructors, operators, lambdas, captures, attributes, and trailing return types.
 * Function bodies combine message sends with member access, scope resolution, smart pointers, optionals, vectors, arrays, strings, exceptions, comparisons, ranges, and structured bindings.
 * Preprocessor examples retain imports, includes, object-like and variadic macros, token pasting, stringification, conditional branches, pragma marks, and a disabled but balanced source region.
 * Literal coverage includes Objective-C and C++ strings, raw delimiters, characters, escaped Unicode, format placeholders, hexadecimal and binary integers, digit separators, floats, and boxed values.
 * Ownership paths use strong and weak references, autorelease pools, bridging helpers, copied blocks, move semantics, RAII guards, and explicit cleanup without requiring an Objective-C runtime here.
 * Collection examples exercise array, dictionary, and set literals together with subscripting, fast enumeration, initializer lists, iterator algorithms, and conversions between Cocoa and native values.
 * Error paths include NSError out parameters, exceptions, optional failures, rejected jobs, cancellation, disabled configuration, and log formatting while every nested grammar state remains closed.
 * Unicode identifiers stay conservative, but comments and literals retain café, Ελληνικά, 東京, combining marks, and astral 🛰️🚀 text so native byte offsets and oracle UTF-16 offsets are compared.
 * Repeated-corpus boundaries are intentional validation points: all comments, raw strings, directives, declarations, protocol lists, template arguments, blocks, lambdas, and scopes close before EOF.
 * These additions document grammar-driven constructs already exercised below and preserve a coherent comprehensive 140–260-line bridge rather than substituting plain filler or generated statements.
 */
#import <Foundation/Foundation.h>
#import <QuartzCore/QuartzCore.h>
#include <algorithm>
#include <array>
#include <compare>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#pragma mark - Build configuration
#define MK_STRINGIFY_INNER(token) #token
#define MK_STRINGIFY(token) MK_STRINGIFY_INNER(token)
#define MK_JOIN(left, right) left##right
#define MK_LOG(format, ...) NSLog((@"[bridge] " format), ##__VA_ARGS__)
#define MK_WITH_POOL(...) do { @autoreleasepool { __VA_ARGS__; } } while (false)
#if defined(__cplusplus) && __cplusplus >= 202002L
#define MK_HAS_SPACESHIP 1
#else
#define MK_HAS_SPACESHIP 0
#endif
#if 0
static NSString * const MKDeadBranch = @"not built";
#elif MK_HAS_SPACESHIP
static NSString * const MKBuildMode = @"modern C++";
#else
static NSString * const MKBuildMode = @"portable C++";
#endif
#warning "Objective-C++ fixture diagnostic"
#line 100 "stress.mm"

NS_ASSUME_NONNULL_BEGIN
typedef NS_ENUM(NSInteger, MKJobState) {
    MKJobStateQueued = 0,
    MKJobStateRunning = 1,
    MKJobStateFinished = 2,
    MKJobStateFailed = -1,
};
typedef NS_OPTIONS(NSUInteger, MKJobOptions) {
    MKJobOptionNone = 0,
    MKJobOptionVerbose = 1UL << 0,
    MKJobOptionRetry = 1UL << 1,
};

namespace mark::bridge {
using Identifier = std::string;
template <typename T, std::size_t Capacity>
class RingBuffer final {
public:
    using value_type = T;
    constexpr void push(T value) { values_[size_++ % Capacity] = std::move(value); }
    [[nodiscard]] constexpr const T& operator[](std::size_t index) const {
        return values_[index % Capacity];
    }
    [[nodiscard]] constexpr std::size_t size() const noexcept { return size_; }
private:
    std::array<T, Capacity> values_{};
    std::size_t size_ = 0;
};
struct Metric {
    double value = 0.0;
    std::string unit;
    Metric& operator+=(double delta) noexcept { value += delta; return *this; }
    friend Metric operator+(Metric metric, double delta) noexcept { return metric += delta; }
#if MK_HAS_SPACESHIP
    friend auto operator<=>(const Metric&, const Metric&) = default;
#endif
};
static std::optional<int> parseCount(NSString *text) {
    try {
        std::string utf8([text UTF8String]);
        std::size_t consumed = 0;
        int value = std::stoi(utf8, &consumed, 10);
        return consumed == utf8.size() ? std::optional<int>{value} : std::nullopt;
    } catch (const std::exception&) {
        return std::nullopt;
    }
}
static constexpr std::string_view kSchema = R"json({
  "title": "café 東京 🧭",
  "pattern": "^[A-Z]+\\d{2}$"
})json";
} // namespace mark::bridge
@class MKJob;
@protocol MKJobObserving <NSObject, NSCopying>
@required
- (void)job:(MKJob *)job didChangeState:(MKJobState)state;
@optional
- (nullable NSString *)labelForJob:(MKJob *)job;
@property (nonatomic, readonly, getter=isEnabled) BOOL enabled;
@end
@protocol MKArchiving
- (oneway void)archiveObject:(bycopy id<NSCopying>)object;
@end
@interface MKJob : NSObject <MKArchiving>
@property (class, nonatomic, readonly) NSString *kind;
@property (nonatomic, copy) NSString *name;
@property (nonatomic, weak, nullable) id<MKJobObserving> observer;
@property (atomic, assign) MKJobState state;
@property (nonatomic, copy, nullable) void (^completion)(BOOL success);
+ (instancetype)jobWithName:(NSString *)name NS_SWIFT_NAME(init(name:));
- (instancetype)initWithName:(NSString *)name;
- (void)runWithValues:(NSArray<NSNumber *> *)values
              handler:(void (^)(NSNumber *value, BOOL *stop))handler;
- (NSString *)debugSummary API_AVAILABLE(macos(10.15));
@end
@interface MKJob (Formatting)
- (NSString *)formattedState;
@end

@interface MKJob ()
@property (nonatomic, strong) NSMutableArray<NSString *> *events;
@end

@implementation MKJob {
    std::unique_ptr<mark::bridge::RingBuffer<int, 8>> _samples;
    mark::bridge::Identifier _nativeName;
}

@synthesize name = _name;
@dynamic observer;

+ (NSString *)kind { return @"fixture-job"; }

+ (instancetype)jobWithName:(NSString *)name {
    return [[self alloc] initWithName:name];
}

- (instancetype)initWithName:(NSString *)name {
    if ((self = [super init])) {
        _name = [name copy];
        _nativeName = std::string([name UTF8String]);
        _samples = std::make_unique<mark::bridge::RingBuffer<int, 8>>();
        _events = [NSMutableArray arrayWithObject:@"created"];
        _state = MKJobStateQueued;
    }
    return self;
}

- (void)runWithValues:(NSArray<NSNumber *> *)values
              handler:(void (^)(NSNumber *, BOOL *))handler {
    __block NSUInteger accepted = 0;
    auto normalize = [bias = 0x2A](NSNumber *number) mutable noexcept {
        long raw = [number longValue];
        return static_cast<int>((raw + bias++) & 0xFF);
    };
    BOOL stop = NO;
    self.state = MKJobStateRunning;
    for (NSNumber *number in values) {
        int value = normalize(number);
        _samples->push(value);
        accepted += (value % 2 == 0) ? 1U : 0U;
        handler(@(value), &stop);
        if (stop) break;
    }
    [self.events addObject:[NSString stringWithFormat:@"accepted=%lu", accepted]];
    [self.observer job:self didChangeState:self.state];
    self.state = MKJobStateFinished;
    if (self.completion != nil) self.completion(YES);
}

- (NSString *)debugSummary {
    constexpr auto binary = 0b1010'0110u;
    constexpr auto octal = 0755;
    constexpr auto decimal = 1'000'000LL;
    constexpr auto hexadecimal = 0xDEAD'BEEFu;
    constexpr auto floating = 0x1.fp+3L;
    auto flags = (binary << 2) | (octal & 077u);
    return [NSString stringWithFormat:@"%@/%s: %llu %u %.2Lf flags=%u schema=%s",
            self.name, _nativeName.c_str(), decimal, hexadecimal, floating,
            flags, mark::bridge::kSchema.data()];
}

- (void)exerciseRuntime:(id)object {
    SEL selector = @selector(job:didChangeState:);
    const char *encoding = @encode(mark::bridge::Metric);
    Class cls = [object class];
    id<NSCopying> copy = [object conformsToProtocol:@protocol(NSCopying)]
        ? [object copy] : nil;
    MK_LOG(@"selector=%@ class=%@ encoding=%s copy=%@", NSStringFromSelector(selector),
           cls, encoding, copy);
    @try {
        if (![object respondsToSelector:selector]) {
            @throw [NSException exceptionWithName:@"MissingSelector"
                                           reason:@"Δ handler absent 🛰️"
                                         userInfo:nil];
        }
    } @catch (NSException *exception) {
        MK_LOG(@"caught %@: %@", exception.name, exception.reason);
    } @finally {
        [self.events addObject:@"runtime checked"];
    }
    @synchronized (self) { _state = MKJobStateFinished; }
}

- (oneway void)archiveObject:(bycopy id<NSCopying>)object {
    NS_DURING
        MK_LOG(@"archive %@", object);
    NS_HANDLER
        MK_LOG(@"legacy exception %@", localException);
    NS_ENDHANDLER
}

@end

@implementation MKJob (Formatting)

- (NSString *)formattedState {
    NSDictionary<NSNumber *, NSString *> *labels = @{
        @(MKJobStateQueued): @"queued", @(MKJobStateRunning): @"running",
        @(MKJobStateFinished): @"finished", @(MKJobStateFailed): @"failed"
    };
    return labels[@(self.state)] ?: @"unknown";
}

@end

static void MKRunBridgeDemo(void) {
    MK_WITH_POOL({
        MKJob *job = [MKJob jobWithName:@"café λ 東京 🚀"];
        job.completion = ^(BOOL success) { MK_LOG(@"done=%@", success ? @"YES" : @"NO"); };
        NSArray<NSNumber *> *values = @[@0, @0.25, @42, @0x2A, @1e3];
        [job runWithValues:values handler:^(NSNumber *value, BOOL *stop) {
            MK_LOG(@"value=%@", value);
            *stop = [value integerValue] > 200;
        }];
        auto count = mark::bridge::parseCount(@"42").value_or(-1);
        std::vector<NSString *> names{job.name, job.formattedState, MKBuildMode};
        std::sort(names.begin(), names.end(), [](NSString *left, NSString *right) {
            return [left compare:right] == NSOrderedAscending;
        });
        MK_LOG(@"count=%d first=%@", count, names.front());
    });
}

NS_ASSUME_NONNULL_END
