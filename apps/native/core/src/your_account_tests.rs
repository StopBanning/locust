use crate::own_models::SecretBox;
use crate::profile_backup::{LEFT_OUT, apply_pending_restore, read_backup, request_restore};
use crate::profile_sync::{
    HttpSyncRemote, ListedCopy, PROFILE_SYNC_FILE, SyncAccountRemote, SyncHttpFetch,
    SyncHttpReply, SyncRefused, SyncRemote, SyncUsage, ask_id, grant_id_of, new_ask_keys,
    new_profile_key, profile_key_from_code, recovery_code,
};
use crate::relay_crypto::{random_text, to_base64url};
use crate::your_account::{
    ACCOUNT_SYNC_FILE, AccountLink, AccountSyncState, BringHereResult, YourAccount,
    YourAccountOptions,
};
use chrono::{Local, TimeZone, Utc};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-acct-{tag}-{}", random_text(10)));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct FakeSecretBox {
    available: bool,
}

impl FakeSecretBox {
    fn new(available: bool) -> Arc<Self> {
        Arc::new(Self { available })
    }
}

impl SecretBox for FakeSecretBox {
    fn available(&self) -> bool {
        self.available
    }

    fn seal(&self, text: &str) -> Result<String, String> {
        if !self.available {
            return Err("unavailable".into());
        }
        let hex: String = text.bytes().map(|b| format!("{b:02x}")).collect();
        Ok(format!("locked:{hex}"))
    }

    fn open(&self, sealed: &str) -> Result<String, String> {
        let hex = sealed.strip_prefix("locked:").ok_or("bad sealed format")?;
        let bytes: Result<Vec<u8>, _> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
            .collect();
        let bytes = bytes.map_err(|_| "bad hex".to_owned())?;
        String::from_utf8(bytes).map_err(|_| "bad utf8".to_owned())
    }
}

const RELAY_QUOTA: u64 = 1024 * 1024 * 1024;

struct FakeRelay {
    user: u64,
    full: bool,
    held: Mutex<HashMap<String, (Vec<u8>, u64)>>,
    gets: Mutex<Vec<String>>,
    removed_all: AtomicUsize,
    clock_ms: AtomicI64,
}

impl FakeRelay {
    fn new(user: u64, full: bool) -> Arc<Self> {
        Arc::new(Self {
            user,
            full,
            held: Mutex::new(HashMap::new()),
            gets: Mutex::new(Vec::new()),
            removed_all: AtomicUsize::new(0),
            clock_ms: AtomicI64::new(1_761_933_600_000),
        })
    }

    fn tick(&self) -> u64 {
        self.clock_ms.fetch_add(10, Ordering::SeqCst) as u64
    }

    fn remote_factory(self: &Arc<Self>) -> Arc<dyn Fn(&AccountLink) -> Arc<dyn SyncAccountRemote> + Send + Sync> {
        let relay = Arc::clone(self);
        Arc::new(move |link: &AccountLink| {
            Arc::new(FakeRemote {
                relay: Arc::clone(&relay),
                room: link.room.clone(),
            }) as Arc<dyn SyncAccountRemote>
        })
    }

    fn remote_for(self: &Arc<Self>, room: &str) -> FakeRemote {
        FakeRemote {
            relay: Arc::clone(self),
            room: room.to_owned(),
        }
    }

    fn held_keys(&self) -> Vec<String> {
        self.held.lock().unwrap().keys().cloned().collect()
    }

    fn ids_at(&self, room: &str) -> Vec<String> {
        let needle = format!("/{room}/");
        self.held
            .lock()
            .unwrap()
            .keys()
            .filter(|k| k.contains(&needle))
            .filter_map(|k| k.rsplit('/').next().map(str::to_owned))
            .collect()
    }
}

struct FakeRemote {
    relay: Arc<FakeRelay>,
    room: String,
}

impl SyncRemote for FakeRemote {
    fn put(&self, id: &str, boxed: &[u8]) -> Result<(), SyncRefused> {
        if self.relay.full {
            return Err(SyncRefused {
                status: 507,
                message: "Your copy on Locust\u{2019}s relay is full (1 GB).".into(),
            });
        }
        let key = format!("u/{}/{}/{id}", self.relay.user, self.room);
        let at = self.relay.tick();
        self.relay
            .held
            .lock()
            .unwrap()
            .insert(key, (boxed.to_vec(), at));
        Ok(())
    }

