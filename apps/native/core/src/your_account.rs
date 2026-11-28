//! SETTINGS › YOUR ACCOUNT (Electron `main/your-account.ts`, accounts slice 3/4/4b).
//!
//! Sealed profile copy tied to the person's GitHub sign-in: creates and OS-locks the 32-byte
//! profile key, shows the Crockford base32 recovery code once, pushes/pulls through
//! `profile_sync`, supports recovery-code restore and X25519 computer-to-computer approval.

use crate::own_models::SecretBox;
use crate::profile_sync::{
    AskKeys, HttpSyncRemote, LastSync, ListedCopy, PROFILE_KEY_BYTES, PushOptions,
    SYNC_QUIET_MS, SyncAccountRemote, SyncPulled, SyncPushed, SyncRefused, SyncSchedule,
    SyncUsage, UreqSyncHttpFetch, ask_id, forget_sync, grant_id_of, last_sync, manifest_id,
    match_of, new_ask_keys, new_profile_key, open_grant, profile_key_from_code, pull_profile,
    push_profile, recovery_code, seal_grant, sync_keys,
};
use crate::relay_crypto::{from_base64url, to_base64url};
use chrono::{DateTime, TimeZone, Utc};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const ACCOUNT_SYNC_FILE: &str = "account-sync.json";
pub const ACCOUNT_LOOK_MS: u64 = 10 * 60_000;
pub const ACCOUNT_COPIES_FOLDER: &str = "account-copies";
pub const COPIES_FRESH_MS: u64 = 10_000;
pub const ASK_EVERY_MS: u64 = 5_000;
pub const ASK_FOR_MS: u64 = 10 * 60_000;
const ASK_NAME_MAX: usize = 60;

