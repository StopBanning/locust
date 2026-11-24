use crate::profile_backup::read_backup;
use crate::profile_sync::{
    CHUNK_BYTES, HttpSyncRemote, PROFILE_SYNC_FILE, PushOptions, SEAL_OVERHEAD, SYNC_QUIET_MS,
    SyncHttpFetch, SyncHttpReply, SyncPulled, SyncPushed, SyncRefused, SyncRemote, SyncSchedule,
    chunk_id_of, chunks_of, file_id_of, manifest_id, new_profile_key, open_file,
    profile_key_from_code, pull_profile, push_profile, recovery_code, seal_file, sync_keys,
};
use crate::relay_crypto::random_text;
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

const SECRET: &str = "NEVER-LEAVES-THIS-COMPUTER-q7";

struct TempDir(PathBuf);
impl TempDir {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("locust-sync-{tag}-{}", random_text(10)));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn dt(iso: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(iso).unwrap().with_timezone(&Utc)
}

fn room_id() -> String {
    random_text(16)
}

struct MemorySyncRelay {
    user: u64,
    held: Mutex<HashMap<String, Vec<u8>>>,
    puts: Mutex<Vec<(String, Vec<u8>)>>,
    removes: Mutex<Vec<String>>,
}

impl MemorySyncRelay {
    fn new(user: u64) -> Arc<Self> {
        Arc::new(Self {
            user,
            held: Mutex::new(HashMap::new()),
            puts: Mutex::new(Vec::new()),
            removes: Mutex::new(Vec::new()),
        })
    }

    fn remote_for(self: &Arc<Self>, room: &str) -> MemorySyncRemote {
        MemorySyncRemote {
            relay: Arc::clone(self),
            room: room.to_owned(),
        }
    }
}

struct MemorySyncRemote {
    relay: Arc<MemorySyncRelay>,
    room: String,
}

impl SyncRemote for MemorySyncRemote {
    fn put(&self, id: &str, boxed: &[u8]) -> Result<(), SyncRefused> {
        let key = format!("u/{}/{}/{id}", self.relay.user, self.room);
        self.relay.held.lock().unwrap().insert(key, boxed.to_vec());
        self.relay
            .puts
            .lock()
            .unwrap()
            .push((id.to_owned(), boxed.to_vec()));
        Ok(())
    }

    fn get(&self, from: &str, id: &str) -> Result<Option<Vec<u8>>, SyncRefused> {
        let key = format!("u/{}/{from}/{id}", self.relay.user);
        Ok(self.relay.held.lock().unwrap().get(&key).cloned())
    }

    fn remove(&self, id: &str) -> Result<(), SyncRefused> {
        let key = format!("u/{}/{}/{id}", self.relay.user, self.room);
        self.relay.held.lock().unwrap().remove(&key);
        self.relay.removes.lock().unwrap().push(id.to_owned());
        Ok(())
    }
}

fn a_profile(tag: &str) -> TempDir {
    let dir = TempDir::new(tag);
    let root = &dir.0;
    fs::write(
        root.join("teammates.json"),
        format!("{}\n", json!({ "schemaVersion": 15, "teammates": [{ "name": "Wren" }, { "name": "Atlas" }] })),
    )
    .unwrap();
    fs::write(
        root.join("memories.json"),
        format!("{}\n", json!({ "schemaVersion": 2, "memories": [{ "text": "Colin ships on Mondays" }] })),
    )
    .unwrap();
    fs::create_dir_all(root.join("mission-ledger")).unwrap();
    fs::write(
        root.join("mission-ledger/mission_a.jsonl"),
        format!("{}\n", json!({ "type": "started", "text": "first turn" })),
    )
    .unwrap();
    fs::write(
        root.join("mission-ledger/mission_b.jsonl"),
        format!("{}\n", json!({ "type": "started", "text": "other turn" })),
    )
    .unwrap();
    fs::write(root.join("mission-ledger/.writable"), "").unwrap();
    // What a backup leaves out, each holding the secret: none of it may reach the relay, sealed or not.
    fs::write(
        root.join("own-models.json"),
        format!("{}\n", json!({ "keys": { "openai": SECRET } })),
    )
    .unwrap();
    fs::write(
        root.join("phone-relay.json"),
        format!("{}\n", json!({ "token": SECRET })),
    )
    .unwrap();
    fs::create_dir_all(root.join("agent-accounts")).unwrap();
    fs::write(
        root.join("agent-accounts/auth.json"),
        format!("{}\n", json!({ "token": SECRET })),
    )
    .unwrap();
    fs::create_dir_all(root.join("checkpoints")).unwrap();
    fs::write(root.join("checkpoints/.env"), format!("KEY={SECRET}\n")).unwrap();
    dir
}