    fn get(&self, from: &str, id: &str) -> Result<Option<Vec<u8>>, SyncRefused> {
        self.relay.gets.lock().unwrap().push(id.to_owned());
        let key = format!("u/{}/{from}/{id}", self.relay.user);
        Ok(self
            .relay
            .held
            .lock()
            .unwrap()
            .get(&key)
            .map(|(b, _)| b.clone()))
    }

    fn remove(&self, id: &str) -> Result<(), SyncRefused> {
        let key = format!("u/{}/{}/{id}", self.relay.user, self.room);
        self.relay.held.lock().unwrap().remove(&key);
        Ok(())
    }
}

impl SyncAccountRemote for FakeRemote {
    fn usage(&self) -> Result<SyncUsage, SyncRefused> {
        let prefix = format!("u/{}/", self.relay.user);
        let used: u64 = self
            .relay
            .held
            .lock()
            .unwrap()
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(_, (b, _))| b.len() as u64)
            .sum();
        Ok(SyncUsage {
            used,
            quota: RELAY_QUOTA,
        })
    }

    fn copies(&self) -> Result<Vec<ListedCopy>, SyncRefused> {
        let prefix = format!("u/{}/", self.relay.user);
        let mut by_room: HashMap<String, (u64, u64, Vec<String>)> = HashMap::new();
        for (k, (b, at)) in self.relay.held.lock().unwrap().iter() {
            let Some(rest) = k.strip_prefix(&prefix) else {
                continue;
            };
            let Some((room, id)) = rest.split_once('/') else {
                continue;
            };
            let entry = by_room
                .entry(room.to_owned())
                .or_insert_with(|| (0, 0, Vec::new()));
            entry.0 += b.len() as u64;
            entry.1 = entry.1.max(*at);
            entry.2.push(id.to_owned());
        }
        Ok(by_room
            .into_iter()
            .map(|(room, (bytes, at, ids))| ListedCopy {
                room,
                bytes,
                at,
                ids,
            })
            .collect())
    }

    fn remove_all(&self) -> Result<(), SyncRefused> {
        self.relay.removed_all.fetch_add(1, Ordering::SeqCst);
        let prefix = format!("u/{}/", self.relay.user);
        self.relay
            .held
            .lock()
            .unwrap()
            .retain(|k, _| !k.starts_with(&prefix));
        Ok(())
    }
}

fn a_profile(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    fs::write(
        dir.0.join("teammates.json"),
        format!("{}\n", json!({ "schemaVersion": 15, "teammates": [{ "name": "Wren" }] })),
    )
    .unwrap();
    fs::write(
        dir.0.join("memories.json"),
        format!("{}\n", json!({ "schemaVersion": 2, "memories": [{ "text": "Colin ships on Mondays" }] })),
    )
    .unwrap();
    dir
}

fn link_as(user: u64, login: &str) -> AccountLink {
    AccountLink {
        relay: "https://relay.test".into(),
        room: random_text(16),
        token: "t".repeat(43),
        user,
        login: login.to_owned(),
    }
}

struct AccountHarness {
    account: YourAccount,
    relay: Arc<FakeRelay>,
    link_slot: Arc<Mutex<Option<AccountLink>>>,
    clock_ms: Arc<AtomicI64>,
}

impl AccountHarness {
    fn sign_in(&self, next: Option<AccountLink>) {
        *self.link_slot.lock().unwrap() = next;
    }

    fn advance_ms(&self, delta: i64) {
        self.clock_ms.fetch_add(delta, Ordering::SeqCst);
    }
}

