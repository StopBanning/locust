//! Shared chat-app rules (Electron `shared/chat-apps.ts`, 0.831-0.832): teammate commands, addressing a teammate
//! by name or `@command`, pairing codes, and settings shapes shared by Discord, Slack, and Telegram.

use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChatAppId {
    Discord,
    Slack,
    Telegram,
}

impl ChatAppId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Discord => "discord",
            Self::Slack => "slack",
            Self::Telegram => "telegram",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Discord => "Discord",
            Self::Slack => "Slack",
            Self::Telegram => "Telegram",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "discord" => Some(Self::Discord),
            "slack" => Some(Self::Slack),
            "telegram" => Some(Self::Telegram),
            _ => None,
        }
    }
}

/// The order Settings shows them in: the ones most people already have, first.
pub const CHAT_APPS: [ChatAppId; 3] = [ChatAppId::Discord, ChatAppId::Slack, ChatAppId::Telegram];

/// A message older than this, when Locust reads it, came while it was closed: it starts nothing.
pub const CHAT_STALE_MS: u64 = 10 * 60_000;
/// A pairing link or code works once, for this long.
pub const CHAT_LINK_MS: u64 = 10 * 60_000;
/// An answer longer than this many messages ends with where the rest is.
pub const CHAT_MAX_PARTS: usize = 4;
pub const CHAT_REST_IN_LOCUST: &str = "The rest is in Locust on your computer.";

