use super::*;
use crate::pet_faces::Sheet;
use locust_core::bot_spec::{PetRef, PetSource};
use std::cell::RefCell;

const SLOT: &str = "buddy-screen-test";

fn look(id: &str, screen: Option<bool>) -> FaceLook {
    FaceLook {
        hue: "blue".into(),
        parts: Some([0, 0, 0]),
        bot: Some(("circle".into(), "eyes".into())),
        bot_screen: None,
        pet: Some(PetRef {
            source: PetSource::Codex,
            id: id.into(),
            screen,
        }),
    }
}

thread_local! {
    static DRAW: RefCell<Option<(FaceLook, FaceActivity, f32, BotMotionLevel)>> = const { RefCell::new(None) };
}

fn view(s: &crate::State, _: &mut rune::Cx<crate::State>) -> El {
    let (look, activity, size, motion) = DRAW.with(|d| d.borrow().clone().unwrap());
    let options = BotFaceOpts {
        activity,
        motion,
        a11y_label: Some("Buddy"),
        ..Default::default()
    };
    let l = super::super::controls::Look::new(true);
    row().child(bot_face_with_look(s, SLOT, "tm_buddy", "Buddy", Some(&look), size, options, &l))
}

fn draw(
    mut s: crate::State,
    look: FaceLook,
    terminal: bool,
    activity: FaceActivity,
    size: f32,
    motion: BotMotionLevel,
) -> rune::Runtime<crate::State> {
    s.settings_panel.settings.terminal_faces = terminal;
    DRAW.with(|d| *d.borrow_mut() = Some((look, activity, size, motion)));
    let mut rt = rune::Runtime::new(s, view, (200.0, 200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn drawn() -> String {
    format!("{:?}", drawn_shape(SLOT).expect("Buddy's drawn rig"))
}

fn screened() -> bool {
    // The dark glass fill in bots::draw_screen_ground, absent in Buddy's drawn face.
    drawn().contains(&format!(
        "color: {:?}",
        super::super::bots::color_from_rgb_f64([12.0, 16.0, 22.0], 1.0)
    ))
}

// case: wears the screen while Terminal faces is on, saying what he is doing
#[test]
fn buddy_wears_his_screen_and_shows_activity_with_his_name() {
    let rt = draw(
        crate::State::default(),
        look("codex-buddy", None),
        true,
        FaceActivity::Working,
        44.0,
        BotMotionLevel::Full,
    );
    assert!(rt.presented("bot-buddy-screen-test").is_some());
    assert!(
        rt.accessibility_json()
            .to_string()
            .contains("\"name\":\"Buddy\"")
    );
    assert!(screened());
    let working = drawn();
    let _rt = draw(
        crate::State::default(),
        look("codex-buddy", None),
        true,
        FaceActivity::Blocked,
        44.0,
        BotMotionLevel::Full,
    );
    assert!(screened());
    assert_ne!(
        working,
        drawn(),
        "his screen responds to the requested activity"
    );
}

// case: lifts everything he has, as the face you talk to and beside a name: all of his lifts are standing ones
#[test]
fn buddy_works_all_four_standing_lifts_both_sizes() {
    for (size, motion) in [(96.0, BotMotionLevel::Full), (34.0, BotMotionLevel::Subtle)] {
        let rt = draw(
            crate::State::default(),
            look("codex-buddy", None),
            true,
            FaceActivity::Working,
            size,
            motion,
        );
        assert_eq!(
            rt.presented("bot-buddy-screen-test").unwrap().0[2..],
            [size, size]
        );
        assert!(screened());
        let ask = locust_core::buddy_moves::PetScreenAsk::from_activity(
            "working",
            FaceActivity::Working,
            None,
            motion,
        );
        let workout = ask.r#move;
        assert_eq!(workout.name, "workout");
        assert_eq!(workout.lifts, ["press", "curls", "alternating", "hammer"]);
        for t in (0..workout.ms as usize).step_by(20) {
            assert!(workout.at(t as f64).pose.dip < 3.5);
        }
    }
}

// case: is drawn as his maker drew him with Terminal faces off, or when his teammate asks for that
#[test]
fn buddy_is_as_drawn_when_terminal_faces_is_off_or_he_asks() {
    for (terminal, screen, want) in [
        (false, None, false),
        (true, Some(false), false),
        (true, Some(true), true),
    ] {
        let _rt = draw(
            crate::State::default(),
            look("codex-buddy", screen),
            terminal,
            FaceActivity::Working,
            44.0,
            BotMotionLevel::Full,
        );
        assert_eq!(screened(), want);
    }
}

// case: leaves every other pet as its maker drew it, even given a table
#[test]
fn another_pet_does_not_use_the_buddy_screen_table() {
    let s = crate::State::default();
    let cache = rune::ImageCache::new(std::sync::Arc::new(|| {}));
    let (w, h) = (CODEX_BUDDY.frame_width, CODEX_BUDDY.frame_height);
    let cells = std::collections::HashMap::from([(
        (0, 0),
        cache.rgba(w, h, vec![200; (w * h * 4) as usize]),
    )]);
    s.pet_faces.sheets.borrow_mut().insert(
        "codex/other-cat".into(),
        Sheet::Ready {
            cells,
            rows: 9,
            puppet: None,
        },
    );
    let rt = draw(
        s,
        look("other-cat", None),
        true,
        FaceActivity::Working,
        44.0,
        BotMotionLevel::Full,
    );
    assert!(rt.presented("pet-buddy-screen-test").is_some());
    assert_eq!(drawn_pet(SLOT).as_deref(), Some("other-cat"));
    assert_eq!(drawn_puppet(SLOT), None);
    assert!(rt.presented("bot-buddy-screen-test").is_none());
}