#[test]
// case: sends the backup set sealed, and nothing a backup leaves out
fn sends_the_backup_set_sealed_and_nothing_a_backup_leaves_out() {
    let profile = a_profile("sealed");
    let relay = MemorySyncRelay::new(7);
    let mine = room_id();
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let remote = relay.remote_for(&mine);
    let pushed = push_profile(
        &profile.0,
        &PushOptions {
            keys: &keys,
            remote: &remote,
            room: &mine,
            computer: "Colin's PC",
            app_version: "0.845.0",
            now: dt("2026-10-31T18:00:00Z"),
        },
    );
    assert!(matches!(pushed, SyncPushed::Ok { files: 4, sent: 4, .. }));

    let puts = relay.puts.lock().unwrap();
    let mid = manifest_id();
    assert_eq!(puts.last().unwrap().0, mid);
    let manifest_bytes = open_file(&keys, &mid, &puts.last().unwrap().1).unwrap();
    let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
    let mut paths: Vec<&str> = manifest["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "memories.json",
            "mission-ledger/mission_a.jsonl",
            "mission-ledger/mission_b.jsonl",
            "teammates.json"
        ]
    );
    assert_eq!(manifest["computer"], "Colin's PC");
    for (id, boxed) in puts.iter() {
        let opened = String::from_utf8_lossy(&open_file(&keys, id, boxed).unwrap()).into_owned();
        assert!(!opened.contains(SECRET));
        let raw = String::from_utf8_lossy(boxed);
        assert!(!raw.contains("Wren"));
        assert!(!raw.contains("first turn"));
        assert!(!raw.contains("teammates"));
        assert!(!raw.contains("mission_a"));
    }
    let held_keys: String = relay
        .held
        .lock()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(!held_keys.contains("teammates"));
    assert!(!held_keys.contains("mission"));
    assert!(!held_keys.contains(".json"));
}

#[test]
// case: sends only what changed: a new turn sends its conversation’s last piece and the manifest
fn sends_only_what_changed_a_new_turn_sends_its_conversations_last_piece_and_the_manifest() {
    let profile = a_profile("delta");
    let relay = MemorySyncRelay::new(7);
    let mine = room_id();
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let remote = relay.remote_for(&mine);
    let opts = PushOptions {
        keys: &keys,
        remote: &remote,
        room: &mine,
        computer: "Colin's PC",
        app_version: "0.845.0",
        now: dt("2026-10-31T18:00:00Z"),
    };
    push_profile(&profile.0, &opts);
    relay.puts.lock().unwrap().clear();

    let mut f = OpenOptions::new()
        .append(true)
        .open(profile.0.join("mission-ledger/mission_a.jsonl"))
        .unwrap();
    writeln!(f, "{}", json!({ "type": "turn", "text": "second turn" })).unwrap();
    let res = push_profile(&profile.0, &opts);
    assert!(matches!(res, SyncPushed::Ok { sent: 1, removed: 0, .. }));
    assert_eq!(relay.puts.lock().unwrap().len(), 2);

    relay.puts.lock().unwrap().clear();
    let res2 = push_profile(&profile.0, &opts);
    assert!(matches!(res2, SyncPushed::Ok { sent: 0, .. }));
    assert_eq!(relay.puts.lock().unwrap().len(), 1);
}

