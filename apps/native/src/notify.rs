//! System notifications: macOS's notification center for the app bundle (`Locust Native.app`). A binary run outside a
//! bundle has no notification identity, and macOS's center refuses it, so there nothing is shown and `supported` says
//! so. Other systems show nothing yet.
//!
//! Each notification carries the conversation it is about. Pressing it brings Locust forward (macOS does that) and opens
//! that conversation (main/attention.ts): the delegate keeps the id, and the window's poll takes it (`take_clicked`).

use std::sync::Mutex;

/// The conversation of the notification pressed last, until the window opens it.
static CLICKED: Mutex<Option<String>> = Mutex::new(None);

/// Whether this copy can show a notification at all.
pub fn supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// Shows one notification about `conversation`. The first asks macOS for permission; until it is given, nothing shows.
pub fn show(title: &str, body: &str, conversation: Option<&str>) {
    if !supported() {
        return;
    }
    #[cfg(target_os = "macos")]
    mac::show(title, body, conversation);
    #[cfg(not(target_os = "macos"))]
    let _ = (title, body, conversation);
}

/// A notification was pressed: the window opens this conversation next.
pub fn clicked(conversation: &str) {
    *crate::invariants::lock(&CLICKED) = Some(conversation.to_owned());
}

/// The conversation whose notification was pressed, once.
pub fn take_clicked() -> Option<String> {
    crate::invariants::lock(&CLICKED).take()
}

#[cfg(target_os = "macos")]
mod mac {
    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool, ProtocolObject};
    use objc2::{MainThreadMarker, MainThreadOnly, define_class, msg_send};
    use objc2_foundation::{NSDictionary, NSError, NSObject, NSObjectProtocol, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNNotificationResponse,
        UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };
    use std::cell::RefCell;
    use std::sync::Once;

    const KEY: &str = "locustConversation";
    static ASKED: Once = Once::new();

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        struct ClickDelegate;
        unsafe impl NSObjectProtocol for ClickDelegate {}
        unsafe impl UNUserNotificationCenterDelegate for ClickDelegate {
            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(&self, _center: &UNUserNotificationCenter, response: &UNNotificationResponse, done: &DynBlock<dyn Fn()>) {
                let info = response.notification().request().content().userInfo();
                let key = NSString::from_str(KEY);
                let key: &AnyObject = key.as_ref();
                if let Some(id) = info.objectForKey(key).and_then(|v| v.downcast::<NSString>().ok()) {
                    super::clicked(&id.to_string());
                }
                done.call(());
            }
        }
    );

    thread_local! {
        // The center holds its delegate weakly; this keeps it alive for the app's life.
        static DELEGATE: RefCell<Option<Retained<ClickDelegate>>> = const { RefCell::new(None) };
    }

    fn install(center: &UNUserNotificationCenter) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        DELEGATE.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_none() {
                let delegate: Retained<ClickDelegate> = unsafe { msg_send![ClickDelegate::alloc(mtm), init] };
                center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
                *slot = Some(delegate);
            }
        });
    }

    pub fn show(title: &str, body: &str, conversation: Option<&str>) {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        install(&center);
        ASKED.call_once(|| {
            let done = RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
            center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &done);
        });
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        if let Some(id) = conversation {
            let key = NSString::from_str(KEY);
            let value = NSString::from_str(id);
            let info: Retained<NSDictionary<NSString, NSString>> = NSDictionary::from_slices(&[&*key], &[&*value]);
            // The same dictionary, as the untyped one `userInfo` takes.
            let info: Retained<NSDictionary> = unsafe { Retained::cast_unchecked(info) };
            unsafe { content.setUserInfo(&info) };
        }
        let id = NSString::from_str(&format!("locust-{}", locust_core::invariants::unique_nanos()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
        center.addNotificationRequest_withCompletionHandler(&request, None);
    }
}
