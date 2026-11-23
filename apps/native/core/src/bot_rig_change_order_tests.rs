//! Port of `apps/desktop/src/renderer/src/a-change-is-performed-in-order.test.ts` (16 cases).
use super::*;

const FRAME: f64 = 1.0 / 30.0;

struct Asked {
    eyes: [&'static str; 2],
    tone: Phosphor,
    flash: Option<Phosphor>,
    key: Option<&'static str>,
}

impl Asked {
    fn new(eyes: [&'static str; 2]) -> Self {
        Self {
            eyes,
            tone: Phosphor::Cyan,
            flash: None,
            key: None,
        }
    }

    fn shown(&self) -> ShownFace {
        ShownFace {
            pair: Some([self.eyes[0].to_owned(), self.eyes[1].to_owned()]),
            tone: self.tone,
            key: self.key.map(str::to_owned),
        }
    }
}

struct FilmFrame {
    s: f64,
    change: FaceChange,
    lit: String,
    motion: [GlyphMotion; 2],
}

fn film(at: impl Fn(f64) -> Asked, from: f64, to: f64, rested: f64) -> Vec<FilmFrame> {
    let first = at(from);
    let mut changes = FaceChanges::new(first.shown());
    let mut frames = Vec::new();
    let mut i = 0usize;
    while from + (i as f64) * FRAME <= to + 1e-9 {
        let s = from + (i as f64) * FRAME;
        let asked = at(s);
        let change = changes.at(s, asked.shown(), asked.flash, None, POWER_ON_S);
        let (lit, _glow) = screen_light(changes.shown().tone, changes.flash(), s);
        let pair_str = change
            .eyes
            .as_ref()
            .map(|[a, b]| format!("{a}{b}"))
            .unwrap_or_default();
        let loop_s = if rested >= 1.0 { 0.0 } else { (s - change.since).max(0.0) };
        let motion = [
            glyph_motion(&pair_str, 0, loop_s),
            glyph_motion(&pair_str, 1, loop_s),
        ];
        frames.push(FilmFrame { s, change, lit, motion });
        i += 1;
    }
    frames
}

fn eyes_of(change: &FaceChange) -> String {
    change
        .eyes
        .as_ref()
        .map(|[a, b]| format!("{a}{b}"))
        .unwrap_or_default()
}

fn blinks_of<'a>(changes: impl IntoIterator<Item = &'a FaceChange>) -> Vec<Vec<&'a FaceChange>> {
    let mut runs = Vec::new();
    let mut run = Vec::new();
    for ch in changes {
        if ch.shut > 0.0 {
            run.push(ch);
        } else if !run.is_empty() {
            runs.push(std::mem::take(&mut run));
        }
    }
    if !run.is_empty() {
        runs.push(run);
    }
    runs
}

fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn rest_with(yaw: f64, blink: f64) -> BotAvatarPose {
    let mut p = rest_pose(BotAvatarState::Default);
    p.yaw = yaw;
    p.roll = 0.03;
    p.blink_l = blink;
    p.blink_r = blink;
    p
}

struct Answered {
    s: f64,
    change: FaceChange,
    frame: BodyFrame,
}

fn answer(
    screen: bool,
    at: impl Fn(f64) -> Asked,
    wants: impl Fn(f64) -> BodyWants,
    from: f64,
    to: f64,
    heading: f64,
    seed: f64,
    resting_at_start: bool,
) -> Vec<Answered> {
    let first = at(from);
    let mut changes = FaceChanges::new(first.shown());
    let mut body = BodyConductor::new(seed, screen, false, resting_at_start);
    let mut hop_taken = false;
    let mut frames = Vec::new();
    let mut i = 0usize;
    while from + (i as f64) * FRAME <= to + 1e-9 {
        let s = from + (i as f64) * FRAME;
        let asked = at(s);
        let change = changes.at(s, asked.shown(), asked.flash, None, POWER_ON_S);
        let mut w = wants(s);
        w.hop = w.hop && !hop_taken;
        let frame = body.frame(s, &change, rest_with(heading, 0.0), Some(heading), w);
        if frame.hop {
            hop_taken = true;
        }
        frames.push(Answered { s, change, frame });
        i += 1;
    }
    frames
}

// --- a change, on a screen

