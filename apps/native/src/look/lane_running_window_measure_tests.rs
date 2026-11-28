//! speed-3 row 0: how heavy the window is while a lane runs. Colin's window stays open while lanes run, so the state
//! that matters is "a turn running", not "at rest". Two states of the real window (`view` and `poll`), focused, the
//! wide sidebar listing 60 finished conversations and one running, an open conversation of 24 items:
//! - A: a lane running elsewhere. A listed row is running (its band moves); the conversation on screen is idle.
//! - B: the running conversation open, its live line drawn (the sweep), text landing every 100 ms of window time.
//!
//! Frames come on the window's own clock, as its host takes them (`rune::window::pacing`): the next vsync while it
//! animates, else the next wake it asked for but never sooner than a frame interval after the last poll; text that lands
//! while it animates waits for the next vsync, and at rest takes a frame of its own. The wall clock only paces the run
//! (each frame waits for its window time) and times each frame, so `ps` time over wall time is what the window costs:
//! percent of ONE core. Headless: no GPU, present or raster; this is poll, view, layout and instance build, not the
//! whole frame.
use super::*;
use rune::{Env, Runtime};
use std::time::Instant;

const AT: &str = "2026-11-27T05:00:00.000Z";
const FINISHED_ROWS: usize = 60;
const LANDING_EVERY: f64 = 0.1;
/// Text first lands half way between two of the band's 50 ms beats.
const LANDING_PHASE: f64 = 0.025;
/// `rune::window::TIMER_FRAME`: a timer's poll comes at most once a frame interval after the last.
const TIMER_FRAME: f64 = 0.016_667;
const VSYNC: f64 = 1.0 / 60.0;
const CHUNK: &str = " and then the parser reads the next token,";

/// sidebar_shimmer_rest_tests.rs's `listed` (private there).
fn listed(s: &mut State, id: &str, state: &str) {
    s.recent.push(ConversationRow { id: id.into(), title: format!("Conversation {id}"), teammate: "Wren".into(),
        started_by: "native".into(), state: state.into(), updated: AT.into() });
}

/// The conversation on screen before its live reply: a dozen exchanges of the person's words, a tool and a reply in
/// markdown.
fn earlier_items() -> Vec<ledger::Item> {
    let mut items = Vec::new();
    for n in 0..8 {
        items.push(ledger::Item::User(format!("Step {n}: fix the parser where it drops the last token")));
        items.push(ledger::Item::Tool {
            name: "shell".into(),
            status: ledger::Status::Done,
            summary: format!("cargo test parser_{n}"),
            detail: Box::new(ledger::ToolDetail { id: format!("t_{n}"), ..Default::default() }),
        });
        items.push(ledger::Item::Assistant {
            item_id: format!("a_{n}"),
            text: format!(
                "Fixed **step {n}**. The parser now keeps the last token:\n\n- `read_token` stops at the end\n- \
                 `parse` checks the tail\n\n```rust\nfn parse(s: &str) -> Vec<Token> {{ lex(s).collect() }}\n```\n\nAll \
                 tests pass."
            ),
            r#final: true,
        });
    }
    items
}

