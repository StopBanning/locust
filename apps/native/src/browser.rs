//! Browser state and address rules. No AppKit, user profile or implicit store access.
use serde_json::{Value, json};
use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use url::Url;

pub const TEMPORARY_SIGN_INS: &str = "On this macOS, teammate sign-ins last until Locust quits.";
pub type Shared = Arc<Mutex<Host>>;
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Profile(pub Option<String>);
impl Profile {
    pub fn owns(&self, other: &Self) -> bool {
        self == other
    }
}
#[derive(Clone, Debug)]
pub enum Command {
    Load(String),
    Back,
    Forward,
    Reload,
    Stop,
    Evaluate {
        script: String,
        expected_url: String,
        cancel: Arc<std::sync::atomic::AtomicBool>,
        reply: std::sync::mpsc::SyncSender<Result<Value, String>>,
    },
}
pub struct Tab {
    pub id: u64,
    pub profile: Profile,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub back: bool,
    pub forward: bool,
    pub origins: HashSet<String>,
    pub tool_origins: HashSet<String>,
    pub tool_control: bool,
    pub console: VecDeque<Value>,
    pub network: VecDeque<Value>,
    pub commands: VecDeque<Command>,
}
pub struct Host {
    pub tabs: Vec<Tab>,
    pub active: Option<u64>,
    pub next: u64,
    pub open: bool,
    pub rect: [f32; 4],
    pub error: Option<String>,
    pub workspace: Option<PathBuf>,
    pub profiles: Vec<(String, String)>,
    pub preview: crate::page_preview::Pages,
    pub temporary: bool,
    pub refusal: Option<String>,
    pub namespace: String,
    pub occluded: bool,
    pub forgotten: Vec<Profile>,
    pub clearing: HashSet<Profile>,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            tabs: vec![],
            active: None,
            next: 0,
            open: false,
            rect: [0.; 4],
            error: None,
            workspace: None,
            profiles: vec![],
            preview: Default::default(),
            temporary: false,
            refusal: None,
            namespace: String::new(),
            occluded: false,
            forgotten: vec![],
            clearing: HashSet::new(),
        }
    }
}
impl Host {
    pub fn add(&mut self, profile: Profile) -> Result<u64, String> {
        if let Some(refusal) = &self.refusal {
            return Err(refusal.clone());
        }
        if self.clearing.contains(&profile) {
            return Err(
                "Browser sign-ins are being cleared. Try again when clearing has finished.".into(),
            );
        }
        if profile
            .0
            .as_ref()
            .is_some_and(|id| !self.profiles.iter().any(|(p, _)| p == id))
        {
            return Err("That teammate does not have its own browser sign-ins any more, so no tab was opened.".into());
        }
        if self.tabs.len() >= 32 {
            return Err("Close a browser tab before opening another.".into());
        }
        self.next += 1;
        self.tabs.push(Tab {
            id: self.next,
            profile,
            url: String::new(),
            title: "New tab".into(),
            loading: false,
            back: false,
            forward: false,
            origins: HashSet::new(),
            tool_origins: HashSet::new(),
            tool_control: false,
            console: VecDeque::new(),
            network: VecDeque::new(),
            commands: VecDeque::new(),
        });
        self.active = Some(self.next);
        self.open = true;
        Ok(self.next)
    }
    pub fn forget(&mut self, profile: Profile) {
        let ids = self
            .tabs
            .iter()
            .filter(|t| t.profile == profile)
            .map(|t| t.id)
            .collect::<Vec<_>>();
        for id in ids {
            self.close(id);
        }
        self.clearing.insert(profile.clone());
        self.forgotten.push(profile);
    }
    pub fn close(&mut self, id: u64) {
        let index = self.tabs.iter().position(|t| t.id == id);
        self.tabs.retain(|t| t.id != id);
        if self.active == Some(id) {
            self.active = index
                .and_then(|i| self.tabs.get(i.min(self.tabs.len().saturating_sub(1))))
                .map(|t| t.id);
        }
    }
    pub fn navigate(&mut self, id: u64, raw: &str, person: bool) -> Result<(), String> {
        if let Some(refusal) = &self.refusal {
            return Err(refusal.clone());
        }
        let resolved = resolve(raw, self.workspace.as_deref())?;
        let address = if resolved.scheme() == "file" {
            self.preview.url_for(
                &resolved.to_file_path().map_err(|_| "Not an address.")?,
                self.workspace.as_deref(),
            )?
        } else {
            resolved.to_string()
        };
        let tab = self
            .tabs
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or("That browser tab is no longer open.")?;
        if person {
            tab.origins.insert(origin(&resolved));
            tab.tool_control = false;
        } else {
            tab.tool_control = true;
            tab.tool_origins.insert(origin(&resolved));
        }
        tab.commands.push_back(Command::Load(address));
        Ok(())
    }
    pub fn scoped(&self, id: u64, profile: &Profile) -> Result<&Tab, String> {
        let tab = self
            .tabs
            .iter()
            .find(|t| t.id == id)
            .ok_or("That browser tab is no longer open.")?;
        if !profile.owns(&tab.profile) {
            return Err(format!(
                "Tab \"tab_{id}\" is signed in as someone else, so it is not yours to use. Open a tab of your own with browser_tabs."
            ));
        }
        Ok(tab)
    }
    pub fn state(&self, profile: &Profile) -> Value {
        json!(self.tabs.iter().filter(|t|profile.owns(&t.profile)).map(|t|json!({"id":format!("tab_{}",t.id),"title":t.title,"url":t.url,"loading":t.loading,"canGoBack":t.back,"canGoForward":t.forward})).collect::<Vec<_>>())
    }
}
pub fn bounded_log(entries: &mut VecDeque<Value>, entry: Value) {
    entries.push_back(entry);
    while entries.len() > 100 {
        entries.pop_front();
    }
}
pub fn origin(u: &Url) -> String {
    if u.scheme() == "file" || u.scheme() == "locust-page" {
        "file://workspace".into()
    } else {
        u.origin().ascii_serialization().to_lowercase()
    }
}
pub fn loopback(u: &Url) -> bool {
    u.host_str().is_some_and(|s| {
        s == "localhost" || s.ends_with(".localhost") || s == "127.0.0.1" || s == "[::1]"
    })
}
fn real_with_suffix(path: &Path) -> Result<PathBuf, String> {
    let mut base = path.to_path_buf();
    let mut rest = vec![];
    while !base.exists() {
        rest.push(base.file_name().ok_or("Not an address.")?.to_owned());
        if !base.pop() {
            return Err("Not an address.".into());
        }
    }
    let mut real = base.canonicalize().map_err(|e| e.to_string())?;
    for part in rest.iter().rev() {
        real.push(part);
    }
    // Normalize remaining .. after following the existing prefix.
    let mut out = PathBuf::new();
    for c in real.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    Ok(out)
}
/// Whether a page's own navigation goes through the address rules (Electron: will-navigate / will-redirect, main
/// frame only, about:blank let through). A preview tab checks every navigation: it may only show its own pages.
pub fn navigation_is_checked(main_frame: bool, url: &str, preview: bool) -> bool {
    preview || (main_frame && url != "about:blank")
}

