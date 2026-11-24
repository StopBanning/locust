//! THE WORKING SPARK (`main/the-working-spark-is-claude-codes.test.ts`, 5 cases). Electron reads the stylesheet; these
//! draw the spark and read what the window shows at each moment, and draw the screens that wear it (Compare's working
//! column and its judge reading, a benchmark rerun, the post-edit check).
use super::*;
use rune::{Env, Runtime};

/// Electron's frames with the text-form selector taken off, as its test compares them.
const ELECTRON_FRAMES: [&str; 12] = ["·", "✢", "✳", "✶", "✻", "✽", "✽", "✻", "✶", "✳", "✢", "·"];

/// The glyph lit `seconds` into the turn, as the drawn layers' cycles have it.
fn frame_at(seconds: f64) -> Option<&'static str> {
    let mut lit = layers().filter(|(_, cycle)| cycle.sample(seconds) > 0.5).map(|(glyph, _)| glyph);
    let first = lit.next();
    if lit.next().is_some() { None } else { first }
}

fn bare(glyph: &str) -> String {
    glyph.replace('\u{FE0E}', "")
}

fn spark_window(reduced: bool) -> Runtime<bool> {
    fn view(_: &bool, _: &mut rune::Cx<bool>) -> El {
        let l = crate::look::controls::Look::new(true);
        row().padding(20.0).child(working_spark(&l, "spark"))
    }
    let mut rt = Runtime::new(reduced, view, (100.0, 100.0), Env::new(true, 1.0));
    rt.set_system_reduced_motion(reduced);
    rt.frame(0.0);
    rt
}

/// The glyphs drawn at full strength now.
fn lit<S: 'static>(rt: &Runtime<S>, within: &str) -> Vec<String> {
    let all = rt.elements();
    let Some(root) = all.iter().position(|e| e.key.as_deref() == Some(within)) else { return Vec::new() };
    let inside = |mut at: usize| loop {
        if at == root {
            return true;
        }
        match all[at].parent {
            Some(up) => at = up,
            None => return false,
        }
    };
    all.iter()
        .enumerate()
        .filter(|(at, e)| e.text.is_some() && e.opacity > 0.5 && inside(*at))
        .map(|(_, e)| bare(e.text.as_deref().unwrap()))
        .collect()
}

#[test]
fn turns_through_claude_codes_glyphs_and_back_one_every_120_ms() {
    // case: turns through Claude Code's glyphs and back, one every 120 ms
    assert_eq!(PERIOD_SECONDS, 1.44, "twelve slots of 120 ms");
    for turn in 0..2 {
        for (slot, expected) in ELECTRON_FRAMES.iter().enumerate() {
            let at = (turn * 12 + slot) as f64 * 0.12 + 0.06;
            assert_eq!(frame_at(at).map(bare).as_deref(), Some(*expected), "slot {slot} of turn {turn}, at {at:.2} s");
        }
    }
    // Held for its whole slot (`steps(1, end)`), not faded between.
    assert_eq!(frame_at(0.12 * 4.0 + 0.01).map(bare).as_deref(), Some("✻"));
    assert_eq!(frame_at(0.12 * 5.0 - 0.01).map(bare).as_deref(), Some("✻"));
    // And the window draws it: one glyph lit at a time, the same one the clock says.
    let rt = spark_window(false);
    assert_eq!(lit(&rt, "spark").len(), 1, "one glyph at a time: {:?}", lit(&rt, "spark"));
}

#[test]
fn asks_for_the_text_form_of_every_dingbat_or_windows_draws_as_an_emoji() {
    // case: asks for the text form of every dingbat, or Windows draws ✳ as an emoji
    for frame in FRAMES {
        if frame == "\u{00B7}" {
            continue;
        }
        assert!(frame.ends_with('\u{FE0E}'), "{frame:?}");
    }
    assert_eq!(STILL, "\u{273B}\u{FE0E}");
    let rt = spark_window(false);
    let drawn: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert_eq!(drawn.len(), 6, "the six glyphs: {drawn:?}");
    assert!(drawn.iter().filter(|g| g.as_str() != "\u{00B7}").all(|g| g.ends_with('\u{FE0E}')), "{drawn:?}");
}

