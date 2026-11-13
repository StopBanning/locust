//! Workspace settings bag — port of `main/teammate-store.ts` settings parse/write.
//!
//! Stored in native `teammates.json` `settings` (seed-once from Electron). Appearance
//! lives in native `appearance.json` (also seed-once), matching Electron's split.

use crate::store::Store;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub const DEFAULT_RELAY_HOP_CAP: u32 = 12;
pub const MIN_RELAY_HOP_CAP: u32 = 1;
pub const MAX_RELAY_HOP_CAP: u32 = 24;
pub const DEFAULT_MEMORY_MODE: &str = "auto";

pub const PROJECT_FOLDER_LEDE: &str = "Your teammates are the same in every folder. The folder is what they work on: its files, its conversations, what they remember about it, and its LOCUST.md. Anything remembered for everywhere goes with them.";
pub const DATA_FOLDER_HEADING: &str = "Native data folder";
pub const MEMORY_WHEN: &str = "When a teammate writes a memory";
pub const MEMORY_KEEP: &str = "Keep and tell me";
pub const MEMORY_ASK: &str = "Ask me first";
pub const MEMORY_OFF: &str = "Off";
pub const THEME_HEADING: &str = "Theme";
pub const THEME_DARK: &str = "Dark";
pub const THEME_LIGHT: &str = "Light";
pub const THEME_SYSTEM: &str = "Match computer";

#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceSettings {
    pub swarm: bool,
    pub relay: bool,
    pub relay_hop_cap: u32,
    pub interrupt: bool,
    pub memory_mode: String,
    pub auto_mode: bool,
    pub ask_connectors: bool,
    pub keep_a_todo_list: bool,
    pub layout: String,
    pub tube: String,
    pub reply_size: String,
    pub metal: String,
    pub metal_strength: String,
    pub metal_motion: String,
    pub metal_bend: bool,
    pub terminal_faces: bool,
    pub plush: bool,
    pub claude_own_skills: bool,
    pub check_commands: BTreeMap<String, String>,
    pub check_fixes: BTreeMap<String, u32>,
    pub about_you: Option<String>,
}

impl Default for WorkspaceSettings {
    fn default() -> Self {
        Self {
            swarm: false,
            relay: true,
            relay_hop_cap: DEFAULT_RELAY_HOP_CAP,
            interrupt: false,
            memory_mode: DEFAULT_MEMORY_MODE.into(),
            auto_mode: false,
            ask_connectors: false,
            keep_a_todo_list: true,
            layout: "auto".into(),
            tube: "full".into(),
            reply_size: "standard".into(),
            metal: "silver".into(),
            metal_strength: "standard".into(),
            metal_motion: "hover".into(),
            metal_bend: false,
            terminal_faces: true,
            plush: false,
            claude_own_skills: false,
            check_commands: BTreeMap::new(),
            check_fixes: BTreeMap::new(),
            about_you: None,
        }
    }
}

fn one_of(value: &Value, allowed: &[&str], fallback: &str) -> String {
    value
        .as_str()
        .filter(|s| allowed.contains(s))
        .unwrap_or(fallback)
        .to_owned()
}

fn hop_cap(value: &Value) -> u32 {
    value
        .as_u64()
        .filter(|&n| n >= u64::from(MIN_RELAY_HOP_CAP) && n <= u64::from(MAX_RELAY_HOP_CAP))
        .map(|n| n as u32)
        .unwrap_or(DEFAULT_RELAY_HOP_CAP)
}

fn check_command(value: &Value) -> Option<String> {
    let command = value.as_str()?.trim();
    if command.is_empty()
        || command.len() > 500
        || command.chars().any(|c| c == '\r' || c == '\n' || c == '\0')
    {
        return None;
    }
    Some(command.to_owned())
}

fn parse_check_commands(value: &Value) -> BTreeMap<String, String> {
    let Some(obj) = value.as_object() else {
        return BTreeMap::new();
    };
    let mut kept = BTreeMap::new();
    for (folder, command) in obj.iter().take(200) {
        if folder.is_empty() || folder.len() > 200 {
            continue;
        }
        if let Some(parsed) = check_command(command) {
            kept.insert(folder.clone(), parsed);
        }
    }
    kept
}