pub fn resolve(raw: &str, workspace: Option<&Path>) -> Result<Url, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Enter a web address or a file inside this conversation folder. The current page stayed open.".into());
    }
    let lower = raw.to_lowercase();
    let local = regex::Regex::new(r"(?i)^(localhost|127\.0\.0\.1|\[::1\])(?::\d+)?(/|$)")
        .map_err(|e| e.to_string())?
        .is_match(raw);
    let hostport = regex::Regex::new(
        r"(?i)^(localhost|127\.0\.0\.1|\[::1\]|[a-z0-9-]+(?:\.[a-z0-9-]+)+):\d+(/|$|[?#])",
    )
    .map_err(|e| e.to_string())?
    .is_match(raw);
    let relative = workspace.is_some()
        && (raw.starts_with("./")
            || raw.starts_with("../")
            || lower
                .split(['?', '#'])
                .next()
                .is_some_and(|p| p.ends_with(".html") || p.ends_with(".htm")));
    let scheme = regex::Regex::new(r"^[a-zA-Z][a-zA-Z0-9+.-]*:")
        .map_err(|e| e.to_string())?
        .is_match(raw)
        && !hostport;
    let candidate = if !scheme && relative {
        let cut = raw.find(['?', '#']).unwrap_or(raw.len());
        let mut u = Url::from_file_path(
            workspace
                .ok_or("No conversation folder.")?
                .join(&raw[..cut]),
        )
        .map_err(|_| "Not an address.")?;
        let tail = &raw[cut..];
        if let Some((_, hash)) = tail.split_once('#') {
            u.set_fragment(Some(hash));
        }
        if let Some(query) = tail.strip_prefix('?') {
            u.set_query(Some(query.split('#').next().unwrap_or_default()));
        }
        u.to_string()
    } else if !scheme {
        format!("{}://{raw}", if local { "http" } else { "https" })
    } else {
        raw.into()
    };
    let mut u = Url::parse(&candidate)
        .map_err(|_| format!("\"{raw}\" is not a valid address. The current page stayed open."))?;
    match u.scheme() {
        "http"|"https"=>Ok(u),
        "file"=>{
            let workspace=workspace.ok_or("Local file URLs require an open conversation folder. The current page stayed open.")?;
            let path=real_with_suffix(&u.to_file_path().map_err(|_|"Not an address.")?)?;
            if !path.starts_with(real_with_suffix(workspace)?) {return Err("That file is outside this conversation folder. The current page stayed open.".into());}
            let query=u.query().map(str::to_owned);let hash=u.fragment().map(str::to_owned);
            u=Url::from_file_path(path).map_err(|_|"Not an address.")?;u.set_query(query.as_deref());u.set_fragment(hash.as_deref());Ok(u)
        },
        blocked if ["javascript","data","vbscript","blob","chrome","chrome-extension","devtools","about","view-source","ws","wss","ftp","mailto","tel","locust-page"].contains(&blocked)=>Err(format!("The {blocked}: scheme is not allowed in the Locust browser. The current page stayed open.")),
        _=>Err("Only http:, https:, and workspace file: addresses can open in the Locust browser. The current page stayed open.".into())
    }
}

#[path = "browser_actions.rs"]
pub mod actions;

pub fn policy_refusal(value: &Value, path: &Path) -> Option<String> {
    let invalid = !value.is_object()
        || (!value["version"].is_null() && value["version"] != 1)
        || [
            "otherApps",
            "phone",
            "chatApps",
            "cloud",
            "terminal",
            "browser",
            "accounts",
        ]
        .iter()
        .any(|k| !value[k].is_null() && !value[k].is_boolean());
    if invalid {
        return Some(format!(
            "Organization policy could not be read ({}). Nothing was started.",
            path.display()
        ));
    }
    if value["browser"] == false {
        Some(format!(
            "The browser panel is off. Set by your organization (browser in {}). Nothing was started.",
            path.display()
        ))
    } else {
        None
    }
}
