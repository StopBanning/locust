//! The model picker's rows: Electron's `status.ts` `orderRouteRows` (with `RUNTIME_BAND` and `flagshipRank`) and
//! `nextFreeModel`, `RoutePicker.tsx`'s one-line detail, full hover title and older-versions fold (`unfoldedRows`,
//! `olderByGroup`), `routeName.ts` `freeTagOf`, and `missionView.ts` `quietFreeRun`.

use crate::effort_levels::levels_line;
use crate::engine::Model;

/// One row of the picker, as `orderRouteRows` reads it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RouteRow {
    pub key: String,
    pub group: String,
    pub model: Option<String>,
    /// READY, SIGN IN, AT LIMIT, NOT INSTALLED, PLANNED, EXPERIMENTAL, or ACTIVE on the route in use.
    pub tag: Option<String>,
}

/// status.ts OWN_MODELS_GROUP: the picker's group for the models a person added themselves.
pub const OWN_MODELS_GROUP: &str = "Your models";

/// status.ts RUNTIME_BAND: what can run first; signing in is a press away, installing a download.
fn runtime_band(tag: &str) -> Option<u32> {
    Some(match tag {
        "READY" => 0,
        "EXPERIMENTAL" => 1,
        "SIGN IN" | "AT LIMIT" => 2,
        "NOT INSTALLED" => 3,
        "PLANNED" => 4,
        _ => return None,
    })
}

/// status.ts FLAGSHIP_MODELS, as case-insensitive patterns: where a row sits among the rest of its group.
pub fn flagship_rank(model_id: &str) -> Option<usize> {
    let id = model_id.to_lowercase();
    let tests: [&dyn Fn(&str) -> bool; 10] = [
        &|s| s == "auto",
        &|s| s.contains("grok-4.6"),
        &|s| s.contains("composer-2"),
        &|s| s.contains("opus-5"),
        &|s| s.contains("fable"),
        &|s| s.contains("sonnet-5"),
        &|s| s.contains("gpt-5.3-codex"),
        &|s| s.contains("gemini-3"),
        &|s| s.contains("gpt-6"),
        &|s| s.contains("gpt-5.6"),
    ];
    tests.iter().position(|t| t(&id))
}

/// orderRouteRows: groups by what can run (a group on its BEST row), the person's own models leading their band, then
/// alphabetical; inside a group the route in use first, then what was used most recently, the flagships, the rest.
/// Returns the rows' indexes in their new order.
pub fn order_route_rows(rows: &[RouteRow], recent: &[String]) -> Vec<usize> {
    // A group on its BEST row, read once per group (Electron's groupBand).
    let mut bands: std::collections::HashMap<&str, u32> = std::collections::HashMap::new();
    for r in rows {
        let band = match r.tag.as_deref() {
            Some("ACTIVE") => 0,
            tag => runtime_band(tag.unwrap_or("")).unwrap_or(3),
        };
        let best = bands.entry(r.group.as_str()).or_insert(9);
        *best = (*best).min(band);
    }
    let band_of = |group: &str| -> u32 { bands.get(group).copied().unwrap_or(9) };
    let score = |row: &RouteRow| -> u64 {
        if row.tag.as_deref() == Some("ACTIVE") {
            return 0;
        }
        if row.model.as_deref() == Some("kilo/kilo-auto/free") {
            return 1 + 500_000;
        }
        if let Some(used) = recent.iter().position(|k| *k == row.key) {
            return 1 + used as u64;
        }
        1 + match flagship_rank(row.model.as_deref().unwrap_or(&row.key)) {
            Some(f) => 1_000_000 + f as u64,
            None => 2_000_000,
        }
    };
    let mut order: Vec<usize> = (0..rows.len()).collect();
    // A stable sort, as Array.prototype.sort is: equal rows keep their listed order.
    order.sort_by(|&a, &b| {
        let (l, r) = (&rows[a], &rows[b]);
        band_of(&l.group)
            .cmp(&band_of(&r.group))
            .then_with(|| (r.group == OWN_MODELS_GROUP).cmp(&(l.group == OWN_MODELS_GROUP)))
            .then_with(|| l.group.to_lowercase().cmp(&r.group.to_lowercase()))
            .then_with(|| score(l).cmp(&score(r)))
    });
    order
}

