//! Disabled-send reasons — port of `sendBlockedReason` in `renderer/src/status.ts`
//! (tested by `a-disabled-send-says-why.test.ts`).

#[derive(Clone, Copy, Debug, Default)]
pub struct SendBlocked {
    pub nothing_installed: bool,
    pub runtime_ready: bool,
    pub route_can_run: bool,
    pub busy: bool,
    pub empty: bool,
}

/// Why Send is grey, or `None` when it is live. Priority matches Electron.
pub fn send_blocked_reason(blocked: SendBlocked) -> Option<&'static str> {
    if blocked.nothing_installed {
        return Some("Nothing can run yet — install an AI agent from the list above.");
    }
    if !blocked.runtime_ready {
        return Some(
            "The model this is set to is not ready on this machine. Pick another above, or set it up in Settings.",
        );
    }
    if !blocked.route_can_run {
        return Some(
            "This mode cannot run on the model this is set to. Pick another mode or model above.",
        );
    }
    if blocked.busy {
        return Some("Something else is starting. This goes as soon as it settles.");
    }
    if blocked.empty {
        return Some("Write a message first.");
    }
    None
}

/// Live Send title when nothing blocks it (Composer.tsx).
pub const SEND_LIVE_TITLE: &str = "Send — Shift+Enter for a new line";

#[cfg(test)]
mod tests {
    use super::*;

    fn clear() -> SendBlocked {
        SendBlocked {
            nothing_installed: false,
            runtime_ready: true,
            route_can_run: true,
            busy: false,
            empty: false,
        }
    }

    #[test]
    fn live_when_clear() {
        assert_eq!(send_blocked_reason(clear()), None);
    }

    #[test]
    fn install_first() {
        assert!(
            send_blocked_reason(SendBlocked {
                nothing_installed: true,
                ..clear()
            })
            .unwrap()
            .contains("install an AI agent")
        );
    }

    #[test]
    fn empty_only_when_real_reason() {
        assert_eq!(
            send_blocked_reason(SendBlocked {
                empty: true,
                ..clear()
            }),
            Some("Write a message first.")
        );
    }

    #[test]
    fn install_wins_when_everything_wrong() {
        let all = SendBlocked {
            nothing_installed: true,
            runtime_ready: false,
            route_can_run: false,
            busy: true,
            empty: false,
        };
        assert!(
            send_blocked_reason(all)
                .unwrap()
                .contains("install an AI agent")
        );
        assert!(
            send_blocked_reason(SendBlocked { empty: true, ..all })
                .unwrap()
                .contains("install an AI agent")
        );
    }

    #[test]
    fn separates_ready_from_mode() {
        assert!(
            send_blocked_reason(SendBlocked {
                runtime_ready: false,
                ..clear()
            })
            .unwrap()
            .contains("not ready on this machine")
        );
        assert!(
            send_blocked_reason(SendBlocked {
                route_can_run: false,
                ..clear()
            })
            .unwrap()
            .contains("This mode cannot run")
        );
    }
}
