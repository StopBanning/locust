//! homeCoverStatus.test.ts, home-fits-two-rows-of-teammates.test.ts, home-holds-still-when-tight-spacing-shrinks-it,
//! the pure half of the-cover-grows-into-spare-height.test.tsx and the coverScale cases of
//! the-home-screen-is-the-cover.test.tsx, case for case.
use super::*;

// --- homeCoverStatus.test.ts: what the cover's glass says while work runs (0.610)

#[test]
fn names_who_is_working_and_who_waits_on_you_waiting_first() {
    assert_eq!(glass_status(&["Codex"], &["Casper"]).as_deref(), Some("Casper waiting on you · Codex working"));
    assert_eq!(glass_status(&["Codex"], &[]).as_deref(), Some("Codex working"));
    assert_eq!(glass_status(&["Codex", "Robin"], &[]).as_deref(), Some("Codex working · Robin working"));
}

#[test]
fn counts_past_two_so_it_fits_the_glass() {
    assert_eq!(glass_status(&["Codex", "Robin", "Bro"], &["Casper"]).as_deref(), Some("1 waiting on you · 3 working"));
}

#[test]
fn says_nothing_when_nothing_runs_or_waits_the_glass_keeps_its_claim() {
    assert_eq!(glass_status(&[], &[]), None);
}

// --- home-fits-two-rows-of-teammates.test.ts (0.583), measured at 1209x782: sections 20 + 227 + 49 + 30, five
// children, the cover 816px wide.
const SECTIONS: f64 = 20.0 + 227.0 + 49.0 + 30.0;
const PANE_782: f64 = 608.0;
fn footprint_816() -> f64 {
    (COVER_HEIGHT + COVER_REACH) * (816.0 / 960.0)
}

// case: is short at its normal spacing, so it tightens
#[test]
fn is_short_at_its_normal_spacing_so_it_tightens() {
    assert!(home_is_short(SECTIONS, 5, footprint_816(), PANE_782));
}

// case: then fits: tight spacing saves enough with the cover at its floor
#[test]
fn then_fits_tight_spacing_saves_enough_with_the_cover_at_its_floor() {
    let tight = SECTIONS + footprint_816() * COVER_MIN_GROW + HOME_TIGHT_GAP * 4.0 + HOME_TIGHT_PAD * 2.0;
    assert!(tight <= PANE_782, "{tight}");
}

// case: is not short with room to spare (control), nor with nothing measured
#[test]
fn is_not_short_with_room_to_spare_control_nor_with_nothing_measured() {
    assert!(!home_is_short(SECTIONS, 5, footprint_816(), 760.0));
    assert!(!home_is_short(f64::NAN, 5, footprint_816(), PANE_782));
    assert!(!home_is_short(SECTIONS, 5, footprint_816(), 0.0));
}

// case: counts the bots' reach above the cover, which the old measure left out
#[test]
fn counts_the_bots_reach_above_the_cover_which_the_old_measure_left_out() {
    let box_only = COVER_HEIGHT * (816.0 / 960.0);
    assert!(!home_is_short(SECTIONS, 5, box_only, 630.0));
    assert!(home_is_short(SECTIONS, 5, footprint_816(), 630.0));
}

// --- home-holds-still-when-tight-spacing-shrinks-it.test.ts (0.838): three teammates, loose 409px, tight 395px.
const FOOTPRINT: f64 = 220.0;
fn pane() -> f64 {
    (400..1200).map(f64::from).find(|&p| home_is_short(409.0, 4, FOOTPRINT, p) && !home_is_short(395.0, 4, FOOTPRINT, p)).unwrap_or(0.0)
}
fn step(seen: HomeSpacingSeen, tight_now: bool, pane: f64) -> (HomeSpacingSeen, bool) {
    let drawn = if tight_now { 395.0 } else { 409.0 };
    let (seen, sections) = sections_at_normal_spacing(seen, 4, drawn, tight_now);
    (seen, home_is_short(sections, 4, FOOTPRINT, pane))
}

#[test]
fn the_two_heights_argue_each_other_short_at_the_normal_spacing_not_short_as_drawn_tight() {
    let pane = pane();
    assert!(pane > 0.0);
    assert!(home_is_short(409.0, 4, FOOTPRINT, pane));
    assert!(!home_is_short(395.0, 4, FOOTPRINT, pane));
}

#[test]
fn goes_tight_once_and_stays_tight() {
    let pane = pane();
    let (mut seen, mut tight) = (HomeSpacingSeen::default(), false);
    let mut classes = Vec::new();
    for _ in 0..12 {
        (seen, tight) = step(seen, tight, pane);
        classes.push(tight);
    }
    assert_eq!(classes, vec![true; 12]);
}

