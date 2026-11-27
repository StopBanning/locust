//! THE CHIP IS THE RUN (run_effort.rs, App.tsx `runEffortFor`): the level the model chip shows, drawn headless through
//! `thread_view`. The run's side of each case, what the runtime's command carries, is in
//! core/src/native/codex_mission_tests.rs; the rule's own cases are in core/src/run_effort_tests.rs.
use super::*;
use locust_core::engine::{Model, Models, Teammate};
use rune::{Env, Runtime};

/// The words the model chip reads, for a Claude teammate on `model` (listing `levels`) with `saved` as its own level.
fn chip_reads(model: &str, name: &str, levels: &[&str], saved: Option<&str>) -> String {
    chip_reads_on("claude", model, name, levels, saved, None)
}

/// The same on `runtime`, with `picked` as the level picked in the box for this turn (the window sends it as the turn's).
fn chip_reads_on(runtime: &str, model: &str, name: &str, levels: &[&str], saved: Option<&str>, picked: Option<&str>) -> String {
    let mut s = State::default();
    s.teammates.push(Teammate {
        id: "tm_1".into(),
        name: "Wren".into(),
        role: "Code".into(),
        runtime: Some(runtime.into()),
        model: Some(model.into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.selected = 0;
    s.conversations.insert("tm_1".into(), Conversation::default());
    s.models.insert(
        "tm_1".into(),
        vec![Models {
            runtime: runtime.into(),
            model: Some(model.into()),
            effort: saved.map(str::to_owned),
            teammate: None,
            models: vec![Model { id: model.into(), name: name.into(), efforts: levels.iter().map(|l| (*l).to_owned()).collect() }],
        }],
    );
    if let Some(level) = picked {
        s.picks.insert("tm_1".into(), TurnPick { model: model.into(), effort: Some(level.into()), runtime: None, account: None });
    }
    fn view(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view, (1000.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let ([x, y, w, h], _) = rt.presented("model-chip").expect("the model chip is drawn");
    rt.elements()
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// case: agree when nobody has chosen a level
// The chip's half: status.test.ts expects `medium` for a model listing low, medium and high, and the run is sent it
// (codex_mission_tests.rs a_teammate_with_no_level_chosen_is_sent_the_level_its_chip_shows).
#[test]
fn a_teammate_with_no_level_chosen_shows_the_level_its_run_is_sent() {
    assert_eq!(chip_reads("sonnet", "Sonnet 5.5", &["low", "medium", "high"], None), "Sonnet 5.5 · Medium");
}

// case: agree when someone has
// The chip's half: the level picked is the level shown, and the window sends it as the turn's (status.test.ts `high`).
#[test]
fn a_level_someone_chose_is_the_level_the_chip_shows() {
    assert_eq!(chip_reads_on("claude", "sonnet", "Sonnet 5.5", &["low", "medium", "high"], None, Some("high")), "Sonnet 5.5 · High, this turn");
    // And a teammate's own saved level, which send_from reads from the team when nothing is picked.
    assert_eq!(chip_reads("sonnet", "Sonnet 5.5", &["low", "medium", "high"], Some("high")), "Sonnet 5.5 · High");
}

// case: agree on a Cursor family, whose id already names a level
// The chip's half: a family standing on its low variant reads Low, not Medium (status.test.ts), and one on its
// `-medium` variant reads Medium.
#[test]
fn a_cursor_family_shows_the_level_its_id_names_when_nobody_chose_one() {
    assert_eq!(chip_reads_on("cursor", "grok-4.7-low", "Grok 4.7", &["low", "medium", "high"], None, None), "Grok 4.7 · Low");
    assert_eq!(
        chip_reads_on("cursor", "claude-opus-5-5-medium", "Claude Opus 5.5 1M", &["low", "medium", "high", "max"], None, None),
        "Claude Opus 5.5 1M · Medium"
    );
    assert_eq!(chip_reads_on("cursor", "grok-4.7-low", "Grok 4.7", &["low", "medium", "high"], None, Some("high")), "Grok 4.7 · High, this turn");
}

// case: agree on a model with no levels: both say nothing, and nothing is sent
// The chip's half: no level, which the chip words "Fixed" (Composer.tsx effortIsGenuinelyFixed).
#[test]
fn a_model_that_lists_no_levels_shows_no_level() {
    assert_eq!(chip_reads_on("copilot", "auto", "Auto", &[], None, None), "Auto · Fixed");
}