fn an_account(
    profile: &Path,
    link: Option<AccountLink>,
    relay: Option<Arc<FakeRelay>>,
    available: bool,
    computer: &str,
    restore: Option<Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>>,
    ask_for_ms: Option<u64>,
) -> AccountHarness {
    let user = link.as_ref().map_or(7, |l| l.user);
    let relay = relay.unwrap_or_else(|| FakeRelay::new(user, false));
    let link_slot = Arc::new(Mutex::new(link));
    let link_reader = Arc::clone(&link_slot);
    let computer_name = computer.to_owned();
    let clock_ms = Arc::new(AtomicI64::new(1_761_933_600_000));
    let clock_reader = Arc::clone(&clock_ms);
    let account = YourAccount::new(YourAccountOptions {
        profile: profile.to_path_buf(),
        link: Arc::new(move || link_reader.lock().unwrap().clone()),
        secrets: FakeSecretBox::new(available),
        computer: Arc::new(move || computer_name.clone()),
        app_version: "0.845.0".into(),
        busy: Arc::new(|| false),
        remote: Some(relay.remote_factory()),
        now: Some(Arc::new(move || {
            let ms = clock_reader.load(Ordering::SeqCst);
            Utc.timestamp_millis_opt(ms).single().unwrap()
        })),
        quiet_ms: Some(0),
        copies_fresh_ms: Some(0),
        ask_for_ms,
        changed: None,
        restore,
    });
    AccountHarness {
        account,
        relay,
        link_slot,
        clock_ms,
    }
}

fn is_recovery_code(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 11
        && parts
            .iter()
            .all(|p| p.len() == 5 && p.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase()))
}

fn is_match_code(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 2
        && parts
            .iter()
            .all(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_digit() || b.is_ascii_uppercase()))
}

fn two_computers(tag: &str) -> (Arc<FakeRelay>, String, TempDir, AccountHarness, Arc<Mutex<Vec<PathBuf>>>) {
    let relay = FakeRelay::new(7, false);
    let pc_profile = a_profile(&format!("{tag}-pc"));
    let pc = an_account(&pc_profile.0, Some(link_as(7, "colin")), Some(Arc::clone(&relay)), true, "Colin's PC", None, None);
    let first = pc.account.turn_on();
    assert!(pc.account.status().last_synced.is_some());
    pc.account.stop();

    let mac_profile = TempDir::new(&format!("{tag}-mac"));
    fs::write(
        mac_profile.0.join("teammates.json"),
        format!("{}\n", json!({ "schemaVersion": 15, "teammates": [{ "name": "Atlas" }] })),
    )
    .unwrap();
    let restores = Arc::new(Mutex::new(Vec::new()));
    let restores_cb = Arc::clone(&restores);
    let mac_root = mac_profile.0.clone();
    let mac = an_account(
        &mac_profile.0,
        Some(link_as(7, "colin")),
        Some(Arc::clone(&relay)),
        true,
        "Colin's Mac",
        Some(Arc::new(move |folder: &Path| {
            restores_cb.lock().unwrap().push(folder.to_path_buf());
            request_restore(folder, &mac_root, "0.845.0", 0, Utc::now()).map(|_| ())
        })),
        None,
    );
    (relay, first.code.unwrap(), mac_profile, mac, restores)
}

fn pc_and_mac(
    tag: &str,
    ask_for_ms: Option<u64>,
) -> (
    Arc<FakeRelay>,
    TempDir,
    AccountHarness,
    TempDir,
    AccountHarness,
    Arc<Mutex<Vec<PathBuf>>>,
) {
    let relay = FakeRelay::new(7, false);
    let pc_profile = a_profile(&format!("{tag}-pc"));
    let pc = an_account(
        &pc_profile.0,
        Some(link_as(7, "colin")),
        Some(Arc::clone(&relay)),
        true,
        "Colin's PC",
        None,
        None,
    );
    pc.account.turn_on();
    assert!(pc.account.status().last_synced.is_some());

    let mac_profile = TempDir::new(&format!("{tag}-mac"));
    fs::write(
        mac_profile.0.join("teammates.json"),
        format!("{}\n", json!({ "schemaVersion": 15, "teammates": [{ "name": "Atlas" }] })),
    )
    .unwrap();
    let restores = Arc::new(Mutex::new(Vec::new()));
    let restores_cb = Arc::clone(&restores);
    let mac_root = mac_profile.0.clone();
    let mac = an_account(
        &mac_profile.0,
        Some(link_as(7, "colin")),
        Some(Arc::clone(&relay)),
        true,
        "Colin's Mac",
        Some(Arc::new(move |folder: &Path| {
            restores_cb.lock().unwrap().push(folder.to_path_buf());
            request_restore(folder, &mac_root, "0.845.0", 0, Utc::now()).map(|_| ())
        })),
        ask_for_ms,
    );
    (relay, pc_profile, pc, mac_profile, mac, restores)
}

// ---- Slice 3: Your account ------------------------------------------------------------------------------------------

