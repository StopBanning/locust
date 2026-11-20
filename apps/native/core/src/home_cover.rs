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

// --- The machine and its cast, on the cover's grid (HomeCover.tsx `COVER_MACHINE`, `COVER_CAST`, `FACE`) ---

/// A box on the cover's grid, or in pixels once placed: left, top, width, height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// The machine: the boot screen's bezel and glass, centred on the cover. The lockup and the claim are on its glass.
pub const COVER_MACHINE: CoverBox = CoverBox { x: 200.0, y: 86.0, width: COVER_MACHINE_WIDTH, height: 162.0 };
/// How big each bot is on the cover's grid.
pub const COVER_FACE: f64 = 96.0;

/// Each bot's part on the title screen (`cover_script`): Prompt works, the ghost thinks, the locust sleeps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverPart {
    Worker,
    Thinker,
    Sleeper,
}

/// One of the three on the machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverBot {
    pub key: &'static str,
    /// The bot's shape (`bot_spec`'s names).
    pub bot: &'static str,
    /// A teammate hue, or none for the shape's own colour (the ghost's white).
    pub hue: Option<&'static str>,
    /// The rig's state once the cover is on (`BotAvatarState`'s names).
    pub state: &'static str,
    /// Floats on a slow bob instead of hopping.
    pub floats: bool,
    pub part: CoverPart,
    /// Top-left on the cover's grid.
    pub x: f64,
    pub y: f64,
}

/// The cast, in the cover's order: a white ghost that floats and thinks; Prompt, the Codex nod, in the teammate blue,
/// working; and Claw'd's critter in clay, asleep (0.583: the Hopper's thin legs stepped at this size, jagged in Plush).
/// Each box sits so its body stands on the bezel's top edge (Prompt's body fills its box, so it sits 4 higher).
pub const COVER_CAST: [CoverBot; 3] = [
    CoverBot { key: "wren", bot: "ghost", hue: None, state: "default", floats: true, part: CoverPart::Thinker, x: 245.0, y: -10.0 },
    CoverBot { key: "atlas", bot: "prompt", hue: Some("blue"), state: "default", floats: false, part: CoverPart::Worker, x: 432.0, y: -4.0 },
    CoverBot { key: "sable", bot: "critter", hue: Some("clay"), state: "sleeping", floats: false, part: CoverPart::Sleeper, x: 619.0, y: -2.0 },
];

/// The cover placed on a card: everything in whole pixels from the card's top-left, except the card's own height and
/// the reach kept above it, which scale as the stylesheet's `calc(254px * k)` and `calc(48px * k)` do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverPlace {
    /// The card's width's scale (`cover_scale`).
    pub scale: f64,
    /// What everything is drawn at (`cover_drawn`).
    pub drawn: f64,
    /// Half of what the canvas outgrew (or gave back), so the drawing grows about the card's centre.
    pub origin_x: f64,
    pub machine: CoverBox,
    /// Each of `COVER_CAST`'s boxes, in its order; width and height are the bot's size.
    pub faces: [CoverBox; 3],
    pub height: f64,
    pub reach: f64,
}

/// The cover on a card this wide, grown by `grow` (`cover_grow_for`): HomeCover.tsx's `drawn`, `originX`, `at`, `atX`.
pub fn cover_place(card_width: f64, grow: f64) -> CoverPlace {
    let scale = cover_scale(card_width);
    let drawn = cover_drawn(scale, grow);
    let origin_x = ((scale - drawn) * COVER_WIDTH / 2.0).round();
    let at = |v: f64| (v * drawn).round();
    let machine = CoverBox { x: origin_x + at(COVER_MACHINE.x), y: at(COVER_MACHINE.y), width: at(COVER_MACHINE.width), height: at(COVER_MACHINE.height) };
    let face = |bot: &CoverBot| CoverBox { x: origin_x + at(bot.x), y: at(bot.y), width: at(COVER_FACE), height: at(COVER_FACE) };
    CoverPlace {
        scale,
        drawn,
        origin_x,
        machine,
        faces: [face(&COVER_CAST[0]), face(&COVER_CAST[1]), face(&COVER_CAST[2])],
        height: COVER_HEIGHT * drawn,
        reach: COVER_REACH * drawn,
    }
}

/// The claim's type size on the glass: 13px at the cover's own size, never under 10.5px (`.lc-cover__claim`).
pub fn claim_size(drawn: f64) -> f64 {
    (13.0 * drawn).max(10.5)
}

// --- One measure of Home, as FirstLaunch.tsx runs it on every resize ---

/// What one measure decides: whether Home's spacing is tight, and how much the cover grows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HomeFit {
    pub tight: bool,
    pub grow: f64,
}

/// Home's padding and gap at a spacing.
pub fn home_spacing(tight: bool) -> (f64, f64) {
    if tight { (HOME_TIGHT_PAD, HOME_TIGHT_GAP) } else { (HOME_PAD, HOME_GAP) }
}

/// FirstLaunch.tsx's measure step. `sections` is everything in the column but the cover, each with its margins, as
/// drawn now; `count` the column's children, the cover among them; `card_width` the cover's card; `pane_height` the
/// pane Home scrolls in. Tight or not is decided at the normal spacing (`sections_at_normal_spacing`, `home_is_short`)
/// so it never argues itself back off; the room is measured from what does not move with the cover, so the two never
/// chase each other.
///
/// The cover's footprint here is at its width's scale, without Home's `COVER_HOME_SCALE` (as Electron's): the cover is
/// drawn 10% smaller than this counts it, so Home tightens and gives back a little early and keeps that as air.
pub fn home_fit(seen: HomeSpacingSeen, count: usize, sections: f64, tight_now: bool, card_width: f64, pane_height: f64) -> (HomeSpacingSeen, HomeFit) {
    let (pad, gap) = home_spacing(tight_now);
    let usable = pane_height - pad * 2.0;
    let base = (COVER_HEIGHT + COVER_REACH) * cover_scale(card_width);
    let others = sections + gap * count.saturating_sub(1) as f64;
    let (seen, normal) = sections_at_normal_spacing(seen, count, sections, tight_now);
    let tight = home_is_short(normal, count, base, pane_height);
    (seen, HomeFit { tight, grow: cover_grow_for(cover_room_for(usable, others, base), base) })
}

#[cfg(test)]
#[path = "home_cover_tests.rs"]
mod tests;
