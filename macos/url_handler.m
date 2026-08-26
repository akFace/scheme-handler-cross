#import <Cocoa/Cocoa.h>
#import <Carbon/Carbon.h>

static void (*g_url_callback)(const char *) = NULL;
static id g_handler = nil;

@interface USHURLHandler : NSObject
@end

@implementation USHURLHandler
- (void)handleGetURLEvent:(NSAppleEventDescriptor *)event
           withReplyEvent:(NSAppleEventDescriptor *)replyEvent {
    NSAppleEventDescriptor *descriptor =
        [event paramDescriptorForKeyword:keyDirectObject];
    NSString *url = [descriptor stringValue];
    if (url && g_url_callback) {
        g_url_callback([url UTF8String]);
    }
}
@end

void ush_macos_install_url_handler(void (*callback)(const char *)) {
    g_url_callback = callback;
    if (!g_handler) {
        g_handler = [[USHURLHandler alloc] init];
    }

    NSAppleEventManager *manager = [NSAppleEventManager sharedAppleEventManager];
    [manager setEventHandler:g_handler
                 andSelector:@selector(handleGetURLEvent:withReplyEvent:)
               forEventClass:kInternetEventClass
                  andEventID:kAEGetURL];
}