#[test]
// case: removes the pieces of a conversation deleted since, once the manifest no longer names them
fn removes_the_pieces_of_a_conversation_deleted_since_once_the_manifest_no_longer_names_them() {
    let profile = a_profile("remove");
    let relay = MemorySyncRelay::new(7);
    let mine = room_id();
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let remote = relay.remote_for(&mine);
    let opts = PushOptions {
        keys: &keys,
        remote: &remote,
        room: &mine,
        computer: "Colin's PC",
        app_version: "0.845.0",
        now: dt("2026-10-31T18:00:00Z"),
    };
    push_profile(&profile.0, &opts);
    let before = relay.held.lock().unwrap().len();
    fs::remove_file(profile.0.join("mission-ledger/mission_b.jsonl")).unwrap();
    let res = push_profile(&profile.0, &opts);
    assert!(matches!(res, SyncPushed::Ok { removed: 1, files: 3, .. }));
    assert_eq!(relay.held.lock().unwrap().len(), before - 1);
}

#[test]
// case: cuts a big conversation into pieces, and sends again only the piece that grew
fn cuts_a_big_conversation_into_pieces_and_sends_again_only_the_piece_that_grew() {
    let profile = a_profile("chunks");
    fs::write(
        profile.0.join("mission-ledger/mission_a.jsonl"),
        "x".repeat(CHUNK_BYTES + 10),
    )
    .unwrap();
    let relay = MemorySyncRelay::new(7);
    let mine = room_id();
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let remote = relay.remote_for(&mine);
    let opts = PushOptions {
        keys: &keys,
        remote: &remote,
        room: &mine,
        computer: "Colin's PC",
        app_version: "0.845.0",
        now: dt("2026-10-31T18:00:00Z"),
    };
    assert!(matches!(
        push_profile(&profile.0, &opts),
        SyncPushed::Ok { sent: 5, .. }
    ));
    let mut f = OpenOptions::new()
        .append(true)
        .open(profile.0.join("mission-ledger/mission_a.jsonl"))
        .unwrap();
    write!(f, "y").unwrap();
    assert!(matches!(
        push_profile(&profile.0, &opts),
        SyncPushed::Ok { sent: 1, bytes: 11, .. }
    ));
}

#[test]
// case: sends everything again for another account or a new room, and says so plainly when the relay refuses
fn sends_everything_again_for_another_account_or_a_new_room_and_says_so_plainly_when_the_relay_refuses()
{
    struct RefusingRemote;
    impl SyncRemote for RefusingRemote {
        fn put(&self, _id: &str, _boxed: &[u8]) -> Result<(), SyncRefused> {
            Err(SyncRefused {
                status: 507,
                message: "Your copy is full.".into(),
            })
        }
        fn get(&self, _from: &str, _id: &str) -> Result<Option<Vec<u8>>, SyncRefused> {
            Ok(None)
        }
        fn remove(&self, _id: &str) -> Result<(), SyncRefused> {
            Ok(())
        }
    }

    let profile = a_profile("room-switch");
    let relay = MemorySyncRelay::new(7);
    let mine = room_id();
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let remote1 = relay.remote_for(&mine);
    push_profile(
        &profile.0,
        &PushOptions {
            keys: &keys,
            remote: &remote1,
            room: &mine,
            computer: "Colin's PC",
            app_version: "0.845.0",
            now: dt("2026-10-31T18:00:00Z"),
        },
    );
    let other = room_id();
    let remote2 = relay.remote_for(&other);
    assert!(matches!(
        push_profile(
            &profile.0,
            &PushOptions {
                keys: &keys,
                remote: &remote2,
                room: &other,
                computer: "Colin's PC",
                app_version: "0.845.0",
                now: dt("2026-10-31T18:00:00Z"),
            },
        ),
        SyncPushed::Ok { sent: 4, .. }
    ));
    let state_before = fs::read_to_string(profile.0.join(PROFILE_SYNC_FILE)).unwrap();
    let failed = push_profile(
        &profile.0,
        &PushOptions {
            keys: &keys,
            remote: &RefusingRemote,
            room: &other,
            computer: "Colin's PC",
            app_version: "0.845.0",
            now: dt("2026-10-31T18:00:00Z"),
        },
    );
    assert_eq!(
        failed,
        SyncPushed::Err {
            reason: "Your copy is full.".into()
        }
    );
    assert_eq!(
        fs::read_to_string(profile.0.join(PROFILE_SYNC_FILE)).unwrap(),
        state_before
    );
}