/// The window: Wren and Codec at rest (their faces still: a moving face adds frames, not commits), the wide sidebar
/// with 60 finished rows and `running_row` running, and conversation m_1 open, running (`live`) or idle.
pub(crate) fn window(running_row: &str, live: bool) -> Runtime<State> {
    let mut s = crate::messages::tests_live_state();
    s.layout_pref = "wide".into();
    let codec = s.ledger.as_ref().unwrap().header.clone();
    let wren = Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: String::new(),
        runtime: Some("codex".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    };
    apply_team(&mut s, vec![wren, codec]);
    for n in 0..FINISHED_ROWS {
        listed(&mut s, &format!("m_done_{n}"), if n % 7 == 0 { "failed" } else { "completed" });
    }
    listed(&mut s, running_row, "running");
    if running_row != "m_1" {
        listed(&mut s, "m_1", "completed");
    }
    let pane = s.ledger.as_mut().unwrap();
    pane.snapshot.tip = "m_1".into();
    let mut items = earlier_items();
    if live {
        items.push(ledger::Item::User("Now tidy the tests".into()));
        items.push(ledger::Item::Assistant { item_id: "a_live".into(), text: "Tidying".into(), r#final: false });
    } else {
        pane.snapshot.running = false;
        pane.snapshot.live = None;
    }
    pane.items = items.clone();
    pane.snapshot.items = items;
    s.window_focused = true;
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.poll = Some(poll);
    rt.frame(0.0);
    rt
}

/// A short chunk lands on the live reply, as a ledger read that found more text puts it (`pane.items` and the
/// snapshot's together).
pub(crate) fn land_text(s: &mut State) {
    let pane = s.ledger.as_mut().expect("the conversation is open");
    for items in [&mut pane.items, &mut pane.snapshot.items] {
        match items.last_mut() {
            Some(ledger::Item::Assistant { text, .. }) => text.push_str(CHUNK),
            other => panic!("the live reply is last: {other:?}"),
        }
    }
}

fn cpu_seconds() -> f64 {
    let out = std::process::Command::new("ps").args(["-o", "cputime=", "-p", &std::process::id().to_string()]).output().unwrap();
    // [[hh:]mm:]ss.cc
    String::from_utf8_lossy(&out.stdout).trim().split(':').fold(0.0, |acc, part| acc * 60.0 + part.parse::<f64>().unwrap_or(0.0))
}

fn quantile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    sorted[((sorted.len() as f64 * q) as usize).min(sorted.len() - 1)]
}

/// When the host frames next: the next vsync while anything animates, else the window's next wake, floored at a frame
/// interval after the last poll. Exactly there: Rune's headless step takes a timer deadline before its target as a step
/// of its own (`frame_split`), so a target past it would poll twice.
pub(crate) fn next_frame(rt: &Runtime<State>, clock: f64, animating: bool) -> f64 {
    if animating {
        clock + VSYNC
    } else {
        rt.wake_deadline().map_or(clock + 0.25, |wake| wake.max(clock + TIMER_FRAME))
    }
}

/// One frame at window time `at`: the frame's wall milliseconds, its commits, and whether it animates on.
pub(crate) fn step(rt: &mut Runtime<State>, clock: &mut f64, at: f64) -> (f64, u64, bool) {
    let commits = rt.stats.commits;
    let t0 = Instant::now();
    let animating = rt.frame(at - *clock);
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    *clock = at;
    (ms, rt.stats.commits - commits, animating)
}

#[derive(Default)]
struct Tally {
    frames: u64,
    commits: u64,
    /// Frames that committed more than once.
    doubled: u64,
    /// Committing frames by what took them: a text landing at rest, a vsync while animating (any landing since the
    /// last frame rides it), a timer wake (the band's or sweep's 50 ms beat, or another wake).
    text: u64,
    vsync: u64,
    vsync_with_text: u64,
    timer: u64,
    commit_ms: Vec<f64>,
    still_ms: Vec<f64>,
    /// Texts Rune shaped (`FrameStats::shaped`) on frames that committed, and on frames that did not.
    commit_shaped: u64,
    still_shaped: u64,
    gaps: std::collections::BTreeMap<u64, u64>,
}

/// Runs the window for `LOCUST_WINDOW_SECS` (default 20) seconds of window time, paced to the wall clock, and prints
/// one line of numbers. `text`: a chunk lands on the live reply every 100 ms (`LOCUST_WINDOW_TEXT=0` turns it off).
fn measure(name: &str, mut rt: Runtime<State>, live: bool) {
    let secs: f64 = std::env::var("LOCUST_WINDOW_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(20.0);
    let text = live && std::env::var("LOCUST_WINDOW_TEXT").map_or(true, |v| v != "0");
    let trace = std::env::var_os("LOCUST_WINDOW_TRACE").is_some();
    let mut clock = 0.0;
    let mut animating = false;
    // Two seconds' warm-up on the window's clock, unpaced.
    while clock < 2.0 {
        let at = next_frame(&rt, clock, animating);
        animating = step(&mut rt, &mut clock, at).2;
    }
    // Guards: the band moves (and in B the sweep), and the window asks for its next beat within 50 ms.
    assert!(crate::sidebar::shimmering(&rt.state), "{name}: a listed row is running and drawn");
    assert_eq!(crate::messages::live_sweeping(&rt.state), live, "{name}: the live line is drawn only in B");
    let ahead = rt.wake_deadline().expect("a moving band asks for its next beat") - clock;
    assert!(ahead <= 0.05 + 1e-6, "{name}: the next wake is {ahead:.3} s away");

    let (cpu0, commits0, clock0, wall0) = (cpu_seconds(), rt.stats.commits, clock, Instant::now());
    let mut next_text = clock0 + LANDING_EVERY + LANDING_PHASE;
    let (mut t, mut landings, mut held, mut clock_drift) = (Tally::default(), 0u64, false, 0u64);
    while clock - clock0 < secs {
        let next = next_frame(&rt, clock, animating);
        // Text that lands while the window animates waits for the next vsync; at rest it takes a frame of its own.
        let mut landing = false;
        while text && next_text <= next {
            if animating {
                held = true;
                rt.update(|s, _| land_text(s));
                next_text += LANDING_EVERY;
                landings += 1;
            } else {
                landing = true;
                break;
            }
        }
        let at = if landing { next_text } else { next };
        let due = wall0 + Duration::from_secs_f64(at - clock0);
        if let Some(wait) = due.checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
        if landing {
            rt.update(|s, _| land_text(s));
            next_text += LANDING_EVERY;
            landings += 1;
        }
        let was_animating = animating;
        *t.gaps.entry(((at - clock) * 1000.0).round() as u64).or_default() += 1;
        let (ms, commits, now_animating) = step(&mut rt, &mut clock, at);
        animating = now_animating;
        // Frames whose clock is not where the band stands (since row 1 the band stands at its step's start).
        if rt.state.frame_clock != clock {
            clock_drift += 1;
        }
        t.frames += 1;
        if trace && t.frames <= 40 {
            println!(
                "window-trace {name} t_ms={:.1} kind={} commits={commits} shaped={} animating={animating} next_wake_ms={:?} frame_ms={ms:.2}",
                (clock - clock0) * 1000.0,
                if landing { "text" } else if was_animating { "vsync" } else { "timer" },
                rt.stats.shaped,
                rt.wake_deadline().map(|w| ((w - clock) * 10_000.0).round() / 10.0),
            );
        }
        let shaped = u64::from(rt.stats.shaped);
        if commits == 0 {
            t.still_ms.push(ms);
            t.still_shaped += shaped;
        } else {
            t.commit_shaped += shaped;
            t.commits += commits;
            t.doubled += u64::from(commits > 1);
            t.commit_ms.push(ms / commits as f64);
            if landing {
                t.text += 1;
            } else if was_animating {
                t.vsync += 1;
                t.vsync_with_text += u64::from(std::mem::take(&mut held));
            } else {
                t.timer += 1;
            }
        }
        if !was_animating || commits > 0 {
            held = false;
        }
    }
    let wall = wall0.elapsed().as_secs_f64();
    let window_secs = clock - clock0;
    let cpu = cpu_seconds() - cpu0;
    assert_eq!(t.commits, rt.stats.commits - commits0);
    t.commit_ms.sort_by(f64::total_cmp);
    t.still_ms.sort_by(f64::total_cmp);
    let rows = rt.state.recent.len();
    let items = rt.state.ledger.as_ref().map_or(0, |p| p.items.len());
    let reply_chars = match rt.state.ledger.as_ref().and_then(|p| p.items.last()) {
        Some(ledger::Item::Assistant { text, .. }) => text.len(),
        _ => 0,
    };
    let per_s = |n: u64| n as f64 / wall;
    println!(
        "window-measure {name} text={text} cpu_percent_of_one_core={:.1} cpu_s_per_window_s={:.3} commits_per_s={:.1} \
         commit_ms_median={:.2} commit_ms_p95={:.2} frames_per_s={:.1} timer_commits_per_s={:.1} \
         text_frame_commits_per_s={:.1} vsync_commits_per_s={:.1} vsync_commits_carrying_text_per_s={:.1} \
         landings_per_s={:.1} doubled_frames={} still_frames={} still_frame_ms_median={:.3} shaped_per_commit={:.1} shaped_per_still_frame={:.2} clock_off_step_frames={clock_drift} \
         gaps_ms={:?} wall_s={wall:.1} window_s={window_secs:.1} rows={rows} items={items} live_reply_chars={reply_chars}",
        cpu / wall * 100.0,
        cpu / window_secs,
        per_s(t.commits),
        quantile(&t.commit_ms, 0.5),
        quantile(&t.commit_ms, 0.95),
        per_s(t.frames),
        per_s(t.timer),
        per_s(t.text),
        per_s(t.vsync),
        per_s(t.vsync_with_text),
        per_s(landings),
        t.doubled,
        t.still_ms.len(),
        quantile(&t.still_ms, 0.5),
        t.commit_shaped as f64 / t.commit_ms.len().max(1) as f64,
        t.still_shaped as f64 / t.still_ms.len().max(1) as f64,
        t.gaps,
    );
}

/// State A, run by hand: `cargo test --release -p locust-native -- --ignored measure_a_lane_running_elsewhere
/// --nocapture --test-threads=1`.
#[test]
#[ignore = "a measure, run by hand: speed-3 row 0"]
fn measure_a_lane_running_elsewhere() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    measure("A", window("m_other", false), false);
}

/// State B, run by hand: `cargo test --release -p locust-native -- --ignored
/// measure_b_the_running_conversation_open_with_text_landing --nocapture --test-threads=1`.
#[test]
#[ignore = "a measure, run by hand: speed-3 row 0"]
fn measure_b_the_running_conversation_open_with_text_landing() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    measure("B", window("m_1", true), true);
}

