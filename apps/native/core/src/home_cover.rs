//! Home's cover and the room it is given (Electron `HomeCover.tsx`, `homeCoverStatus.ts`): how big the cover is
//! drawn, how much it grows into spare height or gives back when the page is short, when Home tightens its spacing,
//! and what the cover's glass says while work runs. Pure: no drawing, so the window and the tests read the same rules.

/// The cover's canvas, in its own units. Everything on it is placed on this grid.
pub const COVER_WIDTH: f64 = 960.0;
pub const COVER_HEIGHT: f64 = 254.0;
/// The narrowest reading column, and so the scale the cover starts at before it is measured.
const FIRST_WIDTH: f64 = 760.0;
/// The most the drawing grows into spare height: past this the machine reads as the page, not an object on it.
pub const COVER_MAX_GROW: f64 = 1.45;
/// The most it gives back when the page is short: below this the claim no longer fits its glass.
pub const COVER_MIN_GROW: f64 = 0.85;
/// The bots' reach above the cover's box, in the cover's units. The cover's footprint on the page is
/// (COVER_HEIGHT + COVER_REACH) at its scale; measured against the box alone a page "fitted" and ran 35px long.
pub const COVER_REACH: f64 = 48.0;
/// The machine on the cover: its widest part, which bounds how far the drawing may grow.
pub const COVER_MACHINE_WIDTH: f64 = 560.0;
/// Home draws the whole machine a size smaller (0.863), giving the space to the team below.
pub const COVER_HOME_SCALE: f64 = 0.9;
/// Home's normal spacing: the column's padding and the gap between its sections. Tight uses less.
pub const HOME_PAD: f64 = 32.0;
pub const HOME_GAP: f64 = 12.0;
pub const HOME_TIGHT_PAD: f64 = 12.0;
pub const HOME_TIGHT_GAP: f64 = 8.0;

fn thousandths(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// The most the drawing can grow and keep 3% of the card clear each side, read from the machine.
fn width_grow() -> f64 {
    0.94 * COVER_WIDTH / COVER_MACHINE_WIDTH
}

/// The cover drawn to fit a card this many pixels wide; before it is measured, the narrowest column's.
pub fn cover_scale(width: f64) -> f64 {
    if !width.is_finite() || width <= 0.0 {
        return thousandths(FIRST_WIDTH / COVER_WIDTH);
    }
    thousandths(width / COVER_WIDTH)
}

/// The smallest cover is one size at every width (0.799): the narrowest column's floor, never above COVER_MIN_GROW.
/// A wider column once kept a bigger smallest cover, so 1366x768 scrolled where 1205x803 fitted.
pub fn cover_floor(footprint: f64) -> f64 {
    let smallest = (COVER_HEIGHT + COVER_REACH) * (FIRST_WIDTH / COVER_WIDTH) * COVER_MIN_GROW;
    if !footprint.is_finite() || footprint <= 0.0 {
        return COVER_MIN_GROW;
    }
    thousandths(COVER_MIN_GROW.min(smallest / footprint))
}

/// The grow factor for this much spare height above a cover of this height; short of room, below 1.
pub fn cover_grow_for(room: f64, base_height: f64) -> f64 {
    if !room.is_finite() || !base_height.is_finite() || base_height <= 0.0 || room == 0.0 {
        return 1.0;
    }
    if room < 0.0 {
        return thousandths(cover_floor(base_height).max(1.0 + room / base_height));
    }
    thousandths(COVER_MAX_GROW.min(width_grow()).min(1.0 + room / base_height))
}

/// The room `cover_grow_for` is given: a page that fits grows past some kept air, or stays as it is; only a page
/// that does not fit is short, by what it lacks and a little air (0.402: the air once read as a shortfall).
pub fn cover_room_for(usable: f64, others: f64, base: f64) -> f64 {
    let spare = usable - others - base;
    if !spare.is_finite() {
        return 0.0;
    }
    if spare >= 0.0 {
        return (spare - 48.0_f64.max(usable * 0.1)).max(0.0);
    }
    spare - 24.0
}

/// What the cover is finally drawn at: its width's scale, grown (or given back) and held between the floor and the
/// width's limit, a size smaller on Home.
pub fn cover_drawn(scale: f64, grow: f64) -> f64 {
    let floor = cover_floor((COVER_HEIGHT + COVER_REACH) * scale);
    thousandths(scale * grow.max(floor).min(COVER_MAX_GROW).min(width_grow()) * COVER_HOME_SCALE)
}

/// Home is short when, at its NORMAL spacing, even the smallest cover does not fit; then its spacing tightens.
/// Decided from what the spacing cannot move, so tightening never argues itself back off.
pub fn home_is_short(sections: f64, count: usize, footprint: f64, pane_height: f64) -> bool {
    if ![sections, footprint, pane_height].iter().all(|v| v.is_finite()) || pane_height <= 0.0 {
        return false;
    }
    let gaps = HOME_GAP * count.saturating_sub(1) as f64;
    sections + footprint * cover_floor(footprint) + gaps + HOME_PAD * 2.0 > pane_height + 0.5
}

/// What Home has seen of its sections' spacing, for one count of sections.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HomeSpacingSeen {
    pub count: Option<usize>,
    /// The sections' height last measured at the normal spacing.
    pub loose: Option<f64>,
    /// How much tight spacing took off them, measured as it went tight.
    pub took: Option<f64>,
    /// Tight when last measured; `None` before any measure, which is not the same as loose.
    pub was_tight: Option<bool>,
}

/// The sections at the normal spacing, even while tight (0.838). Tight spacing also shortens the sections, so measured
/// tight they read shorter, argued for loosening, grew back, and Home flicked every frame. While tight, what the
/// spacing took is added back, measured the moment it went tight; a new count of sections starts over.
pub fn sections_at_normal_spacing(seen: HomeSpacingSeen, count: usize, sections: f64, tight_now: bool) -> (HomeSpacingSeen, f64) {
    let base = if seen.count == Some(count) { seen } else { HomeSpacingSeen { count: Some(count), ..HomeSpacingSeen::default() } };
    if !tight_now {
        return (HomeSpacingSeen { count: Some(count), loose: Some(sections), took: base.took, was_tight: Some(false) }, sections);
    }
    let took = match (base.was_tight, base.loose) {
        (Some(false), Some(loose)) => Some((loose - sections).max(0.0)),
        _ => base.took,
    };
    (HomeSpacingSeen { count: Some(count), loose: base.loose, took, was_tight: Some(true) }, sections + took.unwrap_or(0.0))
}

/// What the cover's glass says while work runs (0.610): who waits on you, then who is working. Two names at most;
/// past that, counts, so the line fits the glass. Nothing running or waiting: `None`, and the glass keeps its claim.
pub fn glass_status(working: &[&str], waiting: &[&str]) -> Option<String> {
    if working.len() + waiting.len() == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if working.len() + waiting.len() <= 2 {
        parts.extend(waiting.iter().map(|name| format!("{name} waiting on you")));
        parts.extend(working.iter().map(|name| format!("{name} working")));
    } else {
        if !waiting.is_empty() {
            parts.push(format!("{} waiting on you", waiting.len()));
        }
        if !working.is_empty() {
            parts.push(format!("{} working", working.len()));
        }
    }
    Some(parts.join(" · "))
}

#[cfg(test)]
#[path = "home_cover_tests.rs"]
mod tests;