#[test]
// case: opens another computer’s copy as a backup the existing restore reads
fn opens_another_computers_copy_as_a_backup_the_existing_restore_reads() {
    let profile = a_profile("pull");
    let relay = MemorySyncRelay::new(7);
    let pc = room_id();
    let mac = room_id();
    let raw_key = new_profile_key();
    let keys = sync_keys(&raw_key, 7).unwrap();
    let pc_remote = relay.remote_for(&pc);
    push_profile(
        &profile.0,
        &PushOptions {
            keys: &keys,
            remote: &pc_remote,
            room: &pc,
            computer: "Colin's PC",
            app_version: "0.845.0",
            now: dt("2026-10-31T18:00:00Z"),
        },
    );
    let dest = TempDir::new("copy");
    let folder = dest.0.join("copy");
    let mac_keys = sync_keys(&raw_key, 7).unwrap();
    let mac_remote = relay.remote_for(&mac);
    let pulled = pull_profile(&mac_keys, &mac_remote, &pc, &folder);
    assert!(matches!(
        pulled,
        SyncPulled::Ok { ref computer, files: 4, .. } if computer == "Colin's PC"
    ));
    let convo = fs::read_to_string(folder.join("profile/mission-ledger/mission_a.jsonl")).unwrap();
    assert!(convo.contains("first turn"));
    let reading = read_backup(&folder, "0.845.0").unwrap();
    assert_eq!(reading.manifest.counts.teammates, 2);
    assert_eq!(reading.manifest.counts.memories, 1);
    assert_eq!(reading.manifest.counts.conversations, 2);
}

#[test]
// case: does not open with another key, and refuses a copy caught halfway through a sync
fn does_not_open_with_another_key_and_refuses_a_copy_caught_halfway_through_a_sync() {
    let profile = a_profile("torn");
    let relay = MemorySyncRelay::new(7);
    let pc = room_id();
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let pc_remote = relay.remote_for(&pc);
    let opts = PushOptions {
        keys: &keys,
        remote: &pc_remote,
        room: &pc,
        computer: "Colin's PC",
        app_version: "0.845.0",
        now: dt("2026-10-31T18:00:00Z"),
    };
    push_profile(&profile.0, &opts);

    let wrong_keys = sync_keys(&new_profile_key(), 7).unwrap();
    let dest_a = TempDir::new("wrong");
    let wrong = pull_profile(&wrong_keys, &relay.remote_for(&room_id()), &pc, &dest_a.0.join("a"));
    assert!(matches!(
        wrong,
        SyncPulled::Err { ref reason } if reason.contains("did not open with this key")
    ));

    let mid = manifest_id();
    let manifest_key = format!("u/7/{pc}/{mid}");
    let old_manifest = relay.held.lock().unwrap().get(&manifest_key).unwrap().clone();
    let mut f = OpenOptions::new()
        .append(true)
        .open(profile.0.join("mission-ledger/mission_a.jsonl"))
        .unwrap();
    writeln!(f, "{}", json!({ "type": "turn", "text": "second turn" })).unwrap();
    push_profile(&profile.0, &opts);
    relay
        .held
        .lock()
        .unwrap()
        .insert(manifest_key, old_manifest);

    let dest_b = TempDir::new("torn-b");
    let torn = pull_profile(&keys, &relay.remote_for(&room_id()), &pc, &dest_b.0.join("b"));
    assert_eq!(
        torn,
        SyncPulled::Err {
            reason: "Colin's PC was syncing while this was read. Try again in a minute.".into()
        }
    );
}

