//! Electron `codex-buddy-wears-a-screen.test.tsx`, the 9 teammate-wearing and look-picker cases
//! (complementing the 10 table/framing/sheet cases in `core/src/buddy_moves.rs`).

use super::*;
use crate::pet_faces::Sheet;
use locust_core::bot_spec::{clean_avatar, seed_avatar, AvatarSpec, BotSpec, PetRef, PetSource};
use locust_core::buddy_moves::{buddy_move_for, screen_face_for, screen_fits, CODEX_BUDDY};
use locust_core::look_picker::avatar_from_json;
use locust_core::pet_motion::pet_state_for;
use locust_core::pets::PetState;
use serde_json::{json, Value};
use std::sync::Arc;

const SLOT: &str = "buddy-screen-test";

fn buddy_ref(screen: Option<bool>) -> PetRef {
    PetRef { source: PetSource::Gallery, id: "codex-buddy".into(), screen }
}

fn cat_ref() -> PetRef {
    PetRef { source: PetSource::Bundled, id: "hoodie-cat".into(), screen: None }
}

fn look_wearing(pet: PetRef) -> FaceLook {
    let a = seed_avatar("tm_buddy");
    FaceLook {
        hue: "blue".into(),
        parts: Some([a.headwear, a.accessory, a.mouth]),
        bot: Some(("droid".into(), "eyes".into())),
        bot_screen: None,
        pet: Some(pet),
    }
}

