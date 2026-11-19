//! The order runtimes read in on a settings list, and what a re-check keeps (status.ts `runtimeListOrder` and
//! `keepWhatWasKnown`; `the-frame-pass.test.ts`).

/// What the two rules read of a runtime's status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeEntry {
    pub id: String,
    pub display_name: String,
    pub installed: bool,
    pub version: Option<String>,
    pub status: String,
}

fn band(tag: &str) -> u8 {
    match tag {
        "READY" => 0,
        "EXPERIMENTAL" => 1,
        "SIGN IN" | "AT LIMIT" => 2,
        "NOT INSTALLED" => 3,
        "PLANNED" => 4,
        _ => 3,
    }
}

/// READY, EXPERIMENTAL, SIGN IN, NOT INSTALLED, PLANNED; alphabetical (case-folded) inside a band, so the list is
/// stable between launches. Electron's `localeCompare` is approximated by comparing lower-cased names.
pub fn runtime_list_order(runtimes: &[RuntimeEntry], tag_of: impl Fn(&RuntimeEntry) -> String) -> Vec<RuntimeEntry> {
    let mut out = runtimes.to_vec();
    out.sort_by(|a, b| {
        band(&tag_of(a)).cmp(&band(&tag_of(b))).then_with(|| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()))
    });
    out
}

/// `probe-failed` and `offline` mean "no answer", not "a bad answer".
fn no_answer_yet(status: &str) -> bool {
    matches!(status, "probe-failed" | "offline")
}

/// A re-check does not make the screen go backwards: silence keeps the last answer, every definite answer wins.
pub fn keep_what_was_known(previous: &[RuntimeEntry], next: &[RuntimeEntry]) -> Vec<RuntimeEntry> {
    next.iter()
        .map(|runtime| {
            let Some(held) = previous.iter().find(|p| p.id == runtime.id) else { return runtime.clone() };
            if no_answer_yet(&runtime.status) && !no_answer_yet(&held.status) {
                return held.clone();
            }
            if runtime.version.is_none() && held.version.is_some() && runtime.installed && held.installed {
                return RuntimeEntry { version: held.version.clone(), ..runtime.clone() };
            }
            runtime.clone()
        })
        .collect()
}
