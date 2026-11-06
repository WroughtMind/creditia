// CI-only AppKit driver. The packaged executable and its resources remain
// byte-for-byte identical to the installer. This code runs only in the
// particular test process the harness launches, on its AppKit event loop.
#import <AppKit/AppKit.h>
#import <stdlib.h>
#import <string.h>

static NSMenuItem *itemNamed(NSMenu *menu, NSArray<NSString *> *names) {
    for (NSMenuItem *item in menu.itemArray) {
        if ([names containsObject:item.title]) return item;
        NSMenuItem *nested = item.submenu ? itemNamed(item.submenu, names) : nil;
        if (nested) return nested;
    }
    return nil;
}

static NSOpenPanel *visiblePanel(void) {
    for (NSWindow *window in NSApp.windows) {
        if (window.attachedSheet.visible && [window.attachedSheet isKindOfClass:NSOpenPanel.class]) return (NSOpenPanel *)window.attachedSheet;
        if (window.visible && [window isKindOfClass:NSOpenPanel.class]) return (NSOpenPanel *)window;
    }
    return nil;
}

__attribute__((constructor))
static void installInputDriver(void) {
    const char *directory = getenv("CREDITIA_TEST_APPKIT_DIR");
    if (!directory || strcmp(getprogname(), "creditia-gpui") != 0) return;
    NSString *root = [NSString stringWithUTF8String:directory];
    unsetenv("DYLD_INSERT_LIBRARIES");
    unsetenv("CREDITIA_TEST_APPKIT_DIR");
    dispatch_async(dispatch_get_main_queue(), ^{
        NSString *commandPath = [root stringByAppendingPathComponent:@"command"];
        NSString *resultPath = [root stringByAppendingPathComponent:@"reply"];
        NSTimer *timer = [NSTimer timerWithTimeInterval:0.1 repeats:YES block:^(NSTimer *unused) {
            NSString *command = [NSString stringWithContentsOfFile:commandPath encoding:NSUTF8StringEncoding error:nil];
            if (!command) return;
            [[NSFileManager defaultManager] removeItemAtPath:commandPath error:nil];
            void (^reply)(NSString *) = ^(NSString *message) {
                [message writeToFile:resultPath atomically:YES encoding:NSUTF8StringEncoding error:nil];
            };
            if ([command isEqualToString:@"visible"]) {
                NSOpenPanel *panel = visiblePanel();
                if (!panel) {
                    NSMutableArray *windows = [NSMutableArray array];
                    for (NSWindow *window in NSApp.windows) if (window.visible)
                        [windows addObject:@[NSStringFromClass(window.class), window.title ?: @""]];
                    [windows writeToFile:[root stringByAppendingPathComponent:@"windows.plist"] atomically:YES];
                }
                reply(panel ? @"yes" : @"no");
            } else if ([command isEqualToString:@"cancel"]) {
                NSOpenPanel *panel = visiblePanel();
                if (!panel) { reply(@"No visible NSOpenPanel"); return; }
                [panel cancel:nil];
                reply(@"ok");
            } else {
                NSArray *titles = [command isEqualToString:@"copy"] ? @[@"Copy archive",@"复制档案"] : @[@"Add folder…",@"添加文件夹…"];
                NSMenuItem *item = itemNamed(NSApp.mainMenu, titles);
                if (!item) { reply(@"Production menu item missing"); return; }
                [item.menu update];
                if (!item.enabled) { reply(@"Production menu item disabled"); return; }
                reply(@"ok");
                [item.menu performActionForItemAtIndex:[item.menu indexOfItem:item]];
            }
        }];
        [[NSRunLoop mainRunLoop] addTimer:timer forMode:NSRunLoopCommonModes];
        [[NSRunLoop mainRunLoop] addTimer:timer forMode:NSModalPanelRunLoopMode];
    });
}