fn parse_check_fixes(value: &Value) -> BTreeMap<String, u32> {
    let Some(obj) = value.as_object() else {
        return BTreeMap::new();
    };
    let mut kept = BTreeMap::new();
    for (folder, tries) in obj.iter().take(200) {
        if folder.is_empty() || folder.len() > 200 {
            continue;
        }
        if let Some(n) = tries.as_u64().filter(|&n| (1..=5).contains(&n)) {
            kept.insert(folder.clone(), n as u32);
        }
    }
    kept
}

pub fn parse_settings(raw: &Value) -> WorkspaceSettings {
    let empty = Value::Object(Default::default());
    let raw = if raw.is_object() { raw } else { &empty };
    WorkspaceSettings {
        swarm: raw.get("swarm") == Some(&Value::Bool(true)),
        relay: raw.get("relay") != Some(&Value::Bool(false)),
        relay_hop_cap: hop_cap(raw.get("relayHopCap").unwrap_or(&Value::Null)),
        interrupt: raw.get("interrupt") == Some(&Value::Bool(true)),
        memory_mode: one_of(
            raw.get("memoryMode").unwrap_or(&Value::Null),
            &["auto", "ask", "off"],
            DEFAULT_MEMORY_MODE,
        ),
        auto_mode: raw.get("autoMode") == Some(&Value::Bool(true)),
        ask_connectors: raw.get("askConnectors") == Some(&Value::Bool(true)),
        keep_a_todo_list: match raw.get("keepATodoList") {
            Some(Value::Bool(v)) => *v,
            _ => true,
        },
        layout: one_of(
            raw.get("layout").unwrap_or(&Value::Null),
            &["auto", "compact", "wide"],
            "auto",
        ),
        tube: one_of(
            raw.get("tube").unwrap_or(&Value::Null),
            &["full", "subtle", "off"],
            "full",
        ),
        reply_size: one_of(
            raw.get("replySize").unwrap_or(&Value::Null),
            &["standard", "large", "largest"],
            "standard",
        ),
        metal: one_of(
            raw.get("metal").unwrap_or(&Value::Null),
            &["off", "chromatic", "silver", "gold"],
            "silver",
        ),
        metal_strength: one_of(
            raw.get("metalStrength").unwrap_or(&Value::Null),
            &["subtle", "standard", "strong"],
            "standard",
        ),
        metal_motion: one_of(
            raw.get("metalMotion").unwrap_or(&Value::Null),
            &["hover", "always"],
            "hover",
        ),
        metal_bend: match raw.get("metalBend") {
            Some(Value::Bool(v)) => *v,
            _ => false,
        },
        terminal_faces: raw.get("terminalFaces") != Some(&Value::Bool(false)),
        plush: raw.get("plush") == Some(&Value::Bool(true)),
        claude_own_skills: raw.get("claudeOwnSkills") == Some(&Value::Bool(true)),
        check_commands: parse_check_commands(raw.get("checkCommands").unwrap_or(&Value::Null)),
        check_fixes: parse_check_fixes(raw.get("checkFixes").unwrap_or(&Value::Null)),
        about_you: raw
            .get("aboutYou")
            .and_then(Value::as_str)
            .map(str::to_owned),
    }
}

pub fn settings_to_json(settings: &WorkspaceSettings) -> Value {
    let mut obj = json!({
        "swarm": settings.swarm,
        "relay": settings.relay,
        "relayHopCap": settings.relay_hop_cap,
        "interrupt": settings.interrupt,
        "memoryMode": settings.memory_mode,
        "autoMode": settings.auto_mode,
        "askConnectors": settings.ask_connectors,
        "keepATodoList": settings.keep_a_todo_list,
        "layout": settings.layout,
        "tube": settings.tube,
        "replySize": settings.reply_size,
        "metal": settings.metal,
        "metalStrength": settings.metal_strength,
        "metalMotion": settings.metal_motion,
        "metalBend": settings.metal_bend,
        "terminalFaces": settings.terminal_faces,
        "plush": settings.plush,
        "claudeOwnSkills": settings.claude_own_skills,
    });
    if !settings.check_commands.is_empty() {
        obj["checkCommands"] = json!(settings.check_commands);
    }
    if !settings.check_fixes.is_empty() {
        obj["checkFixes"] = json!(settings.check_fixes);
    }
    if let Some(note) = &settings.about_you {
        obj["aboutYou"] = json!(note);
    }
    obj
}

