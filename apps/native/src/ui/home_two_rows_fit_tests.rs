//! Electron `home-fits-two-rows-of-teammates.test.ts` (T957), drawn in the real window at
//! `1209x782` with six teammates (two rows of three), folded ready accounts, and an open project
//! folder: the entire Home page fits inside the pane with no vertical overflow.

use super::*;
use locust_core::agents::{Auth, Found, Probe};
use locust_core::home_cover::{COVER_HEIGHT, COVER_MIN_GROW, COVER_REACH, home_is_short};

// Measured in Electron at 1209x782 (0.582): sections 20 + 227 + 49 + 30, five children, cover 816px wide.
const ELECTRON_SECTIONS: f64 = (20 + 227 + 49 + 30) as f64;
const ELECTRON_FOOTPRINT: f64 = (COVER_HEIGHT + COVER_REACH) * (816.0 / 960.0);
const ELECTRON_PANE_782: f64 = 608.0;

fn ready_agent(id: &str, name: &str) -> Found {
    Found {
        id: id.into(),
        display_name: name.into(),
        installed: true,
        version: Some("1.0.0".into()),
        auth: Auth::Authenticated,
        ready: true,
        status: Probe::Ready,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

fn six_teammates_home() -> State {
    let mut s = State::default();
    s.home.home_open = true;
    s.home.folder_path = Some("/Users/locust/work/locust".into());
    s.home.folder_invented = false;
    s.settings_panel.agents = Some(vec![
        ready_agent("codex", "Codex CLI"),
        ready_agent("claude", "Claude Code"),
    ]);
    crate::team_scene(&mut s);
    for mate in &mut s.teammates {
        mate.busy = false;
    }
    let more: Vec<_> = s
        .teammates
        .iter()
        .cloned()
        .map(|mut m| {
            m.id = format!("{}-2", m.id);
            m.name = format!("{} Two", m.name);
            m
        })
        .collect();
    s.teammates.extend(more);
    assert_eq!(s.teammates.len(), 6);
    s
}

fn run_window(height: f32) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(
        six_teammates_home(),
        crate::view,
        (1209.0, height),
        rune::Env::new(true, 1.0),
    );
    for _ in 0..16 {
        rt.frame(0.25);
    }
    rt
}

// case: is short at its normal spacing, so it tightens
#[test]
fn home_at_1209x782_is_short_at_normal_spacing_so_it_tightens() {
    assert!(home_is_short(ELECTRON_SECTIONS, 5, ELECTRON_FOOTPRINT, ELECTRON_PANE_782));
    let rt = run_window(782.0);
    assert!(
        rt.state.home.fit.tight,
        "six teammates in two rows at 1209x782 should tighten: {:?}",
        rt.state.home.fit
    );
}

// case: then fits: tight spacing saves enough with the cover at its floor
#[test]
fn six_teammates_in_two_rows_fit_in_the_1209x782_window_with_no_scroll() {
    let electron_tight = ELECTRON_SECTIONS + ELECTRON_FOOTPRINT * COVER_MIN_GROW + 8.0 * 4.0 + 12.0 * 2.0;
    assert!(electron_tight <= ELECTRON_PANE_782);

    let rt = run_window(782.0);
    let fit = rt.state.home.fit;
    let pane = fit.pane_height.expect("pane measured");
    let (column_h, _, _, _) = fit.column.expect("column measured");
    assert!(
        column_h <= pane + 1.0,
        "six teammates in two rows must fit at 1209x782 without scrolling: column {column_h:.1}px > pane {pane:.1}px (fit={fit:?})"
    );
    // All 6 teammate cards are drawn in 2 distinct rows, all inside the pane's vertical bounds.
    let pane_rect = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("home-pane"))
        .expect("home-pane")
        .rect;
    let mut ys = std::collections::BTreeSet::new();
    let cards: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| {
            e.key.as_deref().is_some_and(|k| {
                k.starts_with("home-card-")
                    && !k.contains("-route-")
                    && !k.contains("-model-")
                    && !k.contains("-mark-")
                    && !k.contains("-free-")
                    && k != "home-card-watch"
            })
        })
        .collect();
    assert_eq!(cards.len(), 6, "all 6 teammate cards drawn");
    for card in &cards {
        ys.insert(card.rect[1].round() as i32);
        assert!(
            card.rect[1] + card.rect[3] <= pane_rect[1] + pane_rect[3] + 1.0,
            "card {:?} overflows pane {:?}: {:?}",
            card.key,
            pane_rect,
            card.rect
        );
    }
    assert_eq!(ys.len(), 2, "two rows of three teammates: {ys:?}");
}

// case: is not short with room to spare (control), nor with nothing measured
#[test]
fn home_is_not_short_with_room_to_spare_nor_with_nothing_measured() {
    assert!(!home_is_short(ELECTRON_SECTIONS, 5, ELECTRON_FOOTPRINT, 760.0));
    assert!(!home_is_short(f64::NAN, 5, ELECTRON_FOOTPRINT, ELECTRON_PANE_782));
    assert!(!home_is_short(ELECTRON_SECTIONS, 5, ELECTRON_FOOTPRINT, 0.0));

    let rt = run_window(1100.0);
    assert!(
        !rt.state.home.fit.tight,
        "tall window with room to spare stays loose: {:?}",
        rt.state.home.fit
    );
}

// case: counts the bots' reach above the cover, which the old measure left out
#[test]
fn home_measure_counts_the_bots_reach_above_the_cover() {
    let box_only = COVER_HEIGHT * (816.0 / 960.0);
    assert!(!home_is_short(ELECTRON_SECTIONS, 5, box_only, 630.0));
    assert!(home_is_short(ELECTRON_SECTIONS, 5, ELECTRON_FOOTPRINT, 630.0));
}

#[test]
fn tight_home_shrinks_card_and_starter_heights_compared_to_loose() {
    let measure = |height: f32| -> (bool, f32, f32) {
        let mut rt = rune::Runtime::new(
            six_teammates_home(),
            |s, _| home_view(s, &Palette::new(true)),
            (1209.0, height),
            rune::Env::new(true, 1.0),
        );
        for _ in 0..16 {
            rt.frame(0.25);
        }
        let card_h = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("home-card-codec"))
            .expect("codec card")
            .rect[3];
        let starter_h = rt
            .elements()
            .into_iter()
            .find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("starter-")))
            .expect("starter")
            .rect[3];
        (rt.state.home.fit.tight, card_h, starter_h)
    };
    let (short_tight, short_card, short_starter) = measure(580.0);
    let (tall_tight, tall_card, tall_starter) = measure(1400.0);
    assert!(short_tight && !tall_tight);
    assert!(short_card < tall_card, "tight card {short_card} < loose {tall_card}");
    assert!(short_starter < tall_starter, "tight starter {short_starter} < loose {tall_starter}");
}
