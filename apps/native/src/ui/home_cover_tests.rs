//! Home drawing its cover to the column and fitting itself to the pane, headless at the window sizes the Electron tests
//! name: `the-home-screen-is-the-cover.test.tsx`, `the-cover-grows-into-spare-height.test.tsx` (the drawn cases),
//! `home-fits-one-screen.test.tsx`, and the drawn half of `home-fits-two-rows-of-teammates` and
//! `home-holds-still-when-tight-spacing-shrinks-it`. The pure rules are in core/src/home_cover_tests.rs and
//! core/src/cover_script_tests.rs. Measured on the drawn frame; no window is opened.
use super::*;
use locust_core::agents::{Auth, Found, Probe};
use locust_core::home_cover::{cover_place, COVER_MACHINE, COVER_MAX_GROW, COVER_MIN_GROW, COVER_WIDTH};

fn ready_agent(id: &str, name: &str) -> Found {
    Found { id: id.into(), display_name: name.into(), installed: true, version: Some("1.0.0".into()), auth: Auth::Authenticated, ready: true,
        status: Probe::Ready, unsupported_note: None, through_cli: false, checking: false }
}

fn agents() -> Vec<Found> {
    vec![ready_agent("codex", "Codex CLI"), ready_agent("claude", "Claude Code")]
}

/// Home drawn at a window, and given frames enough for its measure to settle.
fn drawn(s: State, size: (f32, f32)) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), size, rune::Env::new(true, 1.0));
    // Long frames: the measure settles in a few, and Rune's springs carry a size change over a few hundred ms.
    for _ in 0..16 {
        rt.frame(0.25);
    }
    rt
}

fn rect(rt: &rune::Runtime<State>, key: &str) -> [f32; 4] {
    rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect).unwrap_or_else(|| panic!("no {key}"))
}

fn inside(outer: [f32; 4], inner: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.5 && inner[1] >= outer[1] - 0.5 && inner[0] + inner[2] <= outer[0] + outer[2] + 0.5 && inner[1] + inner[3] <= outer[1] + outer[3] + 0.5
}

fn cast() -> Vec<CoverDrawn> {
    DRAWN_COVER.with(|d| d.borrow().clone())
}

/// A first run with the agents answered: the cover is on.
fn on() -> State {
    let mut s = State::default();
    s.settings_panel.agents = Some(agents());
    s
}

// --- the-home-screen-is-the-cover.test.tsx: what the home screen draws

#[test]
fn has_the_lockup_and_under_it_the_claim_both_on_the_machines_glass() {
    let rt = drawn(on(), (1215.0, 1600.0));
    let glass = rect(&rt, "home-cover-glass");
    let lockup = rect(&rt, "home-cover-lockup");
    let claim = rect(&rt, "home-cover-line");
    assert!(inside(glass, lockup) && inside(glass, claim), "glass {glass:?}, lockup {lockup:?}, claim {claim:?}");
    assert!(lockup[1] + lockup[3] <= claim[1] + 0.5, "the claim is under the lockup: {lockup:?} {claim:?}");
    let all = rt.elements();
    let line = all.iter().find(|e| e.key.as_deref() == Some("home-cover-line")).and_then(|e| e.text.clone());
    assert_eq!(line.as_deref(), Some(COVER_CLAIM.to_uppercase().as_str()));
    // Under the tube's scanlines, like the boot screen's: the scan layer covers the glass and is drawn after the lockup.
    let scan = rect(&rt, "home-cover-scan");
    assert!(inside(scan, lockup) && inside(scan, claim), "scan {scan:?}");
    let at = |key: &str| all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap();
    assert!(at("home-cover-scan") > at("home-cover-lockup") && at("home-cover-scan") > at("home-cover-line"));
}

#[test]
fn has_the_three_teammates_on_the_machine_in_the_cover_order_a_ghost_prompt_a_locust() {
    let rt = drawn(on(), (1215.0, 1600.0));
    let drawn = cast();
    assert_eq!(drawn.iter().map(|d| d.key).collect::<Vec<_>>(), ["wren", "atlas", "sable"]);
    assert_eq!(drawn.iter().map(|d| d.bot).collect::<Vec<_>>(), ["ghost", "prompt", "critter"]);
    assert_eq!(drawn.iter().map(|d| d.state).collect::<Vec<_>>(), ["default", "default", "sleeping"]);
    // Each in its own part from the first frame: thinking, typing, asleep.
    assert_eq!(drawn.iter().map(|d| d.beat).collect::<Vec<_>>(), ["thinking", "typing", "asleep"]);
    // They stand on the machine's top edge, left to right.
    let machine = rect(&rt, "home-cover-machine");
    let k = cover_place(760.0, rt.state.home.fit.grow.unwrap_or(1.0)).drawn as f32;
    let mut last_x = f32::MIN;
    for key in ["wren", "atlas", "sable"] {
        let face = rect(&rt, &format!("home-cover-face-{key}"));
        assert!((face[1] + face[3] - machine[1]).abs() <= 18.0 * k + 1.0, "{key} {face:?} on {machine:?}");
        assert!(face[0] > last_x);
        last_x = face[0];
    }
}