#[test]
// case: without a GitHub sign-in it says where to sign in and keeps nothing
fn without_a_github_sign_in_it_says_where_to_sign_in_and_keeps_nothing() {
    let profile = a_profile("signed-out");
    let h = an_account(&profile.0, None, None, true, "Colin's Mac", None, None);
    let st = h.account.status();
    assert_eq!(st.signed_in, None);
    assert_eq!(st.sync, AccountSyncState::Off);
    assert!(!st.has_key);
    assert!(!st.syncing);
    let answer = h.account.turn_on();
    assert_eq!(answer.code, None);
    assert_eq!(
        answer.status.message.as_deref(),
        Some("Sign in with GitHub first, under Use from anywhere.")
    );
    assert!(!profile.0.join(ACCOUNT_SYNC_FILE).exists());
}

#[test]
// case: turning sync on shows the recovery code once, locks the key, and sends the copy
fn turning_sync_on_shows_the_recovery_code_once_locks_the_key_and_sends_the_copy() {
    let profile = a_profile("turn-on");
    let h = an_account(&profile.0, Some(link_as(7, "colin")), None, true, "Colin's Mac", None, None);
    let answer = h.account.turn_on();
    let code = answer.code.as_deref().unwrap();
    assert!(is_recovery_code(code), "bad code: {code}");
    assert_eq!(answer.status.signed_in.as_ref().map(|s| s.login.as_str()), Some("colin"));
    assert_eq!(answer.status.sync, AccountSyncState::On);
    assert!(answer.status.has_key);
    let key = profile_key_from_code(code).unwrap();
    let saved = fs::read_to_string(profile.0.join(ACCOUNT_SYNC_FILE)).unwrap();
    assert!(!saved.contains(&to_base64url(&key)));
    assert!(!saved.contains(code));
    assert!(LEFT_OUT.iter().any(|(k, _)| *k == ACCOUNT_SYNC_FILE));
    let status = h.account.status();
    assert_eq!(status.last_synced.as_ref().map(|s| s.files), Some(2));
    let storage = status.storage.unwrap();
    assert!(storage.used > 0);
    assert!(storage.quota > storage.used);
    assert!(!h.relay.held.lock().unwrap().is_empty());
    for (_, (bytes, _)) in h.relay.held.lock().unwrap().iter() {
        assert!(!String::from_utf8_lossy(bytes).contains("Colin ships on Mondays"));
    }
    h.account.stop();
}

#[test]
// case: off keeps the key, and on again shows no new code and seals with the same key
fn off_keeps_the_key_and_on_again_shows_no_new_code_and_seals_with_the_same_key() {
    let profile = a_profile("off-on");
    let h = an_account(&profile.0, Some(link_as(7, "colin")), None, true, "Colin's Mac", None, None);
    let first = h.account.turn_on();
    let before: Value =
        serde_json::from_str(&fs::read_to_string(profile.0.join(ACCOUNT_SYNC_FILE)).unwrap()).unwrap();
    assert_eq!(h.account.turn_off().sync, AccountSyncState::Off);
    let st = h.account.status();
    assert_eq!(st.sync, AccountSyncState::Off);
    assert!(st.has_key);
    let again = h.account.turn_on();
    assert_eq!(again.code, None);
    assert_eq!(again.status.sync, AccountSyncState::On);
    let after: Value =
        serde_json::from_str(&fs::read_to_string(profile.0.join(ACCOUNT_SYNC_FILE)).unwrap()).unwrap();
    assert_eq!(after["key"], before["key"]);
    let code = first.code.unwrap();
    assert_eq!(
        recovery_code(&profile_key_from_code(&code).unwrap()).unwrap(),
        code
    );
    h.account.stop();
}

#[test]
// case: a computer that cannot lock a key keeps sync off and says why
fn a_computer_that_cannot_lock_a_key_keeps_sync_off_and_says_why() {
    let profile = a_profile("no-lock");
    let h = an_account(&profile.0, Some(link_as(7, "colin")), None, false, "Colin's Mac", None, None);
    let answer = h.account.turn_on();
    assert_eq!(answer.code, None);
    assert_eq!(answer.status.sync, AccountSyncState::Off);
    assert!(!answer.status.has_key);
    assert_eq!(
        answer.status.message.as_deref(),
        Some("This computer cannot lock a key, so sync stays off. Your Locust is safe here.")
    );
}