/// Why state A frames as often as it does, run by hand: every poll while the band moves asks for a wake 50 ms on
/// (main.rs `poll`, `working_shimmer::FRAME_MS`), so a frame between two beats (a hover, a landing) may start a chain of
/// wakes of its own beside the first. A fresh window A framed only at its own wakes, then with one frame put in between
/// two beats, twice: frames a second over two seconds of window time after each.
#[test]
#[ignore = "a measure, run by hand: speed-3 row 0"]
fn measure_why_a_running_row_frames_so_often() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let mut rt = window("m_other", false);
    let mut clock = 0.0;
    let mut first = Vec::new();
    while clock < 0.4 {
        let at = next_frame(&rt, clock, false);
        first.push(((at - clock) * 1000.0 * 10.0).round() / 10.0);
        step(&mut rt, &mut clock, at);
    }
    let rate = |rt: &mut Runtime<State>, clock: &mut f64| {
        let (start, mut frames) = (*clock, 0u32);
        while *clock - start < 2.0 {
            let at = next_frame(rt, *clock, false);
            step(rt, clock, at);
            frames += 1;
        }
        frames as f64 / (*clock - start)
    };
    let alone = rate(&mut rt, &mut clock);
    let mut after = Vec::new();
    for _ in 0..3 {
        // One frame 20 ms on, between two beats, as an input's frame comes: no floor.
        let wake = rt.wake_deadline().expect("the band asks");
        let at = (clock + 0.02).min(wake - 0.001).max(clock + 0.001);
        step(&mut rt, &mut clock, at);
        after.push(rate(&mut rt, &mut clock));
    }
    println!("window-chains first_gaps_ms={first:?} frames_per_s_alone={alone:.1} after_each_extra_frame={after:.1?}");
}

