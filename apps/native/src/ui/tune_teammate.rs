//! `/tune` teammate modal (`TuneTeammate.tsx` + `main/index.ts` `tune:draft`).
//!
//! Drafts two deliberately different edits (`How it works` and `What it knows` or a focused `TuneSkill`),
//! lets the person inspect/edit them beside `Current`, warns on repeats, and starts a 3-column blind
//! comparison (`changes: true, blind: true`) without changing saved teammate instructions.

use super::*;
use locust_core::compare::{CompareRoute, shuffled_columns};
use locust_core::engine_api::api;
use locust_core::native::{self, Origin, SlotOrigin};
use locust_core::skill_runtimes::takes_skills;
use locust_core::tune::{
    TUNE_DRAFT_COST, TUNE_FALLBACK, TUNE_SKILL_INCOMPLETE, TuneEdit, TuneSkill, current_drafts,
    parse_tune_drafts, repeats_of, tune_draft_prompt, tune_refusal, tune_routes, validate_tune_skill,
};
use locust_core::tune_lesson_store;

#[derive(Clone)]
pub(super) struct TuneCardState {
    pub(super) teammate_id: String,
    pub(super) teammate_name: String,
    pub(super) runtime: String,
    pub(super) model: String,
    pub(super) account: Option<String>,
    pub(super) effort: Option<String>,
    pub(super) mode: String,
    pub(super) current_instructions: String,
    pub(super) takes_skills: bool,
    pub(super) ask: TextEdit,
    pub(super) summary: String,
    pub(super) work_inst: TextEdit,
    pub(super) work_why: String,
    pub(super) know_inst: TextEdit,
    pub(super) know_why: String,
    pub(super) know_skill_mode: bool,
    pub(super) skill_name: TextEdit,
    pub(super) skill_desc: TextEdit,
    pub(super) skill_body: TextEdit,
    pub(super) skill_aside: Option<TuneSkill>,
    pub(super) drafting: bool,
    pub(super) draft_mission_id: Option<String>,
    pub(super) draft_note: Option<String>,
    pub(super) problem: Option<String>,
}

fn text_edit(val: &str) -> TextEdit {
    let mut t = TextEdit::default();
    t.replace_text(val);
    t
}