#[test]
// case: a key made for another GitHub account is not used: a new sign-in starts over with its own code
fn a_key_made_for_another_github_account_is_not_used_a_new_sign_in_starts_over_with_its_own_code() {
    let profile = a_profile("other-acct");
    let relay = FakeRelay::new(7, false);
    let h = an_account(&profile.0, Some(link_as(7, "colin")), Some(relay), true, "Colin's Mac", None, None);
    h.account.turn_on();
    h.sign_in(Some(link_as(8, "someone")));
    let st = h.account.status();
    assert_eq!(st.signed_in.as_ref().map(|s| s.login.as_str()), Some("someone"));
    assert_eq!(st.sync, AccountSyncState::Off);
    assert!(!st.has_key);
    assert!(h.account.turn_on().code.is_some());
    h.account.stop();
}

#[test]
// case: Sync now says when the relay is full, and nothing on the computer changes
fn sync_now_says_when_the_relay_is_full_and_nothing_on_the_computer_changes() {
    let profile = a_profile("full");
    let relay = FakeRelay::new(7, true);
    let h = an_account(&profile.0, Some(link_as(7, "colin")), Some(relay), true, "Colin's Mac", None, None);
    h.account.turn_on();
    let status = h.account.sync_now();
    assert!(status.message.as_deref().unwrap_or("").contains("full (1 GB)"));
    assert_eq!(status.good, Some(false));
    assert!(fs::read_to_string(profile.0.join("teammates.json")).unwrap().contains("Wren"));
    h.account.stop();
}

#[test]
// case: Delete my copy empties the relay, turns sync off and forgets what was sent
fn delete_my_copy_empties_the_relay_turns_sync_off_and_forgets_what_was_sent() {
    let profile = a_profile("delete");
    let h = an_account(&profile.0, Some(link_as(7, "colin")), None, true, "Colin's Mac", None, None);
    h.account.turn_on();
    assert!(h.account.status().last_synced.is_some());
    let status = h.account.delete_copy();
    assert_eq!(h.relay.removed_all.load(Ordering::SeqCst), 1);
    assert!(h.relay.held.lock().unwrap().is_empty());
    assert_eq!(status.sync, AccountSyncState::Off);
    assert!(status.has_key);
    assert_eq!(status.good, Some(true));
    assert_eq!(
        status.message.as_deref(),
        Some("Your copy on Locust\u{2019}s relay is deleted. Everything on this computer stays.")
    );
    assert_eq!(status.last_synced, None);
    assert!(!profile.0.join(PROFILE_SYNC_FILE).exists());
    assert!(fs::read_to_string(profile.0.join("teammates.json")).unwrap().contains("Wren"));
    // On again, everything goes up again.
    h.account.turn_on();
    assert!(h.account.status().last_synced.is_some());
    assert!(!h.relay.held.lock().unwrap().is_empty());
    h.account.stop();
}

// ---- Slice 4: new computer with recovery code -----------------------------------------------------------------------

#[test]
// case: a new computer is told another computer keeps a copy
fn a_new_computer_is_told_another_computer_keeps_a_copy() {
    let (_relay, _code, _mac_profile, mac, _restores) = two_computers("elsewhere");
    let st = mac.account.status();
    assert_eq!(st.elsewhere.as_ref().map(|e| e.computers), Some(1));
}

#[test]
// case: the code opens the newest copy, restores it at the next start, and keeps the key for sync
fn the_code_opens_the_newest_copy_restores_it_at_the_next_start_and_keeps_the_key_for_sync() {
    let (_relay, code, mac_profile, mac, restores) = two_computers("bring");
    let typed = code.to_lowercase().replace('-', " ");
    let brought = mac.account.bring_here(&typed);
    assert_eq!(
        brought,
        BringHereResult::Ok {
            computer: "Colin's PC".into()
        }
    );
    let folders = restores.lock().unwrap().clone();
    assert_eq!(folders.len(), 1);
    assert!(read_backup(&folders[0], "0.845.0").is_ok());
    let applied = apply_pending_restore(&mac_profile.0, "0.845.0", Local::now()).unwrap();
    assert!(applied.ok);
    assert!(fs::read_to_string(mac_profile.0.join("teammates.json")).unwrap().contains("Wren"));
    let st = mac.account.status();
    assert_eq!(st.sync, AccountSyncState::Off);
    assert!(st.has_key);
    assert_eq!(mac.account.turn_on().code, None);
    assert!(mac.account.status().last_synced.is_some());
    mac.account.stop();
}

