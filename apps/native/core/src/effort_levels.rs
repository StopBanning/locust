//! Reasoning effort in words and as a scale: Electron's `effortLevels.ts`, `effortScale.ts` and the geometry of
//! `components/EffortSlider.tsx` (Claude Code's effort control: "Effort Max", Faster to Smarter, a track with a stop
//! for every level, a 16 x 20 white thumb).
//!
//! The levels are not ours: they come from whatever the runtime listed. A level this file knows gets its word and its
//! cost; one it does not is shown as the runtime wrote it, capitalised, with NO description rather than a guess.

/// effortLevels.ts DESCRIPTIONS: about time and cost, never about how good the answer will be.
fn description_of(level: &str) -> Option<&'static str> {
    Some(match level {
        "minimal" => "fastest, cheapest",
        "low" => "fast, cheap",
        "fast" => "lowest latency",
        "medium" => "in between",
        "balanced" => "in between",
        "high" => "slower, costlier",
        "high-fast" => "slower, on the fast variant",
        "low-fast" => "fast, on the fast variant",
        "medium-fast" => "in between, on the fast variant",
        "max" => "slowest, costliest",
        "xhigh" => "slowest, costliest",
        "xhigh-fast" => "slowest, on the fast variant",
        _ => return None,
    })
}

/// effortLevels.ts NAMES: Claude Code's words for the levels runtimes list as keys.
fn name_of(level: &str) -> Option<&'static str> {
    Some(match level {
        "none" => "None",
        "minimal" => "Minimal",
        "low" => "Low",
        "medium" => "Medium",
        "balanced" => "Balanced",
        "high" => "High",
        "xhigh" => "Extra high",
        "max" => "Max",
        "ultra" => "Ultra",
        "fast" => "Fast",
        _ => return None,
    })
}

/// effortName: "Max", "Extra high", "Medium"; a word this build does not know keeps the runtime's own, capitalised.
pub fn effort_name(level: &str) -> String {
    if let Some(known) = name_of(&level.to_lowercase()) {
        return known.to_owned();
    }
    let mut chars = level.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// effortDescription: what the level costs, or `None` when this build cannot say. Not case sensitive.
pub fn effort_description(level: &str) -> Option<&'static str> {
    description_of(&level.to_lowercase())
}

/// effortFooter: the line under the control, only where something true can be said about the runtime.
pub fn effort_footer(runtime: &str) -> Option<&'static str> {
    (runtime == "cursor").then_some("Cursor puts the effort in the model name.")
}

/// effortScale.ts splitEffort: `high-fast` is the base `high` on its fast variant.
pub fn split_effort(level: &str) -> (&str, bool) {
    match level.strip_suffix("-fast") {
        Some(base) if !base.is_empty() => (base, true),
        _ => (level, false),
    }
}

/// effortScale.ts EFFORT_ORDER: least to most. A slider's meaning is "right is more", so the scale is sorted.
const EFFORT_ORDER: [&str; 7] = ["none", "minimal", "low", "medium", "high", "xhigh", "max"];

pub fn effort_rank(base: &str) -> Option<usize> {
    EFFORT_ORDER.iter().position(|known| *known == base)
}

/// effortScale.ts EffortScale: the base levels, lowest first, and whether any has a fast variant (the switch).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EffortScale {
    pub bases: Vec<String>,
    pub has_fast: bool,
}

/// effortScale: known words in their order, unknown ones after them in the runtime's own order.
pub fn effort_scale(supported: &[String]) -> EffortScale {
    let mut bases: Vec<String> = Vec::new();
    let mut has_fast = false;
    for level in supported {
        let (base, fast) = split_effort(level);
        has_fast |= fast;
        if !bases.iter().any(|b| b == base) {
            bases.push(base.to_owned());
        }
    }
    let seen: Vec<String> = bases.clone();
    let first = |b: &str| seen.iter().position(|s| s == b).unwrap_or(0);
    bases.sort_by(|a, b| match (effort_rank(a), effort_rank(b)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => first(a).cmp(&first(b)),
    });
    EffortScale { bases, has_fast }
}

