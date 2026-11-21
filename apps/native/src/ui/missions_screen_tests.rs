//! The Conversations screen, drawn with no window: `missions-lists-conversations.test.tsx`,
//! `missions-can-be-deleted-together.test.ts` and `unreadable-ledger-is-said.test.tsx`. Electron's cases read its TSX
//! source or render it to markup; these draw the real screen, press it as a pointer would, and read what is left in a
//! temporary data folder afterwards.
use super::*;
use crate::store::Store;
use rune::{Env, Key, KeyEvent, Runtime};
use std::path::PathBuf;

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Dirs(PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn iso(ago_minutes: i64) -> String {
    (chrono::Utc::now() - chrono::Duration::minutes(ago_minutes)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// A turn as the listing hands it over (call newest first), with its record's summary and its owner.
fn turn(s: &mut State, id: &str, parent: Option<&str>, words: &str, created_ago: i64, updated_ago: i64) {
    s.recent.push(ConversationRow {
        id: id.into(),
        teammate: "Wren".into(),
        title: words.into(),
        started_by: "native".into(),
        state: "completed".into(),
        updated: iso(updated_ago),
    });
    s.ledger_summaries.insert(
        id.into(),
        ledger::Summary {
            mission_id: id.into(),
            title: words.into(),
            words: words.into(),
            parent: parent.map(str::to_owned),
            runtime: "codex".into(),
            model: "account-default".into(),
            created_at: iso(created_ago),
            updated: iso(updated_ago),
            ..Default::default()
        },
    );
    s.mission_owners.insert(id.into(), "tm_wren".into());
}

fn wren() -> Teammate {
    Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some("codex".into()),
        model: Some("account-default".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

/// `missionsList.test`'s five turns, newest first: a three-turn conversation, one alone, and a follow-up whose earlier
/// turn is gone. Ages in minutes: the conversation's newest turn two hours back and its first three, each turn two
/// minutes long.
fn five(s: &mut State) {
    turn(s, "m_ship", Some("m_signup"), "Good, ship it", 122, 120);
    turn(s, "m_signup", Some("m_login"), "Now the signup page", 132, 130);
    // Written between the conversation's turns: it lists after the conversation's newest turn, before its first.
    turn(s, "m_crm", None, "Tidy the CRM export", 135, 133);
    turn(s, "m_login", None, "Fix the login redirect", 202, 200);
    turn(s, "m_orphan", Some("m_missing"), "Carry on from yesterday", 1_502, 1_500);
    // The sidebar puts a row it reads as live first; this screen keeps the listing's order.
    s.recent.last_mut().unwrap().state = "running".into();
}

fn drawn(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| missions_screen::view(s, &Palette::new(true), false), (1400.0, 800.0), Env::new(true, 1.0));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn element(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element {key}"))
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn said(rt: &Runtime<State>, key: &str) -> String {
    element(rt, key).text.unwrap_or_default()
}

fn delete_words(rt: &Runtime<State>) -> String {
    element(rt, "missions-delete").label.unwrap_or_default()
}

/// The rows' keys, top to bottom.
fn rows(rt: &Runtime<State>) -> Vec<String> {
    let mut found: Vec<(f32, String)> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| Some((e.rect[1], e.key?.strip_prefix("missions-row-")?.to_owned())))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, key)| key).collect()
}

/// A screen over a temporary data folder, with a record on disk for every turn `fill` lists.
fn with_records(fill: impl FnOnce(&mut State)) -> (Dirs, Store, PathBuf, native::hooks::Guard, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-missions-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let hook = native::hooks::install(store.clone(), None);
    let ledger = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&ledger).unwrap();
    let mut s = State::default();
    s.data = Some(store.clone());
    s.teammates = vec![wren()];
    fill(&mut s);
    for row in &s.recent {
        let line = serde_json::json!({
            "schemaVersion": 1, "recordType": "mission.created", "occurredAt": row.updated,
            "metadata": {"missionId": row.id, "runId": format!("run_{}", row.id), "prompt": row.title, "runtime": "codex",
                "model": "account-default", "createdAt": row.updated}
        });
        std::fs::write(ledger.join(format!("{}.jsonl", row.id)), format!("{line}\n")).unwrap();
    }
    s.missions.open = true;
    let rt = drawn(s);
    (Dirs(root), store, ledger, hook, rt)
}

fn kept(ledger: &std::path::Path, id: &str) -> bool {
    ledger.join(format!("{id}.jsonl")).exists()
}

fn trashed(ledger: &std::path::Path, id: &str) -> bool {
    ledger.join(format!(".trash/{id}.jsonl")).exists()
}

/// Six conversations of one turn each, newest first.
fn six(s: &mut State) {
    for (i, id) in ["c1", "c2", "c3", "c4", "c5", "c6"].into_iter().enumerate() {
        let at = 10 * (i as i64 + 1);
        turn(s, id, None, &format!("Conversation {}", i + 1), at + 1, at);
    }
}

// ---- missions-lists-conversations.test.tsx ----

// case: is one row per conversation, where its newest turn stood
#[test]
fn one_row_per_conversation_where_its_newest_turn_stood() {
    let mut s = State::default();
    five(&mut s);
    let keys: Vec<_> = conversations(&s).into_iter().map(|e| e.key).collect();
    assert_eq!(keys, ["m_login", "m_crm", "m_orphan"]);
    let rt = drawn(s);
    assert_eq!(rows(&rt), ["m_login", "m_crm", "m_orphan"], "drawn in that order, a row each");
}

// case: names it by its first turn, stands it on its newest, and holds every turn oldest first
#[test]
fn named_by_its_first_turn_standing_on_its_newest_holding_every_turn() {
    let mut s = State::default();
    five(&mut s);
    let entry = conversations(&s).remove(0);
    assert_eq!(entry.root, "m_login");
    assert_eq!(entry.leaf, "m_ship");
    assert_eq!(entry.members, ["m_login", "m_signup", "m_ship"]);
    let rt = drawn(s);
    assert_eq!(said(&rt, "missions-title-m_login"), "Fix the login redirect", "named by its first turn");
    assert_eq!(said(&rt, "missions-when-m_login"), "2h", "when its newest turn was written, not its first");
}

// case: ends a hand-edited cycle instead of spinning
#[test]
fn ends_a_hand_edited_cycle() {
    let mut s = State::default();
    turn(&mut s, "a", Some("b"), "A", 60, 60);
    turn(&mut s, "b", Some("a"), "B", 120, 120);
    let members: usize = conversations(&s).iter().map(|e| e.members.len()).sum();
    assert_eq!(members, 2);
}

// case: lists conversations, and deletes a conversation with every turn
#[test]
fn lists_conversations_and_deletes_one_with_every_turn() {
    let (_dirs, _store, ledger, _hook, mut rt) = with_records(five);
    assert_eq!(said(&rt, "missions-meta"), "3 conversations");
    click(&mut rt, "missions-pick-m_login");
    click(&mut rt, "missions-delete");
    click(&mut rt, "missions-delete");
    for id in ["m_login", "m_signup", "m_ship"] {
        assert!(!kept(&ledger, id) && trashed(&ledger, id), "{id}: every turn of the conversation went");
    }
    assert!(kept(&ledger, "m_crm") && kept(&ledger, "m_orphan"), "the others stay");
    assert_eq!(rows(&rt), ["m_crm", "m_orphan"]);
    assert_eq!(said(&rt, "missions-meta"), "2 conversations");
}

// case: says when, and how many turns
#[test]
fn says_when_and_how_many_turns() {
    let mut s = State::default();
    five(&mut s);
    let rt = drawn(s);
    assert_eq!(said(&rt, "missions-when-m_login"), "2h");
    assert_eq!(said(&rt, "missions-stats-m_login"), "3 turns · 6m 00s", "three turns of two minutes");
    assert_eq!(said(&rt, "missions-when-m_crm"), "2h");
    assert_eq!(said(&rt, "missions-stats-m_crm"), "2m 00s", "one turn says its time alone");
    assert_eq!(said(&rt, "missions-when-m_orphan"), "1d");
}

// ---- missions-can-be-deleted-together.test.ts ----

// case: will not offer a running mission
#[test]
fn will_not_offer_a_running_conversation() {
    let (_dirs, _store, _ledger, _hook, mut rt) = with_records(six);
    rt.update(|s, _| {
        s.missions.running.insert("c2".into());
    });
    settle(&mut rt);
    let boxed = element(&rt, "missions-pick-c2");
    assert!(!boxed.focusable, "a running conversation's box cannot be reached");
    assert_eq!(boxed.label.as_deref(), Some("Conversation 2 is still running"));
    click(&mut rt, "missions-pick-c2");
    assert!(rt.state.missions.picked.is_empty(), "pressing it picks nothing");
    assert!(!has(&rt, "missions-pickbar"));
    click(&mut rt, "missions-pick-c1");
    assert_eq!(said(&rt, "missions-select-all"), "Select all 5");
}

// case: says why the box is disabled rather than leaving it dead
#[test]
fn says_why_the_box_is_disabled() {
    let (_dirs, _store, _ledger, _hook, mut rt) = with_records(six);
    rt.update(|s, _| {
        s.missions.running.insert("c2".into());
    });
    settle(&mut rt);
    let (x, y) = rt.find("missions-pick-c2").unwrap();
    rt.pointer_move(x, y);
    rt.frame(0.1);
    rt.frame(0.6);
    rt.frame(0.05);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("missions-pick-tip-c2-tooltip"));
    assert_eq!(tip.and_then(|e| e.label).as_deref(), Some("Still running — stop it first"));
}

// case: re-filters at the moment of deleting, not only at the moment of picking
#[test]
fn re_filters_at_the_moment_of_deleting() {
    let (_dirs, _store, ledger, _hook, mut rt) = with_records(six);
    click(&mut rt, "missions-pick-c1");
    click(&mut rt, "missions-pick-c2");
    click(&mut rt, "missions-delete");
    assert_eq!(delete_words(&rt), "Delete 2 for good?");
    // c2 starts running between the pick and the confirm.
    rt.update(|s, _| {
        s.missions.running.insert("c2".into());
    });
    settle(&mut rt);
    click(&mut rt, "missions-delete");
    assert!(trashed(&ledger, "c1"), "the one still deletable went");
    assert!(kept(&ledger, "c2") && !trashed(&ledger, "c2"), "the one that started running stays");
}

// case: appears only once something is selected
#[test]
fn the_bar_appears_only_once_something_is_selected() {
    let (_dirs, _store, _ledger, _hook, mut rt) = with_records(six);
    assert!(!has(&rt, "missions-pickbar"));
    click(&mut rt, "missions-pick-c3");
    assert_eq!(said(&rt, "missions-pickbar-count"), "1 selected");
    click(&mut rt, "missions-pick-c3");
    assert!(!has(&rt, "missions-pickbar"), "a second press unticks it, and the bar goes");
    click(&mut rt, "missions-pick-c3");
    click(&mut rt, "missions-clear");
    assert!(!has(&rt, "missions-pickbar"), "gone again once nothing is");
}

// case: is absent entirely when the screen has no way to delete
#[test]
fn no_boxes_and_no_bar_with_no_way_to_delete() {
    let mut s = State::default();
    six(&mut s);
    s.missions.open = true;
    let rt = drawn(s);
    assert_eq!(rows(&rt).len(), 6);
    assert!(!rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("missions-pick-"))));
    assert!(!has(&rt, "missions-pickbar"));
}

