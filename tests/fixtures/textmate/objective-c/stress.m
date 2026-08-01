/* = Objective-C grammar stress fixture = */
#import <Foundation/Foundation.h>
#include <stdint.h>
#import "MKCompatibility.h"
#pragma mark - Build configuration
#define MK_HEX_MASK 0xFF00u
#define MK_BINARY_FLAGS 0b1010u
#define MK_SQUARE(x) ((x) * (x))
#define MK_LOG(fmt, ...) NSLog((@"[mark] " fmt), ##__VA_ARGS__)
#define MK_WITH_LOCK(lock, code) do { \
    @synchronized (lock) { code; } \
} while (0)
#if 1
static NSString * const MKBuildFlavor = @"enabled";
#else
static NSString * const MKBuildFlavor = @"disabled";
#endif
#if 0
static void MKNeverBuilt(void) { NSLog(@"dead branch"); }
#elif 1
static const BOOL MKFeatureCompiled = YES;
#else
static const BOOL MKFeatureCompiled = NO;
#endif
#if defined(DEBUG) && !defined(MK_SILENT)
#define MK_TRACE 1
#else
#define MK_TRACE 0
#endif
#warning "Fixture-only diagnostic"
#line 40 "objective-c-stress.m"
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
#undef MK_UNUSED_FLAG
typedef NS_ENUM(NSInteger, MKTaskState) {
    MKTaskStatePending = 0,
    MKTaskStateRunning = 1,
    MKTaskStateFinished = 2,
    MKTaskStateFailed = -1,
};
typedef struct MKCoordinate {
    double latitude;
    double longitude;
} MKCoordinate;
static inline double MKDistanceSquared(MKCoordinate a, MKCoordinate b) API_AVAILABLE(macos(10.15), ios(13.0)) {
    double dx = a.latitude - b.latitude;
    double dy = a.longitude - b.longitude;
    return dx * dx + dy * dy;
}
static int MKAccumulate(const int *values, size_t count) NS_SWIFT_NAME(accumulate(_:count:)) {
    int total = 0;
    for (size_t index = 0; index < count; ++index) {
        if (values[index] < 0) { continue; }
        total += values[index];
    }
    return total;
}
static const char *MKStateCString(MKTaskState state) {
    switch (state) {
        case MKTaskStatePending: return "pending";
        case MKTaskStateRunning: return "running";
        case MKTaskStateFinished: return "finished";
        default: return "failed";
    }
}
// = Protocols and object model =
@class MKTask;
@protocol MKTaskObserving <NSObject, NSCopying>
@required
- (void)task:(MKTask *)task didChangeState:(MKTaskState)state;
@optional
- (nullable NSString *)labelForTask:(MKTask *)task;
@property (nonatomic, readonly, getter=isEnabled) BOOL enabled;
@end
@protocol MKArchiving
- (oneway void)archiveObject:(bycopy id<NSCopying>)object;
@end
@interface MKTask : NSObject <NSCopying, MKArchiving> {
@private
    NSString *_identifier;
@protected
    MKTaskState _state;
@package
    NSUInteger _retryCount;
}
@property (class, nonatomic, readonly) NSString *kind;
@property (nonatomic, copy, nonnull) NSString *title;
@property (nonatomic, weak, nullable) id<MKTaskObserving> observer;
@property (atomic, assign) MKTaskState state;
@property (nonatomic, readonly) NSString *displayName;
+ (instancetype)taskWithTitle:(NSString *)title;
- (instancetype)initWithTitle:(NSString *)title;
- (void)performWithCompletion:(void (^ _Nullable)(BOOL success, NSError * _Nullable error))completion;
- (id<NSCopying>)snapshot;
@end
@interface MKTask (Formatting)
- (NSString *)formattedState;
@end
@implementation MKTask
@synthesize title = _title;
@synthesize observer = _observer;
@synthesize state = _state;
+ (NSString *)kind { return @"task"; }
+ (instancetype)taskWithTitle:(NSString *)title { return [[self alloc] initWithTitle:title]; }
- (instancetype)initWithTitle:(NSString *)title {
    self = [super init];
    if (self != nil) {
        _identifier = [[NSUUID UUID] UUIDString];
        _title = [title copy];
        _state = MKTaskStatePending;
        _retryCount = 0u;
    }
    return self;
}
- (NSString *)displayName { return [NSString stringWithFormat:@"%@ (%@)", self.title, _identifier]; }
- (id)copyWithZone:(NSZone *)zone {
    MKTask *copy = [[[self class] allocWithZone:zone] initWithTitle:self.title];
    copy.state = self.state;
    return copy;
}
- (id<NSCopying>)snapshot { return [self copy]; }
- (oneway void)archiveObject:(bycopy id<NSCopying>)object {
    MK_LOG(@"archive %@", object);
}
- (void)performWithCompletion:(void (^)(BOOL, NSError *))completion {
    __block NSInteger attempts = 0;
    NSArray<NSNumber *> *delays = @[@0, @0.25, @1e1, @0x1.fp3];
    void (^work)(void) = ^{
        attempts++;
        self.state = attempts > 1 ? MKTaskStateFinished : MKTaskStateRunning;
        [self.observer task:self didChangeState:self.state];
        if (completion != nil) {
            completion(YES, nil);
        }
    };
    MK_WITH_LOCK(self, work());
    MK_LOG(@"delays=%@ attempts=%03ld", delays, (long)attempts);
}
- (void)exerciseRuntimeFeatures {
    SEL action = @selector(task:didChangeState:);
    const char *encoding = @encode(MKCoordinate);
    Class cls = [self class];
    BOOL responds = [self respondsToSelector:action];
    id<NSCopying> value = responds ? [self snapshot] : nil;
    NSLog(@"class=%@ encoding=%s value=%@", cls, encoding, value);
    @try {
        if (self.title.length == 0) {
            @throw [NSException exceptionWithName:@"MKEmptyTitle" reason:@"Title must not be empty" userInfo:nil];
        }
    } @catch (NSException *exception) {
        MK_LOG(@"caught %@: %@", exception.name, exception.reason);
    } @finally {
        _retryCount++;
    }
    @synchronized (self) {
        _state = MKTaskStateFinished;
    }
}
- (void)legacyFoundationMacros {
    NS_DURING
        MK_LOG(@"legacy body");
    NS_HANDLER
        MK_LOG(@"legacy exception %@", localException);
    NS_ENDHANDLER
}
@end
@implementation MKTask (Formatting)
- (NSString *)formattedState {
    NSDictionary<NSNumber *, NSString *> *names = @{ @(MKTaskStatePending): @"pending",
        @(MKTaskStateRunning): @"running", @(MKTaskStateFinished): @"finished",
        @(MKTaskStateFailed): @"failed" };
    return names[@(self.state)] ?: @"unknown";
}
@end
@interface MKTaskStore : NSObject
@property (nonatomic, strong) NSMutableArray<MKTask *> *tasks;
@property (nonatomic, copy) void (^changeHandler)(NSArray<MKTask *> *tasks);
- (NSArray<MKTask *> *)tasksMatchingText:(NSString *)text;
@end
@implementation MKTaskStore
- (instancetype)init {
    if ((self = [super init])) {
        _tasks = [NSMutableArray array];
    }
    return self;
}
- (NSArray<MKTask *> *)tasksMatchingText:(NSString *)text {
    NSPredicate *predicate = [NSPredicate predicateWithFormat:@"ANY title CONTAINS[CI] %@ AND state != %ld" argumentArray:@[text, @(MKTaskStateFailed)]];
    NSArray *filtered = [self.tasks filteredArrayUsingPredicate:predicate];
    return [filtered sortedArrayUsingComparator:^NSComparisonResult(MKTask *left, MKTask *right) {
        return [left.title localizedCaseInsensitiveCompare:right.title];
    }];
}
- (void)enumerateTasks {
    [self.tasks enumerateObjectsUsingBlock:^(MKTask *task, NSUInteger index, BOOL *stop) {
        NSLog(@"%2$@ [%1$lu] = %@", (unsigned long)index, @"task", task.displayName);
        if (index >= 10) {
            *stop = YES;
        }
    }];
}
@end
static void MKRunDemo(void) {
    @autoreleasepool {
        MKTaskStore *store = [[MKTaskStore alloc] init];
        NSArray<NSString *> *titles = @[@"café review", @"東京 sync", @"launch 🛰️"];
        for (NSString *title in titles) {
            MKTask *task = [MKTask taskWithTitle:title];
            [store.tasks addObject:task];
        }
        NSArray *matches = [store tasksMatchingText:@"é"];
        NSLog(@"%@ %@ count=%lu ratio=%.2f", MKBuildFlavor, matches, (unsigned long)matches.count, 3.14159);
    }
}
#if TARGET_OS_OSX
static NSRect MKDefaultFrame(void) {
    return NSMakeRect(10.0, 20.0, 640.0, 480.0);
}
#endif
#pragma clang diagnostic pop
