//! Settings pages index — port of `renderer/src/settingsPages.ts`.
//!
//! Eleven pages in five groups. Headings and also-known-as words are exact so
//! search finds what Electron finds.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsGroup {
    Locust,
    Team,
    Agents,
    ThisFolder,
    About,
}

impl SettingsGroup {
    pub fn label(self) -> &'static str {
        match self {
            Self::Locust => "Locust",
            Self::Team => "Team",
            Self::Agents => "Agents",
            Self::ThisFolder => "This folder",
            Self::About => "About",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPageId {
    App,
    Chat,
    Appearance,
    Privacy,
    Teammates,
    Memory,
    Relay,
    Runtimes,
    Models,
    Connectors,
    Cloud,
    Workspace,
    WhatsNew,
}

impl SettingsPageId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::App => "app",
            Self::Chat => "chat",
            Self::Appearance => "appearance",
            Self::Privacy => "privacy",
            Self::Teammates => "teammates",
            Self::Memory => "memory",
            Self::Relay => "relay",
            Self::Runtimes => "runtimes",
            Self::Models => "models",
            Self::Connectors => "connectors",
            Self::Cloud => "cloud",
            Self::Workspace => "workspace",
            Self::WhatsNew => "whatsnew",
        }
    }

    pub fn from_str(id: &str) -> Option<Self> {
        Some(match id {
            "app" => Self::App,
            "chat" => Self::Chat,
            "appearance" => Self::Appearance,
            "privacy" => Self::Privacy,
            "teammates" => Self::Teammates,
            "memory" => Self::Memory,
            "relay" => Self::Relay,
            "runtimes" => Self::Runtimes,
            "models" => Self::Models,
            "connectors" => Self::Connectors,
            "cloud" => Self::Cloud,
            "workspace" => Self::Workspace,
            "whatsnew" => Self::WhatsNew,
            _ => return None,
        })
    }
}

pub struct SettingsPage {
    pub id: SettingsPageId,
    pub label: &'static str,
    pub group: SettingsGroup,
    pub headings: &'static [&'static str],
    /// Words a person types that are not the heading, keyed by heading index.
    pub also_known_as: &'static [(&'static str, &'static [&'static str])],
}