#[test]
fn is_monochrome_from_a_symbol_face_named_ahead_of_any_emoji_face() {
    // case: is monochrome, from a symbol face named ahead of any emoji face
    // `color: var(--lc-text-primary)`, in both themes. The face is Rune's symbol fallback, which names its symbol
    // faces before the emoji one on every platform, and the text-form selector keeps a dingbat out of the emoji face.
    for dark in [true, false] {
        let l = crate::look::controls::Look::new(dark);
        let primary = l.c("text-primary");
        let mut rt = Runtime::new(dark, |dark: &bool, _| {
            row().padding(20.0).child(working_spark(&crate::look::controls::Look::new(*dark), "spark"))
        }, (100.0, 100.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let inks: Vec<_> = rt.elements().into_iter().filter(|e| e.text.is_some()).map(|e| e.text_color).collect();
        assert_eq!(inks.len(), 6);
        assert!(inks.iter().all(|ink| *ink == Some(primary)), "dark {dark}: {inks:?}");
    }
}

#[test]
fn keeps_the_finished_dots_footprint_so_a_run_starting_never_moves_the_row() {
    // case: keeps the finished dot's footprint, so a run starting never moves the row
    let rt = spark_window(false);
    let spark = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("spark")).unwrap();
    assert_eq!((spark.rect[2], spark.rect[3]), (6.0, 6.0), "`.lc-spark`: 6 by 6");
    // The glyph spills out of it (`overflow: visible`): each glyph's layer is wider than the footprint, centred on
    // it, and nothing cuts it to the 6 points.
    let centre = |r: [f32; 4]| (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    let layers: Vec<_> = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("spark-glyph-"))).collect();
    assert_eq!(layers.len(), 6);
    for layer in layers {
        assert!(layer.rect[2] > 6.0 && layer.rect[3] > 6.0, "{:?}", layer.rect);
        assert_eq!(layer.visible, layer.rect, "the glyph is not cut to the footprint");
        let (lx, ly) = centre(layer.rect);
        let (sx, sy) = centre(spark.rect);
        // `translate: -50% -52%`: centred across, a hair above centre down.
        assert!((lx - sx).abs() < 0.01, "{lx} {sx}");
        assert!(ly < sy && sy - ly < 0.5, "{ly} {sy}");
    }
}

#[test]
fn stands_still_for_a_person_who_asked_for_less_motion() {
    // case: stands still for a person who asked for less motion
    let mut rt = spark_window(true);
    for _ in 0..13 {
        assert_eq!(lit(&rt, "spark"), ["✻"], "the spark at rest");
        rt.frame(0.12);
    }
    assert_eq!(rt.wake_deadline(), None, "and nothing wakes the window for it");
}

#[test]
fn the_window_wakes_for_a_spark_and_never_when_nothing_works() {
    let rt = spark_window(false);
    let wake = rt.wake_deadline().expect("a drawn spark wakes the window on its own clock");
    assert!(wake <= 0.12, "within a slot: {wake}");
    let mut idle = Runtime::new((), |_: &(), _| row().padding(20.0).child(text("nothing works")), (100.0, 100.0), Env::new(true, 1.0));
    idle.frame(0.0);
    assert_eq!(idle.wake_deadline(), None);
}

// --- where the spark is worn ---------------------------------------------------------------------------------------