/// joinEffort: the slider and the switch together as a level the runtime listed, never one it did not.
pub fn join_effort(base: &str, fast: bool, supported: &[String]) -> Option<String> {
    let wanted = if fast { format!("{base}-fast") } else { base.to_owned() };
    if supported.contains(&wanted) {
        return Some(wanted);
    }
    supported.iter().any(|s| s == base).then(|| base.to_owned())
}

/// levelsLine: a model's levels in words for a picker row's one line: "Low, High", "Low to Max · Fast".
pub fn levels_line(supported: &[String]) -> Option<String> {
    let EffortScale { bases, has_fast } = effort_scale(supported);
    if bases.is_empty() {
        return None;
    }
    let words: Vec<String> = bases.iter().map(|b| effort_name(b)).collect();
    let span = if words.len() <= 3 { words.join(", ") } else { format!("{} to {}", words[0], words[words.len() - 1]) };
    Some(if has_fast { format!("{span} · Fast") } else { span })
}

/// status.ts defaultEffort: the runtime's own default when it lists it, else medium, else the middle level.
pub fn default_effort(supported: &[String], runtime_default: Option<&str>) -> Option<String> {
    if supported.is_empty() {
        return None;
    }
    if let Some(d) = runtime_default.filter(|d| supported.iter().any(|s| s == d)) {
        return Some(d.to_owned());
    }
    if supported.iter().any(|s| s == "medium") {
        return Some("medium".to_owned());
    }
    supported.get(supported.len() / 2).cloned()
}

/// EffortSlider.tsx EFFORT_TRACK_WIDTH: the track's inner width.
pub const EFFORT_TRACK_WIDTH: f32 = 216.0;
/// EffortSlider.tsx THUMB_WIDTH: Claude Code's thumb, 16 wide (and 20 tall, the track's height).
pub const THUMB_WIDTH: f32 = 16.0;
/// The thumb's height and the track's: 20, measured off Claude Code's frame (shell.css).
pub const TRACK_HEIGHT: f32 = 20.0;
/// The thumb's corners (shell.css `.lc-effortpanel__thumb`).
pub const THUMB_RADIUS: f32 = 4.0;
/// Stops sit this far in from each end of the track.
const INSET: f32 = THUMB_WIDTH / 2.0 + 5.0;

/// EffortSlider.tsx MOVE: squash and stretch, and no tail (Colin, 2026-09-26: "no tail").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Move {
    pub springiness: f32,
    pub wobble: f32,
    pub stretch: f32,
    pub trail: f32,
}
pub const MOVE: Move = Move { springiness: 0.85, wobble: 0.2, stretch: 0.6, trail: 0.0 };

/// stopPosition: where stop `index` of `count` sits along the track, from its left edge.
pub fn stop_position(index: usize, count: usize) -> f32 {
    if count < 2 {
        return EFFORT_TRACK_WIDTH / 2.0;
    }
    INSET + (index as f32 * (EFFORT_TRACK_WIDTH - 2.0 * INSET)) / (count - 1) as f32
}

/// heldAt: a held thumb is where the pointer is along the track, kept between the first stop and the last.
pub fn held_at(pointer_x: f32, track_left: f32) -> f32 {
    (pointer_x - track_left).max(INSET).min(EFFORT_TRACK_WIDTH - INSET)
}

/// nearestStop: the stop nearest a place on the track; never one that is not there.
pub fn nearest_stop(x: f32, count: usize) -> usize {
    if count < 2 {
        return 0;
    }
    let spacing = (EFFORT_TRACK_WIDTH - 2.0 * INSET) / (count - 1) as f32;
    // Math.round rounds halves up; f32::round rounds them away from zero. Floor of x + 0.5 is Math.round.
    let at = ((x - INSET) / spacing + 0.5).floor();
    at.max(0.0).min((count - 1) as f32) as usize
}

#[cfg(test)]
#[path = "effort_levels_tests.rs"]
mod tests;