// case: is one blink: the eyes shut, the new ones come in under the shut lids, they open, and only then does its state begin
#[test]
fn is_one_blink_the_eyes_shut_the_new_ones_come_in_under_the_shut_lids_they_open_and_only_then_does_its_state_begin() {
    let frames = film(
        |s| {
            let mut a = if s < 10.0 { Asked::new(["•", "•"]) } else { Asked::new([">", "▮"]) };
            a.key = Some(if s < 10.0 { "thinking" } else { "working" });
            a
        },
        9.5,
        11.0,
        0.0,
    );
    let runs = blinks_of(frames.iter().map(|f| &f.change));
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    let deepest = run
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.shut.partial_cmp(&b.shut).unwrap())
        .unwrap()
        .0;
    for i in 1..run.len() {
        let step = run[i].shut - run[i - 1].shut;
        if i <= deepest {
            assert!(step > 0.0, "shutting at index {i}");
        } else {
            assert!(step < 0.0, "opening at index {i}");
        }
    }
    assert!(run[deepest].shut > 0.9);
    let swapped = frames.iter().position(|f| eyes_of(&f.change) == ">▮").unwrap();
    assert!(frames[swapped..].iter().all(|f| eyes_of(&f.change) == ">▮"));
    assert!(frames[swapped].change.shut > 0.9);
    let begun = frames.iter().find(|f| f.change.count > 0).unwrap().s;
    assert!(approx(begun, 10.0, 1e-9));
    for f in &frames {
        assert_eq!(
            f.change.open,
            f.s < begun - 1e-9 || f.s >= begun + SWAP_S - 1e-9,
            "at {:.3}",
            f.s
        );
    }
    assert!(approx(frames.last().unwrap().change.since, begun + SWAP_S, 1e-9));
    assert_eq!(frames.last().unwrap().change.count - frames[0].change.count, 1);
}

// case: blinks once for a change of what it is doing, its eyes and their colour the same; and not at all with no change
#[test]
fn blinks_once_for_a_change_of_what_it_is_doing_its_eyes_and_their_colour_the_same_and_not_at_all_with_no_change() {
    let changed = film(
        |s| Asked {
            eyes: ["o", "o"],
            tone: Phosphor::Amber,
            flash: None,
            key: Some(if s < 10.0 { "waiting" } else { "receiving" }),
        },
        9.5,
        11.0,
        0.0,
    );
    assert_eq!(blinks_of(changed.iter().map(|f| &f.change)).len(), 1);
    assert_eq!(changed.last().unwrap().change.count, 1);

    let unchanged = film(
        |_| Asked {
            eyes: ["o", "o"],
            tone: Phosphor::Amber,
            flash: None,
            key: Some("waiting"),
        },
        9.5,
        11.0,
        0.0,
    );
    assert_eq!(blinks_of(unchanged.iter().map(|f| &f.change)).len(), 0);
    assert_eq!(unchanged.last().unwrap().change.count, 0);
}

// case: asked for again mid-change, goes on from where its lids are: never springing open to shut again
#[test]
fn asked_for_again_mid_change_goes_on_from_where_its_lids_are_never_springing_open_to_shut_again() {
    let most = FRAME / (SWAP_S / 2.0) + 1e-9;
    for again in [0.06, 0.2] {
        let frames = film(
            |s| {
                if s < 10.0 {
                    Asked::new(["•", "•"])
                } else if s < 10.0 + again {
                    Asked::new([">", "▮"])
                } else {
                    Asked::new(["^", "^"])
                }
            },
            9.5,
            11.0,
            0.0,
        );
        assert_eq!(blinks_of(frames.iter().map(|f| &f.change)).len(), 1, "again at +{again}");
        for i in 1..frames.len() {
            assert!(
                (frames[i].change.shut - frames[i - 1].change.shut).abs() <= most,
                "at {:.3}",
                frames[i].s
            );
        }
        assert_eq!(eyes_of(&frames.last().unwrap().change), "^^");
        assert_eq!(frames.last().unwrap().change.count, 2);
    }
    let early = film(
        |s| {
            if s < 10.0 {
                Asked::new(["•", "•"])
            } else if s < 10.06 {
                Asked::new([">", "▮"])
            } else {
                Asked::new(["^", "^"])
            }
        },
        9.5,
        11.0,
        0.0,
    );
    assert!(!early.iter().any(|f| eyes_of(&f.change) == ">▮"));
}

