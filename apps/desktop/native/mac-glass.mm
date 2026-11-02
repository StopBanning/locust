// LIQUID GLASS BEHIND A MAC WINDOW'S PAGE (Settings > General > Look, macOS 26 and later).
//
// macOS 26 made NSGlassEffectView public: the material Finder, Messages and Settings are drawn on. Electron has no
// option for it, so this small addon puts two of them behind the page: the window's whole ground, and the floating
// panel the sidebar sits on. The page lets them through where it paints nothing (shell.css `[data-look='liquid']`).
// The class is looked up by name and set by key, so this builds against an older SDK too and answers "not here" on
// a Mac that has no glass. Built by scripts/build-mac-glass.cjs; loaded by main/mac-glass.ts.
#include <node_api.h>
#import <AppKit/AppKit.h>
#import <objc/runtime.h>

static const void *kGround = &kGround;
static const void *kPanel = &kPanel;
static const void *kBackdrop = &kBackdrop;

static NSView *rootOf(napi_env env, napi_value value) {
  void *data = nullptr;
  size_t length = 0;
  if (napi_get_buffer_info(env, value, &data, &length) != napi_ok || length < sizeof(void *)) return nil;
  return (__bridge NSView *)(*reinterpret_cast<void **>(data));
}

static double field(napi_env env, napi_value object, const char *key, double otherwise) {
  bool has = false;
  if (napi_has_named_property(env, object, key, &has) != napi_ok || !has) return otherwise;
  napi_value value;
  napi_get_named_property(env, object, key, &value);
  double out = otherwise;
  napi_get_value_double(env, value, &out);
  return out;
}

static void onMain(dispatch_block_t block) {
  if ([NSThread isMainThread]) block();
  else dispatch_sync(dispatch_get_main_queue(), block);
}

static NSView *glassView(NSRect frame, double radius) {
  Class glass = NSClassFromString(@"NSGlassEffectView");
  if (glass == nil) return nil;
  NSView *view = [[glass alloc] initWithFrame:frame];
  [view setValue:@(radius) forKey:@"cornerRadius"];
  return view;
}

// The panel's frame from a rectangle measured from the window's top left, as the page measures it.
static NSRect panelFrame(NSView *root, double x, double y, double width, double height) {
  if (root.isFlipped) return NSMakeRect(x, y, width, height);
  return NSMakeRect(x, root.bounds.size.height - y - height, width, height);
}

static napi_value Supported(napi_env env, napi_callback_info) {
  napi_value out;
  napi_get_boolean(env, NSClassFromString(@"NSGlassEffectView") != nil, &out);
  return out;
}

// attach(handle) -> true when the window's ground is glass now.
static napi_value Attach(napi_env env, napi_callback_info info) {
  size_t argc = 1;
  napi_value argv[1];
  napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr);
  NSView *root = argc > 0 ? rootOf(env, argv[0]) : nil;
  __block bool attached = false;
  onMain(^{
    if (root == nil) return;
    NSView *ground = objc_getAssociatedObject(root, kGround);
    if (ground == nil) {
      ground = glassView(root.bounds, 0);
      if (ground == nil) return;
      ground.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
      // Lowest of all, but over a drive's picture when there is one (backdrop, below).
      NSView *picture = objc_getAssociatedObject(root, kBackdrop);
      if (picture != nil) [root addSubview:ground positioned:NSWindowAbove relativeTo:picture];
      else [root addSubview:ground positioned:NSWindowBelow relativeTo:nil];
      objc_setAssociatedObject(root, kGround, ground, OBJC_ASSOCIATION_RETAIN);
    }
    attached = true;
  });
  napi_value out;
  napi_get_boolean(env, attached, &out);
  return out;
}