#[test]
fn has_a_white_ghost_that_floats_and_clawd_asleep_0_583_the_hopper_was_jagged_in_plush() {
    let rt = drawn(on(), (1215.0, 1600.0));
    let drawn = cast();
    let ghost = drawn.iter().find(|d| d.bot == "ghost").unwrap();
    assert_eq!((ghost.hue, ghost.floating), (None, true));
    let sleeper = drawn.iter().find(|d| d.zzz).unwrap();
    assert_eq!((sleeper.bot, sleeper.hue), ("critter", Some("clay")));
    // The float is the renderer's own loop on the ghost, so it costs no view.
    let looping = |rt: &rune::Runtime<State>| rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("home-cover-bot-wren")) && e.loop_phase.is_some());
    assert!(looping(&rt), "the ghost floats once the cover is on");
    // Nothing floats before the agents have answered.
    let before = drawn_before();
    assert!(!looping(&before));
    assert!(cast().iter().all(|d| !d.floating));
}

fn drawn_before() -> rune::Runtime<State> {
    drawn(State::default(), (1215.0, 1600.0))
}

#[test]
fn wakes_the_teammates_with_the_lockup_still_and_unmarked_until_the_runtimes_answer() {
    let before = drawn_before();
    let still = cast();
    assert_eq!(still.iter().map(|d| d.state).collect::<Vec<_>>(), ["still", "still", "still"]);
    assert_eq!(still.iter().map(|d| d.beat).collect::<Vec<_>>(), ["still", "still", "still"]);
    assert!(before.find("home-cover-zzz").is_none());
    // Then the thinker and the worker play and the sleeper sleeps, its z's drifting: all three unmarked, no presence
    // dot and no waiting ring on a character (0.610).
    let after = drawn(on(), (1215.0, 1600.0));
    assert!(after.find("home-cover-zzz").is_some());
    let shown = cast();
    assert!(shown.iter().all(|d| d.state != "still"));
    let marks = after.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("presence") || k.contains("ring"))).count();
    assert_eq!(marks, 0);
    // The worker waits on you only in its waiting beat, not from the first frame.
    assert_ne!(shown[1].tone, locust_core::bot_spec::Phosphor::Amber);
}

#[test]
fn places_each_bot_on_the_cover_grid_at_the_starting_scale_in_whole_pixels() {
    // Grown to no room at all (a pane just tall enough), the ghost is 68 across at (213, -7) on the 760 card, as
    // Electron's server-rendered cover is at the starting scale.
    let mut s = on();
    s.home.fit.grow = Some(1.0);
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 1600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let card = rect(&rt, "home-cover");
    let ghost = rect(&rt, "home-cover-face-wren");
    assert_eq!(card[2], 760.0);
    assert_eq!((ghost[0] - card[0], ghost[1] - card[1], ghost[2], ghost[3]), (213.0, -7.0, 68.0, 68.0));
    for e in rt.elements() {
        if e.key.as_deref().is_some_and(|k| k.starts_with("home-cover-face-")) {
            let on_card = [e.rect[0] - card[0], e.rect[1] - card[1], e.rect[2], e.rect[3]];
            assert!(on_card.iter().all(|v| (v - v.round()).abs() < 0.01), "{:?} {on_card:?}", e.key);
        }
    }
}

#[test]
fn is_decorative_apart_from_its_words_no_bot_answers_to_a_name() {
    let rt = drawn(on(), (1215.0, 1600.0));
    let bots: Vec<_> = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("home-cover-bot-"))).collect();
    assert_eq!(bots.len(), 3);
    assert!(bots.iter().all(|b| b.label.is_none()));
    let tree = rt.accessibility_json().to_string();
    assert!(!tree.contains("home-cover-bot-"), "a bot reached the accessibility tree");
    assert!(tree.contains("Locust. "), "the words are named");
}

