//! every-teammate-is-a-bot.test.tsx, "what a teammate bot draws": Electron reads TeammateBot's markup. Here the same three
//! cases read the real face in a window: what it is drawn as, what the rig is told, and its accessibility tree.

use super::*;
use crate::look::bot_stage::PresenceDot;
use locust_core::bot_spec::seed_avatar;
use serde_json::Value;

/// `{ ...seedAvatar('tm_atlas'), bot: { shape: 'droid', face: 'eyes' } }` in `hue`.
fn atlas(hue: &str) -> FaceLook {
    let a = seed_avatar("tm_atlas");
    FaceLook {
        hue: hue.to_owned(),
        parts: Some([a.headwear, a.accessory, a.mouth]),
        bot: Some(("droid".to_owned(), "eyes".to_owned())),
        bot_screen: None,
        pet: None,
    }
}

thread_local! {
    /// The face the test window draws: its hue, size and options (a view is a plain `fn`, so it reads them here).
    static CASE: std::cell::Cell<(&'static str, f32, FaceActivity, Option<PresenceDot>, Option<&'static str>)> =
        const { std::cell::Cell::new(("teal", 26.0, FaceActivity::Idle, None, None)) };
}

fn view(s: &crate::State, _: &mut rune::Cx<crate::State>) -> El {
    let (hue, size, activity, presence, a11y_label) = CASE.with(std::cell::Cell::get);
    let l = super::super::controls::Look::new(true);
    let opts = BotFaceOpts { activity, presence, a11y_label, ..Default::default() };
    row().child(bot_face_with_look(s, "atlas", "tm_atlas", "Atlas", Some(&atlas(hue)), size, opts, &l))
}

fn draw(hue: &'static str, size: f32, activity: FaceActivity, presence: Option<PresenceDot>, name: Option<&'static str>) -> rune::Runtime<crate::State> {
    CASE.with(|case| case.set((hue, size, activity, presence, name)));
    let mut rt = rune::Runtime::new(crate::State::default(), view, (200.0, 200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| node(child, key))
}

/// The amber ring (`.lc-bot__ring`) is the one stroke drawn under the body, so it comes first.
fn ringed(key: &str) -> bool {
    format!("{:?}", drawn_shape(key).expect("drawn")).starts_with("Shape { ops: [Stroke")
}

#[test]
fn keeps_the_hooks_the_face_had_the_activity_the_teammate_and_its_own_shape() {
    let rt = draw("teal", 26.0, FaceActivity::Working, None, None);
    // data-activity="working"
    assert_eq!(rt.state.bot_stage.desired_activity("atlas"), Some(FaceActivity::Working));
    // data-bot="droid"
    assert_eq!(drawn_look("atlas"), Some(("droid".to_owned(), "teal".to_owned())));
    // data-teammate="tm_atlas": the face is the teammate's own, its rig seeded from the id.
    assert_eq!(rt.state.bot_stage.seed("atlas"), Some(locust_core::bot_spec::seed_of("tm_atlas")));
    // aria-hidden="true": drawn, but not in the accessibility tree.
    assert!(rt.presented("bot-atlas").is_some());
    assert!(node(&rt.accessibility_json(), "bot-atlas").is_none());
}

#[test]
fn wears_locusts_marks_the_amber_ring_and_dot_of_a_teammate_waiting_on_you() {
    let _rt = draw("blue", 32.0, FaceActivity::Waiting, Some(PresenceDot::Waiting), None);
    assert!(ringed("atlas"), "lc-bot__ring");
    assert_eq!(drawn_presence("atlas"), Some(Some(PresenceDot::Waiting)), "lc-presence--amber");
    let _rt = draw("blue", 32.0, FaceActivity::Idle, None, None);
    assert!(!ringed("atlas"), "no ring at rest");
}

// TeammateBot.tsx 445: the ring is the waiting activity's; the amber dot alone does not bring it.
#[test]
fn the_ring_is_the_waiting_activitys_not_the_dots() {
    let _rt = draw("blue", 32.0, FaceActivity::Waiting, None, None);
    assert!(ringed("atlas"));
    let _rt = draw("blue", 32.0, FaceActivity::Working, Some(PresenceDot::Waiting), None);
    assert!(!ringed("atlas"));
}

#[test]
fn answers_to_the_name_where_it_stands_in_for_one() {
    let rt = draw("rose", 32.0, FaceActivity::Idle, None, Some("Atlas"));
    let tree = rt.accessibility_json();
    let face = node(&tree, "bot-atlas").expect("in the accessibility tree");
    assert_eq!(face["name"], "Atlas");
    assert_eq!(face["role"], "image");
}