pub const SETTINGS_PAGES: &[SettingsPage] = &[
    SettingsPage {
        id: SettingsPageId::App,
        label: "General",
        group: SettingsGroup::Locust,
        headings: &[
            "Managed by your organization",
            "Let your other AI apps use Locust",
            "Your account",
            "Your phone",
            "Chat apps",
            "Voice typing",
            "Talk from any app",
            "Scout keeps watch",
            "Startup",
            "Updates",
            "Report a problem",
            "Help",
        ],
        also_known_as: &[
            (
                "Managed by your organization",
                &[
                    "organization",
                    "organisation",
                    "policy",
                    "administrator",
                    "IT",
                    "managed settings",
                ],
            ),
            (
                "Let your other AI apps use Locust",
                &["MCP server", "stdio", "Claude Code", "Codex", "Cursor", "other apps"],
            ),
            (
                "Your account",
                &[
                    "Locust account",
                    "sync",
                    "recovery code",
                    "new computer",
                    "move computers",
                    "cloud copy",
                ],
            ),
            (
                "Your phone",
                &[
                    "phone",
                    "mobile",
                    "QR code",
                    "iPhone",
                    "Android",
                    "away from your desk",
                ],
            ),
            (
                "Chat apps",
                &[
                    "Discord",
                    "Discord bot",
                    "Slack",
                    "Slack app",
                    "Telegram",
                    "Telegram bot",
                    "BotFather",
                    "chat app",
                    "messaging",
                ],
            ),
            (
                "Voice typing",
                &["dictation", "microphone", "speech", "whisper", "transcription"],
            ),
            (
                "Talk from any app",
                &[
                    "shortcut",
                    "hotkey",
                    "keyboard shortcut",
                    "voice call",
                    "Ctrl+Alt+Space",
                ],
            ),
            (
                "Scout keeps watch",
                &[
                    "Scout",
                    "proactive",
                    "keeps watch",
                    "noticed",
                    "failing checks",
                    "review requests",
                ],
            ),
            ("Startup", &["login item", "tray", "keep running", "sign-in"]),
            ("Updates", &["version", "upgrade"]),
            ("Report a problem", &["bug", "feedback", "crash"]),
            ("Help", &["email", "contact", "support", "issue"]),
        ],
    },
    SettingsPage {
        id: SettingsPageId::Chat,
        label: "Chat",
        group: SettingsGroup::Locust,
        headings: &["Your commands", "How to write one"],
        also_known_as: &[
            (
                "Your commands",
                &["custom command", "command mode", "markdown command"],
            ),
            (
                "How to write one",
                &["command file", "arguments placeholder"],
            ),
        ],
    },
    SettingsPage {
        id: SettingsPageId::Appearance,
        label: "Appearance",
        group: SettingsGroup::Locust,
        headings: &[
            "Reply text size",
            "Terminal faces",
            "What a face says",
            "Plush",
            "Sidebar",
            "Send button",
            "Boot screen",
        ],
        also_known_as: &[
            ("Sidebar", &["theme", "dark", "light", "colour", "color", "width"]),
            (
                "Send button",
                &[
                    "metal",
                    "chromatic",
                    "silver",
                    "gold",
                    "shine",
                    "shimmer",
                    "hover",
                    "send",
                    "button",
                    "effect",
                    "bend",
                    "gooey",
                    "animation",
                    "motion",
                ],
            ),
            (
                "Reply text size",
                &[
                    "text",
                    "text size",
                    "font",
                    "font size",
                    "bigger",
                    "smaller",
                    "type",
                    "reading",
                ],
            ),
            (
                "Terminal faces",
                &[
                    "face",
                    "faces",
                    "screen",
                    "eyes",
                    "bot",
                    "bots",
                    "avatar",
                    "terminal",
                    "mascot",
                    "look",
                ],
            ),
            (
                "What a face says",
                &[
                    "legend",
                    "expression",
                    "expressions",
                    "mood",
                    "moods",
                    "meaning",
                    "status",
                    "state",
                    "states",
                ],
            ),
            (
                "Plush",
                &[
                    "plush",
                    "fur",
                    "furry",
                    "fabric",
                    "soft",
                    "fluffy",
                    "plastic",
                    "material",
                    "texture",
                ],
            ),
            ("Boot screen", &["splash", "startup", "launch"]),
        ],
    },
    SettingsPage {
        id: SettingsPageId::Privacy,
        label: "Privacy & data",
        group: SettingsGroup::Locust,
        headings: &["Privacy & local data", "Trash", "Back up and restore"],
        also_known_as: &[
            (
                "Privacy & local data",
                &["ledger", "log", "logs", "telemetry", "privacy", "where is my data"],
            ),
            (
                "Trash",
                &[
                    "deleted",
                    "delete",
                    "restore",
                    "undo",
                    "recycle",
                    "recycle bin",
                    "bin",
                ],
            ),
            (
                "Back up and restore",
                &[
                    "backup",
                    "back up",
                    "export",
                    "import",
                    "new laptop",
                    "move",
                    "copy",
                    "save my data",
                    "transfer",
                ],
            ),
        ],
    },
    SettingsPage {
        id: SettingsPageId::Teammates,
        label: "Teammates",
        group: SettingsGroup::Team,
        headings: &[
            "Swarm",
            "Auto mode",
            "Plans",
            "Your skills",
            "Skills from GitHub",
            "Saved approvals",
        ],
        also_known_as: &[
            ("Swarm", &["parallel", "at once", "concurrent"]),
            ("Auto mode", &["permission", "approve", "ask first", "sandbox"]),
            ("Plans", &["plan", "steps"]),
            (
                "Your skills",
                &["skill", "skills", "claude code", "plugin", "slash"],
            ),
            (
                "Skills from GitHub",
                &["github", "repository", "library", "download", "skill folder"],
            ),
            (
                "Saved approvals",
                &["rule", "rules", "always", "allow", "deny", "never", "saved"],
            ),
        ],
    },
    SettingsPage {
        id: SettingsPageId::Memory,
        label: "Memory",
        group: SettingsGroup::Team,
        headings: &["What your team remembers"],
        also_known_as: &[(
            "What your team remembers",
            &["memory", "remember", "remembers", "notes", "recall"],
        )],
    },
    SettingsPage {
        id: SettingsPageId::Relay,
        label: "Between teammates",
        group: SettingsGroup::Team,
        headings: &["Between teammates"],
        also_known_as: &[(
            "Between teammates",
            &[
                "relay",
                "reply",
                "replies",
                "answer",
                "message",
                "messages",
                "interrupt",
                "role",
                "roles",
                "name",
                "rename",
            ],
        )],
    },
    SettingsPage {
        id: SettingsPageId::Runtimes,
        label: "AI agents",
        group: SettingsGroup::Agents,
        headings: &[
            "AI agents & accounts",
            "What Locust can stop",
            "When an agent cannot go on",
            "Your own agents",
        ],
        also_known_as: &[
            (
                "AI agents & accounts",
                &[
                    "runtime",
                    "runtimes",
                    "node",
                    "node.js",
                    "npm",
                    "install",
                    "cli",
                    "model",
                    "models",
                    "sign in",
                    "account",
                    "api key",
                    "remote control",
                    "claude.ai",
                ],
            ),
            (
                "What Locust can stop",
                &[
                    "approval",
                    "approvals",
                    "permissions",
                    "card",
                    "cards",
                    "gate",
                    "stop",
                    "safety",
                    "security",
                ],
            ),
            (
                "When an agent cannot go on",
                &[
                    "route",
                    "limit",
                    "quota",
                    "rate limit",
                    "usage",
                    "fallback",
                    "fallback chain",
                    "backup agent",
                    "backup model",
                    "carry on",
                    "ran out",
                    "out of credits",
                    "plan runs out",
                    "provider error",
                    "outage",
                    "offline",
                    "network",
                    "signed out",
                ],
            ),
            (
                "Your own agents",
                &[
                    "ACP",
                    "agent client protocol",
                    "add an agent",
                    "Goose",
                    "Qwen Code",
                    "Kimi",
                    "custom agent",
                ],
            ),
        ],
    },
    SettingsPage {
        id: SettingsPageId::Models,
        label: "Your own models",
        group: SettingsGroup::Agents,
        headings: &["Your own models"],
        also_known_as: &[(
            "Your own models",
            &[
                "own model",
                "custom model",
                "add model",
                "endpoint",
                "base url",
                "address",
                "openai",
                "compatible",
                "ollama",
                "vllm",
                "lm studio",
                "local model",
                "company",
                "key",
            ],
        )],
    },
    SettingsPage {
        id: SettingsPageId::Connectors,
        label: "Connectors",
        group: SettingsGroup::Agents,
        headings: &["GitHub", "Connectors", "Add a connector"],
        also_known_as: &[
            (
                "GitHub",
                &[
                    "gh",
                    "git hub",
                    "sign in to github",
                    "github account",
                    "pull request",
                    "pull requests",
                    "push",
                    "source control",
                ],
            ),
            ("Connectors", &["mcp", "tools", "server"]),
            (
                "Add a connector",
                &["add mcp", "mcp server", "every agent", "all agents"],
            ),
        ],
    },
    SettingsPage {
        id: SettingsPageId::Cloud,
        label: "Cloud computers",
        group: SettingsGroup::Agents,
        headings: &["Cloud computers"],
        also_known_as: &[(
            "Cloud computers",
            &[
                "cloud",
                "codespace",
                "codespaces",
                "vm",
                "virtual machine",
                "remote",
                "cloud teammate",
                "github codespaces",
                "free hours",
            ],
        )],
    },
    SettingsPage {
        id: SettingsPageId::Workspace,
        label: "Project folder",
        group: SettingsGroup::ThisFolder,
        headings: &["Project folder"],
        also_known_as: &[(
            "Project folder",
            &[
                "worktree",
                "worktrees",
                "branch",
                "branches",
                "folder",
                "path",
                "directory",
                "project",
                "repo",
                "workspace",
                "check after edits",
                "tests",
                "npm test",
                "lint",
                "fix until it passes",
            ],
        )],
    },
    SettingsPage {
        id: SettingsPageId::WhatsNew,
        label: "What's new",
        group: SettingsGroup::About,
        headings: &["Changelog"],
        also_known_as: &[(
            "Changelog",
            &[
                "what’s new",
                "what's new",
                "what changed",
                "release notes",
                "releases",
                "history",
                "versions",
                "new",
            ],
        )],
    },
];