fn mates(s: &mut State, list: &[(&str, &str, bool)]) {
    crate::team_scene(s);
    s.teammates.truncate(list.len());
    let mut owners = serde_json::Map::new();
    for (mate, (id, name, busy)) in s.teammates.iter_mut().zip(list) {
        mate.id = (*id).into();
        mate.name = (*name).into();
        mate.busy = *busy;
        mate.runtime = Some("codex".into());
        owners.insert(format!("m_{id}"), serde_json::json!(id));
    }
    s.people_panel.team = serde_json::json!({ "missionOwners": owners });
}

fn glass_line(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().find(|e| e.key.as_deref() == Some("home-cover-line")).and_then(|e| e.text.clone()).unwrap()
}

#[test]
fn says_who_is_working_in_place_of_the_claim_and_the_claim_when_nothing_runs() {
    let mut s = on();
    mates(&mut s, &[("tm_codex", "Codex", true), ("tm_casper", "Casper", true)]);
    s.home.waiting_for.insert("m_tm_casper".into(), "A decision".into());
    let working = drawn(s, (1215.0, 1600.0));
    assert_eq!(glass_line(&working), "CASPER WAITING ON YOU · CODEX WORKING");
    assert!(!crate::scout_cards::tests::collect_rt_text(&working).contains(&COVER_CLAIM.to_uppercase()));
    let mut s = on();
    mates(&mut s, &[("tm_codex", "Codex", false)]);
    assert_eq!(glass_line(&drawn(s, (1215.0, 1600.0))), COVER_CLAIM.to_uppercase());
}

#[test]
fn the_parts_move_on_with_the_clock_and_hold_still_with_reduced_motion() {
    let mut s = on();
    s.home.cover_clock = locust_core::cover_script::CoverClock::at(6.6);
    let _rt = drawn(s, (1215.0, 1600.0));
    assert_eq!(cast()[1].beat, "done", "6.6 s in, the worker has finished");
    assert_eq!(cast()[1].tone, locust_core::bot_spec::Phosphor::Green);
    let mut s = on();
    s.home.cover_clock = locust_core::cover_script::CoverClock::at(6.6);
    s.reduced_motion.set(true);
    let _rt = drawn(s, (1215.0, 1600.0));
    assert_eq!(cast()[1].beat, "typing", "reduced motion holds each at its first beat");
}

#[test]
fn the_parts_clock_runs_once_the_cover_is_on_and_not_before() {
    let before = drawn_before();
    assert_eq!(before.state.home.cover_clock.elapsed(cover_now() + 60.0), 0.0, "no clock before the agents answer");
    let after = drawn(on(), (1215.0, 1600.0));
    let now = cover_now();
    assert!(after.state.home.cover_clock.elapsed(now + 1.0) > after.state.home.cover_clock.elapsed(now), "the clock runs");
}

// --- the-cover-grows-into-spare-height.test.tsx: the grown drawing, as Home measures its room

fn machine_on_card(rt: &rune::Runtime<State>) -> ([f32; 4], [f32; 4]) {
    (rect(rt, "home-cover-machine"), rect(rt, "home-cover"))
}

#[test]
fn stays_centred_on_its_card_at_its_own_size_and_grown() {
    let mut grows = Vec::new();
    for height in [900.0, 1100.0, 1300.0, 2400.0] {
        let rt = drawn(on(), (1215.0, height));
        let (m, card) = machine_on_card(&rt);
        let left = m[0] - card[0];
        assert!((left - (card[2] - m[2]) / 2.0).abs() <= 1.0, "at {height}: machine {m:?} on {card:?}");
        grows.push(rt.state.home.fit.grow.unwrap());
    }
    assert!(grows.windows(2).all(|w| w[1] >= w[0]), "a taller window grows it more: {grows:?}");
    assert_eq!(*grows.last().unwrap(), COVER_MAX_GROW, "{grows:?}");
}

/// A Home with six teammates, which is taller than its pane at the heights below.
fn crowded() -> State {
    let mut s = on();
    crate::team_scene(&mut s);
    let more: Vec<_> = s.teammates.iter().cloned().map(|mut m| {
        m.id = format!("{}-2", m.id);
        m.name = format!("{} Two", m.name);
        m
    }).collect();
    s.teammates.extend(more);
    s
}

#[test]
fn draws_smaller_when_the_page_gives_less_than_it_needs_and_no_smaller_than_the_floor() {
    let own = cover_place(760.0, 1.0).machine.width as f32;
    let rt = drawn(crowded(), (1215.0, 500.0));
    let (m, card) = machine_on_card(&rt);
    assert!(rt.state.home.fit.grow.unwrap() < 1.0, "{:?}", rt.state.home.fit);
    assert_eq!(m[2], (own as f64 * COVER_MIN_GROW).round() as f32, "the floor: {m:?}");
    assert!(((m[0] - card[0]) - (card[2] - m[2]) / 2.0).abs() <= 1.0);
}