// case: brings its flash in with its new eyes, never on the old ones as they shut, and fades from there
#[test]
fn brings_its_flash_in_with_its_new_eyes_never_on_the_old_ones_as_they_shut_and_fades_from_there() {
    let frames = film(
        |s| {
            if s < 10.0 {
                Asked { eyes: [">", "▮"], tone: Phosphor::Cyan, flash: None, key: Some("working") }
            } else {
                Asked { eyes: ["^", "^"], tone: Phosphor::Cyan, flash: Some(Phosphor::Green), key: Some("done") }
            }
        },
        9.5,
        12.0,
        0.0,
    );
    let swapped = frames.iter().position(|f| eyes_of(&f.change) == "^^").unwrap();
    assert!(swapped > 0);
    for f in &frames[..swapped] {
        assert_eq!(f.lit, Phosphor::Cyan.lit_css(), "at {:.3}", f.s);
    }
    assert_eq!(frames[swapped].lit, mix_phosphor_css(Phosphor::Green.lit_css(), Phosphor::Cyan.lit_css(), 0.0));
    let came_in = 10.0 + SWAP_S / 2.0;
    for f in frames.iter().filter(|f| f.s >= came_in + FLASH_S + FRAME) {
        assert_eq!(f.lit, Phosphor::Cyan.lit_css(), "at {:.3}", f.s);
    }
}

// case: starts its state's loops as the eyes open on it, on its own clock: the eyes are the loop's, from its start
#[test]
fn starts_its_states_loops_as_the_eyes_open_on_it_on_its_own_clock_the_eyes_are_the_loops_from_its_start() {
    for at in [10.0, 10.4, 10.8, 11.2, 11.6] {
        let frames = film(
            |s| if s < at { Asked::new(["•", "•"]) } else { Asked::new(["^", "^"]) },
            at - 0.5,
            at + 2.0,
            0.0,
        );
        let opened: Vec<&FilmFrame> = frames
            .iter()
            .filter(|f| f.change.open && eyes_of(&f.change) == "^^")
            .collect();
        assert!(opened.len() > 40);
        for f in opened {
            let elapsed = f.s - f.change.since;
            assert!(approx(f.change.since, at + SWAP_S, FRAME + 1e-6));
            let own = [glyph_motion("^^", 0, elapsed), glyph_motion("^^", 1, elapsed)];
            assert_eq!(f.motion, own, "changed at {at}, drawn at {:.3}", f.s);
        }
    }
}

// case: performs nothing on a still bot (reduced motion): the eyes asked for, at once, open
#[test]
fn performs_nothing_on_a_still_bot_reduced_motion_the_eyes_asked_for_at_once_open() {
    let mut changes = FaceChanges::new(Asked::new(["•", "•"]).shown());
    assert_eq!(
        changes.at(0.0, Asked::new(["•", "•"]).shown(), None, None, POWER_ON_S),
        FaceChange {
            since: 0.0,
            open: true,
            shut: 0.0,
            quiet: true,
            count: 0,
            eyes: Some(["•".into(), "•".into()]),
        }
    );
    assert_eq!(
        changes.at(0.0, Asked::new([">", "▮"]).shown(), None, None, POWER_ON_S),
        FaceChange {
            since: 0.0,
            open: true,
            shut: 0.0,
            quiet: true,
            count: 0,
            eyes: Some([">".into(), "▮".into()]),
        }
    );
}

// --- the body, answering a change

// case: blinks a face of plastic once a change, as the change begins; never a screen, whose swap is its blink
#[test]
fn blinks_a_face_of_plastic_once_a_change_as_the_change_begins_never_a_screen_whose_swap_is_its_blink() {
    let doing = |s: f64| Asked {
        eyes: ["•", "•"],
        tone: Phosphor::Cyan,
        flash: None,
        key: Some(if s < 10.0 { "thinking" } else if s < 11.0 { "working" } else { "done" }),
    };
    let wants = |_| BodyWants { mood: None, hop: false, glancing: false, resting: false };
    let plastic = answer(false, doing, wants, 9.5, 12.0, 0.0, 0.4, false);
    let blinked: Vec<&Answered> = plastic.iter().filter(|f| f.frame.blink).collect();
    let counts: Vec<u32> = blinked.iter().map(|f| f.change.count).collect();
    assert_eq!(counts, vec![1, 2]);
    for f in &blinked {
        assert!(!f.change.open);
    }
    assert!(approx(blinked[0].s, 10.0, 1e-9));

    let screen = answer(true, doing, wants, 9.5, 12.0, 0.0, 0.4, false);
    assert_eq!(screen.iter().filter(|f| f.frame.blink).count(), 0);
}