/// routeName.ts freeTagOf: a trailing " Free" is drawn as its own tag, outside the part of the name that is cut.
pub fn free_tag_of(label: &str) -> (String, bool) {
    let n = label.chars().count();
    if n > 5 && label.to_lowercase().ends_with(" free") {
        (label.chars().take(n - 5).collect(), true)
    } else {
        (label.to_owned(), false)
    }
}

/// RoutePicker's row facts: what the fold and the search read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FoldRow {
    pub group: String,
    pub label: String,
    pub older: bool,
}

/// unfoldedRows: older versions stay folded until a search reaches into them; a Recent shortcut is never folded.
pub fn unfolded_rows(rows: &[FoldRow], searching: bool) -> Vec<&FoldRow> {
    rows.iter().filter(|r| searching || !r.older || r.group == "Recent").collect()
}

/// olderByGroup: each group's older versions, for its fold, in the order groups first hold one.
pub fn older_by_group(rows: &[FoldRow]) -> Vec<(String, Vec<&FoldRow>)> {
    let mut out: Vec<(String, Vec<&FoldRow>)> = Vec::new();
    for row in rows.iter().filter(|r| r.older && r.group != "Recent") {
        match out.iter_mut().find(|(g, _)| *g == row.group) {
            Some((_, list)) => list.push(row),
            None => out.push((row.group.clone(), vec![row])),
        }
    }
    out
}

/// RoutePicker's row line: what the catalogue says and what the route resolved to; a model the catalogue says nothing
/// about keeps its levels, in words ("Low to Max · Fast").
pub fn row_detail(description: Option<&str>, resolved: Option<&str>, efforts: &[String]) -> String {
    let said: Vec<&str> = [description, resolved].into_iter().flatten().filter(|p| !p.is_empty()).collect();
    if !said.is_empty() {
        return said.join(" · ");
    }
    levels_line(efforts).unwrap_or_else(|| efforts_said(efforts))
}

fn efforts_said(efforts: &[String]) -> String {
    if efforts.is_empty() {
        return "no effort levels reported".to_owned();
    }
    format!("{} effort level{} · {}", efforts.len(), if efforts.len() == 1 { "" } else { "s" }, efforts.join(", "))
}

/// RoutePicker's row hover: everything known about it, effort levels included ("label · fullDetail").
pub fn row_title(label: &str, description: Option<&str>, resolved: Option<&str>, efforts: &[String]) -> String {
    let efforts = efforts_said(efforts);
    let measured = match resolved {
        Some(name) => format!("{name} · {efforts}"),
        None => efforts,
    };
    match description.filter(|d| !d.is_empty()) {
        Some(d) => format!("{label} · {d} · {measured}"),
        None => format!("{label} · {measured}"),
    }
}

/// status.ts FREE_START_RUNTIME: the runtime whose free models need no account.
const FREE_START_RUNTIME: &str = "opencode";

/// nextFreeModel: the next of OpenCode's free models, best first, after the one the run is on, round to the first
/// after the last; skipping one down right now unless every other is down too. None for a paid route, a model of the
/// person's own, another runtime, or a lone free model.
pub fn next_free_model<'a>(runtime: &str, model: Option<&str>, models: &'a [Model], down: &[String]) -> Option<&'a Model> {
    let free = crate::connectors_off::free_models_best_first(models);
    let at = free.iter().position(|m| Some(m.id.as_str()) == model)?;
    if runtime != FREE_START_RUNTIME || free.len() < 2 {
        return None;
    }
    (1..free.len()).map(|step| free[(at + step) % free.len()]).find(|m| !down.contains(&m.id)).or(Some(free[(at + 1) % free.len()]))
}