// case: confirms in two steps, the shape the row menu already uses
#[test]
fn confirms_in_two_steps() {
    let (_dirs, _store, ledger, _hook, mut rt) = with_records(six);
    for id in ["c1", "c2", "c3"] {
        click(&mut rt, &format!("missions-pick-{id}"));
    }
    assert_eq!(delete_words(&rt), "Delete 3");
    assert_eq!(said(&rt, "missions-select-all"), "Select all 6");
    click(&mut rt, "missions-delete");
    assert_eq!(delete_words(&rt), "Delete 3 for good?");
    assert!(["c1", "c2", "c3"].iter().all(|id| kept(&ledger, id)), "the first press deletes nothing");
    click(&mut rt, "missions-delete");
    assert!(["c1", "c2", "c3"].iter().all(|id| trashed(&ledger, id)));
    assert!(["c4", "c5", "c6"].iter().all(|id| kept(&ledger, id)));
    assert!(!has(&rt, "missions-pickbar"), "nothing is picked once they went");
}

// case: offers select-all over what is actually deletable, not what is listed
#[test]
fn select_all_counts_only_what_may_be_deleted() {
    let (_dirs, _store, _ledger, _hook, mut rt) = with_records(six);
    rt.update(|s, _| {
        s.missions.running.insert("c6".into());
    });
    click(&mut rt, "missions-pick-c1");
    assert_eq!(said(&rt, "missions-select-all"), "Select all 5");
    click(&mut rt, "missions-select-all");
    assert_eq!(said(&rt, "missions-pickbar-count"), "5 selected");
    assert!(!rt.state.missions.picked.contains("c6"));
    assert_eq!(said(&rt, "missions-select-all"), "Select none");
}