#[test]
fn the_post_edit_check_wears_the_spark_while_it_runs() {
    let mut rt = Runtime::new((), |_: &(), _| crate::edit_check_card::running_card("cargo test", &Palette::new(true)), (600.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    assert!(all.iter().any(|e| e.key.as_deref() == Some("edit-check-spark")), "EditCheckCard.tsx: `<WorkingSpark />` then the words");
    assert!(all.iter().any(|e| e.text.as_deref() == Some("Running the check cargo test after this turn…")));
    assert!(rt.wake_deadline().is_some());
}

fn route(label: &str) -> locust_core::compare::CompareRoute {
    let mut route = locust_core::compare::CompareRoute::new("codex", "synthetic-model");
    route.label = Some(label.into());
    route
}

fn slot(id: locust_core::compare::CompareSlotId, label: &str, mission: &str) -> locust_core::compare::CompareSlot {
    locust_core::compare::CompareSlot { slot: id, route: route(label), mission_ids: vec![mission.into()], refused: None, retried: vec![], folder: None }
}

fn comparison(id: &str, reruns: Vec<String>, rerun_of: Option<String>) -> locust_core::compare::Compare {
    use locust_core::compare::CompareSlotId::{A, B};
    locust_core::compare::Compare {
        compare_id: id.into(),
        teammate_id: None,
        prompt: "Which sort is stable?".into(),
        created_at: "2026-11-20T12:00:00.000Z".into(),
        slots: vec![slot(A, "model-alpha", &format!("{id}_a")), slot(B, "model-beta", &format!("{id}_b"))],
        changes: false,
        changes_in: None,
        blind: false,
        kept: None,
        judge: Some(locust_core::compare::CompareJudge {
            route: route("judge-model"),
            mission_ids: vec![format!("{id}_judge")],
            criteria: None,
            panel: vec![],
            panel_mission_ids: vec![],
        }),
        base: None,
        rerun_of,
        reruns,
        settled: false,
        point_drafts: vec![],
    }
}

fn snapshot(running: bool) -> locust_core::ledger::Snapshot {
    locust_core::ledger::Snapshot { running, ..Default::default() }
}

/// A benchmark (one comparison run again once) whose column A and judge are `working`, and a rerun `going`.
fn benchmark(working: bool, going: bool) -> State {
    let mut s = State::default();
    s.demo = true;
    s.compare.compares = vec![comparison("cmp_origin", vec!["cmp_rerun".into()], None), comparison("cmp_rerun", vec![], Some("cmp_origin".into()))];
    s.compare.active_id = Some("cmp_origin".into());
    s.compare.snapshots.insert("cmp_origin_a".into(), snapshot(working));
    s.compare.snapshots.insert("cmp_origin_b".into(), snapshot(false));
    s.compare.snapshots.insert("cmp_origin_judge".into(), snapshot(working));
    if going {
        *s.compare.rerun_progress.lock().unwrap() = vec![locust_core::compare::RerunProgress {
            origin_id: "cmp_origin".into(),
            run: 2,
            of: 3,
            step: locust_core::compare::RerunStep::Answering,
            compare_id: None,
        }];
    }
    s
}

fn sparks<S: 'static>(rt: &Runtime<S>) -> Vec<String> {
    let mut keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.ends_with("-spark")).collect();
    keys.sort();
    keys.dedup();
    keys
}

#[test]
fn a_comparisons_working_column_its_judge_reading_and_a_rerun_wear_the_spark() {
    let draw = |s: State| {
        let mut rt = Runtime::new(s, |s, _| crate::compare_view::view(s, &Palette::new(true)), (1280.0, 2400.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    };
    let rt = draw(benchmark(true, true));
    // CompareView.tsx: the column's state (`{column.running && <WorkingSpark />}{column.state}`), the judge's view
    // (`{judge.running && <WorkingSpark />}`) and CompareBenchmark.tsx's progress line.
    assert_eq!(sparks(&rt), ["benchmark-progress-spark", "compare-judge-spark", "compare-state-a-spark"]);
    let words: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(words.iter().any(|w| w.ends_with(" · reading")), "{words:?}");
    assert!(rt.wake_deadline().is_some());
    // Nothing working: no spark, and nothing wakes the window for one.
    let rt = draw(benchmark(false, false));
    assert!(sparks(&rt).is_empty(), "{:?}", sparks(&rt));
    assert_eq!(rt.wake_deadline(), None);
}

#[test]
fn a_benchmark_running_again_wears_the_spark_in_its_chip() {
    let draw = |s: State| {
        let mut rt = Runtime::new(s, |s, _| crate::benchmarks_panel::view(s, &Palette::new(true)), (1200.0, 900.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    };
    let rt = draw(benchmark(false, true));
    assert_eq!(sparks(&rt), ["benchmark-chip-cmp_origin-spark"]);
    // BenchmarksScreen.tsx's words beside it.
    assert!(rt.elements().into_iter().filter_map(|e| e.text).any(|w| w == "running again, 2 of 3 · answering"));
    let rt = draw(benchmark(false, false));
    assert!(sparks(&rt).is_empty());
    assert_eq!(rt.wake_deadline(), None);
}