#[test]
fn the_cover_takes_the_spare_height_and_leaves_only_the_air() {
    // Grown, the page is the pane less the air kept above the cover (cover_room_for), plus the 10% the measure counts
    // the cover over what Home draws (COVER_HOME_SCALE). A measure that read the cover as one of the other sections
    // would leave its whole footprint again as empty space.
    let mut checked = 0;
    for height in (900..=1800).step_by(20) {
        let rt = drawn(on(), (1215.0, height as f32));
        let fit = rt.state.home.fit;
        let grow = fit.grow.unwrap();
        if grow <= 1.0 || grow >= COVER_MAX_GROW || fit.tight {
            continue;
        }
        let (pane, column) = (fit.pane_height.unwrap(), fit.column.unwrap().0);
        let (pad, _) = locust_core::home_cover::home_spacing(false);
        let air = 48.0_f64.max((pane - 2.0 * pad) * 0.1);
        let base = (locust_core::home_cover::COVER_HEIGHT + locust_core::home_cover::COVER_REACH) * locust_core::home_cover::cover_scale(760.0);
        let left = pane - column;
        assert!(left >= air - 2.0 && left <= air + 0.1 * base * grow + 4.0, "at {height}: {left:.0} left, air {air:.0}, grow {grow}");
        checked += 1;
    }
    assert!(checked >= 2, "only {checked} heights grew without reaching the cap");
}

#[test]
fn keeps_clear_of_the_card_edges_at_its_largest() {
    let rt = drawn(on(), (1215.0, 2400.0));
    let (m, card) = machine_on_card(&rt);
    assert!(m[0] > card[0] && m[0] + m[2] < card[0] + card[2], "machine {m:?} on {card:?}");
    assert!(f64::from(m[2]) > COVER_MACHINE.width * (760.0 / COVER_WIDTH));
}

// --- home-fits-one-screen.test.tsx (0.799): cover, accounts, team, starters

fn top_of(rt: &rune::Runtime<State>, hit: impl Fn(&rune::ElementInfo) -> bool) -> Option<f32> {
    rt.elements().iter().find(|e| hit(e)).map(|e| e.rect[1])
}

#[test]
fn puts_the_accounts_between_the_cover_and_the_team_and_the_starters_after_the_team() {
    let mut s = on();
    crate::team_scene(&mut s);
    let rt = drawn(s, (1215.0, 4000.0));
    let cover = rect(&rt, "home-cover")[1];
    let accounts = rect(&rt, "home-agents")[1];
    let team = top_of(&rt, |e| e.text.as_deref() == Some(crate::starters::YOUR_TEAM)).expect("the team");
    let starters = rt.elements().iter().find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("starter-"))).map(|e| e.rect[1]).expect("the starters");
    assert!(cover < accounts && accounts < team && team < starters, "cover {cover}, accounts {accounts}, team {team}, starters {starters}");
}

#[test]
fn keeps_the_order_it_had_before_there_is_a_team_what_to_add_first_the_accounts_after() {
    let rt = drawn(on(), (1215.0, 4000.0));
    let cover = rect(&rt, "home-cover")[1];
    let accounts = rect(&rt, "home-agents")[1];
    let templates = top_of(&rt, |e| e.text.as_deref() == Some(crate::starters::START_WITH_A_TEAM)).expect("the templates");
    assert!(cover < templates && templates < accounts, "cover {cover}, templates {templates}, accounts {accounts}");
}

fn watching_scout(looking: bool, last_look: Option<&str>) -> State {
    use locust_core::scout::types::{ScoutSettings, ScoutState};
    let mut s = on();
    crate::team_scene(&mut s);
    for mate in &mut s.teammates {
        if mate.id == "scout" {
            mate.role = "Keeps watch".into();
        }
    }
    s.scout.now_ms = Some(chrono::DateTime::parse_from_rfc3339("2026-10-19T08:00:00.000Z").unwrap().timestamp_millis());
    s.scout.looking = looking;
    s.scout.state = Some(ScoutState {
        settings: ScoutSettings { on: true, teammate_id: Some("scout".into()), ..ScoutSettings::default() },
        cards: Vec::new(),
        look_ids: Vec::new(),
        last_look_at: last_look.map(str::to_owned),
        model_said: None,
        today: None,
        notes: Vec::new(),
    });
    s
}

fn description(rt: &rune::Runtime<State>, key: &str) -> Option<String> {
    fn find<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
        if node["key"] == key {
            return Some(node);
        }
        node["children"].as_array()?.iter().find_map(|child| find(child, key))
    }
    find(&rt.accessibility_json(), key).and_then(|n| n["description"].as_str().map(str::to_owned))
}