// case: counts a change's blink as the rig's own: the rig's next blink waits as after any of its blinks, so a change never blinks twice
#[test]
fn counts_a_changes_blink_as_the_rigs_own_the_rigs_next_blink_waits_as_after_any_of_its_blinks_so_a_change_never_blinks_twice() {
    for fire in [true, false] {
        for seed in [0.11, 0.26, 0.4, 0.57, 0.83] {
            let mut sim = BotAvatarSim::new(seed, BotAvatarState::Default);
            let mut steps = 0;
            while !(sim.blink_at - sim.t < 0.3 && sim.blink_at - sim.t > 0.1 && !sim.blink.active()) && steps < 3000 {
                sim.update(FRAME);
                steps += 1;
            }
            assert!(steps < 3000);
            sim.blink_now(fire);
            let mut blinks_seen = if sim.blink.active() { 1 } else { 0 };
            let mut was = sim.blink.active();
            for _ in 0..60 {
                sim.update(FRAME);
                if sim.blink.active() && !was {
                    blinks_seen += 1;
                }
                was = sim.blink.active();
            }
            assert_eq!(blinks_seen, if fire { 1 } else { 0 }, "seed {seed}, fire={fire}");
        }
    }
}

// case: wears a new mood only once its eyes are open: through the blink its head holds, then it tips
#[test]
fn wears_a_new_mood_only_once_its_eyes_are_open_through_the_blink_its_head_holds_then_it_tips() {
    for screen in [false, true] {
        let frames = answer(
            screen,
            |s| Asked {
                eyes: ["o", "o"],
                tone: Phosphor::Cyan,
                flash: None,
                key: Some(if s < 10.0 { "working" } else { "waiting" }),
            },
            |s| BodyWants {
                mood: if s < 10.0 { None } else { Some(BotMood::Curious) },
                hop: false,
                glancing: false,
                resting: false,
            },
            9.5,
            11.5,
            0.0,
            0.4,
            false,
        );
        let tipped = 0.03 + tilt_side(0.4) * CURIOUS_TILT;
        for f in &frames {
            if f.s < 10.0 + SWAP_S - 1e-9 {
                assert!(approx(f.frame.pose.roll, 0.03, 1e-12), "screen={screen} at {:.3}", f.s);
            }
        }
        let after: Vec<&Answered> = frames.iter().filter(|f| f.s >= 10.0 + SWAP_S - 1e-9).collect();
        assert!(approx(after[0].frame.pose.roll, 0.03, 1e-12));
        for i in 1..after.len() {
            assert!((tipped - after[i].frame.pose.roll).abs() <= (tipped - after[i - 1].frame.pose.roll).abs() + 1e-12);
        }
        let settled = frames.iter().find(|f| f.s >= 10.0 + SWAP_S + HEAD_GLANCE_S).unwrap();
        assert!(approx(settled.frame.pose.roll, tipped, 1e-9));
    }
}

// case: hops once its eyes are open on the change that asked for it, and not before
#[test]
fn hops_once_its_eyes_are_open_on_the_change_that_asked_for_it_and_not_before() {
    for screen in [false, true] {
        let frames = answer(
            screen,
            |s| Asked {
                eyes: if s < 10.0 { [">", "▮"] } else { ["^", "^"] },
                tone: Phosphor::Cyan,
                flash: None,
                key: Some(if s < 10.0 { "working" } else { "done" }),
            },
            |s| BodyWants {
                mood: if s < 10.0 { None } else { Some(BotMood::Glad) },
                hop: s >= 10.0,
                glancing: false,
                resting: false,
            },
            9.5,
            11.0,
            0.0,
            0.4,
            false,
        );
        let hops: Vec<&Answered> = frames.iter().filter(|f| f.frame.hop).collect();
        assert_eq!(hops.len(), 1);
        assert!(hops[0].change.open);
        let first_open = frames.iter().find(|f| f.s >= 10.0 + SWAP_S - 1e-9).unwrap().s;
        assert!(approx(hops[0].s, first_open, 1e-9));
    }
}

