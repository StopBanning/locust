//! This computer's side of keep-awake and start-at-login (Electron's `powerSaveBlocker` and `app.setLoginItemSettings`).
//!
//! Keep awake is Apple's `caffeinate -i -w <this process>`: it holds idle sleep off, and it ends with this process
//! whatever happens, so a crash can never leave the Mac kept awake. Start at login is a launch agent in the person's
//! own LaunchAgents folder that opens the app with `--background`; only a copy of the app from its bundle registers,
//! never a development build. Tests use the core's stand-ins and touch neither.

use locust_core::keep_awake::PowerBlocker;
use locust_core::login_item::{HostLoginItemSettings, LoginItemHost, LoginItemQuery};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct Caffeinate {
    next: u64,
    held: HashMap<u64, std::process::Child>,
}

impl PowerBlocker for Caffeinate {
    fn start(&mut self) -> Result<u64, String> {
        let child = std::process::Command::new("/usr/bin/caffeinate")
            .args(["-i", "-w", &std::process::id().to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.next += 1;
        self.held.insert(self.next, child);
        Ok(self.next)
    }
    fn stop(&mut self, id: u64) {
        if let Some(mut child) = self.held.remove(&id) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

type Awake = locust_core::keep_awake::KeepAwake<Caffeinate>;
static AWAKE: Mutex<Option<Awake>> = Mutex::new(None);

/// Hold the Mac awake while a turn runs, or always when the person asked to stay awake.
pub fn keep_awake(live_runs: usize, stay_awake: bool) {
    if cfg!(test) {
        return;
    }
    let mut held = AWAKE.lock().unwrap_or_else(|e| e.into_inner());
    held.get_or_insert_with(|| Awake::new(Caffeinate::default())).update(live_runs, stay_awake);
}

/// Locust is quitting: the hold goes with it (caffeinate would end with the process anyway).
pub fn release_awake() {
    if let Some(awake) = AWAKE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        awake.dispose();
    }
}

pub const LAUNCH_AGENT: &str = "lol.locust.native";

/// A launch agent in the person's own LaunchAgents folder.
pub struct LaunchAgent {
    pub file: PathBuf,
}

impl LaunchAgent {
    pub fn for_user() -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        Some(Self { file: home.join("Library").join("LaunchAgents").join(format!("{LAUNCH_AGENT}.plist")) })
    }
}

fn escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl LoginItemHost for LaunchAgent {
    fn set_login_item_settings(&mut self, open_at_login: bool, query: Option<&LoginItemQuery>) {
        if !open_at_login {
            let _ = std::fs::remove_file(&self.file);
            return;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        let mut args = vec![exe.display().to_string()];
        args.extend(query.map(|q| q.args.clone()).unwrap_or_else(|| vec![locust_core::login_item::BACKGROUND_ARG.to_owned()]));
        let program: String = args.iter().map(|a| format!("<string>{}</string>", escape(a))).collect();
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>{LAUNCH_AGENT}</string><key>ProgramArguments</key><array>{program}</array><key>RunAtLoad</key><true/></dict></plist>\n"
        );
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.file, plist);
    }
    fn get_login_item_settings(&self, _query: Option<&LoginItemQuery>) -> HostLoginItemSettings {
        HostLoginItemSettings { open_at_login: self.file.exists(), status: None }
    }
}

/// Whether this is the app from its bundle, the only copy that registers at login.
pub fn packaged() -> bool {
    std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
}

/// The login item this window offers: the real one in the app, the stand-in in tests.
pub fn login_item() -> Box<dyn LoginItemHost> {
    if cfg!(test) {
        return Box::new(locust_core::login_item::UnwiredLoginItemHost::default());
    }
    match LaunchAgent::for_user() {
        Some(agent) => Box::new(agent),
        None => Box::new(locust_core::login_item::UnwiredLoginItemHost::default()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_agent_opens_this_program_in_the_background_and_goes_when_switched_off() {
        let dir = std::env::temp_dir().join(format!("locust-launch-agent-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
        let mut agent = LaunchAgent { file: dir.join(format!("{LAUNCH_AGENT}.plist")) };
        assert!(!agent.get_login_item_settings(None).open_at_login);
        agent.set_login_item_settings(true, None);
        let plist = std::fs::read_to_string(&agent.file).unwrap();
        assert!(plist.contains("<key>RunAtLoad</key><true/>") && plist.contains("<string>--background</string>") && plist.contains(LAUNCH_AGENT));
        assert!(agent.get_login_item_settings(None).open_at_login);
        agent.set_login_item_settings(false, None);
        assert!(!agent.file.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