#[test]
fn draws_scouts_quiet_watch_on_its_own_card_with_look_now_as_the_eye_there() {
    let rt = drawn(watching_scout(false, Some("2026-10-19T07:59:00.000Z")), (1215.0, 4000.0));
    let card = rect(&rt, "home-card-scout");
    let line = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Looked 1 min ago · all clear")).expect("the watch line");
    assert!(inside(card, line.rect), "on Scout's own card: {card:?} {:?}", line.rect);
    let eye = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("scout-look-now")).expect("the eye");
    assert_eq!(eye.label.as_deref(), Some("Scout: look now"));
    // Its role is still its card's hover; the other cards are as they were.
    assert_eq!(description(&rt, "home-card-watch").as_deref(), Some("Keeps watch"));
    let shown = crate::scout_cards::tests::collect_rt_text(&rt);
    assert!(shown.contains("Code & Migrations"), "{shown}");
    assert!(!shown.contains("Scout keeps watch") && !shown.contains("Scout is watching"), "{shown}");
}

#[test]
fn says_looking_while_a_look_is_out_and_a_first_look_on_its_way_before_any() {
    let looking = drawn(watching_scout(true, Some("2026-10-19T07:59:00.000Z")), (1215.0, 4000.0));
    assert!(crate::scout_cards::tests::collect_rt_text(&looking).contains("Looking…"));
    let first = drawn(watching_scout(false, None), (1215.0, 4000.0));
    assert!(crate::scout_cards::tests::collect_rt_text(&first).contains("First look on its way"));
}

// --- home-fits-two-rows-of-teammates / home-holds-still-when-tight-spacing-shrinks-it, drawn: Home decides from its
// own measure, tightens when short, and holds still.

/// Every frame's decision at a window, after the first few frames' measure.
fn decisions(s: State, size: (f32, f32), frames: usize) -> Vec<(bool, f64)> {
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), size, rune::Env::new(true, 1.0));
    (0..frames).map(|_| {
        rt.frame(0.1);
        (rt.state.home.fit.tight, rt.state.home.fit.grow.unwrap_or(f64::NAN))
    }).collect()
}

#[test]
fn a_short_home_tightens_its_spacing_and_draws_the_cover_at_its_floor() {
    let rt = drawn(crowded(), (1209.0, 500.0));
    assert!(rt.state.home.fit.tight, "{:?}", rt.state.home.fit);
    let pane = rect(&rt, "home-pane");
    let card = rect(&rt, "home-cover");
    // The tight spacing is drawn: the column starts 12 below the pane's top, not 32 (the close row comes first).
    let close = rect(&rt, "home-close-row")[1];
    assert!((close - pane[1] - 12.0).abs() <= 1.0, "pane {pane:?}, close row at {close}");
    assert!(card[3] > 0.0);
    let roomy = drawn(on(), (1209.0, 2400.0));
    assert!(!roomy.state.home.fit.tight, "room to spare stays loose");
}

#[test]
fn home_holds_still_at_every_height_once_it_has_measured() {
    for height in (420..=1500).step_by(40) {
        for s in [crowded(), on()] {
            let seen = decisions(s, (1209.0, height as f32), 16);
            let settled = &seen[6..];
            assert!(settled.windows(2).all(|w| w[0] == w[1]), "at {height} it flicked: {seen:?}");
        }
    }
}

/// Home inside the whole window, beside the sidebar and under the title bar, as a person sees it.
fn in_window(mut s: State, size: (f32, f32)) -> rune::Runtime<State> {
    s.home.home_open = true;
    let mut rt = rune::Runtime::new(s, crate::view, size, rune::Env::new(true, 1.0));
    for _ in 0..16 {
        rt.frame(0.25);
    }
    rt
}

#[test]
fn in_the_window_a_home_with_room_grows_its_cover_and_stays_loose() {
    // In the window the scroll pane stretches its content to its own height; Home measures its sections, not that.
    let rt = in_window(on(), (1215.0, 1400.0));
    let fit = rt.state.home.fit;
    let (pane, column) = (fit.pane_height.expect("measured the pane"), fit.column.expect("measured the column").0);
    assert!(column < pane - 20.0, "the page's own height, not the pane's: {column} in {pane}");
    assert!(!fit.tight && fit.grow.unwrap() > 1.0, "{fit:?}");
    // And a crowded Home in a short window still tightens there, and scrolls in the window rather than past it.
    let short = in_window(crowded(), (1215.0, 700.0)).state.home.fit;
    assert!(short.tight && short.grow == Some(COVER_MIN_GROW), "{short:?}");
}