/// Locust's own commands, which no teammate's name may take.
pub const CHAT_OWN_COMMANDS: [&str; 5] = ["start", "help", "who", "new", "waiting"];
const RESERVED: [&str; 9] = ["start", "help", "who", "new", "waiting", "settings", "stop", "cancel", "pair"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatAppBot {
    pub username: String,
    pub name: String,
    /// Discord: the link that adds the bot to a server of the person's.
    pub invite: Option<String>,
    /// Slack: the workspace the app is in.
    pub team: Option<String>,
    /// Where the bot's own chat opens in the app, once Locust knows.
    pub open: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatAppChat {
    pub name: String,
    pub username: Option<String>,
    pub paired_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatAppLink {
    pub url: String,
    pub until: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatAppCode {
    pub code: String,
    pub until: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChatAppSettings {
    pub bot: Option<ChatAppBot>,
    pub chat: Option<ChatAppChat>,
    pub link: Option<ChatAppLink>,
    pub code: Option<ChatAppCode>,
    pub connected: bool,
    pub message: Option<String>,
    pub alerts: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlackTokens {
    pub bot: String,
    pub app: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChatTeammateRef {
    pub teammate_id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddressedTeammate {
    pub teammate_id: String,
    pub rest: String,
}

/// NFKD decompose, strip combining marks (U+0300..=U+036F), and lowercase.
pub fn fold_nfkd(text: &str) -> String {
    #[cfg(target_os = "macos")]
    let decomposed = {
        use objc2_foundation::NSString;
        NSString::from_str(text).decomposedStringWithCompatibilityMapping().to_string()
    };
    #[cfg(not(target_os = "macos"))]
    let decomposed = text.to_owned();

    decomposed
        .chars()
        .filter(|&c| !('\u{0300}'..='\u{036f}').contains(&c))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Each teammate's command, in the team's order: lowercase letters, digits and underscores, 32 at most,
/// with a number after one that is taken or is one of Locust's own.
pub fn teammate_commands(team: &[ChatTeammateRef]) -> Vec<(String, String)> {
    let reserved: HashSet<&str> = RESERVED.iter().copied().collect();
    let mut taken: HashSet<String> = HashSet::new();
    let mut commands = Vec::with_capacity(team.len());

    for mate in team {
        let folded = fold_nfkd(&mate.name);
        let mut raw = String::with_capacity(folded.len());
        let mut last_underscore = false;
        for ch in folded.chars() {
            if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
                raw.push(ch);
                last_underscore = false;
            } else if !last_underscore {
                raw.push('_');
                last_underscore = true;
            }
        }
        let sliced: String = raw.chars().take(28).collect();
        let trimmed = sliced.trim_matches('_');
        let base = if trimmed.is_empty() { "teammate" } else { trimmed };

        let mut command = base.to_owned();
        let mut next = 2u32;
        while reserved.contains(command.as_str()) || taken.contains(&command) {
            command = format!("{base}{next}");
            next += 1;
        }
        taken.insert(command.clone());
        commands.push((mate.teammate_id.clone(), command));
    }
    commands
}

/// How many bytes of `text` spell `name` from its start, accents and case aside; None when it does not.
fn spelled(text: &str, name: &str) -> Option<usize> {
    let wanted = fold_nfkd(name);
    if wanted.is_empty() {
        return None;
    }
    let mut built = String::new();
    let mut byte_end = 0usize;
    for ch in text.chars() {
        if built.chars().count() >= wanted.chars().count() {
            break;
        }
        let mut buf = [0u8; 4];
        built.push_str(&fold_nfkd(ch.encode_utf8(&mut buf)));
        byte_end += ch.len_utf8();
    }
    if built == wanted { Some(byte_end) } else { None }
}

/// Byte length of separator at start of `after`:
/// - `at == true`: `^(?:\s*[:,]|\s|$)`
/// - `at == false`: `^\s*[:,]`
fn separator_len(after: &str, at: bool) -> Option<usize> {
    let ws_len: usize = after.chars().take_while(|c| c.is_whitespace()).map(char::len_utf8).sum();
    let rest = &after[ws_len..];
    if rest.starts_with(':') || rest.starts_with(',') {
        return Some(ws_len + 1);
    }
    if at && (ws_len > 0 || after.is_empty()) {
        return Some(ws_len);
    }
    None
}

/// A teammate named at the start of a message: "Ada: check the build", "Ada, check the build", or "@ada check the build".
pub fn addressed_teammate(text: &str, team: &[ChatTeammateRef]) -> Option<AddressedTeammate> {
    let trimmed = text.trim_start();
    let commands = teammate_commands(team);
    let mut best: Option<(String, String, usize)> = None;

    for (idx, mate) in team.iter().enumerate() {
        let cmd = &commands[idx].1;
        let name_trimmed = mate.name.trim();
        let candidates: Vec<&str> = if name_trimmed == cmd { vec![name_trimmed] } else { vec![name_trimmed, cmd.as_str()] };
        for said in candidates {
            for at in [true, false] {
                let head = if at {
                    if trimmed.starts_with('@') { 1 } else { continue }
                } else {
                    0
                };
                let Some(len) = spelled(&trimmed[head..], said) else { continue };
                let after = &trimmed[head + len..];
                let Some(sep_len) = separator_len(after, at) else { continue };
                let total = head + len;
                if best.as_ref().is_some_and(|(_, _, best_len)| *best_len >= total) {
                    continue;
                }
                let rest = after[sep_len..].trim().to_owned();
                best = Some((mate.teammate_id.clone(), rest, total));
            }
        }
    }

    best.map(|(teammate_id, rest, _)| AddressedTeammate { teammate_id, rest })
}

/// The 8-digit pairing code a message carries once spaces and dashes are out of it; None when it is something else.
pub fn pairing_code_in(text: &str) -> Option<String> {
    let digits: String = text.trim().chars().filter(|c| !c.is_whitespace() && *c != '-').collect();
    if digits.len() == 8 && digits.bytes().all(|b| b.is_ascii_digit()) {
        Some(digits)
    } else {
        None
    }
}

/// A code as a person reads it out: two groups of four.
pub fn spaced_code(code: &str) -> String {
    if code.len() == 8 {
        format!("{} {}", &code[..4], &code[4..])
    } else {
        code.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team() -> Vec<ChatTeammateRef> {
        vec![
            ChatTeammateRef { teammate_id: "t_ada".into(), name: "Ada".into() },
            ChatTeammateRef { teammate_id: "t_lovelace".into(), name: "Ada Lovelace".into() },
            ChatTeammateRef { teammate_id: "t_zoe".into(), name: "Zoë Ng-Smith".into() },
            ChatTeammateRef { teammate_id: "t_wren".into(), name: "Wren".into() },
        ]
    }

    #[test]
    fn reads_a_name_and_a_colon_or_a_comma_or_an_at_in_any_case_and_without_its_accents() {
        let t = team();
        assert_eq!(addressed_teammate("Ada: check the build", &t), Some(AddressedTeammate { teammate_id: "t_ada".into(), rest: "check the build".into() }));
        assert_eq!(addressed_teammate("ada, check the build", &t), Some(AddressedTeammate { teammate_id: "t_ada".into(), rest: "check the build".into() }));
        assert_eq!(addressed_teammate("  @ADA check the build", &t), Some(AddressedTeammate { teammate_id: "t_ada".into(), rest: "check the build".into() }));
        assert_eq!(addressed_teammate("@wren: and the tests", &t), Some(AddressedTeammate { teammate_id: "t_wren".into(), rest: "and the tests".into() }));
        assert_eq!(addressed_teammate("zoe ng-smith: hello", &t), Some(AddressedTeammate { teammate_id: "t_zoe".into(), rest: "hello".into() }));
        assert_eq!(addressed_teammate("zoe_ng_smith: hello", &t), Some(AddressedTeammate { teammate_id: "t_zoe".into(), rest: "hello".into() }));
        assert_eq!(addressed_teammate("Ada Lovelace: the engine", &t), Some(AddressedTeammate { teammate_id: "t_lovelace".into(), rest: "the engine".into() }));
        assert_eq!(addressed_teammate("Wren:", &t), Some(AddressedTeammate { teammate_id: "t_wren".into(), rest: "".into() }));
        assert_eq!(addressed_teammate("@wren", &t), Some(AddressedTeammate { teammate_id: "t_wren".into(), rest: "".into() }));
    }

    #[test]
    fn leaves_alone_a_message_that_only_begins_with_a_name_or_a_word_like_one() {
        let t = team();
        assert_eq!(addressed_teammate("Ada said the build is fine", &t), None);
        assert_eq!(addressed_teammate("Adam: hello", &t), None);
        assert_eq!(addressed_teammate("@adam hello", &t), None);
        assert_eq!(addressed_teammate("Note: the build failed", &t), None);
        assert_eq!(addressed_teammate("check Ada: later", &t), None);
        assert_eq!(addressed_teammate("", &t), None);
    }

    #[test]
    fn reads_a_pairing_code_however_it_was_spaced_and_nothing_else_as_one() {
        assert_eq!(pairing_code_in("4729 1836"), Some("47291836".into()));
        assert_eq!(pairing_code_in(" 4729-1836 "), Some("47291836".into()));
        assert_eq!(pairing_code_in("47291836"), Some("47291836".into()));
        assert_eq!(pairing_code_in("4729 183"), None);
        assert_eq!(pairing_code_in("my code is 47291836"), None);
        assert_eq!(spaced_code("47291836"), "4729 1836");
    }

    #[test]
    fn never_names_a_teammate_by_a_command_locust_keeps_for_itself() {
        let cmds = teammate_commands(&[
            ChatTeammateRef { teammate_id: "a".into(), name: "Pair".into() },
            ChatTeammateRef { teammate_id: "b".into(), name: "Waiting".into() },
        ]);
        let vals: Vec<&str> = cmds.iter().map(|(_, v)| v.as_str()).collect();
        assert_eq!(vals, ["pair2", "waiting2"]);
    }
}
