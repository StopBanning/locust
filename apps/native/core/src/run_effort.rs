//! THE CHIP IS THE RUN: App.tsx `runEffortFor`. The level a run is started with is the level its chip shows, decided
//! once, here, for both.
//!
//! Electron shipped the bug this guards against: the chip showed "medium" (`effort ?? defaultEffort(...)`) while the
//! run was started with no level at all, because the chosen level is empty until someone opens the dropdown (status.ts,
//! above `defaultEffort`). The Rust port had the same split: the chip's level slider showed the chosen level or a
//! default, the chip itself only a chosen or saved one, and `native::send_from` sent the saved level or nothing.
//! The chip (src/ui/thread_view.rs) and `send_from` now both ask `run_effort`.

use serde_json::Value;

/// runEffortFor: the level a model's chip shows and its run is sent. The level chosen; else, on Cursor, the level the
/// model's id already names (a Cursor family stands on one of its own variants: `grok-4.7-low` is Grok 4.7 at Low, and
/// is not run at Medium); else the model's default (status.ts defaultEffort: the runtime's own where it lists it, else
/// medium, else the middle level). Nothing for a model that lists no levels when nobody chose one.
///
/// `runtime_default` is the catalogue's `defaultEffort` where the caller has it; the window's copy of the catalogue
/// does not carry it, and for that case the only runtime with a default of its own beside Cursor's (which is the level
/// its id names, read here) is Muse.
pub fn run_effort(runtime: &str, model: &str, levels: &[String], runtime_default: Option<&str>, chosen: Option<&str>) -> Option<String> {
    if let Some(chosen) = chosen {
        return Some(chosen.to_owned());
    }
    let named_by_id = (runtime == "cursor").then(|| crate::models::split_cursor_model_id(model).1).flatten().filter(|level| levels.contains(level));
    let runtime_default = runtime_default.or((runtime == "muse").then_some(crate::models::MUSE_DEFAULT_EFFORT));
    named_by_id.or_else(|| crate::effort_levels::default_effort(levels, runtime_default))
}

/// `send_from`'s half: the level a run on `runtime` and `model` is sent, from the catalogue this process has already
/// read. Never read afresh here, so a start does not wait on every agent's model list; before the first read it knows no
/// model, and a run nobody chose a level for is sent none, as a model the catalogue has never heard of is.
pub fn run_effort_at_start(runtime: &str, model: Option<&str>, chosen: Option<&str>) -> Option<String> {
    if chosen.is_some() {
        return chosen.map(str::to_owned);
    }
    let model = model.filter(|m| !m.is_empty()).unwrap_or(crate::models::ACCOUNT_DEFAULT);
    let catalog = catalog_as_read();
    // runEffortFor finds the family by its own id on its runtime.
    let family = catalog.iter().find(|m| m["id"] == model && m["runtime"] == runtime);
    let levels: Vec<String> = family
        .and_then(|f| f["supportedEfforts"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|level| level.as_str().map(str::to_owned))
        .collect();
    let runtime_default = family.and_then(|f| f["defaultEffort"].as_str());
    run_effort(runtime, model, &levels, runtime_default, None)
}

thread_local! {
    #[cfg(test)]
    static CATALOG: std::cell::RefCell<Option<Vec<Value>>> = const { std::cell::RefCell::new(None) };
}

/// The catalogue as read: this computer's, or under test only the one a test set on its thread (so no test reads
/// another's, or this machine's).
fn catalog_as_read() -> Vec<Value> {
    #[cfg(test)]
    {
        CATALOG.with(|c| c.borrow().clone()).unwrap_or_default()
    }
    #[cfg(not(test))]
    {
        crate::models::local_as_read()
    }
}

#[cfg(test)]
pub(crate) fn use_catalog_on_this_thread(models: Option<Vec<Value>>) {
    CATALOG.with(|c| *c.borrow_mut() = models);
}

#[cfg(test)]
#[path = "run_effort_tests.rs"]
mod tests;