#[test]
// case: a code that does not open the copy changes nothing
fn a_code_that_does_not_open_the_copy_changes_nothing() {
    let (_relay, _code, mac_profile, mac, restores) = two_computers("wrong-key");
    let wrong = recovery_code(&new_profile_key()).unwrap();
    let brought = mac.account.bring_here(&wrong);
    match brought {
        BringHereResult::Err { reason } => assert!(reason.contains("did not open with this key"), "{reason}"),
        BringHereResult::Ok { .. } => panic!("expected Err"),
    }
    assert!(restores.lock().unwrap().is_empty());
    assert!(!mac_profile.0.join(ACCOUNT_SYNC_FILE).exists());
    assert!(fs::read_to_string(mac_profile.0.join("teammates.json")).unwrap().contains("Atlas"));
}

#[test]
// case: a code copied wrong is caught before the relay is asked
fn a_code_copied_wrong_is_caught_before_the_relay_is_asked() {
    let (_relay, code, _mac_profile, mac, _restores) = two_computers("bad-code");
    let ch = if &code[2..3] == "0" { "1" } else { "0" };
    let bad = format!("{}{ch}{}", &code[..2], &code[3..]);
    assert_eq!(
        mac.account.bring_here(&bad),
        BringHereResult::Err {
            reason: "That code does not read right. It is 55 letters and digits in groups of five; check each group.".into()
        }
    );
}

#[test]
// case: with no other computer keeping a copy, it says to turn Keep a copy on there first
fn with_no_other_computer_keeping_a_copy_it_says_to_turn_keep_a_copy_on_there_first() {
    let profile = a_profile("alone");
    let alone = an_account(&profile.0, Some(link_as(7, "colin")), None, true, "Colin's Mac", None, None);
    let own = alone.account.turn_on();
    assert!(alone.account.status().last_synced.is_some());
    assert_eq!(
        alone.account.bring_here(own.code.as_deref().unwrap()),
        BringHereResult::Err {
            reason: "No other computer keeps a copy for colin yet. Turn on Keep a copy there first.".into()
        }
    );
    alone.account.stop();
}

// ---- HTTP remote list/usage/room validation -------------------------------------------------------------------------

#[test]
// case: reads the relay’s own list into copies and usage, and refuses a room name it should not
fn reads_the_relays_own_list_into_copies_and_usage_and_refuses_a_room_name_it_should_not() {
    let relay = FakeRelay::new(7, false);
    let pc_profile = a_profile("http-list");
    let pc = an_account(&pc_profile.0, Some(link_as(7, "colin")), Some(Arc::clone(&relay)), true, "Colin's PC", None, None);
    pc.account.turn_on();
    assert!(pc.account.status().last_synced.is_some());
    pc.account.stop();

    let listed = relay.remote_for(&random_text(16)).copies().unwrap();
    let mut computers: Vec<Value> = listed
        .iter()
        .map(|copy| {
            let mut files: Vec<Value> = copy
                .ids
                .iter()
                .map(|id| json!({ "id": id, "size": 1, "at": copy.at }))
                .collect();
            files.push(json!({ "id": "../not-an-id", "size": 1, "at": 0 }));
            json!({ "room": copy.room, "bytes": copy.bytes, "files": files })
        })
        .collect();
    computers.push(json!({ "room": "../other", "bytes": 5, "files": [] }));
    let body = serde_json::to_vec(&json!({
        "used": 1234,
        "quota": 1u64 << 30,
        "computers": computers,
    }))
    .unwrap();

    struct MockFetch {
        asked: Mutex<Vec<(String, String, String)>>,
        replies: Mutex<VecDeque<SyncHttpReply>>,
    }
    impl SyncHttpFetch for MockFetch {
        fn send(
            &self,
            url: &str,
            method: &str,
            auth: &str,
            _body: Option<&[u8]>,
        ) -> Result<SyncHttpReply, String> {
            self.asked
                .lock()
                .unwrap()
                .push((url.to_owned(), method.to_owned(), auth.to_owned()));
            Ok(self.replies.lock().unwrap().pop_front().unwrap())
        }
    }

    let mock = Arc::new(MockFetch {
        asked: Mutex::new(Vec::new()),
        replies: Mutex::new(VecDeque::from([
            SyncHttpReply { status: 200, body: body.clone() },
            SyncHttpReply { status: 200, body },
        ])),
    });
    let room = "R".repeat(22);
    let remote = HttpSyncRemote::new("https://relay.test/", &room, "secret-token", mock.clone());
    assert_eq!(remote.copies().unwrap(), listed);
    assert_eq!(
        remote.usage().unwrap(),
        SyncUsage {
            used: 1234,
            quota: 1u64 << 30
        }
    );
    let asked = mock.asked.lock().unwrap();
    assert_eq!(
        asked[0],
        (
            format!("https://relay.test/v1/room/{room}/sync"),
            "GET".to_owned(),
            "Bearer secret-token".to_owned(),
        )
    );
}

