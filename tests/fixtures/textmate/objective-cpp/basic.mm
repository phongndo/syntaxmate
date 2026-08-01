#import <Foundation/Foundation.h>
#include <string>
#include <vector>

template <typename T> T doubled(T value) { return value + value; }

@protocol MKRenderable <NSObject>
- (NSString *)renderValue:(NSInteger)value;
@end

@interface MKCppGreeter : NSObject <MKRenderable>
@property (nonatomic, copy) NSString *prefix;
- (void)visitValues:(void (^)(NSString *text))visitor;
@end

@implementation MKCppGreeter
- (NSString *)renderValue:(NSInteger)value {
    auto decorate = [suffix = std::string{" λ🚀"}](long n) {
        return std::to_string(doubled(n)) + suffix;
    };
    return [NSString stringWithFormat:@"%@ — café %s", self.prefix,
            decorate(value).c_str()];
}
- (void)visitValues:(void (^)(NSString *))visitor {
    for (int value : std::vector<int>{1, 2, 3}) visitor([self renderValue:value]);
}
@end