pub fn load(store: &Store) -> Result<WorkspaceSettings, String> {
    let file = crate::people::team(store)?;
    Ok(parse_settings(&file["settings"]))
}

pub fn save(store: &Store, settings: &WorkspaceSettings) -> Result<(), String> {
    let mut file = crate::people::team(store)?;
    let mut merged = settings_to_json(settings);
    // Keep keys we do not model yet (aboutYouSuggestions, newChatRoute, planRunsOut).
    if let (Some(prev), Some(next)) = (file["settings"].as_object(), merged.as_object_mut()) {
        for (key, value) in prev {
            if !next.contains_key(key) {
                next.insert(key.clone(), value.clone());
            }
        }
    }
    file["settings"] = merged;
    store.save_json("teammates.json", &file)
}

pub fn workspace_id_for(path: &Path) -> String {
    crate::people::workspace_id(path)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    Dark,
    Light,
    System,
}

impl ThemeChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::System => "system",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "light" => Self::Light,
            "system" => Self::System,
            _ => Self::Dark,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => THEME_DARK,
            Self::Light => THEME_LIGHT,
            Self::System => THEME_SYSTEM,
        }
    }

    pub fn lede(self, resolved_dark: Option<bool>) -> String {
        match self {
            Self::Light => "Light. For a bright room, or a desktop that is light.".into(),
            Self::System => format!(
                "Matches your computer, and changes when it does. It is {} now.",
                if resolved_dark.unwrap_or(true) {
                    "dark"
                } else {
                    "light"
                }
            ),
            Self::Dark => "Dark. Locust as it was first drawn.".into(),
        }
    }

    /// Map to State.appearance: Some(true)=dark, Some(false)=light, None=system.
    pub fn to_appearance(self) -> Option<bool> {
        match self {
            Self::Dark => Some(true),
            Self::Light => Some(false),
            Self::System => None,
        }
    }

    pub fn from_appearance(appearance: Option<bool>) -> Self {
        match appearance {
            Some(true) => Self::Dark,
            Some(false) => Self::Light,
            None => Self::System,
        }
    }
}

pub fn load_theme(store: &Store) -> ThemeChoice {
    let empty = json!({"theme":"dark","look":"standard","glassGround":"desktop"});
    match store.seeded_json("appearance.json", empty) {
        Ok(file) => ThemeChoice::from_str(file["theme"].as_str().unwrap_or("dark")),
        Err(_) => ThemeChoice::Dark,
    }
}

pub fn save_theme(store: &Store, theme: ThemeChoice) -> Result<(), String> {
    let empty = json!({"theme":"dark","look":"standard","glassGround":"desktop"});
    let mut file = store.seeded_json("appearance.json", empty)?;
    file["theme"] = json!(theme.as_str());
    if file.get("look").is_none() {
        file["look"] = json!("standard");
    }
    if file.get("glassGround").is_none() {
        file["glassGround"] = json!("desktop");
    }
    store.save_json("appearance.json", &file)
}