#[test]
// case: are called with this computer’s token, and their refusals said in words
fn are_called_with_this_computers_token_and_their_refusals_said_in_words() {
    struct MockFetch {
        calls: Mutex<Vec<(String, String, String)>>,
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
            self.calls
                .lock()
                .unwrap()
                .push((method.to_owned(), url.to_owned(), auth.to_owned()));
            Ok(self.replies.lock().unwrap().pop_front().unwrap())
        }
    }

    let mock = Arc::new(MockFetch {
        calls: Mutex::new(Vec::new()),
        replies: Mutex::new(VecDeque::from([
            SyncHttpReply { status: 204, body: Vec::new() },
            SyncHttpReply { status: 200, body: vec![1, 2] },
            SyncHttpReply { status: 404, body: b"{}".to_vec() },
            SyncHttpReply { status: 507, body: br#"{"error":"quota"}"#.to_vec() },
        ])),
    });
    let remote = HttpSyncRemote::new("https://app.locust.lol/", "ROOM", "TOKEN", mock.clone());
    remote.put("ID", &[9]).unwrap();
    assert_eq!(remote.get("PC", "ID").unwrap(), Some(vec![1, 2]));
    assert_eq!(remote.get("PC", "GONE").unwrap(), None);
    let err = remote.put("ID", &[9]).unwrap_err();
    assert!(err.message.contains("full (1 GB)"));
    let calls = mock.calls.lock().unwrap();
    let lines: Vec<String> = calls.iter().map(|(m, u, _)| format!("{m} {u}")).collect();
    assert_eq!(
        lines,
        vec![
            "PUT https://app.locust.lol/v1/room/ROOM/sync/ID",
            "GET https://app.locust.lol/v1/room/ROOM/sync/PC/ID",
            "GET https://app.locust.lol/v1/room/ROOM/sync/PC/GONE",
            "PUT https://app.locust.lol/v1/room/ROOM/sync/ID",
        ]
    );
    assert!(calls.iter().all(|(_, _, a)| a == "Bearer TOKEN"));
}

#[test]
// case: a quiet minute after the last change, never during a run, and once more for a change made while it ran
fn a_quiet_minute_after_the_last_change_never_during_a_run_and_once_more_for_a_change_made_while_it_ran()
{
    let schedule = SyncSchedule::new(SYNC_QUIET_MS);
    let mut t = 0u64;
    let mut pushes = 0usize;

    schedule.touched(t);
    t += 50_000;
    assert!(!schedule.tick(t, false));
    schedule.touched(t);
    t += 50_000;
    assert!(!schedule.tick(t, false));
    assert_eq!(pushes, 0);

    // Busy at the 60s mark: does not fire, waits another quiet minute.
    t += 10_000;
    assert!(!schedule.tick(t, true));
    assert_eq!(pushes, 0);

    // 60s later, not busy: fires once.
    t += 60_000;
    if schedule.tick(t, false) {
        pushes += 1;
    }
    assert_eq!(pushes, 1);

    // Touched while running: after finishing, waits another 60s then fires a second time.
    schedule.touched(t);
    schedule.finished(t);
    assert!(!schedule.tick(t + 30_000, false));
    t += 60_000;
    if schedule.tick(t, false) {
        pushes += 1;
    }
    assert_eq!(pushes, 2);

    // Stopped: never fires again.
    schedule.stop();
    schedule.touched(t);
    schedule.finished(t);
    t += 120_000;
    assert!(!schedule.tick(t, false));
    assert_eq!(pushes, 2);
}

// ---- a-profile-copy-is-sealed-before-it-leaves.test.ts --------------------------------------------------------------