#[test]
fn adds_back_what_tight_took_and_only_while_tight() {
    let (seen, loose) = sections_at_normal_spacing(HomeSpacingSeen::default(), 4, 409.0, false);
    assert_eq!(loose, 409.0);
    let (seen, tight) = sections_at_normal_spacing(seen, 4, 395.0, true);
    assert_eq!(tight, 409.0);
    // Content that grows while tight still counts in full.
    assert_eq!(sections_at_normal_spacing(seen, 4, 420.0, true).1, 434.0);
}

#[test]
fn a_new_count_of_sections_starts_over_rather_than_borrowing_an_old_height() {
    let (seen, _) = sections_at_normal_spacing(HomeSpacingSeen::default(), 4, 409.0, false);
    let (seen, sections) = sections_at_normal_spacing(seen, 5, 395.0, true);
    assert_eq!(sections, 395.0);
    assert_eq!(seen.took, None);
}

// --- the-cover-grows-into-spare-height.test.tsx, the rules (the drawn cases are Home's, in src/ui/home_cover_tests.rs)
fn base() -> f64 {
    COVER_HEIGHT * (760.0 / COVER_WIDTH)
}

#[test]
fn not_at_all_with_exactly_the_room_it_needs_or_none_measured() {
    assert_eq!(cover_grow_for(0.0, base()), 1.0);
    assert_eq!(cover_grow_for(f64::NAN, base()), 1.0);
}

#[test]
fn shrinks_by_what_the_page_is_short_down_to_0_85x_and_no_further() {
    assert_eq!(COVER_MIN_GROW, 0.85);
    assert_eq!(cover_grow_for(-base() * 0.1, base()), 0.9);
    assert_eq!(cover_grow_for(-120.0, base()), 0.85);
    assert_eq!(cover_grow_for(-base() * 10.0, base()), 0.85);
}

#[test]
fn by_the_room_it_is_given_relative_to_its_own_height() {
    assert_eq!(cover_grow_for(base() * 0.2, base()), 1.2);
}

#[test]
fn at_most_1_45x_however_much_room_there_is() {
    assert_eq!(COVER_MAX_GROW, 1.45);
    assert_eq!(cover_grow_for(base() * 10.0, base()), 1.45);
}

#[test]
fn a_page_that_fits_is_never_short_it_grows_past_some_air_or_stays_as_it_is() {
    assert_eq!(cover_room_for(800.0, 400.0, 200.0), 120.0);
    assert_eq!(cover_room_for(800.0, 560.0, 200.0), 0.0);
    assert_eq!(cover_room_for(800.0, 600.0, 200.0), 0.0);
}

#[test]
fn a_page_that_does_not_fit_is_short_by_what_it_lacks_and_a_little_air() {
    assert_eq!(cover_room_for(726.0, 590.0, 201.0), -89.0);
}

#[test]
fn measures_nothing_into_nothing() {
    assert_eq!(cover_room_for(f64::NAN, 0.0, 200.0), 0.0);
}

#[test]
fn is_the_narrowest_columns_floor_at_every_width_and_never_more_than_0_85x() {
    let footprint = |width: f64| (COVER_HEIGHT + COVER_REACH) * (width / COVER_WIDTH);
    let smallest = footprint(760.0) * COVER_MIN_GROW;
    assert_eq!(cover_floor(footprint(760.0)), 0.85);
    assert_eq!((footprint(840.0) * cover_floor(footprint(840.0))).round(), smallest.round());
    assert!(cover_floor(footprint(840.0)) < 0.85);
    assert_eq!(cover_floor(footprint(600.0)), 0.85);
    assert_eq!(cover_floor(f64::NAN), 0.85);
}

// --- the-home-screen-is-the-cover.test.tsx: the cover, drawn to the column

#[test]
fn is_the_cover_at_the_column_width_960_is_its_own_size_760_is_0_79_of_it() {
    assert_eq!(cover_scale(COVER_WIDTH), 1.0);
    assert_eq!(cover_scale(760.0), 0.792);
    assert_eq!(cover_scale(980.0), 1.021);
}

#[test]
fn starts_at_the_narrowest_column_before_it_has_been_measured() {
    assert_eq!(cover_scale(0.0), 0.792);
    assert_eq!(cover_scale(f64::NAN), 0.792);
}

// --- The cast and where it is placed (HomeCover.tsx's COVER_CAST, COVER_MACHINE, `drawn`, `originX`). The drawn Home
// is checked against these in src/ui/home_cover_tests.rs.

