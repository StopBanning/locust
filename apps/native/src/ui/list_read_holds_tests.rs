//! speed-2 row 2: at launch the conversation list is asked for before the engine is known, and again when the team
//! lands. The second ask used to drop the first read's answer (`Latest`) and start a second cold read of every ledger
//! beside it, so the list, and a conversation waiting to open from it, came only when that one finished. A read
//! running over a native folder that is there answers as the native engine's would, so it is kept.
use super::*;
use crate::store::Store;
use std::path::PathBuf;

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A data folder of this test's own, its native ledger folder made or not.
fn data(ledger_folder: bool) -> (Dir, native::hooks::Guard) {
    let root = std::env::temp_dir().join(format!(
        "locust-list-holds-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    if ledger_folder {
        std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    }
    let hook = native::hooks::install(store, None);
    (Dir(root), hook)
}

/// The first read, asked before the engine was known: it answers once `release` is sent, with one row of its own.
fn first_read(s: &State) -> std::sync::mpsc::Sender<()> {
    let (release, wait) = std::sync::mpsc::channel::<()>();
    s.recents.request((), move |()| {
        let _ = wait.recv();
        Ok(RecentBatch {
            rows: vec![ConversationRow { id: "m_first_read".into(), title: "First".into(), teammate: "Wren".into(),
                started_by: "native".into(), state: "completed".into(), updated: "2026-11-27T05:00:00.000Z".into() }],
            summaries: HashMap::new(),
            workroom_messages: HashMap::new(),
            mission_owners: HashMap::new(),
            relay_hop_cap: locust_core::workspace_settings::DEFAULT_RELAY_HOP_CAP,
            local: true,
            total: None,
            damage: native::ListDamage::default(),
        })
    });
    release
}

/// The team lands as `kind`'s, and the window polls once.
fn team_lands_as(s: &mut State, kind: Kind) {
    s.team.request((), move |()| Ok((kind, Vec::new())));
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while s.team.is_pending() && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    let mut tx = Tx::default();
    poll(s, 0.0, &mut tx);
    assert_eq!(s.engine, Some(kind), "the team landed in that poll");
}

fn team_lands(s: &mut State) {
    team_lands_as(s, Kind::Native);
}

/// The list's answer once it lands: the ids of its rows.
fn list_answer(s: &State) -> Vec<String> {
    let until = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(answer) = s.recents.take() {
            return answer.map(|batch| batch.rows.into_iter().map(|r| r.id).collect()).unwrap_or_default();
        }
        assert!(std::time::Instant::now() < until, "the list never landed");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_list_read_running_when_the_team_lands_is_kept_not_asked_again() {
    let (_dir, _hook) = data(true);
    let mut s = State::default();
    let release = first_read(&s);
    team_lands(&mut s);
    release.send(()).unwrap();
    assert_eq!(list_answer(&s), ["m_first_read"], "the first read's answer lands; no second read replaced it");
}

#[test]
fn with_no_native_folder_yet_the_list_is_asked_again_for_the_native_engine() {
    // `native::list_recent` answers the native engine differently while its folder is not made: asked again.
    let (_dir, _hook) = data(false);
    let mut s = State::default();
    let release = first_read(&s);
    team_lands(&mut s);
    release.send(()).unwrap();
    assert!(!list_answer(&s).contains(&"m_first_read".to_owned()), "the read asked before the engine was known is replaced");
}

#[test]
fn for_the_door_engine_a_running_read_is_kept_with_no_native_folder_too() {
    // The door's list never depends on the native folder: the read asked before the engine was known is its own.
    let (_dir, _hook) = data(false);
    let mut s = State::default();
    let release = first_read(&s);
    team_lands_as(&mut s, Kind::Door);
    release.send(()).unwrap();
    assert_eq!(list_answer(&s), ["m_first_read"]);
}

#[test]
fn a_read_that_already_landed_is_asked_again_when_the_team_lands() {
    let (_dir, _hook) = data(true);
    let mut s = State::default();
    first_read(&s).send(()).unwrap();
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while s.recents.is_pending() && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
    team_lands(&mut s);
    assert!(!list_answer(&s).contains(&"m_first_read".to_owned()), "a new read, of the native folder, lands after the team");
}