pub fn theme_lede(theme: ThemeChoice, resolved_dark: Option<bool>) -> String {
    theme.lede(resolved_dark)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LookChoice {
    #[default]
    Standard,
    Liquid,
}

impl LookChoice {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Liquid => "liquid",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "liquid" => Self::Liquid,
            _ => Self::Standard,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Liquid => "Liquid glass",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GlassGround {
    #[default]
    Desktop,
    Rainbow,
}

impl GlassGround {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Rainbow => "rainbow",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "rainbow" => Self::Rainbow,
            _ => Self::Desktop,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Desktop => "Your desktop",
            Self::Rainbow => "Rainbow",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppearancePrefs {
    pub theme: ThemeChoice,
    pub look: LookChoice,
    pub glass_ground: GlassGround,
}

/// The theme a new Linux install starts on (`appearance.ts:35`): the computer's.
/// Windows and Mac keep Dark (returns None).
pub fn first_theme(platform: &str, profile_is_new: bool) -> Option<ThemeChoice> {
    if platform == "linux" && profile_is_new {
        Some(ThemeChoice::System)
    } else {
        None
    }
}

/// The theme a choice comes to now: the computer's (`system_is_dark`), for Match computer.
pub fn resolved_theme(theme: ThemeChoice, system_is_dark: bool) -> bool {
    match theme {
        ThemeChoice::System => system_is_dark,
        ThemeChoice::Dark => true,
        ThemeChoice::Light => false,
    }
}

/// The window's ground before the page paints: `--lc-bg-app` of each theme.
pub fn window_ground(is_dark: bool) -> &'static str {
    if is_dark { "#090a0c" } else { "#f3f2ee" }
}

pub const THEME_ARGUMENT: &str = "--locust-theme=";
pub const LOOK_ARGUMENT: &str = "--locust-look=";

/// The theme a window was started in, from its command line; dark (true) when unreadable.
pub fn theme_from_arguments<S: AsRef<str>>(argv: &[S]) -> bool {
    let given = argv
        .iter()
        .find_map(|arg| arg.as_ref().strip_prefix(THEME_ARGUMENT));
    given != Some("light")
}

/// Whether a window started drawn in Liquid glass, from its command line.
pub fn liquid_from_arguments<S: AsRef<str>>(argv: &[S]) -> bool {
    argv.iter()
        .find_map(|arg| arg.as_ref().strip_prefix(LOOK_ARGUMENT))
        == Some("liquid")
}

pub fn look_lede(look: LookChoice, platform: &str) -> &'static str {
    let windows = platform == "win32";
    match look {
        LookChoice::Liquid => {
            if windows {
                "The window on Windows’ glass, the sidebar on a floating panel. It shows your desktop through it, so it is brightest over a colourful one."
            } else {
                "The window on macOS’s glass, the sidebar on a floating panel. It shows your desktop through it, so it is brightest over a colourful one."
            }
        }
        LookChoice::Standard => {
            if windows {
                "Locust as it is drawn everywhere."
            } else {
                "Locust as it is drawn everywhere, with the sidebar on the Mac’s glass."
            }
        }
    }
}

pub fn ground_lede(ground: GlassGround) -> &'static str {
    match ground {
        GlassGround::Rainbow => {
            "Locust’s own rainbow, under the glass inside the window, so it glows the same over any desktop."
        }
        GlassGround::Desktop => "Whatever is behind the window: brightest over a colourful desktop.",
    }
}