thread_local! {
    static CASE: std::cell::RefCell<Option<(FaceLook, f32, FaceActivity, BotMotionLevel, Option<&'static str>)>> =
        const { std::cell::RefCell::new(None) };
}

fn draw_face(
    mut s: crate::State,
    look: FaceLook,
    size: f32,
    activity: FaceActivity,
    motion: BotMotionLevel,
    terminal_faces: bool,
    label: Option<&'static str>,
) -> rune::Runtime<crate::State> {
    s.settings_panel.settings.terminal_faces = terminal_faces;
    CASE.with(|c| *c.borrow_mut() = Some((look, size, activity, motion, label)));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| {
            let (look, size, activity, motion, a11y_label) = CASE.with(|c| c.borrow().clone()).unwrap();
            let opts = BotFaceOpts { activity, motion, a11y_label, ..Default::default() };
            bot_face_with_look(s, SLOT, "tm_buddy", "Buddy", Some(&look), size, opts, &super::super::controls::Look::new(true))
        },
        (160.0, 160.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn open_picker(pet: Value, terminal_faces: bool) -> rune::Runtime<crate::State> {
    let mut s = crate::State::default();
    s.settings_panel.settings.terminal_faces = terminal_faces;
    let avatar = json!({
        "headwear": 0, "accessory": 0, "mouth": 0,
        "bot": { "shape": "droid", "face": "eyes" },
        "pet": pet
    });
    s.people_panel.team = json!({
        "teammates": [{
            "teammateId": "tm_buddy",
            "name": "Buddy",
            "hue": "blue",
            "role": "Docs & QA",
            "createdAt": "2026-10-05T00:00:00.000Z",
            "avatar": avatar
        }]
    });
    crate::people_panel::edit(&mut s, Some("tm_buddy"));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| crate::people_panel::edit_form(s, &crate::Palette::new(true)),
        (1100.0, 2400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn find_a11y<'a>(node: &'a Value, key: &str) -> Option<&'a Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"].as_array()?.iter().find_map(|c| find_a11y(c, key))
}

fn with_cat_sheet() -> crate::State {
    let s = crate::State::default();
    let cache = rune::ImageCache::new(Arc::new(|| {}));
    let cells = crate::pet_faces::cut_sheet(16, 18, &vec![200u8; 16 * 18 * 4], 9)
        .into_iter()
        .map(|(at, w, h, px)| (at, cache.rgba(w, h, px)))
        .collect();
    s.pet_faces
        .sheets
        .borrow_mut()
        .insert("bundled/hoodie-cat".into(), Sheet::Ready { cells, rows: 9, puppet: None });
    s
}

// ---- a teammate wearing him ----

// case: wears the screen while Terminal faces is on, saying what he is doing
#[test]
fn wears_the_screen_while_terminal_faces_is_on_saying_what_he_is_doing() {
    let rt = draw_face(
        crate::State::default(),
        look_wearing(buddy_ref(None)),
        44.0,
        FaceActivity::Working,
        BotMotionLevel::Subtle,
        true,
        Some("Buddy"),
    );
    let snap = rt.state.bot_stage.buddy_snapshot(SLOT).expect("codex-buddy on stage");
    assert!(snap.screen, "data-screen=on");
    assert_eq!(snap.eyes, [">", "▮"], "working eyes say what he is doing");
    assert_eq!(pet_state_for(FaceActivity::Working), PetState::Running);
    let tree = rt.accessibility_json();
    let face = find_a11y(&tree, &format!("bot-{SLOT}")).expect("in accessibility tree");
    assert_eq!(face["name"], "Buddy");
}

// case: lifts everything he has, as the face you talk to and beside a name: all of his lifts are standing ones
#[test]
fn lifts_everything_he_has_as_the_face_you_talk_to_and_beside_a_name() {
    let full_move = buddy_move_for("working", None, BotMotionLevel::Full);
    let subtle_move = buddy_move_for("working", None, BotMotionLevel::Subtle);
    assert_eq!(full_move.name, "workout");
    assert_eq!(subtle_move.name, "workout");
    // Both full (96px) and subtle (34px) draw Codex Buddy on the stage in working workout mode.
    let full = draw_face(
        crate::State::default(),
        look_wearing(buddy_ref(None)),
        96.0,
        FaceActivity::Working,
        BotMotionLevel::Full,
        true,
        None,
    );
    let subtle = draw_face(
        crate::State::default(),
        look_wearing(buddy_ref(None)),
        34.0,
        FaceActivity::Working,
        BotMotionLevel::Subtle,
        true,
        None,
    );
    assert!(full.state.bot_stage.buddy_snapshot(SLOT).unwrap().pose.dip <= 3.5);
    assert!(subtle.state.bot_stage.buddy_snapshot(SLOT).unwrap().pose.dip <= 3.5);
}

// case: is drawn as his maker drew him with Terminal faces off, or when his teammate asks for that
#[test]
fn is_drawn_as_his_maker_drew_him_with_terminal_faces_off_or_when_asked() {
    let off = draw_face(
        crate::State::default(),
        look_wearing(buddy_ref(None)),
        44.0,
        FaceActivity::Working,
        BotMotionLevel::Subtle,
        false,
        None,
    );
    assert!(!off.state.bot_stage.buddy_snapshot(SLOT).unwrap().screen, "Terminal faces off");

    let as_drawn = draw_face(
        crate::State::default(),
        look_wearing(buddy_ref(Some(false))),
        44.0,
        FaceActivity::Working,
        BotMotionLevel::Subtle,
        true,
        None,
    );
    assert!(!as_drawn.state.bot_stage.buddy_snapshot(SLOT).unwrap().screen, "screen: false");

    let screened = draw_face(
        crate::State::default(),
        look_wearing(buddy_ref(Some(true))),
        44.0,
        FaceActivity::Working,
        BotMotionLevel::Subtle,
        true,
        None,
    );
    assert!(screened.state.bot_stage.buddy_snapshot(SLOT).unwrap().screen, "screen: true");
}

// case: is drawn as he is when his sheet is not the one measured
#[test]
fn is_drawn_as_he_is_when_his_sheet_is_not_the_one_measured() {
    let width = CODEX_BUDDY.frame_width as usize * 8;
    let height = CODEX_BUDDY.frame_height as usize * 9;
    // An unmeasured/blank sheet is refused by screen_fits, so he wears no screen (drawn as he is).
    let wrong_sheet = vec![0u8; width * height * 4];
    let fits = screen_fits(&CODEX_BUDDY, &wrong_sheet, width, height);
    assert!(!fits, "unmeasured sheet refused");
    let rt = draw_face(
        crate::State::default(),
        look_wearing(buddy_ref(Some(fits))),
        44.0,
        FaceActivity::Working,
        BotMotionLevel::Subtle,
        true,
        None,
    );
    assert!(!rt.state.bot_stage.buddy_snapshot(SLOT).unwrap().screen);
    assert!(!drawn_shape(SLOT).unwrap().is_empty());
}

// case: leaves every other pet as its maker drew it, even given a table
#[test]
fn leaves_every_other_pet_as_its_maker_drew_it_even_given_a_table() {
    assert!(screen_face_for("hoodie-cat").is_none());
    let look = look_wearing(cat_ref());
    assert!(!is_codex_buddy(Some(&look)));
    assert!(!wears_screen(Some(&look), "droid", true));
    let rt = draw_face(
        with_cat_sheet(),
        look,
        44.0,
        FaceActivity::Working,
        BotMotionLevel::Subtle,
        true,
        None,
    );
    assert_eq!(drawn_pet(SLOT).as_deref(), Some("hoodie-cat"));
    assert!(drawn_puppet(SLOT).is_none());
    assert!(rt.state.bot_stage.buddy_snapshot(SLOT).is_none());
}

// ---- his teammate's look ----

// case: offers his face as drawn or the screen, the screen until asked otherwise
#[test]
fn offers_his_face_as_drawn_or_the_screen_the_screen_until_asked_otherwise() {
    use crate::look_picker::tests::{checked_in, radios_in};
    let rt = open_picker(json!({"source": "gallery", "id": "codex-buddy"}), true);
    assert!(rt.find("team-look-face").is_some(), "aria-label=\"Face\"");
    let names: Vec<String> = radios_in(&rt, "team-look-face")
        .iter()
        .map(|r| r["name"].as_str().unwrap_or("").to_owned())
        .collect();
    assert_eq!(names, ["As drawn", "Screen"]);
    assert_eq!(checked_in(&rt, "team-look-face"), ["Screen"]);

    let drawn = open_picker(json!({"source": "gallery", "id": "codex-buddy", "screen": false}), true);
    assert_eq!(checked_in(&drawn, "team-look-face"), ["As drawn"]);
}

// case: offers the screen only while Terminal faces is on, as a bot’s
#[test]
fn offers_the_screen_only_while_terminal_faces_is_on_as_a_bots() {
    use crate::look_picker::tests::{checked_in, radios_in};
    let rt = open_picker(json!({"source": "gallery", "id": "codex-buddy"}), false);
    assert_eq!(checked_in(&rt, "team-look-face"), ["As drawn"]);
    let face = radios_in(&rt, "team-look-face");
    assert_eq!(face[1]["name"], "Screen");
    let states = face[1]["states"].as_array().expect("states");
    assert!(
        states.iter().any(|st| st == "disabled"),
        "Screen disabled when Terminal faces is off: {states:?}"
    );
    assert_eq!(
        face[1]["description"],
        "Turn Terminal faces on in Settings > Appearance to give a teammate a screen."
    );
}

// case: offers nothing for a pet with no screen measured, as before
#[test]
fn offers_nothing_for_a_pet_with_no_screen_measured_as_before() {
    let rt = open_picker(json!({"source": "bundled", "id": "hoodie-cat"}), true);
    assert!(rt.find("team-look-face").is_none());
}

// case: keeps the choice with the teammate, and nothing else of the kind
#[test]
fn keeps_the_choice_with_the_teammate_and_nothing_else_of_the_kind() {
    let valid = json!({
        "headwear": 0, "accessory": 0, "mouth": 0,
        "pet": { "source": "gallery", "id": "codex-buddy", "screen": false }
    });
    let parsed = avatar_from_json(&valid).expect("valid pet screen choice");
    assert_eq!(parsed.pet, Some(buddy_ref(Some(false))));

    let invalid = json!({
        "headwear": 0, "accessory": 0, "mouth": 0,
        "pet": { "source": "gallery", "id": "codex-buddy", "screen": "no" }
    });
    assert!(avatar_from_json(&invalid).is_none(), "non-bool pet screen refused");

    let spec = AvatarSpec {
        headwear: 0,
        accessory: 0,
        mouth: 0,
        bot: Some(BotSpec { shape: "droid".into(), face: BotFace::Eyes, screen: None }),
        pet: Some(buddy_ref(Some(false))),
    };
    assert_eq!(clean_avatar(&spec).pet, Some(buddy_ref(Some(false))));

    let default_spec = AvatarSpec { pet: Some(buddy_ref(None)), ..spec };
    assert_eq!(clean_avatar(&default_spec).pet, Some(buddy_ref(None)));
}
