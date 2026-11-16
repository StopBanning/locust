//! the-kept-pets-are-puppets.test.tsx, the window's cases: "a teammate wearing one" (drawn through the live face) and
//! "its teammate's look" (the real teammate form, read through its accessibility tree). The pure cases are
//! `core/src/pet_puppets_tests.rs`.

use super::*;
use crate::look::faces::{self, BotFaceOpts};
use crate::pet_faces::Sheet;
use crate::State;
use locust_core::bot_spec::{PetRef, PetSource};
use locust_core::native::FaceLook;
use locust_core::pet_puppets::{puppet_for, PET_PUPPETS};
use serde_json::{json, Value};
use std::sync::Arc;

const SLOT: &str = "puppet-test";

fn bitty(screen: Option<bool>) -> PetRef {
    PetRef { source: PetSource::Gallery, id: "bitty".into(), screen }
}

fn look(pet: PetRef) -> FaceLook {
    FaceLook { hue: "lime".into(), parts: Some([1, 2, 3]), bot: Some(("droid".into(), "eyes".into())), bot_screen: None, pet: Some(pet) }
}

/// A state whose Bitty sheet is read: the one measured (a puppet) or another (none), and Terminal faces as given.
fn with_bitty(measured: bool, terminal: bool) -> State {
    let mut s = State::default();
    s.settings_panel.settings.terminal_faces = terminal;
    let cache = rune::ImageCache::new(Arc::new(|| {}));
    let cells = crate::pet_faces::cut_sheet(16, 18, &vec![200u8; 16 * 18 * 4], 9)
        .into_iter()
        .map(|(at, w, h, px)| (at, cache.rgba(w, h, px)))
        .collect();
    let puppet = measured.then(|| (puppet_for("bitty").unwrap(), cache.rgba(2, 2, vec![200u8; 16])));
    s.pet_faces.sheets.borrow_mut().insert("gallery/bitty".into(), Sheet::Ready { cells, rows: 9, puppet });
    s
}

thread_local! {
    static DRAWING: std::cell::RefCell<Option<FaceLook>> = const { std::cell::RefCell::new(None) };
}

fn draw(s: State, look: FaceLook) -> rune::Runtime<State> {
    DRAWING.with(|d| *d.borrow_mut() = Some(look));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| {
            let look = DRAWING.with(|d| d.borrow().clone()).unwrap();
            faces::bot_face_with_look(
                s,
                SLOT,
                "tm_bitty",
                "Bitty",
                Some(&look),
                44.0,
                BotFaceOpts { activity: FaceActivity::Working, a11y_label: Some("Bitty"), ..Default::default() },
                &crate::look::controls::Look::new(true),
            )
        },
        (100.0, 100.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn puppeted(s: State, pet: PetRef) -> bool {
    let _rt = draw(s, look(pet));
    faces::drawn_puppet(SLOT).is_some()
}

// ---- a teammate wearing one ----

#[test]
fn lives_on_a_bots_rig_a_screen_on_its_face_while_terminal_faces_is_on() {
    let rt = draw(with_bitty(true, true), look(bitty(None)));
    assert_eq!(faces::drawn_puppet(SLOT).as_deref(), Some("bitty"), "data-puppet");
    // Still a pet's face (data-pet), playing the running row for work (data-pet-state), on a bot's rig.
    assert_eq!(faces::drawn_pet(SLOT).as_deref(), Some("bitty"));
    assert_eq!(locust_core::pet_motion::pet_state_for(FaceActivity::Working), locust_core::pets::PetState::Running);
    assert_eq!(rt.state.bot_stage.desired_activity(&format!("{SLOT}#puppet")), Some(FaceActivity::Working));
    // With a teammate's hooks: its name said (aria-label).
    let tree = rt.accessibility_json().to_string();
    assert!(tree.contains("\"Bitty\""), "{tree}");
    // The screen is drawn on its face: something is painted over the drawing.
    let p = puppet_for("bitty").unwrap();
    let snap = rt.state.bot_stage.snapshot_puppet(&format!("{SLOT}#puppet"), "tm_bitty", FaceActivity::Working, None, locust_core::bot_rig::BotMotionLevel::Full, false, false);
    let pose = locust_core::pet_puppets::puppet_pose(snap.pose, locust_core::pet_puppets::motion_of(p));
    let m = face_matrix(pose, 44.0, locust_core::pet_puppets::puppet_feet(p));
    assert!(!draw_puppet_shape(&PuppetDrawSpec { puppet: p, pose, matrix: m, size: 44.0, snap: &snap }).is_empty(), "data-screen");
}

#[test]
fn is_drawn_as_its_maker_drew_it_with_terminal_faces_off_when_its_teammate_asks_or_on_a_sheet_not_measured() {
    assert!(!puppeted(with_bitty(true, false), bitty(None)), "Terminal faces off");
    assert!(!puppeted(with_bitty(true, true), bitty(Some(false))), "as drawn");
    assert!(puppeted(with_bitty(true, true), bitty(Some(true))), "the screen");
    let _rt = draw(with_bitty(false, true), look(bitty(None)));
    assert_eq!(faces::drawn_pet(SLOT).as_deref(), Some("bitty"), "a sheet not measured is drawn as it is");
    assert!(faces::drawn_puppet(SLOT).is_none());
}

#[test]
fn bounces_as_it_works_as_a_bot_does_where_a_pet_as_drawn_moves_in_its_own_drawings() {
    // A puppet's motion is a bot's rig, at work as its teammate is; a pet as drawn has none, its rows are its motion.
    let rt = draw(with_bitty(true, true), look(bitty(None)));
    assert_eq!(rt.state.bot_stage.desired_activity(&format!("{SLOT}#puppet")), Some(FaceActivity::Working));
    let drawn = draw(with_bitty(true, true), look(bitty(Some(false))));
    assert_eq!(drawn.state.bot_stage.desired_activity(&format!("{SLOT}#puppet")), None);
    assert_eq!(drawn.state.bot_stage.slot_count(), 0);
}

// ---- the drawing is laid where Electron's transform puts it ----

#[test]
fn the_drawings_element_lands_where_the_transform_puts_its_corners_at_every_pose() {
    let place = |m: [f64; 6], x: f64, y: f64| (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]);
    // At rest, the box's centre is the face's (its canvas half as large again and raised, as Electron's margins put it).
    let rest = locust_core::bot_rig::rest_pose(locust_core::bot_rig::BotAvatarState::Default);
    let centre = place(face_matrix(rest, 44.0, None), 0.0, 0.0);
    assert!((centre.0 - 22.0).abs() < 1e-9 && (centre.1 - 22.0).abs() < 1e-9, "{centre:?}");
    for p in &PET_PUPPETS {
        let to = crop_in_box(p);
        for (roll, sx, sy, y) in [(0.0, 1.0, 1.0, 0.0), (0.2, 1.2, 0.8, -10.0), (-0.3, 0.9, 1.1, 2.0)] {
            let mut pose = locust_core::bot_rig::rest_pose(locust_core::bot_rig::BotAvatarState::Default);
            pose.roll = roll;
            pose.sx = sx;
            pose.sy = sy;
            pose.y = y;
            let m = face_matrix(pose, 44.0, locust_core::pet_puppets::puppet_feet(p));
            let ([x, y, w, h], [ox, oy], turn) = drawing_place(m, pose, to);
            // The element's corner, turned `turn` about its origin point.
            let (px, py) = (f64::from(x + ox * w), f64::from(y + oy * h));
            let corner = |cx: f64, cy: f64| {
                let (dx, dy) = (cx - px, cy - py);
                let (sin, cos) = f64::from(turn).sin_cos();
                (px + dx * cos - dy * sin, py + dx * sin + dy * cos)
            };
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                let got = corner(f64::from(x) + u * f64::from(w), f64::from(y) + v * f64::from(h));
                let want = place(m, to.x + u * to.w, to.y + v * to.h);
                assert!((got.0 - want.0).abs() < 0.01 && (got.1 - want.1).abs() < 0.01, "{} {u},{v}: {got:?} vs {want:?}", p.id);
            }
        }
    }
}

