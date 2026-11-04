// A local ONI's occlusion answers, drawables and alpha, logged: `clang -dynamiclib -framework AppKit -framework QuartzCore -framework Metal`, run under `DYLD_INSERT_LIBRARIES` in a copy signed without the hardened runtime.
// `ONI_PROBE_LOG` names the log; `ONI_PROBE_OCC=alpha0` hides Visible at alpha 0, `=flip` until `ONI_PROBE_FLIP`'s file exists; `ONI_PROBE_RESIZE_AT` resizes the window after that many seconds.
#import <AppKit/AppKit.h>
#import <Metal/Metal.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>
#include <fcntl.h>
#include <mach/mach_time.h>
#include <stdlib.h>
#include <unistd.h>

static int out = 2;
static uint64_t start;
static mach_timebase_info_data_t timebase;
static const char *occlusion_mode;
static const char *flip_file;
static BOOL flipped;

static double now(void) { return (double)(mach_absolute_time() - start) * timebase.numer / timebase.denom / 1e9; }
#define LOG(format, ...) dprintf(out, "%8.3f " format "\n", now(), ##__VA_ARGS__)

static NSUInteger (*original_occlusion)(id, SEL);
static NSUInteger recorded_occlusion(id self, SEL _cmd) {
    NSWindow *window = self;
    NSUInteger real = original_occlusion(self, _cmd), said = real;
    if (occlusion_mode && !strcmp(occlusion_mode, "alpha0") && window.alphaValue == 0.0) said &= ~NSWindowOcclusionStateVisible;
    if (occlusion_mode && !strcmp(occlusion_mode, "flip") && !flipped) said &= ~NSWindowOcclusionStateVisible;
    static NSUInteger last_real = NSUIntegerMax, last_said = NSUIntegerMax;
    static void *last_window;
    if (last_real != real || last_said != said || last_window != (void *)self) {
        LOG("occlusionState %p real=0x%lx said=0x%lx alpha=%.2f visible=%d screen=%s", self, real, said, window.alphaValue, window.isVisible, window.screen.localizedName.UTF8String ?: "none");
        last_real = real, last_said = said, last_window = (void *)self;
    }
    return said;
}

static void (*original_alpha)(id, SEL, CGFloat);
static void recorded_alpha(id self, SEL _cmd, CGFloat alpha) {
    LOG("setAlphaValue %p %.2f", self, alpha);
    original_alpha(self, _cmd, alpha);
}

static void (*original_front)(id, SEL, id);
static void recorded_front(id self, SEL _cmd, id sender) {
    NSWindow *window = self;
    LOG("makeKeyAndOrderFront %p alpha=%.2f frame=%s", self, window.alphaValue, NSStringFromRect(window.frame).UTF8String);
    original_front(self, _cmd, sender);
}

static id (*original_next)(id, SEL);
static id recorded_next(id self, SEL _cmd) {
    static long calls;
    CAMetalLayer *layer = self;
    double began = now();
    id drawable = original_next(self, _cmd);
    double took = (now() - began) * 1000;
    if (++calls <= 12 || took > 8 || calls % 200 == 0) {
        LOG("nextDrawable #%ld %.1f ms drawable=%.0fx%.0f bounds=%.0fx%.0f scale=%.1f sync=%d nil=%d", calls, took, layer.drawableSize.width, layer.drawableSize.height, layer.bounds.size.width, layer.bounds.size.height, layer.contentsScale, layer.displaySyncEnabled, drawable == nil);
    }
    return drawable;
}

static void (*original_size)(id, SEL, CGSize);
static void recorded_size(id self, SEL _cmd, CGSize size) {
    CAMetalLayer *layer = self;
    LOG("setDrawableSize %.0fx%.0f bounds=%.0fx%.0f device=%s", size.width, size.height, layer.bounds.size.width, layer.bounds.size.height, layer.device.name.UTF8String ?: "none");
    original_size(self, _cmd, size);
}

