//! Keyboard shortcut reference — port of `renderer/src/keyboardShortcuts.ts`.
//!
//! Lists Electron's shortcuts in Electron's words. Rows native actually binds
//! are unmarked; others say `runs in Locust for now`.

use crate::settings_pages::RUNS_IN_LOCUST;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutGroup {
    Conversations,
    MessageBox,
    Panes,
    Menus,
}

impl ShortcutGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Conversations => "Conversations",
            Self::MessageBox => "The message box",
            Self::Panes => "Panes",
            Self::Menus => "Menus",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ShortcutRow {
    pub group: ShortcutGroup,
    pub keys: &'static str,
    pub does: &'static str,
    /// True when native `on_key` / composer actually binds this chord.
    pub native: bool,
    pub control_on_mac: bool,
}

pub const SHORTCUTS: &[ShortcutRow] = &[
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+F",
        does: "Find in this conversation",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+K",
        does: "Open the command palette",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+I",
        does: "Open the inspector",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Alt+Up / Alt+Down",
        does: "Previous or next conversation",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Alt+Shift+Up / Alt+Shift+Down",
        does: "A conversation that is waiting for you",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+1",
        does: "Home",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+2",
        does: "Teammates",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+3",
        does: "Settings",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+4",
        does: "Rooms",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+5",
        does: "Memory",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+`",
        does: "Terminal",
        native: false,
        control_on_mac: true,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+Alt+Up / Ctrl+Alt+Down",
        does: "Move among your own messages",
        native: false,
        control_on_mac: true,
    },
    ShortcutRow {
        group: ShortcutGroup::Conversations,
        keys: "Ctrl+O",
        does: "Normal or Verbose steps",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::MessageBox,
        keys: "Enter",
        does: "Send",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::MessageBox,
        keys: "Shift+Enter",
        does: "A new line",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::MessageBox,
        keys: "Ctrl+Enter",
        does: "Send now, while a reply is running",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::MessageBox,
        keys: "Shift+Tab",
        does: "Next permission mode",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::MessageBox,
        keys: "Escape",
        does: "Stop the reply, when the box is focused and no menu is open",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::MessageBox,
        keys: "Up / Down",
        does: "Recall a sent message from an empty box",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Panes,
        keys: "Ctrl+/",
        does: "Keyboard shortcuts",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Panes,
        keys: "+ / − / 0 / 1",
        does: "Zoom an image, fit it, or its actual size",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Panes,
        keys: "Escape",
        does: "Close a dialog",
        native: true,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Menus,
        keys: "Ctrl+Shift+M",
        does: "Permission mode",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Menus,
        keys: "Ctrl+Shift+I",
        does: "Model",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Menus,
        keys: "Ctrl+Shift+E",
        does: "Effort",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Menus,
        keys: "1–9",
        does: "Pick that row in an open menu",
        native: false,
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Menus,
        keys: "1–9",
        does: "Answer a question card",
        native: true, // approval card 1/2/3
        control_on_mac: false,
    },
    ShortcutRow {
        group: ShortcutGroup::Menus,
        keys: "Ctrl+Alt+Space",
        does: "Talk, from any app",
        native: false,
        control_on_mac: true,
    },
];

pub const SHEET_TITLE: &str = "Keyboard shortcuts";

/// Mac remapping: Ctrl→Cmd and Alt→Option, except control_on_mac rows keep Control.
pub fn keys_on(keys: &str, platform: &str, control_on_mac: bool) -> String {
    if platform != "darwin" {
        return keys.to_owned();
    }
    let step = if control_on_mac {
        keys.to_owned()
    } else {
        keys.replace("Ctrl", "Cmd")
    };
    step.replace("Alt", "Option")
}

pub fn shortcut_groups() -> Vec<(ShortcutGroup, Vec<&'static ShortcutRow>)> {
    let names = [
        ShortcutGroup::Conversations,
        ShortcutGroup::MessageBox,
        ShortcutGroup::Panes,
        ShortcutGroup::Menus,
    ];
    names
        .into_iter()
        .map(|name| (name, SHORTCUTS.iter().filter(|r| r.group == name).collect()))
        .collect()
}

pub fn lacking_note(native: bool) -> Option<&'static str> {
    if native { None } else { Some(RUNS_IN_LOCUST) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_and_mac_keys() {
        let groups = shortcut_groups();
        assert_eq!(groups.len(), 4);
        assert_eq!(groups[0].0.label(), "Conversations");
        assert_eq!(keys_on("Ctrl+K", "darwin", false), "Cmd+K");
        assert_eq!(keys_on("Ctrl+`", "darwin", true), "Ctrl+`");
        assert_eq!(
            keys_on("Alt+Up / Alt+Down", "darwin", false),
            "Option+Up / Option+Down"
        );
        assert_eq!(keys_on("Ctrl+K", "linux", false), "Ctrl+K");
    }

    #[test]
    fn native_binds_are_marked_and_lacking_uses_exact_words() {
        assert!(SHORTCUTS.iter().any(|r| r.keys == "Ctrl+F" && r.native));
        assert!(SHORTCUTS.iter().any(|r| r.keys == "Ctrl+K" && r.native));
        assert!(SHORTCUTS.iter().any(|r| r.does == "Terminal" && !r.native));
        assert_eq!(lacking_note(false), Some("runs in Locust for now"));
        assert_eq!(lacking_note(true), None);
        assert_eq!(SHEET_TITLE, "Keyboard shortcuts");
    }
}