#[test]
fn the_cut_is_whole_pixels_inside_its_drawing_and_above_its_neck_holding_all_it_keeps() {
    for p in &PET_PUPPETS {
        let [x, y, w, h] = rest_crop(p);
        let (row, column) = p.rest;
        let (cx, cy) = (column as u32 * p.frame_width, row as u32 * p.frame_height);
        assert!(x >= cx && x + w <= cx + p.frame_width, "{}", p.id);
        assert!(y >= cy && y + h <= cy + p.neck.unwrap_or(p.frame_height), "{}", p.id);
        assert!(x <= cx + p.paint[0] && x + w >= cx + p.paint[2], "{}", p.id);
        assert!(y <= cy + p.paint[1] && y + h >= cy + p.paint[3], "{}", p.id);
    }
}

// ---- its teammate's look ----

fn wren(avatar: Value) -> Value {
    json!({"teammateId":"tm_bitty", "name":"Bitty", "hue":"lime", "role":"Docs & QA", "createdAt":"2026-10-05T00:00:00.000Z", "avatar": avatar})
}

fn open(s: State, pet: Value) -> rune::Runtime<State> {
    let mut s = s;
    let avatar = json!({"headwear":0,"accessory":0,"mouth":0,"bot":{"shape":"droid","face":"eyes"},"pet":pet});
    s.people_panel.team = json!({"teammates": [wren(avatar)]});
    crate::people_panel::edit(&mut s, Some("tm_bitty"));
    let mut rt = rune::Runtime::new(s, |s, _| crate::people_panel::edit_form(s, &crate::Palette::new(true)), (1100.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn offers_face(rt: &rune::Runtime<State>) -> bool {
    rt.find("team-look-face").is_some()
}

#[test]
fn offers_it_as_drawn_or_the_screen_the_screen_until_asked_otherwise() {
    use crate::look_picker::tests::{checked_in, radios_in};
    let rt = open(with_bitty(true, true), json!({"source":"gallery","id":"bitty"}));
    assert!(offers_face(&rt), "aria-label=\"Face\"");
    let names: Vec<String> = radios_in(&rt, "team-look-face").iter().map(|r| r["name"].as_str().unwrap_or("").to_owned()).collect();
    assert_eq!(names, ["As drawn", "Screen"]);
    assert!(rt.accessibility_json().to_string().contains("As its maker drew it."), "its title");
    assert_eq!(checked_in(&rt, "team-look-face"), ["Screen"]);
    let drawn = open(with_bitty(true, true), json!({"source":"gallery","id":"bitty","screen":false}));
    assert_eq!(checked_in(&drawn, "team-look-face"), ["As drawn"]);
}

#[test]
fn offers_nothing_for_a_pet_whose_sheet_was_not_measured() {
    let rt = open(with_bitty(false, true), json!({"source":"gallery","id":"bitty"}));
    assert!(!offers_face(&rt));
}
