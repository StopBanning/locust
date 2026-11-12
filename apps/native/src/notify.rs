//! System notifications: macOS's notification center for the app bundle (`Locust Native.app`). A binary run outside a
//! bundle has no notification identity, and macOS's center refuses it, so there nothing is shown and `supported` says
//! so. Other systems show nothing yet.

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

/// Shows one notification. The first asks macOS for permission; until it is given, nothing shows.
pub fn show(title: &str, body: &str) {
    if !supported() {
        return;
    }
    #[cfg(target_os = "macos")]
    mac::show(title, body);
    #[cfg(not(target_os = "macos"))]
    let _ = (title, body);
}

#[cfg(target_os = "macos")]
mod mac {
    use block2::RcBlock;
    use objc2::runtime::Bool;
    use objc2_foundation::{NSError, NSString};
    use objc2_user_notifications::{UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationRequest, UNUserNotificationCenter};
    use std::sync::Once;

    static ASKED: Once = Once::new();

    pub fn show(title: &str, body: &str) {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        ASKED.call_once(|| {
            let done = RcBlock::new(|_granted: Bool, _error: *mut NSError| {});
            center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &done);
        });
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        let id = NSString::from_str(&format!("locust-{}", locust_core::invariants::unique_nanos()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
        center.addNotificationRequest_withCompletionHandler(&request, None);
    }
}