fn is_b64url_43(s: &str) -> bool {
    s.len() == 43
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_crockford_recovery_code(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 11
        && parts.iter().all(|p| {
            p.len() == 5
                && p.bytes()
                    .all(|b| b"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(&b))
        })
}

#[test]
// case: opens with the same key, account and id, and holds what was sealed
fn opens_with_the_same_key_account_and_id_and_holds_what_was_sealed() {
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let id = file_id_of(&keys, "teammates.json");
    let plain = br#"{"teammates":[]}"#;
    let boxed = seal_file(&keys, &id, plain).unwrap();
    let opened = open_file(&keys, &id, &boxed).unwrap();
    assert_eq!(std::str::from_utf8(&opened).unwrap(), r#"{"teammates":[]}"#);
    assert_eq!(boxed.len(), plain.len() + SEAL_OVERHEAD);
}

#[test]
// case: holds none of the words it was given
fn holds_none_of_the_words_it_was_given() {
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let secret = "Colin asked Codex to refactor the billing page".repeat(4);
    let id = file_id_of(&keys, "x");
    let boxed = seal_file(&keys, &id, secret.as_bytes()).unwrap();
    let latin1: String = boxed.iter().map(|&b| b as char).collect();
    assert!(!latin1.contains("billing"));
    // Sealed twice, the same words make two different boxes: a fresh nonce each time.
    let again = seal_file(&keys, &id, secret.as_bytes()).unwrap();
    assert_ne!(again, boxed);
}

#[test]
// case: does not open with another key, for another account, or under another id
fn does_not_open_with_another_key_for_another_account_or_under_another_id() {
    let profile = new_profile_key();
    let keys = sync_keys(&profile, 7).unwrap();
    let id = file_id_of(&keys, "memories.json");
    let boxed = seal_file(&keys, &id, b"remember this").unwrap();
    assert!(open_file(&sync_keys(&new_profile_key(), 7).unwrap(), &id, &boxed).is_err());
    assert!(open_file(&sync_keys(&profile, 8).unwrap(), &id, &boxed).is_err());
    assert!(open_file(&keys, &file_id_of(&keys, "routines.json"), &boxed).is_err());
    // Nor once a byte of it has changed.
    let mut changed = boxed.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert!(open_file(&keys, &id, &changed).is_err());
}

#[test]
// case: is the same on every computer of the account, and differs by name and by key, but for the list
fn is_the_same_on_every_computer_of_the_account_and_differs_by_name_and_by_key_but_for_the_list() {
    let profile = new_profile_key();
    let pc = sync_keys(&profile, 7).unwrap();
    let mac = sync_keys(&profile, 7).unwrap();
    assert_eq!(
        chunk_id_of(&pc, "mission-ledger/mission_a.jsonl", 0),
        chunk_id_of(&mac, "mission-ledger/mission_a.jsonl", 0)
    );
    assert_ne!(
        chunk_id_of(&pc, "mission-ledger/mission_a.jsonl", 0),
        chunk_id_of(&pc, "mission-ledger/mission_a.jsonl", 1)
    );
    // The list's id is the one id not cut with the key, so a wrong key finds the list and fails to open it.
    let mid = manifest_id();
    assert!(is_b64url_43(&mid));
    assert_ne!(chunk_id_of(&pc, "manifest", 0), mid);
}

#[test]
// case: is a relay file id that says nothing of the name
fn is_a_relay_file_id_that_says_nothing_of_the_name() {
    let keys = sync_keys(&new_profile_key(), 7).unwrap();
    let id = chunk_id_of(&keys, "mission-ledger/mission_secret.jsonl", 0);
    assert!(is_b64url_43(&id));
    assert!(!id.contains("mission"));
}

#[test]
// case: fall at fixed offsets, so an appended file changes only its last piece
fn fall_at_fixed_offsets_so_an_appended_file_changes_only_its_last_piece() {
    assert_eq!(chunks_of(0), vec![(0, 0)]);
    assert_eq!(chunks_of(10), vec![(0, 10)]);
    assert_eq!(chunks_of(CHUNK_BYTES), vec![(0, CHUNK_BYTES)]);
    let before = chunks_of(CHUNK_BYTES * 2 + 5);
    let after = chunks_of(CHUNK_BYTES * 2 + 900);
    assert_eq!(&after[0..2], &before[0..2]);
    assert_eq!(after[2], (CHUNK_BYTES * 2, CHUNK_BYTES * 2 + 900));
}

#[test]
// case: gives back the profile key, however it is typed
fn gives_back_the_profile_key_however_it_is_typed() {
    let key = new_profile_key();
    let code = recovery_code(&key).unwrap();
    assert!(is_crockford_recovery_code(&code), "unexpected format: {code}");
    assert_eq!(profile_key_from_code(&code), Some(key));
    let typed = code.to_lowercase().replace('-', " ");
    assert_eq!(profile_key_from_code(&typed), Some(key));
    // Also check Crockford I/L -> 1 and O -> 0 normalization.
    let with_confusables = code.replace('1', "l").replace('0', "o");
    assert_eq!(profile_key_from_code(&with_confusables), Some(key));
}

#[test]
// case: refuses a code copied wrong
fn refuses_a_code_copied_wrong() {
    let code = recovery_code(&new_profile_key()).unwrap();
    let first = if code.starts_with('0') { '1' } else { '0' };
    let swapped = format!("{first}{}", &code[1..]);
    assert_eq!(profile_key_from_code(&swapped), None);
    assert_eq!(profile_key_from_code(&code[..code.len() - 1]), None);
    assert_eq!(profile_key_from_code(""), None);
}