// case: keeps the destructive control away from the harmless ones
#[test]
fn delete_sits_at_the_far_end() {
    let (_dirs, _store, _ledger, _hook, mut rt) = with_records(six);
    click(&mut rt, "missions-pick-c1");
    let bar = element(&rt, "missions-pickbar").rect;
    let delete = element(&rt, "missions-delete").rect;
    let clear = element(&rt, "missions-clear").rect;
    let all = element(&rt, "missions-select-all").rect;
    assert!(clear[0] < all[0] && all[0] + all[2] < delete[0], "Clear, Select all, then Delete");
    assert!((bar[0] + bar[2]) - (delete[0] + delete[2]) <= 17.0, "Delete ends at the bar's end, inside its padding");
    assert!(delete[0] - (all[0] + all[2]) > bar[2] / 2.0, "and far from the harmless ones");
}

// case: puts the checkbox beside the row, never inside it
#[test]
fn the_box_sits_beside_the_row() {
    let (_dirs, _store, _ledger, _hook, mut rt) = with_records(six);
    let boxed = element(&rt, "missions-pick-c1").rect;
    let button = element(&rt, "missions-open-c1").rect;
    assert!(boxed[0] + boxed[2] <= button[0], "the box ends before the row's button begins");
    click(&mut rt, "missions-pick-c1");
    assert!(rt.state.missions.open, "a press on the box does not open the conversation");
    assert!(rt.state.missions.picked.contains("c1"));
}