// case: begins its thinking look as its dots open, from where the rig has the head, on the thinking's own clock
#[test]
fn begins_its_thinking_look_as_its_dots_open_from_where_the_rig_has_the_head_on_the_thinkings_own_clock() {
    for at in [10.0, 10.5, 11.3] {
        let frames = answer(
            true,
            |s| Asked {
                eyes: if s < at { [">", "▮"] } else { ["•", "•"] },
                tone: Phosphor::Cyan,
                flash: None,
                key: Some(if s < at { "working" } else { "thinking" }),
            },
            |_| BodyWants { mood: None, hop: false, glancing: false, resting: false },
            at - 0.5,
            at + 2.0,
            0.0,
            0.4,
            false,
        );
        let open = at + SWAP_S;
        for f in frames.iter().filter(|f| f.s < open - 1e-9) {
            assert_eq!(f.frame.pose.yaw, 0.0);
        }
        let settled = frames.iter().find(|f| f.s >= open + 1.0).unwrap();
        let expected = thinking_glance(1.0);
        assert!(approx(settled.frame.pose.yaw, expected.yaw, 0.01));
        assert!(approx(settled.frame.pose.pitch, expected.pitch, 0.01));
    }
}

// case: gives its head to a handoff glance: the thinking look fades while the glance holds, and comes back after it
#[test]
fn gives_its_head_to_a_handoff_glance_the_thinking_look_fades_while_the_glance_holds_and_comes_back_after_it() {
    let heading = 0.5;
    let frames = answer(
        true,
        |_| Asked { eyes: ["•", "•"], tone: Phosphor::Cyan, flash: None, key: Some("thinking") },
        |s| BodyWants {
            mood: None,
            hop: false,
            glancing: (20.0..21.5).contains(&s),
            resting: false,
        },
        18.0,
        23.4,
        heading,
        0.4,
        false,
    );
    let before = frames.iter().find(|f| f.s >= 19.9).unwrap();
    assert!((before.frame.pose.yaw - heading).abs() > 0.1);
    for f in frames.iter().filter(|f| f.s >= 20.0 + HEAD_GLANCE_S && f.s < 21.5) {
        assert!(approx(f.frame.pose.yaw, heading, 1e-9), "at {:.3}", f.s);
    }
    for i in 1..frames.len() {
        assert!((frames[i].frame.pose.yaw - frames[i - 1].frame.pose.yaw).abs() < 0.12);
    }
    assert!(frames
        .iter()
        .filter(|f| f.s >= 21.5 + HEAD_GLANCE_S)
        .any(|f| (f.frame.pose.yaw - heading).abs() > 0.1));
}

// --- a face asked to keep still

// case: settles to rest once its eyes are open on the change, facing you, and only then may rest
#[test]
fn settles_to_rest_once_its_eyes_are_open_on_the_change_facing_you_and_only_then_may_rest() {
    for screen in [false, true] {
        let heading = 0.4;
        let frames = answer(
            screen,
            |s| Asked {
                eyes: if s < 10.0 { ["^", "^"] } else { ["|", "|"] },
                tone: Phosphor::Cyan,
                flash: None,
                key: Some(if s < 10.0 { "done" } else { "idle" }),
            },
            |s| BodyWants { mood: None, hop: false, glancing: false, resting: s >= 10.0 },
            9.5,
            11.5,
            heading,
            0.4,
            false,
        );
        let open = frames.iter().find(|f| f.s >= 10.0 + SWAP_S - 1e-9).unwrap().s;
        for f in frames.iter().filter(|f| f.s < open - 1e-9) {
            assert_eq!(f.frame.rest, 0.0, "screen={screen} at {:.3}", f.s);
            assert!(approx(f.frame.pose.yaw, heading, 1e-12));
        }
        let after: Vec<&Answered> = frames.iter().filter(|f| f.s >= open - 1e-9).collect();
        for i in 1..after.len() {
            assert!(after[i].frame.rest >= after[i - 1].frame.rest);
            assert!(after[i].frame.pose.yaw.abs() <= after[i - 1].frame.pose.yaw.abs() + 1e-12);
        }
        let rested: Vec<&Answered> = frames.iter().filter(|f| f.s >= open + HEAD_GLANCE_S).collect();
        assert!(rested.len() > 5);
        for f in rested {
            assert_eq!(f.frame.rest, 1.0);
            assert!(approx(f.frame.pose.yaw, 0.0, 1e-12));
            assert!(f.frame.settled);
        }
        assert_eq!(
            frames
                .iter()
                .filter(|f| f.frame.settled && f.s >= 10.0 && f.s < open + HEAD_GLANCE_S - FRAME)
                .count(),
            0
        );
    }
}