pub(super) fn read_teammate_saved(
    store: &locust_core::store::Store,
    teammate_id: &str,
) -> Option<(String, String, String, String, Option<String>, Option<String>, String)> {
    let file = locust_core::people::team(store).ok()?;
    let t = file["teammates"]
        .as_array()?
        .iter()
        .find(|row| row["teammateId"] == teammate_id)?;
    let name = t["name"].as_str().unwrap_or("teammate").to_owned();
    let instructions = t["instructions"].as_str().unwrap_or("").to_owned();
    let runtime = t["route"]["runtime"]
        .as_str()
        .unwrap_or("codex")
        .to_owned();
    let model = t["route"]["model"]
        .as_str()
        .filter(|m| !m.is_empty())
        .unwrap_or("account-default")
        .to_owned();
    let account = t["route"]["account"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let effort = t["route"]["effort"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let mode = t["route"]["mode"].as_str().unwrap_or("ask").to_owned();
    Some((name, instructions, runtime, model, account, effort, mode))
}

fn last_user_prompt(s: &State) -> String {
    if let Some(ledger) = &s.ledger
        && let Some(u) = ledger.snapshot.items.iter().rev().find_map(|it| match it {
            locust_core::ledger::Item::User(text) => {
                let t = text.trim();
                (!t.is_empty()).then(|| t.to_owned())
            }
            _ => None,
        })
    {
        return u;
    }
    let idx = s.selected;
    if let Some(m) = s.teammates.get(idx)
        && let Some(convo) = s.conversations.get(&m.id)
        && let Some((u, _)) = convo.turns.last()
    {
        let trimmed = u.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    String::new()
}

pub(super) fn account_from_route_id(route_id: Option<&str>) -> Option<String> {
    let raw = route_id?.trim();
    if raw.is_empty() {
        return None;
    }
    // e.g. "provider:account" or resolved route id suffix
    if let Some((_, acc)) = raw.split_once(':') {
        let trimmed = acc.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    } else {
        None
    }
}

fn tip_route_matches(
    store: &locust_core::store::Store,
    tip: &str,
    runtime: &str,
    model: &str,
    account: Option<&str>,
) -> bool {
    let path = store.root().join("mission-ledger").join(format!("{tip}.jsonl"));
    let Ok(raw) = std::fs::read_to_string(path) else {
        return true;
    };
    let mut turn_rt: Option<String> = None;
    let mut turn_mdl: Option<String> = None;
    let mut turn_acc: Option<String> = None;
    for line in raw.lines() {
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if ev["type"] == "run.started" {
            turn_rt = ev["payload"]["runtime"].as_str().map(str::to_owned);
            turn_mdl = ev["payload"]["model"].as_str().map(str::to_owned);
            turn_acc = ev["payload"]["account"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| account_from_route_id(ev["payload"]["resolvedRouteId"].as_str()));
        }
    }
    if let Some(rt) = &turn_rt
        && rt != runtime
    {
        return false;
    }
    if let Some(mdl) = &turn_mdl
        && model != "account-default"
        && mdl != model
    {
        return false;
    }
    if let Some(acc) = account
        && turn_acc.as_deref().is_some_and(|a| a != acc)
    {
        return false;
    }
    true
}

pub(super) fn tune_skill_ready(skill: &TuneSkill) -> Option<String> {
    if validate_tune_skill(skill).is_some() {
        None
    } else {
        Some(TUNE_SKILL_INCOMPLETE.to_owned())
    }
}

pub(super) fn active_folder_path(s: &State, teammate: &Teammate) -> Option<std::path::PathBuf> {
    s.home
        .folder_path
        .as_deref()
        .filter(|p| !p.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            teammate
                .folder
                .as_deref()
                .filter(|p| !p.is_empty())
                .map(std::path::PathBuf::from)
        })
        .or_else(|| std::env::current_dir().ok())
}

pub(super) fn open_from_composer(s: &mut State) {
    let mate = s.teammates.get(s.selected).cloned();
    let turn_running = s.status_tone == StatusTone::Amber
        || s.ledger.as_ref().is_some_and(|l| l.snapshot.running);
    let busy = s.flight.is_some();
    let has_compare = s.compare.active_id.is_some();

    if let Some(why) = tune_refusal(mate.is_some(), !turn_running, busy, has_compare) {
        s.composer_note = Some(why.to_owned());
        return;
    }
    let Some(mate) = mate else { return };
    let Some(store) = s.data.clone() else {
        s.composer_note = Some("Native data folder could not be read.".into());
        return;
    };
    let Some((tm_name, current_inst, rt, model, account, effort, mode)) =
        read_teammate_saved(&store, &mate.id)
    else {
        s.composer_note = Some("This teammate’s saved instructions could not be read.".into());
        return;
    };

    let mut drafts = current_drafts(&current_inst);
    let ask_seed = last_user_prompt(s);
    let skills = takes_skills(Some(&rt));
    let tip = s
        .ledger
        .as_ref()
        .map(|l| l.snapshot.tip.clone())
        .filter(|t| !t.is_empty());

    let mut drafting = false;
    let mut draft_mission_id = None;
    let mut draft_note = None;

    let same_route = tip
        .as_deref()
        .is_none_or(|t| tip_route_matches(&store, t, &rt, &model, account.as_deref()));

    if !same_route {
        draft_note = Some(
            "This conversation uses another model or account. Drafts were not requested; you can write the two alternatives yourself."
                .into(),
        );
    } else {
        let lessons = tune_lesson_store::list(&store, Some(&mate.id));
        let rt_display = locust_core::carry_on::display_name(&rt);
        let prompt = tune_draft_prompt(&current_inst, &lessons, skills, Some(rt_display));
        let folder = active_folder_path(s, &mate);
        let req_model = (model != "account-default").then(|| model.clone());
        let side_tm = locust_core::engine::Teammate {
            id: mate.id.clone(),
            name: tm_name.clone(),
            role: mate.role.clone(),
            runtime: Some(rt.clone()),
            model: req_model.clone(),
            account: None,
            busy: false,
            runs_in: "ask".into(),
            folder: folder.as_ref().map(|p| p.to_string_lossy().into_owned()),
        };
        let key_suffix = tip.clone().unwrap_or_else(|| mate.id.clone());
        match api().send(
            native::Send {
                teammate: side_tm,
                conversation: None,
                prompt,
                model: req_model,
                effort: effort.clone(),
            },
            Origin {
                mode: Some(locust_core::approval::Mode::Ask),
                folder,
                runtime: Some(rt.clone()),
                slot: Some(SlotOrigin {
                    key: format!("tune-draft:{key_suffix}:{}", locust_core::invariants::unique_nanos()),
                    cwd: None,
                    instructions: None,
                    skill: None,
                    sealed: true,
                }),
                ..Default::default()
            },
        ) {
            Ok(started) => {
                drafting = true;
                draft_mission_id = Some(started.conversation_id);
            }
            Err(why) => {
                draft_note = Some(format!("{why} You can write the two alternatives yourself below."));
                drafts = current_drafts(&current_inst);
            }
        }
    }

    s.composer_note = None;
    s.tune_card = Some(TuneCardState {
        teammate_id: mate.id,
        teammate_name: tm_name,
        runtime: rt,
        model,
        account,
        effort,
        mode,
        current_instructions: current_inst,
        takes_skills: skills,
        ask: text_edit(&ask_seed),
        summary: drafts.summary,
        work_inst: text_edit(&drafts.work.instructions),
        work_why: drafts.work.why,
        know_inst: text_edit(&drafts.know.instructions),
        know_why: drafts.know.why,
        know_skill_mode: false,
        skill_name: text_edit(""),
        skill_desc: text_edit(""),
        skill_body: text_edit(""),
        skill_aside: None,
        drafting,
        draft_mission_id,
        draft_note,
        problem: None,
    });
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(mid) = s
        .tune_card
        .as_ref()
        .and_then(|c| c.draft_mission_id.clone())
    else {
        return false;
    };
    let Some(store) = s.data.as_ref() else {
        return false;
    };
    let Ok(ledger_dir) = store.ledger_dir() else {
        return false;
    };
    let electron_ledger = store.electron().join("mission-ledger");
    let Ok(snap) = locust_core::ledger::Session::new(ledger_dir, mid)
        .also_read(electron_ledger)
        .poll()
    else {
        return false;
    };
    if snap.running {
        return false;
    }
    let Some(card) = s.tune_card.as_mut() else {
        return false;
    };
    let raw = snap
        .items
        .iter()
        .rev()
        .find_map(|it| match it {
            locust_core::ledger::Item::Assistant { text, .. } if !text.trim().is_empty() => {
                Some(text.clone())
            }
            _ => None,
        })
        .unwrap_or_default();
    if !snap.failed && let Some(parsed) = parse_tune_drafts(&raw, card.takes_skills) {
        card.summary = parsed.summary;
        card.work_inst.replace_text(parsed.work.instructions);
        card.work_why = parsed.work.why;
        card.know_why = parsed.know.why;
        if let Some(sk) = parsed.know.skill {
            card.know_skill_mode = true;
            card.skill_name.replace_text(sk.name);
            card.skill_desc.replace_text(sk.description);
            card.skill_body.replace_text(sk.body);
            card.know_inst.replace_text(&card.current_instructions);
        } else {
            card.know_skill_mode = false;
            card.know_inst.replace_text(parsed.know.instructions);
        }
        card.draft_note = None;
    } else {
        card.draft_note = Some(TUNE_FALLBACK.to_owned());
    }
    card.drafting = false;
    card.draft_mission_id = None;
    true
}

pub(super) fn toggle_skill_mode(card: &mut TuneCardState) {
    if !card.takes_skills {
        return;
    }
    if card.know_skill_mode {
        let stashed = TuneSkill {
            name: card.skill_name.text.trim().to_owned(),
            description: card.skill_desc.text.trim().to_owned(),
            body: card.skill_body.text.trim().to_owned(),
        };
        if !stashed.name.is_empty() || !stashed.description.is_empty() || !stashed.body.is_empty() {
            card.skill_aside = Some(stashed);
        }
        card.know_skill_mode = false;
    } else {
        if let Some(prev) = card.skill_aside.take() {
            if card.skill_name.text.trim().is_empty() {
                card.skill_name.replace_text(prev.name);
            }
            if card.skill_desc.text.trim().is_empty() {
                card.skill_desc.replace_text(prev.description);
            }
            if card.skill_body.text.trim().is_empty() {
                card.skill_body.replace_text(prev.body);
            }
        }
        card.know_skill_mode = true;
    }
    card.problem = None;
}

pub(super) fn build_tune_routes(card: &TuneCardState) -> Result<(String, Vec<CompareRoute>), String> {
    let ask = card.ask.text.trim().to_owned();
    if ask.is_empty() {
        return Err("Write what you want all three setups to answer.".into());
    }
    let work_txt = card.work_inst.text.trim().to_owned();
    if work_txt.is_empty() || work_txt.contains('\0') || work_txt.chars().count() > 4_000 {
        return Err("How it works instructions must be 1 to 4,000 characters.".into());
    }
    let work = TuneEdit {
        instructions: work_txt,
        why: card.work_why.clone(),
        skill: None,
    };
    let know = if card.know_skill_mode && card.takes_skills {
        let sk = TuneSkill {
            name: card.skill_name.text.trim().to_owned(),
            description: card.skill_desc.text.trim().to_owned(),
            body: card.skill_body.text.trim().to_owned(),
        };
        if let Some(why) = tune_skill_ready(&sk) {
            return Err(why);
        }
        TuneEdit {
            instructions: card.current_instructions.clone(),
            why: card.know_why.clone(),
            skill: Some(sk),
        }
    } else {
        let know_txt = card.know_inst.text.trim().to_owned();
        if know_txt.is_empty() || know_txt.contains('\0') || know_txt.chars().count() > 4_000 {
            return Err("What it knows instructions must be 1 to 4,000 characters.".into());
        }
        TuneEdit {
            instructions: know_txt,
            why: card.know_why.clone(),
            skill: None,
        }
    };
    let routes = tune_routes(
        &card.runtime,
        &card.model,
        card.account.as_deref(),
        card.effort.as_deref(),
        Some(&card.mode),
        &card.current_instructions,
        &work,
        &know,
    );
    Ok((ask, routes))
}

pub(super) fn run_all_three(s: &mut State) {
    let Some(card) = s.tune_card.clone() else {
        return;
    };
    let (ask, routes) = match build_tune_routes(&card) {
        Ok(ok) => ok,
        Err(why) => {
            if let Some(c) = s.tune_card.as_mut() {
                c.problem = Some(why);
            }
            return;
        }
    };
    let mut seed = locust_core::invariants::unique_nanos() as usize;
    let shuffled = shuffled_columns(&routes, |upper| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        seed % upper
    });
    match crate::compare_view::start_tune_comparison(s, &card.teammate_id, &ask, shuffled) {
        Ok(()) => s.tune_card = None,
        Err(why) => {
            if let Some(c) = s.tune_card.as_mut() {
                c.problem = Some(why);
            }
        }
    }
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let card = s.tune_card.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);

    let know_skill = if card.know_skill_mode && card.takes_skills {
        Some(TuneSkill {
            name: card.skill_name.text.trim().to_owned(),
            description: card.skill_desc.text.trim().to_owned(),
            body: card.skill_body.text.trim().to_owned(),
        })
    } else {
        None
    };
    let setups = vec![
        ("Current".to_owned(), Some(card.current_instructions.clone()), None),
        ("How it works".to_owned(), Some(card.work_inst.text.clone()), None),
        (
            "What it knows".to_owned(),
            if know_skill.is_some() {
                Some(card.current_instructions.clone())
            } else {
                Some(card.know_inst.text.clone())
            },
            know_skill,
        ),
    ];
    let repeats = repeats_of(&setups);

    let mut modal = col()
        .key("tune-modal")
        .w(780.0)
        .max_h(680.0)
        .padding(20.0)
        .gap(12.0)
        .radius(12.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(
            row()
                .w_full()
                .gap(8.0)
                .center_y()
                .child(
                    text(format!("Tune {}", card.teammate_name))
                        .font(rune::TextStyle::new(16.0, 600.0))
                        .color(p.text)
                        .grow(1.0),
                )
                .child(
                    crate::look::controls::button(
                        &l,
                        "Cancel",
                        crate::look::controls::Tone::Neutral,
                        false,
                    )
                    .key("tune-cancel")
                    .on_press(|s: &mut State| {
                        s.tune_card = None;
                    }),
                ),
        )
        .child(text(TUNE_DRAFT_COST).font(l.ty.label).color(p.muted).wrap());

    if card.drafting {
        modal = modal.child(
            text("Drafting two ideas…")
                .key("tune-drafting")
                .font(l.ty.body)
                .color(p.amber),
        );
    }
    if let Some(note) = &card.draft_note {
        modal = modal.child(
            text(note.clone())
                .key("tune-draft-note")
                .font(l.ty.label)
                .color(p.amber)
                .wrap(),
        );
    }
    if !card.summary.is_empty() {
        modal = modal.child(
            text(card.summary.clone())
                .key("tune-summary")
                .font(l.ty.body)
                .color(p.secondary)
                .wrap(),
        );
    }

    modal = modal
        .child(text("Ask to run again").font(l.ty.label).color(p.muted))
        .child(
            text_input(&card.ask, type_::BODY)
                .key("tune-ask")
                .label("Ask to run again")
                .w_full()
                .padding(8.0)
                .fill(p.field)
                .color(p.text)
                .on_edit(|s: &mut State| &mut s.tune_card.as_mut().expect("tune_card").ask),
        );

    let mut cols = row().w_full().gap(10.0);

    // Column 1: Current (read-only display)
    let cur_text = if card.current_instructions.trim().is_empty() {
        "No standing instructions saved yet.".to_owned()
    } else {
        card.current_instructions.clone()
    };
    cols = cols.child(
        col()
            .key("tune-current-box")
            .grow(1.0)
            .padding(10.0)
            .gap(6.0)
            .radius(8.0)
            .fill(p.bg)
            .border(1.0, p.hairline)
            .child(text("Current").font(rune::TextStyle::new(13.0, 600.0)).color(p.text))
            .child(text("Unchanged baseline").font(l.ty.label).color(p.muted))
            .child(text(cur_text).font(l.ty.body).color(p.secondary).wrap()),
    );

    // Column 2: How it works
    let mut work_hdr = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .child(
            text("How it works")
                .font(rune::TextStyle::new(13.0, 600.0))
                .color(p.text)
                .grow(1.0),
        );
    if let Some(first) = repeats.get(&1) {
        work_hdr = work_hdr.child(crate::look::controls::chip(
            &l,
            &format!("Same as {first}"),
            false,
        ));
    }
    let mut work_col = col()
        .key("tune-work-box")
        .grow(1.0)
        .padding(10.0)
        .gap(6.0)
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.hairline)
        .child(work_hdr);
    if !card.work_why.is_empty() {
        work_col = work_col.child(text(card.work_why.clone()).font(l.ty.label).color(p.muted).wrap());
    }
    work_col = work_col.child(
        text_input(&card.work_inst, type_::BODY)
            .key("tune-work-input")
            .label("How it works instructions")
            .w_full()
            .padding(8.0)
            .fill(p.field)
            .color(p.text)
            .on_edit(|s: &mut State| &mut s.tune_card.as_mut().expect("tune_card").work_inst),
    );
    cols = cols.child(work_col);

    // Column 3: What it knows (instructions or skill)
    let mut know_hdr = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .child(
            text("What it knows")
                .font(rune::TextStyle::new(13.0, 600.0))
                .color(p.text)
                .grow(1.0),
        );
    if let Some(first) = repeats.get(&2) {
        know_hdr = know_hdr.child(crate::look::controls::chip(
            &l,
            &format!("Same as {first}"),
            false,
        ));
    }
    let mut know_col = col()
        .key("tune-know-box")
        .grow(1.0)
        .padding(10.0)
        .gap(6.0)
        .radius(8.0)
        .fill(p.bg)
        .border(1.0, p.hairline)
        .child(know_hdr);
    if !card.know_why.is_empty() {
        know_col = know_col.child(text(card.know_why.clone()).font(l.ty.label).color(p.muted).wrap());
    }
    if card.takes_skills {
        let toggle_label = if card.know_skill_mode {
            "Use instructions instead"
        } else {
            "Write a skill instead"
        };
        know_col = know_col.child(
            crate::look::controls::button(
                &l,
                toggle_label,
                crate::look::controls::Tone::Neutral,
                false,
            )
            .key("tune-skill-toggle")
            .on_press(|s: &mut State| {
                if let Some(c) = s.tune_card.as_mut() {
                    toggle_skill_mode(c);
                }
            }),
        );
    }
    if card.know_skill_mode && card.takes_skills {
        know_col = know_col
            .child(text("Name").font(l.ty.label).color(p.muted))
            .child(
                text_input(&card.skill_name, type_::BODY)
                    .key("tune-skill-name")
                    .single_line()
                    .placeholder("lowercase-hyphen", p.muted)
                    .w_full()
                    .padding(6.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.tune_card.as_mut().expect("tune_card").skill_name),
            )
            .child(text("When to use it").font(l.ty.label).color(p.muted))
            .child(
                text_input(&card.skill_desc, type_::BODY)
                    .key("tune-skill-desc")
                    .placeholder("Short description", p.muted)
                    .w_full()
                    .padding(6.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.tune_card.as_mut().expect("tune_card").skill_desc),
            )
            .child(text("What it tells the teammate").font(l.ty.label).color(p.muted))
            .child(
                text_input(&card.skill_body, type_::BODY)
                    .key("tune-skill-body")
                    .placeholder("Skill instructions", p.muted)
                    .w_full()
                    .padding(6.0)
                    .fill(p.field)
                    .color(p.text)
                    .on_edit(|s: &mut State| &mut s.tune_card.as_mut().expect("tune_card").skill_body),
            );
    } else {
        know_col = know_col.child(
            text_input(&card.know_inst, type_::BODY)
                .key("tune-know-input")
                .label("What it knows instructions")
                .w_full()
                .padding(8.0)
                .fill(p.field)
                .color(p.text)
                .on_edit(|s: &mut State| &mut s.tune_card.as_mut().expect("tune_card").know_inst),
        );
    }
    cols = cols.child(know_col);

    modal = modal.child(cols);
    if let Some(prob) = &card.problem {
        modal = modal.child(text(prob.clone()).key("tune-problem").font(l.ty.body).color(p.red).wrap());
    }
    modal = modal.child(
        row()
            .w_full()
            .gap(8.0)
            .child(
                text("Blind run: columns are shuffled as Setup A, Setup B, Setup C.")
                    .font(l.ty.label)
                    .color(p.muted)
                    .grow(1.0),
            )
            .child(
                crate::look::controls::button(
                    &l,
                    "Run all three",
                    crate::look::controls::Tone::Active,
                    card.drafting,
                )
                .key("tune-run-all")
                .on_press(run_all_three),
            ),
    );

    Some(
        el().key("tune-backdrop")
            .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
            .center()
            .fill(p.bg.alpha(0.7))
            .child(modal),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_card(runtime: &str) -> TuneCardState {
        TuneCardState {
            teammate_id: "tm_1".into(),
            teammate_name: "Codec".into(),
            runtime: runtime.into(),
            model: "sonnet".into(),
            account: None,
            effort: None,
            mode: "ask".into(),
            current_instructions: "Be concise.".into(),
            takes_skills: takes_skills(Some(runtime)),
            ask: text_edit("Check this patch."),
            summary: String::new(),
            work_inst: text_edit("Step 1: check tests."),
            work_why: "process".into(),
            know_inst: text_edit("Remember edge cases."),
            know_why: "domain".into(),
            know_skill_mode: false,
            skill_name: text_edit(""),
            skill_desc: text_edit(""),
            skill_body: text_edit(""),
            skill_aside: None,
            drafting: false,
            draft_mission_id: None,
            draft_note: None,
            problem: None,
        }
    }

    #[test]
    fn a_tune_says_when_a_setup_repeats() {
        let setups = vec![
            ("Current".into(), Some("Be concise.".into()), None),
            ("How it works".into(), Some("Be concise.".into()), None),
            ("What it knows".into(), Some("Check errors.".into()), None),
        ];
        let rep = repeats_of(&setups);
        assert_eq!(rep.get(&1).copied(), Some("Current"));
        assert!(!rep.contains_key(&2));
    }

    #[test]
    fn a_tune_can_give_a_claude_or_codex_teammate_a_skill_and_stashes_across_toggle() {
        let mut card = sample_card("claude");
        assert!(card.takes_skills);
        toggle_skill_mode(&mut card);
        assert!(card.know_skill_mode);
        card.skill_name.replace_text("concise-review");
        card.skill_desc.replace_text("When reviewing PRs");
        card.skill_body.replace_text("Cite exact line numbers.");

        let (_, routes) = build_tune_routes(&card).unwrap();
        assert_eq!(routes.len(), 3);
        assert_eq!(routes[2].skill.as_ref().unwrap().name, "concise-review");

        // Toggling back stashes skill fields in skill_aside and restores them cleanly.
        toggle_skill_mode(&mut card);
        assert!(!card.know_skill_mode);
        assert_eq!(card.skill_aside.as_ref().unwrap().name, "concise-review");
        card.skill_name.replace_text("");
        card.skill_desc.replace_text("");
        card.skill_body.replace_text("");
        toggle_skill_mode(&mut card);
        assert!(card.know_skill_mode);
        assert_eq!(card.skill_name.text, "concise-review");

        // Incomplete skill is refused cleanly.
        card.skill_name.replace_text("Bad Name");
        assert_eq!(build_tune_routes(&card).unwrap_err(), TUNE_SKILL_INCOMPLETE);
    }

    #[test]
    fn tuning_never_applies_instructions_by_itself() {
        let dir = std::env::temp_dir().join(format!(
            "locust-tune-no-apply-{}-{}",
            std::process::id(),
            locust_core::invariants::unique_nanos()
        ));
        std::fs::create_dir_all(dir.join("native")).unwrap();
        std::fs::create_dir_all(dir.join("electron")).unwrap();
        let store = locust_core::store::Store::new(dir.join("native"), dir.join("electron")).unwrap();
        store
            .save_json(
                "teammates.json",
                &serde_json::json!({
                    "schemaVersion": 1,
                    "teammates": [{
                        "teammateId": "tm_1",
                        "name": "Codec",
                        "role": "Code & Migrations",
                        "instructions": "Original saved instructions.",
                        "route": {"runtime": "codex", "model": "gpt-5", "mode": "ask"}
                    }],
                    "missionOwners": {},
                    "settings": {}
                }),
            )
            .unwrap();

        let card = sample_card("codex");
        let (_, routes) = build_tune_routes(&card).unwrap();
        assert_eq!(routes[0].instructions.as_deref(), Some("Be concise."));
        assert_eq!(routes[1].instructions.as_deref(), Some("Step 1: check tests."));

        // Building routes / drafting leaves teammates.json completely untouched.
        let (_, inst, _, _, _, _, _) = read_teammate_saved(&store, "tm_1").unwrap();
        assert_eq!(inst, "Original saved instructions.");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
