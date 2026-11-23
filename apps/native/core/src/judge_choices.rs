//! The models a judge can be (Electron `App.tsx` `judgeChoicesFor`, 0.810).
//!
//! A free model the person already asked first, then the composer's, then the catalogue with its free models first, and
//! only then the person's own models. YOUR OWN MODELS JUDGE TOO: a model added with their own key (their company's, or
//! Claude on their API credits) is offered, under Your models and by the name they gave it, so a panel of Claude judges
//! on the API can be asked (the credits drive, 2026-10-21). One that only chats is left out: its runs get no tools, and a
//! long brief reaches a judge as files to read.

use crate::route_rows::OWN_MODELS_GROUP;

/// A model on offer (`PublicModel`, as far as a judge needs it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JudgeModel {
    pub runtime: String,
    pub id: String,
    /// The name it is listed by; for a model of the person's own, the name they gave it.
    pub display_name: String,
    /// Added by the person with their own key.
    pub own: bool,
    /// Its runs get no tools.
    pub chat_only: bool,
    /// Kept for old conversations, not offered anew.
    pub older: bool,
    /// The account it runs on, where the person keeps more than one (Rust's pool; Electron's judges have none).
    pub account: Option<String>,
}

/// One judge on offer: `label` is "group / short", as the picker lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgeChoice {
    pub key: String,
    pub runtime: String,
    pub model: String,
    pub account: Option<String>,
    pub label: String,
    pub group: String,
    pub short: String,
}

/// `judgeChoicesFor`, up to where the compared columns are set apart. `ready` says a runtime can run a judge now;
/// `runtime_name` says a runtime as the picker heads it.
pub fn judge_choices_for(
    free_answered: &[String],
    composer: Option<(&str, &str)>,
    models: &[JudgeModel],
    ready: impl Fn(&str) -> bool,
    runtime_name: impl Fn(&str) -> String,
) -> Vec<JudgeChoice> {
    let mut picked: Vec<JudgeChoice> = Vec::new();
    let mut add = |runtime: &str, model: &str, listed: Option<&JudgeModel>| {
        let account = listed.and_then(|one| one.account.clone());
        let key = match &account {
            Some(account) => format!("{runtime}:{model}:{account}"),
            None => format!("{runtime}:{model}"),
        };
        if picked.iter().any(|one| one.key == key) || !ready(runtime) {
            return;
        }
        let own = listed.filter(|one| one.own);
        // A model of your own by the name you gave it, an id-as-name spelled out (0.810): "Claude Haiku 5.5", as its
        // column reads.
        let short = match (own, listed) {
            (Some(own), _) => crate::compare::shown_route_label(&own.id, Some(&own.display_name)).unwrap_or_else(|| own.display_name.clone()),
            (None, Some(one)) if !one.display_name.trim().is_empty() => one.display_name.clone(),
            _ => crate::teammate_tooltip::route_model_name(runtime, model),
        };
        let group = if own.is_none() { runtime_name(runtime) } else { OWN_MODELS_GROUP.to_owned() };
        picked.push(JudgeChoice { key, runtime: runtime.to_owned(), model: model.to_owned(), account, label: format!("{group} / {short}"), group, short });
    };
    let listed = |runtime: &str, model: &str| models.iter().find(|one| one.runtime == runtime && one.id == model && one.account.is_none());
    for model in free_answered {
        add("opencode", model, listed("opencode", model));
    }
    if let Some((runtime, model)) = composer {
        add(runtime, model, listed(runtime, model));
    }
    let catalogue: Vec<&JudgeModel> = models.iter().filter(|one| !one.older && !one.own).collect();
    let free = catalogue.iter().filter(|one| one.id.ends_with("-free"));
    let paid = catalogue.iter().filter(|one| !one.id.ends_with("-free"));
    for model in free.chain(paid) {
        add(&model.runtime, &model.id, Some(model));
    }
    for model in models.iter().filter(|one| one.own && !one.chat_only) {
        add(&model.runtime, &model.id, Some(model));
    }
    picked
}

#[cfg(test)]
#[path = "judge_choices_tests.rs"]
mod tests;