/// What keeps state B framing at every vsync once text lands, run by hand: after warm-up, one chunk lands, and for the
/// next frames each element whose drawn rect is not yet its target, or that a loop moves, is printed by key or path.
#[test]
#[ignore = "a measure, run by hand: speed-3 row 1"]
fn measure_what_moves_after_text_lands() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let mut rt = window("m_1", true);
    let (mut clock, mut animating) = (0.0, false);
    while clock < 2.0 {
        let at = next_frame(&rt, clock, animating);
        animating = step(&mut rt, &mut clock, at).2;
    }
    println!("window-moves before animating={animating}");
    rt.update(|s, _| land_text(s));
    for n in 0..30 {
        let at = next_frame(&rt, clock, animating);
        let (_, commits, now) = step(&mut rt, &mut clock, at);
        animating = now;
        let moving: Vec<String> = rt
            .elements()
            .into_iter()
            .filter(|e| !e.exiting && (e.loop_phase.is_some() || e.rect.iter().zip(e.target).any(|(a, b)| (a - b).abs() > 0.01)))
            .map(|e| format!("{}{:?}->{:?}", e.key.unwrap_or(e.path), e.rect, e.target))
            .take(6)
            .collect();
        println!("window-moves frame={n} commits={commits} animating={animating} moving={moving:?}");
        if n > 0 && n % 6 == 0 {
            rt.update(|s, _| land_text(s));
        }
    }
}
