//! Electron's `a-teammate-can-be-a-pet.test.tsx`, "a teammate wearing a pet": the four cases, drawn through the live face.

use super::*;
use crate::look::bot_stage::PresenceDot;
use crate::look::faces::{self, BotFaceOpts};
use locust_core::native::FaceLook;
use std::sync::Arc;

const SLOT: &str = "pet-test";

fn cat() -> PetRef {
    PetRef { source: PetSource::Bundled, id: "hoodie-cat".into(), screen: None }
}

fn look(screen: Option<bool>) -> FaceLook {
    FaceLook { hue: "lime".into(), parts: Some([1, 2, 3]), bot: Some(("droid".into(), "eyes".into())), bot_screen: screen, pet: Some(cat()) }
}

/// A state whose cat's sheet is cut and ready: a 16x22 sheet, eleven rows of 2x2 cells.
fn with_sheet(sheet: Option<()>) -> State {
    let mut s = State::default();
    s.settings_panel.settings.terminal_faces = true;
    let entry = match sheet {
        Some(()) => {
            let cache = rune::ImageCache::new(Arc::new(|| {}));
            let rgba = vec![200u8; 16 * 22 * 4];
            let cells = cut_sheet(16, 22, &rgba, 11).into_iter().map(|(at, w, h, px)| (at, cache.rgba(w, h, px))).collect();
            Sheet::Ready { cells, rows: 11 }
        }
        None => Sheet::Missing,
    };
    s.pet_faces.sheets.borrow_mut().insert("bundled/hoodie-cat".into(), entry);
    s
}

thread_local! {
    static DRAWING: std::cell::RefCell<Option<(FaceLook, FaceActivity, Option<PresenceDot>)>> = const { std::cell::RefCell::new(None) };
}

fn draw(s: State, look: FaceLook, activity: FaceActivity, presence: Option<PresenceDot>) -> rune::Runtime<State> {
    DRAWING.with(|d| *d.borrow_mut() = Some((look, activity, presence)));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| {
            let (look, activity, presence) = DRAWING.with(|d| d.borrow().clone()).unwrap();
            faces::bot_face_with_look(
                s,
                SLOT,
                "tm_pet",
                "Mochi",
                Some(&look),
                34.0,
                BotFaceOpts { activity, presence, ..Default::default() },
                &crate::look::controls::Look::new(true),
            )
        },
        (100.0, 100.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

#[test]
fn draws_the_pet_where_the_bot_was_saying_what_it_is_doing() {
    let rt = draw(with_sheet(Some(())), look(None), FaceActivity::Working, None);
    assert_eq!(faces::drawn_pet(SLOT).as_deref(), Some("hoodie-cat"));
    // Working plays the running row (data-pet-state="running").
    assert_eq!(pet_state_for(FaceActivity::Working), PetState::Running);
    assert!(rt.state.pet_faces.clocks.borrow().get(SLOT).is_some_and(|(state, _)| *state == PetState::Running));
}

#[test]
fn keeps_a_bots_marks_the_waiting_ring_and_the_presence_dot() {
    let _rt = draw(with_sheet(Some(())), look(None), FaceActivity::Waiting, Some(PresenceDot::Working));
    assert_eq!(faces::drawn_pet(SLOT).as_deref(), Some("hoodie-cat"));
    let bare = crate::look::bots::pet_marks_shape(34.0, false, None, false);
    let ring = crate::look::bots::pet_marks_shape(34.0, true, None, false);
    let dot = crate::look::bots::pet_marks_shape(34.0, false, Some(PresenceDot::Working), false);
    assert!(bare.is_empty() && !ring.is_empty() && !dot.is_empty() && ring != dot);
    assert_eq!(pet_state_for(FaceActivity::Waiting), PetState::Waiting);
}

#[test]
fn never_wears_a_screen_or_code_eyes_even_on_a_shape_that_suits_one_with_terminal_faces_on() {
    let _rt = draw(with_sheet(Some(())), look(Some(true)), FaceActivity::Thinking, None);
    assert_eq!(faces::drawn_pet(SLOT).as_deref(), Some("hoodie-cat"), "the pet, never the droid's screen");
}

#[test]
fn shows_its_bot_when_the_pet_cannot_be_drawn_never_an_empty_box() {
    let _rt = draw(with_sheet(None), look(None), FaceActivity::Idle, None);
    assert_eq!(faces::drawn_pet(SLOT), None);
    assert!(faces::drawn_shape(SLOT).is_some_and(|shape| !shape.is_empty()), "the bot is drawn");
}

#[test]
fn a_pet_with_no_sheet_yet_is_asked_for_and_its_bot_stands_in_meanwhile() {
    let s = State::default();
    let rt = draw(s, look(None), FaceActivity::Idle, None);
    assert_eq!(faces::drawn_pet(SLOT), None);
    assert_eq!(rt.state.pet_faces.wanted.borrow().iter().map(|p| p.id.clone()).collect::<Vec<_>>(), vec!["hoodie-cat".to_owned()]);
}

#[test]
fn a_sheet_is_cut_eight_across_and_its_rows_down() {
    // A 16x6 sheet of three rows: each cell 2x2, its pixels the ones in its square.
    let mut rgba = Vec::new();
    for y in 0..6u8 {
        for x in 0..16u8 {
            rgba.extend_from_slice(&[x, y, 0, 255]);
        }
    }
    let cells = cut_sheet(16, 6, &rgba, 3);
    assert_eq!(cells.len(), 24);
    let (at, w, h, px) = &cells[8 + 3];
    assert_eq!((*at, *w, *h), ((1, 3), 2, 2));
    assert_eq!(px, &vec![6, 2, 0, 255, 7, 2, 0, 255, 6, 3, 0, 255, 7, 3, 0, 255]);
}