// case: wakes on the same rig: its eyes perform the change while it rests, then its body eases back out
#[test]
fn wakes_on_the_same_rig_its_eyes_perform_the_change_while_it_rests_then_its_body_eases_back_out() {
    let heading = 0.4;
    let frames = answer(
        true,
        |s| Asked {
            eyes: if s < 10.0 { ["|", "|"] } else { ["•", "•"] },
            tone: Phosphor::Cyan,
            flash: None,
            key: Some(if s < 10.0 { "idle" } else { "thinking" }),
        },
        |s| BodyWants { mood: None, hop: false, glancing: false, resting: s < 10.0 },
        9.5,
        11.5,
        heading,
        0.4,
        true,
    );
    assert_eq!(blinks_of(frames.iter().map(|f| &f.change)).len(), 1);
    assert_eq!(frames.last().unwrap().change.count, 1);
    let open = frames.iter().find(|f| f.s >= 10.0 + SWAP_S - 1e-9).unwrap().s;
    for f in frames.iter().filter(|f| f.s < open - 1e-9) {
        assert_eq!(f.frame.rest, 1.0);
        assert!(approx(f.frame.pose.yaw, 0.0, 1e-12));
    }
    let after: Vec<&Answered> = frames.iter().filter(|f| f.s >= open - 1e-9).collect();
    for i in 1..after.len() {
        assert!(after[i].frame.rest <= after[i - 1].frame.rest);
    }
    assert_eq!(frames.iter().find(|f| f.s >= open + HEAD_GLANCE_S).unwrap().frame.rest, 0.0);
}

// case: is never settled mid-blink, so it never rests with its eyes half shut
#[test]
fn is_never_settled_mid_blink_so_it_never_rests_with_its_eyes_half_shut() {
    let mut changes = FaceChanges::new(Asked::new(["|", "|"]).shown());
    let mut body = BodyConductor::new(0.4, true, false, true);
    let wants = BodyWants { mood: None, hop: false, glancing: false, resting: true };
    let c0 = changes.at(10.0, Asked::new(["|", "|"]).shown(), None, None, POWER_ON_S);
    assert!(body.frame(10.0, &c0, rest_with(0.0, 0.0), Some(0.0), wants).settled);
    let c1 = changes.at(10.0 + FRAME, Asked::new(["|", "|"]).shown(), None, None, POWER_ON_S);
    assert!(!body.frame(10.0 + FRAME, &c1, rest_with(0.0, 0.5), Some(0.0), wants).settled);
    let c2 = changes.at(10.0 + 2.0 * FRAME, Asked::new(["|", "|"]).shown(), None, None, POWER_ON_S);
    assert!(body.frame(10.0 + 2.0 * FRAME, &c2, rest_with(0.0, 0.0), Some(0.0), wants).settled);
}

// case: draws its eyes' loops at their rest: a resting face never freezes mid-wince
#[test]
fn draws_its_eyes_loops_at_their_rest_a_resting_face_never_freezes_mid_wince() {
    let still = film(|_| Asked::new([">", "<"]), 0.0, 0.0, 0.0)[0].motion;
    let resting = film(|_| Asked::new([">", "<"]), 10.0, 16.0, 1.0);
    for f in &resting {
        assert_eq!(f.motion, still, "at {:.3}", f.s);
    }
    let moving = film(|_| Asked::new([">", "<"]), 10.0, 16.0, 0.0);
    assert!(moving.iter().any(|f| f.motion != still));
}