#[derive(Clone, PartialEq, Eq)]
pub struct AccountLink {
    pub relay: String,
    pub room: String,
    pub token: String,
    pub user: u64,
    pub login: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedInAccount {
    pub login: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AccountSyncState {
    #[default]
    Off,
    On,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElsewhereCopies {
    pub computers: usize,
    pub at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAsk {
    pub room: String,
    pub name: String,
    pub match_code: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountAsking {
    pub match_code: String,
    pub since: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct YourAccountStatus {
    pub signed_in: Option<SignedInAccount>,
    pub sync: AccountSyncState,
    pub has_key: bool,
    pub syncing: bool,
    pub last_synced: Option<LastSync>,
    pub storage: Option<SyncUsage>,
    pub elsewhere: Option<ElsewhereCopies>,
    pub asks: Vec<AccountAsk>,
    pub asking: Option<AccountAsking>,
    pub message: Option<String>,
    pub good: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnOnResult {
    pub status: YourAccountStatus,
    pub code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BringHereResult {
    Ok { computer: String },
    Err { reason: String },
}

pub struct YourAccountOptions {
    pub profile: PathBuf,
    pub link: Arc<dyn Fn() -> Option<AccountLink> + Send + Sync>,
    pub secrets: Arc<dyn SecretBox>,
    pub computer: Arc<dyn Fn() -> String + Send + Sync>,
    pub app_version: String,
    pub busy: Arc<dyn Fn() -> bool + Send + Sync>,
    pub remote: Option<Arc<dyn Fn(&AccountLink) -> Arc<dyn SyncAccountRemote> + Send + Sync>>,
    pub now: Option<Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>>,
    pub quiet_ms: Option<u64>,
    pub copies_fresh_ms: Option<u64>,
    pub ask_for_ms: Option<u64>,
    pub changed: Option<Arc<dyn Fn(YourAccountStatus) + Send + Sync>>,
    pub restore: Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>,
}

#[derive(Clone, Debug)]
struct Saved {
    on: bool,
    account: u64,
    key: String,
}

impl Saved {
    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        if obj.get("v")?.as_u64()? != 1 {
            return None;
        }
        let on = obj.get("on")?.as_bool()?;
        let account = obj.get("account")?.as_u64()?;
        if account == 0 {
            return None;
        }
        let key = obj.get("key")?.as_str()?;
        if key.is_empty() {
            return None;
        }
        Some(Self {
            on,
            account,
            key: key.to_owned(),
        })
    }

    fn to_json(&self) -> Value {
        json!({
            "v": 1,
            "on": self.on,
            "account": self.account,
            "key": self.key,
        })
    }
}

#[derive(Clone, Debug)]
struct InternalAsk {
    room: String,
    name: String,
    match_code: String,
}

#[derive(Clone, Debug)]
struct ElsewhereCache {
    account: u64,
    room: String,
    copies: Vec<ListedCopy>,
    asks: Vec<InternalAsk>,
    looked_at_ms: u64,
}

struct ActiveAsk {
    keys: AskKeys,
    match_code: String,
    since_ms: u64,
    room: String,
    account: u64,
}

struct Inner {
    message: Option<(String, bool)>,
    storage: Option<SyncUsage>,
    syncing: bool,
    elsewhere: Option<ElsewhereCache>,
    ignored: HashSet<String>,
    granted_to: HashSet<String>,
    asking: Option<ActiveAsk>,
}

fn is_ask_key(s: &str) -> bool {
    s.len() == 43
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn sanitize_ask_name(raw: &str) -> String {
    let filtered: String = raw
        .chars()
        .filter(|c| {
            c.is_alphanumeric()
                || matches!(c, ' ' | '\'' | '.' | ',' | '(' | ')' | '_' | '-')
        })
        .collect();
    let collapsed = filtered.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = collapsed.chars().take(ASK_NAME_MAX).collect();
    if truncated.is_empty() {
        "A computer".to_owned()
    } else {
        truncated
    }
}

fn ask_of(bytes: Option<&[u8]>) -> Option<(String, String)> {
    let bytes = bytes?;
    if bytes.len() > 2048 {
        return None;
    }
    let val: Value = serde_json::from_slice(bytes).ok()?;
    let obj = val.as_object()?;
    if obj.get("v")?.as_u64()? != 1 {
        return None;
    }
    let key = obj.get("key")?.as_str()?;
    if !is_ask_key(key) {
        return None;
    }
    let name = obj.get("name")?.as_str()?;
    Some((sanitize_ask_name(name), key.to_owned()))
}

fn iso_from_ms(ms: u64) -> String {
    Utc.timestamp_millis_opt(ms as i64)
        .single()
        .unwrap_or_else(Utc::now)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn refusal_text(err: &SyncRefused) -> String {
    if err.message.is_empty() {
        "Locust\u{2019}s relay could not be reached. Your Locust is safe here; try again in a minute.".into()
    } else {
        err.message.clone()
    }
}

pub struct YourAccount {
    profile: PathBuf,
    link: Arc<dyn Fn() -> Option<AccountLink> + Send + Sync>,
    secrets: Arc<dyn SecretBox>,
    computer: Arc<dyn Fn() -> String + Send + Sync>,
    app_version: String,
    busy: Arc<dyn Fn() -> bool + Send + Sync>,
    remote: Arc<dyn Fn(&AccountLink) -> Arc<dyn SyncAccountRemote> + Send + Sync>,
    now: Arc<dyn Fn() -> DateTime<Utc> + Send + Sync>,
    quiet_ms: u64,
    copies_fresh_ms: u64,
    ask_for_ms: u64,
    changed: Option<Arc<dyn Fn(YourAccountStatus) + Send + Sync>>,
    restore: Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>,
    schedule: SyncSchedule,
    last_look: Mutex<Option<u64>>,
    inner: Mutex<Inner>,
}

impl YourAccount {
    pub fn new(options: YourAccountOptions) -> Self {
        let quiet_ms = options.quiet_ms.unwrap_or(SYNC_QUIET_MS);
        let remote = options.remote.unwrap_or_else(|| {
            Arc::new(|link: &AccountLink| {
                Arc::new(HttpSyncRemote::new(
                    &link.relay,
                    &link.room,
                    &link.token,
                    Arc::new(UreqSyncHttpFetch),
                )) as Arc<dyn SyncAccountRemote>
            })
        });
        let now = options.now.unwrap_or_else(|| Arc::new(Utc::now));
        Self {
            profile: options.profile,
            link: options.link,
            secrets: options.secrets,
            computer: options.computer,
            app_version: options.app_version,
            busy: options.busy,
            remote,
            now,
            quiet_ms,
            copies_fresh_ms: options.copies_fresh_ms.unwrap_or(COPIES_FRESH_MS),
            ask_for_ms: options.ask_for_ms.unwrap_or(ASK_FOR_MS),
            changed: options.changed,
            restore: options.restore,
            schedule: SyncSchedule::new(quiet_ms),
            last_look: Mutex::new(None),
            inner: Mutex::new(Inner {
                message: None,
                storage: None,
                syncing: false,
                elsewhere: None,
                ignored: HashSet::new(),
                granted_to: HashSet::new(),
                asking: None,
            }),
        }
    }

    fn now_ms(&self) -> u64 {
        (self.now)().timestamp_millis().max(0) as u64
    }

    fn read_saved(&self) -> Option<Saved> {
        let text = fs::read_to_string(self.profile.join(ACCOUNT_SYNC_FILE)).ok()?;
        let val: Value = serde_json::from_str(&text).ok()?;
        Saved::from_json(&val)
    }

    fn write_saved(&self, saved: &Saved) -> Result<(), String> {
        fs::write(
            self.profile.join(ACCOUNT_SYNC_FILE),
            format!("{}\n", saved.to_json()),
        )
        .map_err(|e| e.to_string())
    }

    fn key_of(&self, saved: &Saved) -> Option<[u8; PROFILE_KEY_BYTES]> {
        let plain_b64 = self.secrets.open(&saved.key).ok()?;
        let bytes = from_base64url(&plain_b64).ok()?;
        if bytes.len() != PROFILE_KEY_BYTES {
            return None;
        }
        let mut out = [0u8; PROFILE_KEY_BYTES];
        out.copy_from_slice(&bytes);
        Some(out)
    }

    fn seal_key(&self, key: &[u8; PROFILE_KEY_BYTES]) -> Result<String, String> {
        self.secrets.seal(&to_base64url(key))
    }

    fn build_status(&self, link: Option<&AccountLink>) -> YourAccountStatus {
        let saved = self.read_saved();
        let ours = saved
            .as_ref()
            .zip(link)
            .is_some_and(|(s, l)| s.account == l.user);
        let last_synced = if ours {
            link.and_then(|l| last_sync(&self.profile, l.user, &l.room))
        } else {
            None
        };
        let st = self.inner.lock().unwrap();
        let listed = link.and_then(|l| {
            st.elsewhere
                .as_ref()
                .filter(|e| e.account == l.user && e.room == l.room)
        });
        let mid = manifest_id();
        let others: Vec<&ListedCopy> = listed
            .map(|e| {
                e.copies
                    .iter()
                    .filter(|c| c.ids.iter().any(|id| id == &mid))
                    .collect()
            })
            .unwrap_or_default();
        let elsewhere = if others.is_empty() {
            None
        } else {
            let max_at = others.iter().map(|c| c.at).max().unwrap_or(0);
            Some(ElsewhereCopies {
                computers: others.len(),
                at: iso_from_ms(max_at),
            })
        };
        let asks: Vec<AccountAsk> = if ours {
            listed
                .map(|e| {
                    e.asks
                        .iter()
                        .filter(|a| !st.ignored.contains(&a.room))
                        .map(|a| AccountAsk {
                            room: a.room.clone(),
                            name: a.name.clone(),
                            match_code: a.match_code.clone(),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let asking = st.asking.as_ref().and_then(|a| {
            link.filter(|l| a.room == l.room && a.account == l.user).map(|_| AccountAsking {
                match_code: a.match_code.clone(),
                since: iso_from_ms(a.since_ms),
            })
        });
        let sync = if ours && saved.as_ref().is_some_and(|s| s.on) {
            AccountSyncState::On
        } else {
            AccountSyncState::Off
        };
        YourAccountStatus {
            signed_in: link.map(|l| SignedInAccount {
                login: l.login.clone(),
            }),
            sync,
            has_key: ours,
            syncing: st.syncing,
            last_synced,
            storage: if ours { st.storage } else { None },
            elsewhere,
            asks,
            asking,
            message: st.message.as_ref().map(|(m, _)| m.clone()),
            good: st.message.as_ref().map(|(_, g)| *g),
        }
    }

    fn tell(&self, status: YourAccountStatus) {
        if let Some(cb) = &self.changed {
            cb(status);
        }
    }

    fn look_elsewhere(&self, link: &AccountLink) {
        let now_ms = self.now_ms();
        {
            let st = self.inner.lock().unwrap();
            let fresh = st.elsewhere.as_ref().is_some_and(|e| {
                e.account == link.user
                    && e.room == link.room
                    && now_ms.saturating_sub(e.looked_at_ms) < self.copies_fresh_ms
            });
            if fresh {
                return;
            }
        }
        let remote = (self.remote)(link);
        let Ok(raw_listed) = remote.copies() else {
            return;
        };
        let listed: Vec<ListedCopy> = raw_listed.into_iter().filter(|c| c.bytes > 0).collect();
        let others: Vec<ListedCopy> = listed
            .iter()
            .filter(|c| c.room != link.room)
            .cloned()
            .collect();
        let mine = listed.iter().find(|c| c.room == link.room).cloned();
        let saved = self.read_saved();
        let (ignored, granted_snapshot) = {
            let st = self.inner.lock().unwrap();
            (
                st.ignored.clone(),
                st.granted_to.iter().cloned().collect::<Vec<_>>(),
            )
        };
        let aid = ask_id();
        let mut asks = Vec::new();
        if saved.as_ref().is_some_and(|s| s.account == link.user) {
            for copy in &others {
                let has_ask = copy.ids.iter().any(|id| id == &aid);
                let already_granted = mine
                    .as_ref()
                    .is_some_and(|m| m.ids.iter().any(|id| id == &grant_id_of(&copy.room)));
                if !has_ask || ignored.contains(&copy.room) || already_granted {
                    continue;
                }
                if let Ok(bytes) = remote.get(&copy.room, &aid)
                    && let Some((name, key)) = ask_of(bytes.as_deref())
                {
                    let match_code = match_of(&key);
                    asks.push(InternalAsk {
                        room: copy.room.clone(),
                        name,
                        match_code,
                    });
                }
            }
        }
        // A grant whose ask is gone was taken, or given up on: remove it from the relay.
        let mut removed_grants = Vec::new();
        for room in granted_snapshot {
            let still_asking = others
                .iter()
                .any(|c| c.room == room && c.ids.iter().any(|id| id == &aid));
            if !still_asking && remote.remove(&grant_id_of(&room)).is_ok() {
                removed_grants.push(room);
            }
        }
        let changed = {
            let mut st = self.inner.lock().unwrap();
            for room in removed_grants {
                st.granted_to.remove(&room);
            }
            let prev_rooms: Vec<(&str, usize)> = st
                .elsewhere
                .as_ref()
                .map(|e| {
                    e.copies
                        .iter()
                        .map(|c| (c.room.as_str(), c.ids.len()))
                        .collect()
                })
                .unwrap_or_default();
            let next_rooms: Vec<(&str, usize)> =
                others.iter().map(|c| (c.room.as_str(), c.ids.len())).collect();
            let prev_asks: Vec<(&str, &str)> = st
                .elsewhere
                .as_ref()
                .map(|e| {
                    e.asks
                        .iter()
                        .map(|a| (a.room.as_str(), a.match_code.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            let next_asks: Vec<(&str, &str)> = asks
                .iter()
                .map(|a| (a.room.as_str(), a.match_code.as_str()))
                .collect();
            let diff = prev_rooms != next_rooms || prev_asks != next_asks;
            st.elsewhere = Some(ElsewhereCache {
                account: link.user,
                room: link.room.clone(),
                copies: others,
                asks,
                looked_at_ms: now_ms,
            });
            diff
        };
        if changed {
            self.tell(self.build_status(Some(link)));
        }
    }

    pub fn status(&self) -> YourAccountStatus {
        let link = (self.link)();
        if let Some(ref l) = link {
            self.look_elsewhere(l);
        }
        self.build_status(link.as_ref())
    }

    /// Whether a locked profile key is kept here, on or off, read without opening it.
    pub fn keeps_key(&self) -> bool {
        self.read_saved().is_some()
    }

    pub fn touched(&self) {
        self.schedule.touched(self.now_ms());
    }

    /// Forget the old room's transient status before a local host changes rooms or accounts.
    pub fn link_changed(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.message = None;
        inner.storage = None;
        inner.elsewhere = None;
        inner.ignored.clear();
        inner.granted_to.clear();
        inner.asking = None;
        *self.last_look.lock().unwrap() = None;
    }

    /// One tick on the local account worker. A new process first arms the saved opt-in.
    pub fn maintain(&self) -> YourAccountStatus {
        let link = (self.link)();
        if let Some(link) = link.as_ref() {
            let now = self.now_ms();
            if self.read_saved().is_some_and(|saved| saved.on && saved.account == link.user) {
                let mut last = self.last_look.lock().unwrap();
                if last.is_none_or(|at| now.saturating_sub(at) >= ACCOUNT_LOOK_MS) {
                    self.schedule.touched(now);
                    *last = Some(now);
                }
            } else {
                *self.last_look.lock().unwrap() = None;
            }
            if self.schedule.tick(now, (self.busy)()) {
                self.push_once();
                self.schedule.finished(self.now_ms());
            }
        }
        if self.inner.lock().unwrap().asking.is_some() {
            self.check_ask();
        }
        self.status()
    }

    fn push_once(&self) -> bool {
        let Some(link) = (self.link)() else {
            return false;
        };
        let Some(saved) = self.read_saved() else {
            return false;
        };
        if !saved.on {
            return false;
        }
        if saved.account != link.user {
            {
                let mut st = self.inner.lock().unwrap();
                st.message = Some((
                    format!(
                        "Sync was turned on as another GitHub account. Turn it off and on again as {}.",
                        link.login
                    ),
                    false,
                ));
            }
            self.tell(self.build_status(Some(&link)));
            return false;
        }
        let Some(key) = self.key_of(&saved) else {
            {
                let mut st = self.inner.lock().unwrap();
                st.message = Some((
                    "This computer could not unlock the key your copy is sealed with. Turn sync off and on again.".into(),
                    false,
                ));
            }
            self.tell(self.build_status(Some(&link)));
            return false;
        };
        {
            let mut st = self.inner.lock().unwrap();
            st.syncing = true;
        }
        self.tell(self.build_status(Some(&link)));

        let remote = (self.remote)(&link);
        let keys = match sync_keys(&key, link.user) {
            Ok(k) => k,
            Err(reason) => {
                let mut st = self.inner.lock().unwrap();
                st.syncing = false;
                st.message = Some((reason, false));
                drop(st);
                self.tell(self.build_status(Some(&link)));
                return false;
            }
        };
        let computer = (self.computer)();
        let pushed = push_profile(
            &self.profile,
            &PushOptions {
                keys: &keys,
                remote: remote.as_ref(),
                room: &link.room,
                computer: &computer,
                app_version: &self.app_version,
                now: (self.now)(),
            },
        );
        let ok = match pushed {
            SyncPushed::Ok { .. } => {
                let usage = remote.usage().ok();
                let mut st = self.inner.lock().unwrap();
                st.message = None;
                if let Some(u) = usage {
                    st.storage = Some(u);
                }
                true
            }
            SyncPushed::Err { reason } => {
                let mut st = self.inner.lock().unwrap();
                st.message = Some((reason, false));
                false
            }
        };
        {
            let mut st = self.inner.lock().unwrap();
            st.syncing = false;
        }
        self.tell(self.build_status(Some(&link)));
        ok
    }

    pub fn turn_on(&self) -> TurnOnResult {
        let Some(link) = (self.link)() else {
            self.inner.lock().unwrap().message = Some((
                "Sign in with GitHub first, under Use from anywhere.".into(),
                false,
            ));
            return TurnOnResult {
                status: self.build_status(None),
                code: None,
            };
        };
        if let Some(saved) = self.read_saved()
            && saved.account == link.user
            && self.key_of(&saved).is_some()
        {
            let updated = Saved { on: true, ..saved };
            let _ = self.write_saved(&updated);
            self.inner.lock().unwrap().message = None;
            self.schedule.touched(self.now_ms());
            if self.quiet_ms == 0 && !(self.busy)() {
                self.push_once();
            }
            return TurnOnResult {
                status: self.status(),
                code: None,
            };
        }
        if !self.secrets.available() {
            self.inner.lock().unwrap().message = Some((
                "This computer cannot lock a key, so sync stays off. Your Locust is safe here."
                    .into(),
                false,
            ));
            return TurnOnResult {
                status: self.build_status(Some(&link)),
                code: None,
            };
        }
        let key = new_profile_key();
        let Ok(sealed) = self.seal_key(&key) else {
            self.inner.lock().unwrap().message = Some((
                "This computer cannot lock a key, so sync stays off. Your Locust is safe here."
                    .into(),
                false,
            ));
            return TurnOnResult {
                status: self.build_status(Some(&link)),
                code: None,
            };
        };
        let saved = Saved {
            on: true,
            account: link.user,
            key: sealed,
        };
        if let Err(e) = self.write_saved(&saved) {
            self.inner.lock().unwrap().message = Some((e, false));
            return TurnOnResult {
                status: self.build_status(Some(&link)),
                code: None,
            };
        }
        {
            let mut st = self.inner.lock().unwrap();
            st.storage = None;
            st.message = None;
        }
        self.schedule.touched(self.now_ms());
        if self.quiet_ms == 0 && !(self.busy)() {
            self.push_once();
        }
        let code = recovery_code(&key).ok();
        TurnOnResult {
            status: self.status(),
            code,
        }
    }

    pub fn turn_off(&self) -> YourAccountStatus {
        if let Some(saved) = self.read_saved() {
            let _ = self.write_saved(&Saved { on: false, ..saved });
        }
        self.inner.lock().unwrap().message = None;
        self.status()
    }

    pub fn sync_now(&self) -> YourAccountStatus {
        let pushed = self.push_once();
        if pushed {
            self.inner.lock().unwrap().message = Some(("Synced.".into(), true));
        }
        self.status()
    }

    pub fn delete_copy(&self) -> YourAccountStatus {
        let Some(link) = (self.link)() else {
            self.inner.lock().unwrap().message = Some((
                "Sign in with GitHub first, under Use from anywhere.".into(),
                false,
            ));
            return self.build_status(None);
        };
        if let Err(err) = (self.remote)(&link).remove_all() {
            self.inner.lock().unwrap().message = Some((refusal_text(&err), false));
            return self.build_status(Some(&link));
        }
        if let Some(saved) = self.read_saved() {
            let _ = self.write_saved(&Saved { on: false, ..saved });
        }
        forget_sync(&self.profile);
        {
            let mut st = self.inner.lock().unwrap();
            st.storage = None;
            st.message = Some((
                "Your copy on Locust\u{2019}s relay is deleted. Everything on this computer stays."
                    .into(),
                true,
            ));
        }
        self.status()
    }

    fn bring_with_key(&self, link: &AccountLink, key: &[u8; PROFILE_KEY_BYTES]) -> BringHereResult {
        let remote = (self.remote)(link);
        let mid = manifest_id();
        let mut copies: Vec<ListedCopy> = match remote.copies() {
            Ok(list) => list
                .into_iter()
                .filter(|c| c.room != link.room && c.ids.iter().any(|id| id == &mid))
                .collect(),
            Err(err) => {
                return BringHereResult::Err {
                    reason: refusal_text(&err),
                };
            }
        };
        copies.sort_by(|a, b| b.at.cmp(&a.at));
        let Some(newest) = copies.first() else {
            return BringHereResult::Err {
                reason: format!(
                    "No other computer keeps a copy for {} yet. Turn on Keep a copy there first.",
                    link.login
                ),
            };
        };
        let keys = match sync_keys(key, link.user) {
            Ok(k) => k,
            Err(reason) => return BringHereResult::Err { reason },
        };
        let stamp = (self.now)()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
            .replace([':', '.'], "-");
        let folder = self.profile.join(ACCOUNT_COPIES_FOLDER).join(stamp);
        let pulled = pull_profile(&keys, remote.as_ref(), &newest.room, &folder);
        let computer = match pulled {
            SyncPulled::Ok { computer, .. } => computer,
            SyncPulled::Err { reason } => return BringHereResult::Err { reason },
        };
        let sealed = match self.seal_key(key) {
            Ok(s) => s,
            Err(reason) => return BringHereResult::Err { reason },
        };
        if let Err(reason) = self.write_saved(&Saved {
            on: false,
            account: link.user,
            key: sealed,
        }) {
            return BringHereResult::Err { reason };
        }
        if let Some(restore) = &self.restore
            && let Err(reason) = restore(&folder)
        {
            return BringHereResult::Err { reason };
        }
        BringHereResult::Ok { computer }
    }

    pub fn bring_here(&self, code: &str) -> BringHereResult {
        let Some(link) = (self.link)() else {
            return BringHereResult::Err {
                reason: "Sign in with GitHub first, under Use from anywhere.".into(),
            };
        };
        let Some(key) = profile_key_from_code(code) else {
            return BringHereResult::Err {
                reason: "That code does not read right. It is 55 letters and digits in groups of five; check each group.".into(),
            };
        };
        if !self.secrets.available() {
            return BringHereResult::Err {
                reason: "This computer cannot lock a key, so your copy cannot be brought here safely.".into(),
            };
        }
        self.bring_with_key(&link, &key)
    }

    pub fn ask(&self) -> YourAccountStatus {
        let Some(link) = (self.link)() else {
            self.inner.lock().unwrap().message = Some((
                "Sign in with GitHub first, under Use from anywhere.".into(),
                false,
            ));
            return self.build_status(None);
        };
        if !self.secrets.available() {
            self.inner.lock().unwrap().message = Some((
                "This computer cannot lock a key, so your copy cannot be brought here safely."
                    .into(),
                false,
            ));
            return self.build_status(Some(&link));
        }
        let remote = (self.remote)(&link);
        let mid = manifest_id();
        let copies = match remote.copies() {
            Ok(list) => list,
            Err(err) => {
                self.inner.lock().unwrap().message = Some((refusal_text(&err), false));
                return self.build_status(Some(&link));
            }
        };
        let has_other = copies
            .iter()
            .any(|c| c.room != link.room && c.ids.iter().any(|id| id == &mid));
        if !has_other {
            self.inner.lock().unwrap().message = Some((
                format!(
                    "No other computer keeps a copy for {} yet. Turn on Keep a copy there first.",
                    link.login
                ),
                false,
            ));
            return self.build_status(Some(&link));
        }
        let keys = new_ask_keys();
        let name: String = (self.computer)().chars().take(ASK_NAME_MAX).collect();
        let now_ms = self.now_ms();
        let body = serde_json::to_vec(&json!({
            "v": 1,
            "name": name,
            "key": keys.public_key,
            "at": now_ms,
        }))
        .unwrap_or_default();
        if let Err(err) = remote.put(&ask_id(), &body) {
            self.inner.lock().unwrap().message = Some((refusal_text(&err), false));
            return self.build_status(Some(&link));
        }
        let match_code = match_of(&keys.public_key);
        {
            let mut st = self.inner.lock().unwrap();
            st.asking = Some(ActiveAsk {
                keys,
                match_code,
                since_ms: now_ms,
                room: link.room.clone(),
                account: link.user,
            });
            st.message = None;
        }
        self.status()
    }

    pub fn check_ask(&self) -> YourAccountStatus {
        let Some(link) = (self.link)() else {
            self.inner.lock().unwrap().asking = None;
            return self.build_status(None);
        };
        let (ask_keys, since_ms, same_session) = {
            let st = self.inner.lock().unwrap();
            let Some(ref active) = st.asking else {
                drop(st);
                return self.build_status(Some(&link));
            };
            (
                active.keys.clone(),
                active.since_ms,
                active.room == link.room && active.account == link.user,
            )
        };
        if !same_session {
            self.inner.lock().unwrap().asking = None;
            let s = self.build_status(Some(&link));
            self.tell(s.clone());
            return s;
        }
        let remote = (self.remote)(&link);
        if self.now_ms().saturating_sub(since_ms) > self.ask_for_ms {
            self.inner.lock().unwrap().asking = None;
            let _ = remote.remove(&ask_id());
            self.inner.lock().unwrap().message = Some((
                "No computer approved this one in ten minutes. Ask again, or use your recovery code."
                    .into(),
                false,
            ));
            let s = self.build_status(Some(&link));
            self.tell(s.clone());
            return s;
        }
        let gid = grant_id_of(&link.room);
        let Ok(copies) = remote.copies() else {
            return self.build_status(Some(&link));
        };
        for copy in copies {
            if copy.room == link.room || !copy.ids.iter().any(|id| id == &gid) {
                continue;
            }
            let Ok(Some(grant)) = remote.get(&copy.room, &gid) else {
                continue;
            };
            let Some(opened) = open_grant(&grant, &ask_keys, &link.room) else {
                continue;
            };
            self.inner.lock().unwrap().asking = None;
            let _ = remote.remove(&ask_id());
            self.inner.lock().unwrap().message =
                Some(("Approved. Bringing your Locust here\u{2026}".into(), true));
            self.tell(self.build_status(Some(&link)));
            let brought = self.bring_with_key(&link, &opened.profile_key);
            self.inner.lock().unwrap().message = match brought {
                BringHereResult::Ok { computer } => Some((
                    format!("Your Locust from {computer} is ready. Restart Locust to bring it in."),
                    true,
                )),
                BringHereResult::Err { reason } => Some((reason, false)),
            };
            let s = self.build_status(Some(&link));
            self.tell(s.clone());
            return s;
        }
        self.build_status(Some(&link))
    }

    pub fn stop_asking(&self) -> YourAccountStatus {
        let link = (self.link)();
        self.inner.lock().unwrap().asking = None;
        if let Some(ref l) = link {
            let _ = (self.remote)(l).remove(&ask_id());
        }
        self.inner.lock().unwrap().message = None;
        self.status()
    }

    pub fn approve(&self, room: &str, match_code: &str) -> YourAccountStatus {
        let link = (self.link)();
        let saved = self.read_saved();
        let key = link
            .as_ref()
            .zip(saved.as_ref())
            .filter(|(l, s)| s.account == l.user)
            .and_then(|(_, s)| self.key_of(s));
        let (Some(link), Some(key)) = (link, key) else {
            self.inner.lock().unwrap().message = Some((
                "This computer does not hold your Locust\u{2019}s key. Approve from the computer that keeps your copy."
                    .into(),
                false,
            ));
            return self.status();
        };
        let shown_match = self.inner.lock().unwrap().elsewhere.as_ref().and_then(|e| {
            e.asks
                .iter()
                .find(|a| a.room == room)
                .map(|a| a.match_code.clone())
        });
        let remote = (self.remote)(&link);
        let bytes = match remote.get(room, &ask_id()) {
            Ok(b) => b,
            Err(err) => {
                self.inner.lock().unwrap().message = Some((refusal_text(&err), false));
                return self.build_status(Some(&link));
            }
        };
        let Some((name, ask_key)) = ask_of(bytes.as_deref()) else {
            self.inner.lock().unwrap().message =
                Some(("That computer is not asking any more.".into(), false));
            return self.build_status(Some(&link));
        };
        if shown_match.as_deref() != Some(match_code) || match_of(&ask_key) != match_code {
            self.inner.lock().unwrap().message = Some((
                "The code changed. Ask again on the new computer, and approve only if both screens show the same code."
                    .into(),
                false,
            ));
            return self.build_status(Some(&link));
        }
        let sealed = match seal_grant(&key, &ask_key, &link.room, room) {
            Ok(b) => b,
            Err(reason) => {
                self.inner.lock().unwrap().message = Some((reason, false));
                return self.build_status(Some(&link));
            }
        };
        if let Err(err) = remote.put(&grant_id_of(room), &sealed) {
            self.inner.lock().unwrap().message = Some((refusal_text(&err), false));
            return self.build_status(Some(&link));
        }
        {
            let mut st = self.inner.lock().unwrap();
            st.granted_to.insert(room.to_owned());
            if let Some(ref mut e) = st.elsewhere {
                e.asks.retain(|a| a.room != room);
            }
            st.message = Some((
                format!("Approved. {name} can restart to bring your Locust in."),
                true,
            ));
        }
        self.build_status(Some(&link))
    }

    pub fn ignore(&self, room: &str) -> YourAccountStatus {
        {
            let mut st = self.inner.lock().unwrap();
            st.ignored.insert(room.to_owned());
            st.message = Some((
                "Not approved. That computer gets nothing from this one.".into(),
                true,
            ));
        }
        self.status()
    }

    pub fn stop(&self) {
        self.schedule.stop();
        self.inner.lock().unwrap().asking = None;
    }
}