// ---- Slice 4b: Approving a new computer -----------------------------------------------------------------------------

#[test]
// case: the Mac asks, both screens show one code, the PC approves, and the Mac brings the copy in with no recovery code
fn the_mac_asks_both_screens_show_one_code_the_pc_approves_and_the_mac_brings_the_copy_in_with_no_recovery_code()
{
    let (_relay, _pc_p, pc, mac_profile, mac, restores) = pc_and_mac("approve", None);
    let asked = mac.account.ask();
    let match_code = asked.asking.as_ref().unwrap().match_code.clone();
    assert!(is_match_code(&match_code), "bad match: {match_code}");
    let on_pc = pc.account.status();
    assert_eq!(on_pc.asks.len(), 1);
    assert_eq!(on_pc.asks[0].name, "Colin's Mac");
    assert_eq!(on_pc.asks[0].match_code, match_code);
    // The Mac's waiting ask is not mistaken for a copy.
    assert_eq!(on_pc.elsewhere, None);

    let ask_room = on_pc.asks[0].room.clone();
    let approved = pc.account.approve(&ask_room, &match_code);
    assert_eq!(approved.good, Some(true));
    assert_eq!(
        approved.message.as_deref(),
        Some("Approved. Colin's Mac brings your Locust in and restarts.")
    );
    assert!(approved.asks.is_empty());

    mac.account.check_ask();
    assert_eq!(restores.lock().unwrap().len(), 1);
    let status = mac.account.status();
    assert_eq!(status.asking, None);
    assert_eq!(
        status.message.as_deref(),
        Some("Your Locust from Colin's PC is ready. Restarting to bring it in\u{2026}")
    );
    assert!(apply_pending_restore(&mac_profile.0, "0.845.0", Local::now()).unwrap().ok);
    assert!(fs::read_to_string(mac_profile.0.join("teammates.json")).unwrap().contains("Wren"));
    // The Mac now holds the same key: Keep a copy shows no new code.
    assert_eq!(mac.account.turn_on().code, None);
    assert!(mac.account.status().last_synced.is_some());
    pc.account.stop();
    mac.account.stop();
}

#[test]
// case: the ask leaves no trace: the Mac takes it down, and the PC then removes its grant
fn the_ask_leaves_no_trace_the_mac_takes_it_down_and_the_pc_then_removes_its_grant() {
    let (relay, _pc_p, pc, _mac_p, mac, _restores) = pc_and_mac("no-trace", None);
    let asked = mac.account.ask();
    let on_pc = pc.account.status();
    let ask_room = on_pc.asks[0].room.clone();
    pc.account
        .approve(&ask_room, &asked.asking.as_ref().unwrap().match_code);
    mac.account.check_ask();
    assert!(!relay.ids_at(&ask_room).contains(&ask_id()));
    let _ = pc.account.status();
    let gid = grant_id_of(&ask_room);
    assert!(relay.held_keys().iter().all(|k| !k.ends_with(&gid)));
    pc.account.stop();
    mac.account.stop();
}