pub const SEARCH_PLACEHOLDER: &str = "Search settings";
pub const SEARCH_EMPTY: &str =
    "Nothing matches. The pages are still here; clear the search to see them.";
pub const HOW_IT_WORKS: &str = "How it works";
pub const SETTINGS_TITLE: &str = "Settings";
/// Deferred feature note — exact native convention.
pub const RUNS_IN_LOCUST: &str = "runs in Locust for now";

pub fn matches(text: &str, query: &str) -> bool {
    text.to_lowercase().contains(&query.to_lowercase())
}

pub fn matched_headings<'a>(page: &'a SettingsPage, query: &str) -> Vec<&'a str> {
    page.headings
        .iter()
        .copied()
        .filter(|heading| {
            matches(heading, query)
                || page
                    .also_known_as
                    .iter()
                    .find(|(h, _)| *h == *heading)
                    .map(|(_, words)| words.iter().any(|w| matches(w, query)))
                    .unwrap_or(false)
        })
        .collect()
}

pub fn page_matches(page: &SettingsPage, query: &str) -> bool {
    matches(page.label, query) || !matched_headings(page, query).is_empty()
}

pub fn page_by_id(id: SettingsPageId) -> &'static SettingsPage {
    SETTINGS_PAGES
        .iter()
        .find(|p| p.id == id)
        .expect("every SettingsPageId is in SETTINGS_PAGES")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirteen_pages_in_five_groups_like_claude() {
        assert_eq!(SETTINGS_PAGES.len(), 13);
        let groups: Vec<_> = SETTINGS_PAGES.iter().map(|p| p.group.label()).collect();
        assert_eq!(
            groups,
            vec![
                "Locust", "Locust", "Locust", "Locust", "Team", "Team", "Team", "Agents", "Agents",
                "Agents", "Agents", "This folder", "About",
            ]
        );
        assert_eq!(HOW_IT_WORKS, "How it works");
    }

    #[test]
    fn every_page_id_is_unique_and_named() {
        let mut seen = std::collections::HashSet::new();
        for page in SETTINGS_PAGES {
            assert!(seen.insert(page.id.as_str()));
            assert_eq!(SettingsPageId::from_str(page.id.as_str()), Some(page.id));
        }
    }

    #[test]
    fn also_known_as_keys_are_real_headings() {
        for page in SETTINGS_PAGES {
            for (heading, _) in page.also_known_as {
                assert!(
                    page.headings.contains(heading),
                    "{} alias under missing heading {heading}",
                    page.id.as_str()
                );
            }
        }
    }

    #[test]
    fn search_finds_memory_worktree_node_ledger() {
        assert!(page_matches(page_by_id(SettingsPageId::Memory), "memory"));
        assert!(page_matches(page_by_id(SettingsPageId::Workspace), "worktree"));
        assert!(page_matches(page_by_id(SettingsPageId::Runtimes), "node"));
        assert!(page_matches(page_by_id(SettingsPageId::Privacy), "ledger"));
        assert!(!SETTINGS_PAGES.iter().any(|p| page_matches(p, "kombucha")));
    }

    #[test]
    fn empty_search_matches_every_page() {
        assert_eq!(
            SETTINGS_PAGES
                .iter()
                .filter(|p| page_matches(p, ""))
                .count(),
            13
        );
    }
}
