//! Composer key decisions — port of Composer.tsx `keyDown` priorities.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComposerKey {
    ArrowUp,
    ArrowDown,
    Enter,
    Escape,
    Tab,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyMods {
    pub shift: bool,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Leave the key to the editor (e.g. Shift+Enter newline).
    Pass,
    SlashMove {
        delta: i32,
    },
    SlashChoose,
    SlashDismiss,
    AtMove {
        delta: i32,
    },
    AtChoose,
    AtDismiss,
    HistoryUp,
    HistoryDown,
    /// Enter (or Ctrl/Cmd+Enter): submit / queue; `send_now` when modifier while busy.
    Submit {
        send_now: bool,
    },
    StopRun,
    CycleMode,
}

pub struct KeyContext<'a> {
    pub composing: bool,
    pub slash_open: bool,
    pub slash_len: usize,
    pub at_open: bool,
    pub at_len: usize,
    pub running: bool,
    pub menu_open: bool,
    pub value_empty: bool,
    pub recalling: bool,
    pub history_len: usize,
    pub can_queue: bool,
    pub modes: &'a [&'a str],
}

/// Decide what a key does, mirroring Electron's order of checks.
pub fn handle_key(key: ComposerKey, mods: KeyMods, ctx: &KeyContext<'_>) -> KeyAction {
    if ctx.composing {
        return KeyAction::Pass;
    }
    if ctx.slash_open && ctx.slash_len > 0 {
        match key {
            ComposerKey::ArrowDown => return KeyAction::SlashMove { delta: 1 },
            ComposerKey::ArrowUp => return KeyAction::SlashMove { delta: -1 },
            ComposerKey::Enter if !mods.shift => return KeyAction::SlashChoose,
            ComposerKey::Escape => return KeyAction::SlashDismiss,
            _ => {}
        }
    }
    if ctx.at_open && ctx.at_len > 0 {
        match key {
            ComposerKey::ArrowDown => return KeyAction::AtMove { delta: 1 },
            ComposerKey::ArrowUp => return KeyAction::AtMove { delta: -1 },
            ComposerKey::Enter if !mods.shift => return KeyAction::AtChoose,
            ComposerKey::Tab => return KeyAction::AtChoose,
            ComposerKey::Escape => return KeyAction::AtDismiss,
            _ => {}
        }
    }
    if key == ComposerKey::Tab && mods.shift && !ctx.modes.is_empty() {
        return KeyAction::CycleMode;
    }
    if key == ComposerKey::Escape && ctx.running && !ctx.menu_open {
        return KeyAction::StopRun;
    }
    if key == ComposerKey::ArrowUp && ctx.history_len > 0 && (ctx.value_empty || ctx.recalling) {
        return KeyAction::HistoryUp;
    }
    if key == ComposerKey::ArrowDown && ctx.recalling {
        return KeyAction::HistoryDown;
    }
    if key == ComposerKey::Enter && !mods.shift {
        let send_now = (mods.ctrl || mods.meta) && ctx.can_queue;
        return KeyAction::Submit { send_now };
    }
    KeyAction::Pass
}

/// Cycle permission mode the way Shift+Tab does in Electron (next usable mode).
/// Shift+Tab (Composer.tsx `shiftTabMode`, M33): the next of the modes this route can run, with Auto among them only
/// while the workspace's Auto switch is already on (landing on Auto used to turn that saved switch on). Off a mode no
/// longer in the cycle (Auto with the switch off), the first mode. Nothing with fewer than two modes to cycle.
pub fn shift_tab_mode(current: &str, usable: &[&str], auto_allowed: bool) -> Option<String> {
    let cycle: Vec<&str> = usable.iter().copied().filter(|mode| *mode != "auto" || auto_allowed).collect();
    if cycle.len() < 2 {
        return None;
    }
    // indexOf: -1 for a mode not in the cycle, so the next is the first.
    let next = cycle.iter().position(|m| *m == current).map_or(0, |at| (at + 1) % cycle.len());
    Some(cycle[next].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> KeyContext<'static> {
        KeyContext {
            composing: false,
            slash_open: false,
            slash_len: 0,
            at_open: false,
            at_len: 0,
            running: false,
            menu_open: false,
            value_empty: true,
            recalling: false,
            history_len: 0,
            can_queue: false,
            modes: &["ask", "plan", "accept-edits"],
        }
    }

    #[test]
    fn enter_submits_shift_enter_passes() {
        let c = ctx();
        assert_eq!(
            handle_key(
                ComposerKey::Enter,
                KeyMods {
                    shift: false,
                    ctrl: false,
                    meta: false,
                    alt: false
                },
                &c
            ),
            KeyAction::Submit { send_now: false }
        );
        assert_eq!(
            handle_key(
                ComposerKey::Enter,
                KeyMods {
                    shift: true,
                    ctrl: false,
                    meta: false,
                    alt: false
                },
                &c
            ),
            KeyAction::Pass
        );
    }

    #[test]
    fn slash_menu_owns_arrows_and_enter() {
        let mut c = ctx();
        c.slash_open = true;
        c.slash_len = 3;
        assert_eq!(
            handle_key(
                ComposerKey::ArrowDown,
                KeyMods {
                    shift: false,
                    ctrl: false,
                    meta: false,
                    alt: false
                },
                &c
            ),
            KeyAction::SlashMove { delta: 1 }
        );
        assert_eq!(
            handle_key(
                ComposerKey::Enter,
                KeyMods {
                    shift: false,
                    ctrl: false,
                    meta: false,
                    alt: false
                },
                &c
            ),
            KeyAction::SlashChoose
        );
    }

    #[test]
    fn at_menu_tab_chooses() {
        let mut c = ctx();
        c.at_open = true;
        c.at_len = 2;
        assert_eq!(
            handle_key(
                ComposerKey::Tab,
                KeyMods {
                    shift: false,
                    ctrl: false,
                    meta: false,
                    alt: false
                },
                &c
            ),
            KeyAction::AtChoose
        );
    }

    #[test]
    fn history_up_from_empty() {
        let mut c = ctx();
        c.history_len = 2;
        c.value_empty = true;
        assert_eq!(
            handle_key(
                ComposerKey::ArrowUp,
                KeyMods {
                    shift: false,
                    ctrl: false,
                    meta: false,
                    alt: false
                },
                &c
            ),
            KeyAction::HistoryUp
        );
    }

    #[test]
    fn ctrl_enter_send_now_while_queueable() {
        let mut c = ctx();
        c.can_queue = true;
        assert_eq!(
            handle_key(
                ComposerKey::Enter,
                KeyMods {
                    shift: false,
                    ctrl: true,
                    meta: false,
                    alt: false
                },
                &c
            ),
            KeyAction::Submit { send_now: true }
        );
    }
}

#[cfg(test)]
#[path = "shift_tab_tests.rs"]
mod shift_tab_tests;
