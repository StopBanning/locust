use crate::profile_backup::read_backup;
use crate::profile_sync::{
    CHUNK_BYTES, HttpSyncRemote, PROFILE_SYNC_FILE, PushOptions, SYNC_QUIET_MS, SyncHttpFetch,
    SyncHttpReply, SyncRefused, SyncRemote, SyncSchedule, SyncPulled, SyncPushed, manifest_id,
    new_profile_key, open_file, pull_profile, push_profile, sync_keys,
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