pub fn read_theme_str(raw: Result<&str, ()>) -> ThemeChoice {
    let Ok(text) = raw else {
        return ThemeChoice::Dark;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(text) else {
        return ThemeChoice::Dark;
    };
    ThemeChoice::from_str(val["theme"].as_str().unwrap_or("dark"))
}

pub fn load_appearance(store: &Store) -> AppearancePrefs {
    let empty = json!({"theme":"dark","look":"standard","glassGround":"desktop"});
    match store.seeded_json("appearance.json", empty) {
        Ok(file) => AppearancePrefs {
            theme: ThemeChoice::from_str(file["theme"].as_str().unwrap_or("dark")),
            look: LookChoice::from_str(file["look"].as_str().unwrap_or("standard")),
            glass_ground: GlassGround::from_str(file["glassGround"].as_str().unwrap_or("desktop")),
        },
        Err(_) => AppearancePrefs::default(),
    }
}

/// Load appearance for startup, applying `first_theme` when this is a brand-new Linux profile.
pub fn load_appearance_for_platform(store: &Store, platform: &str) -> AppearancePrefs {
    let has_appearance =
        store.root().join("appearance.json").exists() || store.electron().join("appearance.json").exists();
    let has_teammates =
        store.root().join("teammates.json").exists() || store.electron().join("teammates.json").exists();
    let profile_is_new = !has_appearance && !has_teammates;
    if let Some(first) = first_theme(platform, profile_is_new) {
        let mut prefs = load_appearance(store);
        prefs.theme = first;
        let _ = save_appearance(store, prefs);
        return prefs;
    }
    load_appearance(store)
}

pub fn save_appearance(store: &Store, prefs: AppearancePrefs) -> Result<(), String> {
    store.save_json(
        "appearance.json",
        &json!({
            "theme": prefs.theme.as_str(),
            "look": prefs.look.as_str(),
            "glassGround": prefs.glass_ground.as_str(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct DropDir(std::path::PathBuf);
    impl Drop for DropDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp_store() -> (DropDir, Store) {
        let root = std::env::temp_dir().join(format!(
            "locust-ws-{}",
            crate::native::uuid_v4().unwrap_or_else(|_| "test".into())
        ));
        let electron = root.join("electron");
        let native = root.join("native");
        fs::create_dir_all(&electron).unwrap();
        fs::create_dir_all(&native).unwrap();
        let store = Store::new(native, electron).unwrap();
        (DropDir(root), store)
    }

    #[test]
    fn defaults_match_electron() {
        let s = parse_settings(&json!({}));
        assert_eq!(s, WorkspaceSettings::default());
        assert!(s.relay);
        assert!(!s.swarm);
        assert_eq!(s.relay_hop_cap, 12);
        assert_eq!(s.memory_mode, "auto");
        assert!(s.terminal_faces);
        assert!(s.keep_a_todo_list);
    }

    #[test]
    fn hop_cap_clamps_and_terminal_faces_false_only() {
        let s = parse_settings(&json!({"relayHopCap": 99, "terminalFaces": false}));
        assert_eq!(s.relay_hop_cap, 12);
        assert!(!s.terminal_faces);
        let s = parse_settings(&json!({"relayHopCap": 4}));
        assert_eq!(s.relay_hop_cap, 4);
    }

    #[test]
    fn workspace_id_stable() {
        let a = workspace_id_for(Path::new("/tmp/demo"));
        let b = workspace_id_for(Path::new("/tmp/demo"));
        assert_eq!(a, b);
        assert!(a.starts_with("ws_"));
        assert_eq!(a.len(), 35);
    }

    #[test]
    fn load_save_round_trip_preserves_teammates() {
        let (_dir, store) = temp_store();
        store
            .save_json(
                "teammates.json",
                &json!({
                    "schemaVersion": 1,
                    "teammates": [{"teammateId":"tm_1","name":"Codec","role":"Custom","hue":"lime","createdAt":"2026-01-01T00:00:00.000Z","route":{"runtime":"codex","model":"account-default","mode":"ask"}}],
                    "missionOwners": {},
                    "settings": {"swarm": true, "aboutYouSuggestions": [{"id":"ays_1","text":"Keep it short."}]}
                }),
            )
            .unwrap();
        let mut settings = load(&store).unwrap();
        assert!(settings.swarm);
        settings.auto_mode = true;
        settings.memory_mode = "ask".into();
        save(&store, &settings).unwrap();
        let file = crate::people::team(&store).unwrap();
        assert_eq!(file["teammates"][0]["name"], "Codec");
        assert_eq!(file["settings"]["autoMode"], true);
        assert_eq!(file["settings"]["memoryMode"], "ask");
        assert_eq!(
            file["settings"]["aboutYouSuggestions"][0]["text"],
            "Keep it short."
        );
    }

    #[test]
    fn appearance_seed_and_theme_words() {
        let (_dir, store) = temp_store();
        assert_eq!(load_theme(&store), ThemeChoice::Dark);
        save_theme(&store, ThemeChoice::Light).unwrap();
        assert_eq!(load_theme(&store), ThemeChoice::Light);
        assert_eq!(
            theme_lede(ThemeChoice::Dark, None),
            "Dark. Locust as it was first drawn."
        );
        assert!(theme_lede(ThemeChoice::System, Some(false)).contains("light now"));
    }

    #[test]
    fn saved_theme_defaults_and_reads_persons_choice() {
        assert_eq!(read_theme_str(Err(())), ThemeChoice::Dark);
        assert_eq!(read_theme_str(Ok("not json")), ThemeChoice::Dark);
        assert_eq!(read_theme_str(Ok(r#"{"theme":"sepia"}"#)), ThemeChoice::Dark);
        assert_eq!(read_theme_str(Ok(r#"{"theme":"light"}"#)), ThemeChoice::Light);
        assert_eq!(read_theme_str(Ok(r#"{"theme":"system"}"#)), ThemeChoice::System);
    }

    #[test]
    fn first_theme_starts_system_only_on_new_linux_profile() {
        assert_eq!(first_theme("linux", true), Some(ThemeChoice::System));
        assert_eq!(first_theme("linux", false), None);
        assert_eq!(first_theme("win32", true), None);
        assert_eq!(first_theme("darwin", true), None);

        let (_dir, store) = temp_store();
        let prefs = load_appearance_for_platform(&store, "linux");
        assert_eq!(prefs.theme, ThemeChoice::System);
    }

    #[test]
    fn theme_resolution_window_ground_and_cli_args() {
        assert!(resolved_theme(ThemeChoice::System, true));
        assert!(!resolved_theme(ThemeChoice::System, false));
        assert!(!resolved_theme(ThemeChoice::Light, true));
        assert!(resolved_theme(ThemeChoice::Dark, false));

        assert_eq!(window_ground(true), "#090a0c");
        assert_eq!(window_ground(false), "#f3f2ee");

        assert!(!theme_from_arguments(&["locust", "--locust-theme=light"]));
        assert!(theme_from_arguments(&["locust", "--locust-theme=dark"]));
        assert!(theme_from_arguments(&["locust", "--locust-theme=chartreuse"]));
        assert!(theme_from_arguments(&["locust"]));
        assert!(liquid_from_arguments(&["locust", "--locust-look=liquid"]));
        assert!(!liquid_from_arguments(&["locust", "--locust-look=standard"]));
    }

    #[test]
    fn theme_and_look_ledes_match_electron() {
        assert_eq!(
            theme_lede(ThemeChoice::Dark, Some(true)),
            "Dark. Locust as it was first drawn."
        );
        assert_eq!(
            theme_lede(ThemeChoice::Light, Some(false)),
            "Light. For a bright room, or a desktop that is light."
        );
        assert_eq!(
            theme_lede(ThemeChoice::System, Some(false)),
            "Matches your computer, and changes when it does. It is light now."
        );
        assert_eq!(
            theme_lede(ThemeChoice::System, Some(true)),
            "Matches your computer, and changes when it does. It is dark now."
        );
        assert_eq!(
            theme_lede(ThemeChoice::System, None),
            "Matches your computer, and changes when it does. It is dark now."
        );

        assert!(look_lede(LookChoice::Liquid, "darwin").contains("macOS’s glass"));
        assert!(look_lede(LookChoice::Liquid, "win32").contains("Windows’ glass"));
        assert!(look_lede(LookChoice::Liquid, "darwin").contains("desktop"));
        assert!(look_lede(LookChoice::Standard, "darwin").contains("everywhere"));
        assert!(!look_lede(LookChoice::Standard, "win32").contains("Mac"));
        assert!(ground_lede(GlassGround::Rainbow).contains("rainbow"));
        assert!(ground_lede(GlassGround::Desktop).contains("behind the window"));
    }

    #[test]
    fn save_appearance_keeps_all_fields_together() {
        let (_dir, store) = temp_store();
        let prefs = AppearancePrefs {
            theme: ThemeChoice::Light,
            look: LookChoice::Liquid,
            glass_ground: GlassGround::Rainbow,
        };
        save_appearance(&store, prefs).unwrap();
        assert_eq!(load_appearance(&store), prefs);
    }
}
