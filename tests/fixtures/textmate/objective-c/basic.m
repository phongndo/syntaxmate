#import <Foundation/Foundation.h>
#define MKClamp(value, low, high) MIN(MAX((value), (low)), (high))

@protocol MKGreeting <NSObject>
@required
- (NSString *)greetingForName:(NSString *)name;
@optional
@property (nonatomic, readonly) BOOL enthusiastic;
@end

@interface MKGreeter : NSObject <MKGreeting>
@property (nonatomic, copy, nullable) NSString *prefix;
- (instancetype)initWithPrefix:(NSString *)prefix;
@end

@implementation MKGreeter
@synthesize prefix = _prefix;
- (instancetype)initWithPrefix:(NSString *)prefix { self = [super init]; if (self) { _prefix = [prefix copy]; } return self; }
- (NSString *)greetingForName:(NSString *)name {
    NSString *(^decorate)(NSString *) = ^NSString *(NSString *text) { return [text uppercaseString]; };
    NSInteger count = MKClamp(0x2A, 0, 100);
    return [NSString stringWithFormat:@"%@, %@ — café λ 🚀 #%ld", self.prefix, decorate(name), (long)count];
}
@end