#[test]
// case: a code that is not the one the PC was shown sends nothing
fn a_code_that_is_not_the_one_the_pc_was_shown_sends_nothing() {
    let (relay, _pc_p, pc, _mac_p, mac, restores) = pc_and_mac("wrong-match", None);
    mac.account.ask();
    let on_pc = pc.account.status();
    let ask_room = on_pc.asks[0].room.clone();
    let refused = pc.account.approve(&ask_room, "AAAA-AAAA");
    assert_eq!(refused.good, Some(false));
    assert_eq!(
        refused.message.as_deref(),
        Some("The code changed. Ask again on the new computer, and approve only if both screens show the same code.")
    );
    let gid = grant_id_of(&ask_room);
    assert!(!relay.held_keys().iter().any(|k| k.ends_with(&gid)));
    mac.account.check_ask();
    assert!(restores.lock().unwrap().is_empty());
    pc.account.stop();
    mac.account.stop();
}

#[test]
// case: a key swapped at the relay after the PC showed the code is refused: the PC reads the ask again before it seals
fn a_key_swapped_at_the_relay_after_the_pc_showed_the_code_is_refused_the_pc_reads_the_ask_again_before_it_seals()
{
    let (relay, _pc_p, pc, _mac_p, mac, _restores) = pc_and_mac("swapped", None);
    let asked = mac.account.ask();
    let on_pc = pc.account.status();
    let ask_room = on_pc.asks[0].room.clone();
    // The relay swaps its own key into the ask, keeping the Mac's name.
    let forged = serde_json::to_vec(&json!({
        "v": 1,
        "name": "Colin's Mac",
        "key": new_ask_keys().public_key,
        "at": 1_761_933_600_000i64,
    }))
    .unwrap();
    relay.remote_for(&ask_room).put(&ask_id(), &forged).unwrap();
    let refused = pc
        .account
        .approve(&ask_room, &asked.asking.as_ref().unwrap().match_code);
    assert_eq!(refused.good, Some(false));
    let gid = grant_id_of(&ask_room);
    assert!(!relay.held_keys().iter().any(|k| k.ends_with(&gid)));
    pc.account.stop();
    mac.account.stop();
}

#[test]
// case: Not mine puts the ask away on that computer and gives nothing
fn not_mine_puts_the_ask_away_on_that_computer_and_gives_nothing() {
    let (relay, _pc_p, pc, _mac_p, mac, _restores) = pc_and_mac("not-mine", None);
    mac.account.ask();
    let on_pc = pc.account.status();
    let ask_room = on_pc.asks[0].room.clone();
    let after = pc.account.ignore(&ask_room);
    assert!(after.asks.is_empty());
    assert!(pc.account.status().asks.is_empty());
    let gid = grant_id_of(&ask_room);
    assert!(!relay.held_keys().iter().any(|k| k.ends_with(&gid)));
    pc.account.stop();
    mac.account.stop();
}

#[test]
// case: a computer without the key is never shown an ask to approve
fn a_computer_without_the_key_is_never_shown_an_ask_to_approve() {
    let (relay, _pc_p, pc, _mac_p, mac, _restores) = pc_and_mac("third", None);
    mac.account.ask();
    assert_eq!(pc.account.status().asks.len(), 1);
    let third_profile = a_profile("third-comp");
    let third = an_account(
        &third_profile.0,
        Some(link_as(7, "colin")),
        Some(Arc::clone(&relay)),
        true,
        "A third computer",
        None,
        None,
    );
    let before = relay.gets.lock().unwrap().len();
    assert!(third.account.status().asks.is_empty());
    let aid = ask_id();
    let asked_ids: Vec<String> = relay.gets.lock().unwrap()[before..]
        .iter()
        .filter(|id| *id == &aid)
        .cloned()
        .collect();
    assert!(asked_ids.is_empty());
    pc.account.stop();
    mac.account.stop();
}

#[test]
// case: after ten minutes with no approval the ask is given up and taken down
fn after_ten_minutes_with_no_approval_the_ask_is_given_up_and_taken_down() {
    let (relay, _pc_p, pc, _mac_p, mac, _restores) = pc_and_mac("timeout", Some(0));
    mac.account.ask();
    mac.advance_ms(5);
    mac.account.check_ask();
    let status = mac.account.status();
    assert_eq!(status.asking, None);
    assert_eq!(
        status.message.as_deref(),
        Some("No computer approved this one in ten minutes. Ask again, or use your recovery code.")
    );
    let aid = ask_id();
    assert!(!relay.held_keys().iter().any(|k| k.ends_with(&aid)));
    pc.account.stop();
    mac.account.stop();
}
