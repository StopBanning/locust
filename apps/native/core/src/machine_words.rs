//! How this computer names things (Electron `shared/machine-words.ts`, 0.843, the first Mac run of 2026-10-30).
//!
//! A Mac showed "the tray" and "sign in to Windows", words from the only machine Locust had run on. A closed Locust
//! keeps running in Windows' tray and in a Mac's menu bar; it starts when a person signs in to Windows, and when they
//! log in to a Mac. Anything that is not a Mac keeps the words it always had.

/// `MachineWords`: the words a window uses for its own computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachineWords {
    /// Where a closed Locust keeps running.
    pub tray: &'static str,
    /// Starting with the computer, as its own settings say it.
    pub sign_in: &'static str,
    /// What the person calls this computer: "this PC", "this Mac".
    pub computer: &'static str,
    /// The verb alone, for "Start Locust when you sign in".
    pub log_in: &'static str,
    /// The key a shortcut is held with, as a title names it: "Cmd" on a Mac, "Ctrl" elsewhere.
    pub modifier: &'static str,
}

const MAC: MachineWords = MachineWords { tray: "menu bar", sign_in: "log in to this Mac", computer: "Mac", log_in: "log in", modifier: "Cmd" };
const ELSEWHERE: MachineWords = MachineWords { tray: "tray", sign_in: "sign in to Windows", computer: "PC", log_in: "sign in", modifier: "Ctrl" };

/// `machineWords(platform)`: Node's platform name ("darwin", "win32", "linux"), or none when it is not known.
pub fn machine_words(platform: Option<&str>) -> MachineWords {
    if platform == Some("darwin") { MAC } else { ELSEWHERE }
}

/// This computer's platform in Node's names, as Electron's `window.desktop.platform` gives it.
pub fn this_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// `windowWords()`: the words for the computer this runs on.
pub fn this_machine() -> MachineWords {
    machine_words(Some(this_platform()))
}

/// `StayingReachable` (RemoteMcpSettings.tsx): the Startup switches "Use from anywhere" leans on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stay {
    pub open_at_login: bool,
    /// The login item can be set here (`loginItem.available === true`).
    pub login_available: bool,
    pub keep_running: bool,
}

/// `reachable(stay)`: kept running, and started at login where that can be set.
pub fn reachable(stay: Stay) -> bool {
    stay.keep_running && (stay.open_at_login || !stay.login_available)
}

/// The "keep it running" line under From anywhere's address (RemoteMcpSettings.tsx), in this computer's words.
pub fn stay_line(words: MachineWords, stay: Stay) -> String {
    if reachable(stay) {
        let starts = if stay.open_at_login { format!("it starts when you {}, and ", words.sign_in) } else { String::new() };
        format!(
            "Locust stays reachable while this computer is on and awake: {starts}it keeps running in the {tray} when you close its window. Quit from the {tray}, and no app can open it again.",
            tray = words.tray
        )
    } else {
        let start = if stay.login_available { format!(", and start it when you {}", words.sign_in) } else { String::new() };
        format!(
            "An app can reach Locust only while it is running here: once it quits, nothing on this computer can open it. Keep it running in the {} when you close its window{start}.",
            words.tray
        )
    }
}

/// Where a saved OpenAI key for voice typing is locked (VoiceSettings.tsx): a Mac's keychain, else Windows' account.
pub fn voice_key_locked_to(platform: Option<&str>) -> &'static str {
    if platform == Some("darwin") { "this Mac's keychain" } else { "your Windows account" }
}

#[cfg(test)]
#[path = "machine_words_tests.rs"]
mod tests;