/// missionView.ts QUIET_FREE_MS: two minutes with nothing back.
pub const QUIET_FREE_MS: i64 = 2 * 60 * 1000;

/// One recorded event, as `quietFreeRun` reads it: its type, when (ms since the epoch), and a diagnostic's code.
#[derive(Clone, Debug, Default)]
pub struct QuietEvent {
    pub kind: String,
    pub at_ms: Option<i64>,
    pub code: Option<String>,
}

/// quietFreeRun: an OpenCode free model whose run began two minutes ago and has had nothing back but diagnostics,
/// none of them its provider being busy (that note already carries the same press).
pub fn quiet_free_run(runtime: Option<&str>, model: Option<&str>, events: &[QuietEvent], now_ms: i64) -> bool {
    if runtime != Some("opencode") || !model.is_some_and(|m| m.ends_with("-free")) {
        return false;
    }
    let Some(began) = events.first().and_then(|e| e.at_ms) else { return false };
    if now_ms - began < QUIET_FREE_MS {
        return false;
    }
    events.iter().all(|e| {
        e.kind == "run.started"
            || (e.kind == "adapter.diagnostic" && !e.code.as_deref().unwrap_or("").ends_with(".provider_busy.runtime_error"))
    })
}

/// `RoutePicker.tsx` `rowDetailRepeatsHeading`: true when the detail line only repeats the runtime the group heading names.
pub fn row_detail_repeats_heading(detail: &str, group: &str) -> bool {
    const FILLER: &[&str] = &[
        "through", "via", "cli", "model", "models", "tier", "tiers", "the", "a", "an", "its", "and",
        "of", "on", "with", "your", "own", "account",
    ];
    let words = |s: &str| -> Vec<String> {
        s.to_ascii_lowercase()
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let said: Vec<String> = words(detail).into_iter().filter(|w| !FILLER.contains(&w.as_str())).collect();
    if said.is_empty() {
        return !detail.trim().is_empty();
    }
    let head = group.split(" \u{b7} ").next().unwrap_or(group);
    let named: std::collections::HashSet<String> = words(head).into_iter().collect();
    said.iter().all(|w| named.contains(w))
}

/// `quiet.ts`: five minutes without a word before saying so.
pub const QUIET_MINUTES_BEFORE_SAYING_SO: i64 = 5;

/// Whole minutes since the turn was last heard from, once at least 5 minutes; otherwise None.
pub fn quiet_minutes(last_heard_at: Option<&str>, now_ms: i64) -> Option<i64> {
    let raw = last_heard_at?;
    let heard = chrono::DateTime::parse_from_rfc3339(raw).ok()?.timestamp_millis();
    let minutes = (now_ms - heard).div_euclid(60_000);
    (minutes >= QUIET_MINUTES_BEFORE_SAYING_SO).then_some(minutes)
}

/// `ContextMenu.tsx` `placeMenu`: keep a pointer-placed menu inside the window with an 8px margin.
pub fn place_menu(at: (f32, f32), menu: (f32, f32), view: (f32, f32)) -> (f32, f32) {
    const MARGIN: f32 = 8.0;
    let left = if at.0 + menu.0 <= view.0 - MARGIN {
        at.0.max(MARGIN)
    } else {
        (at.0 - menu.0).max(MARGIN).min((view.0 - MARGIN - menu.0).max(MARGIN))
    };
    let top = if at.1 + menu.1 <= view.1 - MARGIN {
        at.1.max(MARGIN)
    } else if at.1 - menu.1 >= MARGIN {
        at.1 - menu.1
    } else {
        (view.1 - MARGIN - menu.1).max(MARGIN)
    };
    (left, top)
}

#[cfg(test)]
#[path = "route_rows_tests.rs"]
mod tests;