// panel(handle, {x, y, width, height, radius} | null): the sidebar's floating panel, or none. Its height follows the
// window's as the window is resized, so only a change of width or position has to be sent.
static napi_value Panel(napi_env env, napi_callback_info info) {
  size_t argc = 2;
  napi_value argv[2];
  napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr);
  NSView *root = argc > 0 ? rootOf(env, argv[0]) : nil;
  napi_valuetype kind = napi_undefined;
  if (argc > 1) napi_typeof(env, argv[1], &kind);
  bool none = kind != napi_object;
  double x = none ? 0 : field(env, argv[1], "x", 0), y = none ? 0 : field(env, argv[1], "y", 0);
  double width = none ? 0 : field(env, argv[1], "width", 0), height = none ? 0 : field(env, argv[1], "height", 0);
  double radius = none ? 0 : field(env, argv[1], "radius", 0);
  onMain(^{
    if (root == nil) return;
    NSView *ground = objc_getAssociatedObject(root, kGround);
    NSView *panel = objc_getAssociatedObject(root, kPanel);
    if (none || ground == nil || width <= 0 || height <= 0) {
      panel.hidden = YES;
      return;
    }
    if (panel == nil) {
      panel = glassView(NSZeroRect, radius);
      if (panel == nil) return;
      // Both its insets stay as they are, top and bottom: only its height takes the window's change.
      panel.autoresizingMask = NSViewHeightSizable;
      [root addSubview:panel positioned:NSWindowAbove relativeTo:ground];
      objc_setAssociatedObject(root, kPanel, panel, OBJC_ASSOCIATION_RETAIN);
    }
    [panel setValue:@(radius) forKey:@"cornerRadius"];
    panel.frame = panelFrame(root, x, y, width, height);
    panel.hidden = NO;
  });
  return nullptr;
}

// backdrop(handle, path): a picture under the glass, inside the window -- for a drive only (a development build with
// LOCUST_GLASS_BACKDROP set, main/mac-glass.ts). The glass then shows a known desktop, and a capture of this window
// alone shows it, with no other window put behind this one and nobody's own windows in the frame.
static napi_value Backdrop(napi_env env, napi_callback_info info) {
  size_t argc = 2;
  napi_value argv[2];
  napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr);
  NSView *root = argc > 0 ? rootOf(env, argv[0]) : nil;
  char path[4096] = {0};
  size_t length = 0;
  if (argc > 1) napi_get_value_string_utf8(env, argv[1], path, sizeof(path), &length);
  NSString *file = [NSString stringWithUTF8String:path];
  onMain(^{
    if (root == nil || file.length == 0) return;
    NSImage *image = [[NSImage alloc] initWithContentsOfFile:file];
    NSView *picture = nil;
    if (image != nil) {
      NSImageView *view = [[NSImageView alloc] initWithFrame:root.bounds];
      view.image = image;
      view.imageScaling = NSImageScaleAxesIndependently;
      picture = view;
    } else {
      // "dark", or a picture that would not load: a near-black desktop.
      NSBox *box = [[NSBox alloc] initWithFrame:root.bounds];
      box.boxType = NSBoxCustom;
      box.borderWidth = 0;
      box.fillColor = [NSColor colorWithSRGBRed:0.043 green:0.047 blue:0.055 alpha:1];
      picture = box;
    }
    picture.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
    [objc_getAssociatedObject(root, kBackdrop) removeFromSuperview];
    [root addSubview:picture positioned:NSWindowBelow relativeTo:nil];
    objc_setAssociatedObject(root, kBackdrop, picture, OBJC_ASSOCIATION_RETAIN);
  });
  return nullptr;
}

// detach(handle): the window goes back to what it was made with.
static napi_value Detach(napi_env env, napi_callback_info info) {
  size_t argc = 1;
  napi_value argv[1];
  napi_get_cb_info(env, info, &argc, argv, nullptr, nullptr);
  NSView *root = argc > 0 ? rootOf(env, argv[0]) : nil;
  onMain(^{
    if (root == nil) return;
    const void *keys[] = {kPanel, kGround, kBackdrop};
    for (const void *key : keys) {
      NSView *view = objc_getAssociatedObject(root, key);
      [view removeFromSuperview];
      objc_setAssociatedObject(root, key, nil, OBJC_ASSOCIATION_RETAIN);
    }
  });
  return nullptr;
}

static napi_value Init(napi_env env, napi_value exports) {
  const napi_property_descriptor methods[] = {
    {"supported", nullptr, Supported, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"attach", nullptr, Attach, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"panel", nullptr, Panel, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"detach", nullptr, Detach, nullptr, nullptr, nullptr, napi_default, nullptr},
    {"backdrop", nullptr, Backdrop, nullptr, nullptr, nullptr, napi_default, nullptr},
  };
  napi_define_properties(env, exports, sizeof(methods) / sizeof(methods[0]), methods);
  return exports;
}

NAPI_MODULE(NODE_GYP_MODULE_NAME, Init)