// case: lets the row keep the width it had
#[test]
fn the_row_keeps_its_width() {
    let (_dirs, _store, _ledger, _hook, rt) = with_records(six);
    let wrap = element(&rt, "missions-row-c1").rect;
    let button = element(&rt, "missions-open-c1").rect;
    assert!((wrap[0] + wrap[2] - (button[0] + button[2])).abs() < 1.0, "the row runs to the list's end");
    assert!(button[2] >= wrap[2] - 31.0, "only the box and its inset are new beside it");
    assert!(element(&rt, "missions-title-c1").rect[2] > 100.0, "the title keeps room to be read");
}

// ---- unreadable-ledger-is-said.test.tsx ----

fn header(rows: usize, damage: native::ListDamage, total: Option<usize>) -> String {
    let mut s = State::default();
    for i in 0..rows {
        turn(&mut s, &format!("m_{i}"), None, "do the thing", i as i64 + 1, i as i64 + 1);
    }
    s.recent_damage = damage;
    s.recent_total = total;
    let mut rt = Runtime::new(s, |s, _| missions_screen::view(s, &Palette::new(true), false), (1400.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    said(&rt, "missions-meta")
}

fn damage(incomplete: usize, unreadable: usize, folder_unreadable: bool) -> native::ListDamage {
    native::ListDamage { incomplete, unreadable, folder_unreadable }
}

// case: says verified only when there is genuinely nothing wrong
#[test]
fn a_healthy_list_says_its_count_alone() {
    assert_eq!(header(1, damage(0, 0, false), None), "1 conversation");
}

// case: never claims verification while a file could not be read
#[test]
fn an_unreadable_file_is_said_with_nothing_listed() {
    let said = header(0, damage(0, 1, false), None);
    assert!(!said.to_lowercase().contains("verified"), "{said}");
    assert_eq!(said, "0 conversations · 1 file could not be read");
}

// case: warns even when a clean mission sits beside the unreadable file
#[test]
fn an_unreadable_file_is_said_beside_a_clean_one() {
    let said = header(1, damage(0, 1, false), None);
    assert!(!said.to_lowercase().contains("verified"), "{said}");
    assert_eq!(said, "1 conversation · 1 file could not be read");
}

// case: keeps the two kinds of damage apart rather than summing them
#[test]
fn the_two_kinds_of_damage_are_kept_apart() {
    let said = header(2, damage(1, 1, false), None);
    assert!(said.contains("1 with an incomplete receipt"), "{said}");
    assert!(said.contains("1 file could not be read"), "{said}");
    assert!(!said.contains("2 with an incomplete receipt"), "{said}");
    assert_eq!(said, "2 conversations · 1 with an incomplete receipt · 1 file could not be read");
}

// case: never claims verification when the ledger could not be read at all
#[test]
fn records_that_would_not_open_are_said() {
    let said = header(0, damage(0, 0, true), None);
    assert!(!said.to_lowercase().contains("verified"), "{said}");
    assert_eq!(said, "0 conversations · the records could not be read");
}

// case: still counts body damage on its own, as it always did
#[test]
fn body_damage_is_counted_on_its_own() {
    let said = header(1, damage(1, 0, false), None);
    assert_eq!(said, "1 conversation · 1 with an incomplete receipt");
    assert!(!said.contains("could not be read"));
}

// case: names how many are listed of how many are kept
#[test]
fn a_list_cut_short_says_how_much_is_listed() {
    let said = header(3, damage(0, 0, false), Some(2_412));
    assert!(said.ends_with(" · only the newest 3 of 2,412 turns are listed"), "{said}");
}

// case: says nothing of it when everything is listed
#[test]
fn a_whole_list_says_nothing_of_it() {
    assert!(!header(3, damage(0, 0, false), None).contains("newest"));
    assert!(!header(3, damage(0, 0, false), Some(3)).contains("newest"));
}

// ---- how the screen is reached ----

#[test]
fn ctrl_1_and_the_palette_open_it_and_escape_closes_it() {
    let mut s = State::default();
    five(&mut s);
    let mut tx = Tx::default();
    let ctrl_1 = KeyEvent { key: Key::Command('1'), mods: rune::Mods { cmd: true, ..Default::default() } };
    assert!(on_key(&mut s, &ctrl_1, &mut tx));
    assert!(s.missions.open);
    assert!(on_key(&mut s, &KeyEvent { key: Key::Escape, mods: Default::default() }, &mut tx));
    assert!(!s.missions.open);
    let actions = default_palette_actions(&[], false, false);
    let go = actions.iter().find(|a| a.id == "go-missions").expect("Go to → Conversations");
    assert_eq!((go.group.as_str(), go.label.as_str()), ("Go to", "Conversations"));
    run_palette_action(&mut s, "go-missions", &mut tx);
    assert!(s.missions.open);
}

#[test]
fn a_teammates_list_is_theirs_alone_until_everyone_is_asked_for() {
    let mut s = State::default();
    s.teammates = vec![wren()];
    five(&mut s);
    s.mission_owners.insert("m_crm".into(), "tm_other".into());
    open_for(&mut s, "tm_wren");
    let mut rt = drawn(s);
    assert_eq!(rows(&rt), ["m_login", "m_orphan"]);
    assert_eq!(said(&rt, "missions-meta"), "3 conversations", "the count is the whole ledger's");
    click(&mut rt, "missions-everyone");
    assert_eq!(rows(&rt), ["m_login", "m_crm", "m_orphan"]);
}

#[test]
fn the_filters_narrow_by_where_each_stands() {
    let mut s = State::default();
    five(&mut s);
    s.ledger_summaries.get_mut("m_crm").unwrap().running = true;
    s.missions.running.insert("m_ship".into());
    let mut rt = drawn(s);
    assert_eq!(said(&rt, "missions-tag-m_login"), "Running");
    assert_eq!(said(&rt, "missions-tag-m_crm"), "Interrupted", "no end in its record, and not running now");
    assert_eq!(said(&rt, "missions-tag-m_orphan"), "Completed");
    click(&mut rt, "missions-filter-running");
    assert_eq!(rows(&rt), ["m_login"]);
    click(&mut rt, "missions-filter-interrupted");
    assert_eq!(rows(&rt), ["m_crm"]);
    click(&mut rt, "missions-filter-completed");
    assert_eq!(rows(&rt), ["m_orphan"]);
    click(&mut rt, "missions-filter-all");
    assert_eq!(rows(&rt).len(), 3);
}

/// A real turn the engine is running (the fake app-server, held mid-turn), seen through Ctrl 1: the screen reads the
/// engine's running list on opening, and the box of that conversation cannot be ticked.
#[test]
fn a_conversation_the_engine_runs_is_not_offered() {
    let root = std::env::temp_dir().join(format!("locust-missions-live-{}-{}", std::process::id(), TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let dirs = Dirs(root.clone());
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json("teammates.json", &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec"}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let script = format!("{}/scripts/fixtures/m1-app-server.py", env!("CARGO_MANIFEST_DIR"));
    let _hook = native::hooks::install(store.clone(), Some((PathBuf::from("python3"), vec![script, "interrupt".into()])));
    let mut s = State::default();
    s.engine = Some(Kind::Native);
    s.data = Some(store.clone());
    apply_team(&mut s, vec![Teammate {
        id: "tm_codec".into(),
        name: "Codec".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(root.join("work").to_string_lossy().into()),
    }]);
    let mut rt = Runtime::new(s, crate::view, (1400.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    click(&mut rt, "composer");
    rt.paste("Synthetic prompt");
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut now = 0.0;
    while !(can_stop(&rt.state) && rt.state.recent.iter().any(|r| locust_core::engine_api::api().running().contains(&r.id))) {
        assert!(std::time::Instant::now() < deadline, "timed out waiting for the turn to run and be listed: {:?}", rt.state.status);
        if rt.state.recents.is_pending() || now as i64 % 3 != 0 {
        } else {
            rt.update(|s, _| refresh_recent(s));
        }
        rt.update(move |s, tx| {
            poll(s, now, tx);
        });
        rt.frame(0.3);
        now += 0.3;
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let live = locust_core::engine_api::api().running()[0].clone();
    rt.update(|s, tx| {
        on_key(s, &KeyEvent { key: Key::Command('1'), mods: rune::Mods { cmd: true, ..Default::default() } }, tx);
    });
    settle(&mut rt);
    assert!(rt.state.missions.open);
    let boxed = element(&rt, &format!("missions-pick-{live}"));
    assert!(!boxed.focusable, "the running conversation's box cannot be reached");
    assert_eq!(said(&rt, &format!("missions-tag-{live}")), "Running");
    click(&mut rt, &format!("missions-pick-{live}"));
    assert!(rt.state.missions.picked.is_empty());
    // The one delete both the row menu and the bar take refuses a conversation any of whose turns runs, whole:
    // its earlier turn is not taken first.
    let ledger = store.ledger_dir().unwrap();
    let line = serde_json::json!({"schemaVersion": 1, "recordType": "mission.created", "occurredAt": iso(5),
        "metadata": {"missionId": "m_before", "runId": "run_before", "prompt": "Earlier", "runtime": "codex", "createdAt": iso(5)}});
    std::fs::write(ledger.join("m_before.jsonl"), format!("{line}\n")).unwrap();
    let refused = crate::sidebar::delete_conversation(&mut rt.state, &store, &["m_before".to_owned(), live.clone()]);
    assert_eq!(refused, Err(locust_core::trash::STILL_RUNNING.to_owned()));
    assert!(kept(&ledger, "m_before") && kept(&ledger, &live), "nothing of it went");
    on_quit(&mut rt.state, &mut Tx::default());
    drop(dirs);
}

/// shell.css's container query: below 960 points of list a row takes two lines, the title keeping room to be read
/// (at 1120 with the sidebar the one-line row measured the title 1 point wide).
#[test]
fn a_narrow_list_takes_two_lines_and_keeps_the_title() {
    let mut s = State::default();
    five(&mut s);
    let mut rt = Runtime::new(s, |s, _| missions_screen::view(s, &Palette::new(true), true), (900.0, 800.0), Env::new(true, 1.0));
    settle(&mut rt);
    let title = element(&rt, "missions-title-m_login").rect;
    let route = element(&rt, "missions-route-m_login").rect;
    let stats = element(&rt, "missions-stats-m_login").rect;
    assert!(title[2] > 200.0, "the title keeps room: {title:?}");
    assert!(route[1] > title[1] + title[3] - 1.0 && stats[1] > title[1] + title[3] - 1.0, "route and counts on the second line");
    assert!((route[0] - title[0]).abs() < 1.0, "the route sits under the title");
}
