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

fn said(rt: &Runtime<State>, key: &str) -> String {
    element(rt, key).text.unwrap_or_default()
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
