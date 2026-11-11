//! Keychain unlock note — port of `main/keychain.ts` + `KeychainNote.tsx`.
//!
//! Only the note and ask-gating that apply without storing any secret.
//! Never reads or copies existing keychain items. Never calls encrypt/decrypt
//! or `isEncryptionAvailable` against a real keychain.

/// Exact Electron `KEYCHAIN_NOTE` (KeychainNote.tsx).
pub const KEYCHAIN_NOTE: &str = "macOS may ask for your Mac’s password so Locust can open the keys it keeps (“@teammate/desktop Safe Storage”). Type it and choose Always Allow. If you don’t see the box, it may be behind this window.";

/// Whether the amber note should be shown. Never touches the OS keychain.
///
/// Quiet when: not Mac, no key is kept, or this build already opened the
/// keychain. An unknown `keeps_key` (caller passes true) shows the note rather
/// than freezing silently — matching Electron's unreadable-kept treatment.
pub fn should_show_note(may_ask: bool, keeps_key: bool, opened_by_matches_version: bool) -> bool {
    may_ask && keeps_key && !opened_by_matches_version
}

/// macOS may ask; elsewhere never.
pub fn may_ask_on_this_os() -> bool {
    cfg!(target_os = "macos")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_without_key_or_when_already_opened() {
        assert!(!should_show_note(true, false, false));
        assert!(!should_show_note(true, true, true));
        assert!(!should_show_note(false, true, false));
        assert!(should_show_note(true, true, false));
    }

    #[test]
    fn note_words_exact() {
        assert!(KEYCHAIN_NOTE.contains("@teammate/desktop Safe Storage"));
        assert!(KEYCHAIN_NOTE.contains("Always Allow"));
        assert!(KEYCHAIN_NOTE.contains("behind this window"));
        let _ = may_ask_on_this_os();
    }
}