#[test]
fn the_cast_stands_on_the_bezel_in_the_cover_order() {
    for mate in &COVER_CAST {
        assert!((mate.y + COVER_FACE - COVER_MACHINE.y).abs() <= 18.0, "{} at {}", mate.key, mate.y);
    }
    assert_eq!(COVER_CAST.map(|m| m.key), ["wren", "atlas", "sable"]);
    assert_eq!(COVER_CAST.map(|m| m.bot), ["ghost", "prompt", "critter"]);
    assert_eq!(COVER_CAST.map(|m| m.state), ["default", "default", "sleeping"]);
    let ghost = COVER_CAST.iter().find(|m| m.bot == "ghost").unwrap();
    assert_eq!((ghost.hue, ghost.floats), (None, true));
    let sleeper = COVER_CAST.iter().find(|m| m.part == CoverPart::Sleeper).unwrap();
    assert_eq!((sleeper.bot, sleeper.hue), ("critter", Some("clay")));
}

#[test]
fn at_the_narrowest_column_the_ghost_is_68_across_at_213_by_minus_7() {
    let at = cover_place(760.0, 1.0);
    assert_eq!(at.drawn, 0.713);
    assert_eq!((at.faces[0].x, at.faces[0].y, at.faces[0].width), (213.0, -7.0, 68.0));
}

#[test]
fn the_machine_is_centred_on_its_card_at_every_growth_and_never_under_the_floor() {
    for grow in [1.0, 1.2, COVER_MAX_GROW] {
        let m = cover_place(760.0, grow).machine;
        assert!((m.x - (760.0 - m.width) / 2.0).abs() <= 1.0, "grow {grow}: {m:?}");
    }
    let own = cover_place(760.0, 1.0).machine;
    let short = cover_place(760.0, 0.5).machine;
    assert_eq!(short.width, (own.width * COVER_MIN_GROW).round());
    let big = cover_place(760.0, COVER_MAX_GROW).machine;
    assert!(big.x > 0.0 && big.x + big.width < 760.0, "{big:?}");
    assert!(big.width > COVER_MACHINE.width * (760.0 / COVER_WIDTH));
}

#[test]
fn the_claim_is_13px_at_the_covers_own_size_and_never_under_10_5() {
    assert_eq!(claim_size(1.0), 13.0);
    assert_eq!(claim_size(0.713), 10.5);
}

/// One measure as Home runs it, at 1209x782's numbers: the sections shrink 14px when tight.
fn fit_steps(pane: f64, loose: f64, tight_off: f64, steps: usize) -> Vec<HomeFit> {
    let (mut seen, mut tight, mut out) = (HomeSpacingSeen::default(), false, Vec::new());
    for _ in 0..steps {
        let sections = if tight { loose - tight_off } else { loose };
        let (next, fit) = home_fit(seen, 5, sections, tight, 816.0, pane);
        seen = next;
        tight = fit.tight;
        out.push(fit);
    }
    out
}

#[test]
fn a_measure_that_tightens_holds_and_shrinks_the_cover_to_fit() {
    let steps = fit_steps(PANE_782, SECTIONS, 14.0, 12);
    assert!(steps.iter().all(|f| f.tight), "{steps:?}");
    assert!(steps.iter().all(|f| f.grow < 1.0 && f.grow >= cover_floor(footprint_816())), "{steps:?}");
    assert!(steps.windows(2).skip(1).all(|w| w[0] == w[1]), "it moved: {steps:?}");
}

#[test]
fn a_measure_with_room_to_spare_stays_loose_and_grows() {
    let steps = fit_steps(1100.0, SECTIONS, 14.0, 6);
    assert!(steps.iter().all(|f| !f.tight && f.grow > 1.0), "{steps:?}");
}

#[test]
fn a_measure_in_the_band_goes_tight_once_and_holds() {
    // home-holds-still's numbers through the whole measure: a card whose footprint is 220, sections 409 loose and 395
    // tight, in a pane short at the normal spacing and not as drawn tight.
    let card = 220.0 / (COVER_HEIGHT + COVER_REACH) * COVER_WIDTH;
    let pane = (400..1200).map(f64::from).find(|&p| home_is_short(409.0, 4, 220.0, p) && !home_is_short(395.0, 4, 220.0, p)).unwrap();
    let (mut seen, mut tight) = (HomeSpacingSeen::default(), false);
    for step in 0..12 {
        let (next, fit) = home_fit(seen, 4, if tight { 395.0 } else { 409.0 }, tight, card, pane);
        seen = next;
        tight = fit.tight;
        assert!(tight, "step {step} went loose");
    }
}
