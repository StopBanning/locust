//! WITH NOTHING INSTALLED, THE INSTALL COMES FIRST (Electron `FirstLaunch.tsx`: `accountsLead`, 0.780).
//!
//! A clean-machine drive of Electron 0.779 found the free agent's Install row under the team templates and the folder
//! card, below the fold: the first screen offered teams that could not run and hid the press that makes them run.
//! Until something is installed the accounts lead; once anything is, Home keeps the order it always had.

/// Whether Home puts its AI agents first. `installed` is discovery's answer, one entry per agent, or None before it
/// has answered: an unanswered Home keeps its usual order rather than jumping when the answer lands.
///
/// An agent still being checked is installed (it was found; its sign-in is what is being asked), so a returning Home
/// never reorders while its agents answer. An answer naming no agents at all leads with nothing: Electron's `every`
/// is true on an empty list, which would put an empty panel first.
pub fn accounts_lead(installed: Option<&[bool]>) -> bool {
    installed.is_some_and(|all| !all.is_empty() && all.iter().all(|i| !i))
}

/// `freeStartStillFree`: whether OpenCode still has a free model, which the first screen promises. Three answers, and
/// "not known" is the ordinary one at first launch (OpenCode not installed, or its list not read yet): only a list
/// read from a ready OpenCode with nothing free in it is evidence, so only `No` withdraws the promise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FreeStart {
    Yes,
    No,
    Unknown,
}

/// `opencode_ready`: OpenCode answered ready. `listed`: the model ids it listed (empty: never read).
pub fn free_start_still_free<'a>(opencode_ready: bool, listed: impl IntoIterator<Item = &'a str>) -> FreeStart {
    if !opencode_ready {
        return FreeStart::Unknown;
    }
    let mut any = false;
    for id in listed {
        if id.ends_with("-free") {
            return FreeStart::Yes;
        }
        any = true;
    }
    if any { FreeStart::No } else { FreeStart::Unknown }
}

#[cfg(test)]
#[path = "home_order_tests.rs"]
mod tests;