static void replace(Class class, SEL selector, IMP replacement, IMP *original) {
    Method method = class_getInstanceMethod(class, selector);
    if (method) *original = method_setImplementation(method, replacement);
    else LOG("no method %s", sel_getName(selector));
}

static void after(double seconds, dispatch_block_t then) {
    dispatch_after(dispatch_time(DISPATCH_TIME_NOW, (int64_t)(seconds * NSEC_PER_SEC)), dispatch_get_main_queue(), then);
}

static void watch_flip(void) {
    if (!flipped && access(flip_file, F_OK) == 0) {
        flipped = YES;
        LOG("the occlusion answer turns truthful");
        for (NSWindow *window in NSApp.windows) [NSNotificationCenter.defaultCenter postNotificationName:NSWindowDidChangeOcclusionStateNotification object:window];
    }
    after(0.1, ^{ watch_flip(); });
}

static void snapshot(void) {
    for (NSWindow *window in NSApp.windows) {
        CALayer *root = window.contentView.layer;
        NSMutableString *layers = [NSMutableString string];
        for (CALayer *layer in root.sublayers) {
            [layers appendFormat:@" %@ %.0fx%.0f", NSStringFromClass(layer.class), layer.bounds.size.width, layer.bounds.size.height];
            if ([layer isKindOfClass:CAMetalLayer.class]) [layers appendFormat:@" drawable %.0fx%.0f", ((CAMetalLayer *)layer).drawableSize.width, ((CAMetalLayer *)layer).drawableSize.height];
        }
        LOG("window %p visible=%d alpha=%.2f occlusion=0x%lx frame=%s root=%.0fx%.0f layers:%s", window, window.isVisible, window.alphaValue, original_occlusion(window, @selector(occlusionState)), NSStringFromRect(window.frame).UTF8String, root.bounds.size.width, root.bounds.size.height, layers.UTF8String);
    }
    after(1, ^{ snapshot(); });
}

static void resize_step(int step) {
    for (NSWindow *window in NSApp.windows) {
        if (!window.canBecomeMainWindow) continue;
        NSRect frame = window.frame;
        CGFloat grow = step % 2 ? 1 : -1;
        frame.size.width += 80 * grow, frame.size.height += 60 * grow, frame.origin.y -= 60 * grow;
        LOG("resize step %d to %.0fx%.0f", step, frame.size.width, frame.size.height);
        [window setFrame:frame display:YES];
    }
    if (step < 6) after(0.7, ^{ resize_step(step + 1); });
}

__attribute__((constructor)) static void install(void) {
    mach_timebase_info(&timebase);
    start = mach_absolute_time();
    const char *path = getenv("ONI_PROBE_LOG");
    if (path) out = open(path, O_WRONLY | O_CREAT | O_APPEND, 0644);
    occlusion_mode = getenv("ONI_PROBE_OCC");
    flip_file = getenv("ONI_PROBE_FLIP");
    Class window = objc_getClass("NSWindow"), layer = objc_getClass("CAMetalLayer");
    replace(window, @selector(occlusionState), (IMP)recorded_occlusion, (IMP *)&original_occlusion);
    replace(window, @selector(setAlphaValue:), (IMP)recorded_alpha, (IMP *)&original_alpha);
    replace(window, @selector(makeKeyAndOrderFront:), (IMP)recorded_front, (IMP *)&original_front);
    replace(layer, @selector(nextDrawable), (IMP)recorded_next, (IMP *)&original_next);
    replace(layer, @selector(setDrawableSize:), (IMP)recorded_size, (IMP *)&original_size);
    dispatch_async(dispatch_get_main_queue(), ^{
        snapshot();
        if (flip_file) watch_flip();
        const char *resize_at = getenv("ONI_PROBE_RESIZE_AT");
        if (resize_at) after(atof(resize_at), ^{ resize_step(1); });
    });
}
