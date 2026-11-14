//! Settings > General > "Scout keeps watch" & "What Scout saw" (`ScoutSetting.tsx`).
//!
//! Covers the Part 2a window controls only: on/off switch, model link, Look now, goal,
//! notes, 14-day look log (read lazily only when opened), Part 2a watches (GitHub,
//! teammates, folder check — omitting Part 2b's `/locust` comment, fix, and away
//! settings), cadence, morning brief, quiet hours, wait while working, and desktop notices.

use super::*;
use crate::look::controls::{self, Look, Tone};
use crate::scout_cards;
use chrono::FixedOffset;
use locust_core::scout::log::{look_lines, look_name};
use locust_core::scout::types::{
    MAX_SCOUT_GOAL, MAX_SCOUT_NOTE, MAX_SCOUT_NOTES, SCOUT_BRIEF_HOURS, SCOUT_EVERY_MINUTES,
    SCOUT_FIXES_PER_DAY, ScoutChange, ScoutFixSettingsPatch, ScoutState, ScoutWatchesPatch,
    hour_words,
};
use locust_core::scout::view::{
    self, SCOUT_GOAL_HEADING, SCOUT_GOAL_LEDE, SCOUT_GOAL_PLACEHOLDER, SCOUT_NO_MODEL_MESSAGE,
    SCOUT_NOTES_PLACEHOLDER, SCOUT_OFF_LEDE, SCOUT_SAW_HEADING, SCOUT_SAW_LEDE, SCOUT_SAW_PAGE,
    SCOUT_SETTING_HEADING,
};
use rune_kit::Checkbox;

fn platform_name() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

fn scout_model_display(s: &State, st: &ScoutState) -> Option<String> {
    let tid = st.settings.teammate_id.as_deref()?;
    let tm = s.teammates.iter().find(|t| t.id == tid)?;
    let model_id = tm.model.as_deref()?.trim();
    if model_id.is_empty() {
        return None;
    }
    for list in s.models.values() {
        for group in list {
            if let Some(found) = group.models.iter().find(|m| m.id == model_id) {
                if !found.name.trim().is_empty() {
                    return Some(found.name.clone());
                }
            }
        }
    }
    Some(model_id.to_owned())
}

fn clip_goal(text: &str) -> String {
    text.chars().take(MAX_SCOUT_GOAL).collect()
}

fn save_goal(s: &mut State) {
    let Some(raw) = s.scout.goal_draft.as_ref().map(|e| e.text.clone()) else {
        return;
    };
    let clipped = clip_goal(&raw);
    s.scout.goal_pending = Some(clipped.trim().to_owned());
    s.scout.goal_draft = None;
    scout_cards::apply_change(
        s,
        ScoutChange {
            goal: Some(clipped),
            ..ScoutChange::default()
        },
    );
}

fn save_notes_draft(s: &mut State) {
    let Some(text) = s.scout.notes_draft.as_ref().map(|e| e.text.clone()) else {
        return;
    };
    s.scout.notes_pending = Some(text.clone());
    s.scout.notes_draft = None;
    scout_cards::save_notes(s, text);
}

fn option_chip(label: impl Into<String>, key: impl Into<String>, selected: bool, disabled: bool, l: &Look) -> El {
    let label = label.into();
    let mut btn = controls::button(
        l,
        &label,
        if selected { Tone::Active } else { Tone::Neutral },
        disabled,
    )
    .role("button")
    .label(label)
    .key(key.into());
    if disabled {
        btn = btn.disabled(true);
    }
    btn
}

fn setting_line(heading: impl Into<String>, lede: impl Into<String>, control: El, l: &Look) -> El {
    row()
        .w_full()
        .gap(12.0)
        .center_y()
        .padding(10.0)
        .child(
            col()
                .grow(1.0)
                .shrink()
                .gap(4.0)
                .child(
                    text(heading.into())
                        .font(type_::TITLE)
                        .color(l.c("text-primary")),
                )
                .child(
                    text(lede.into())
                        .font(type_::LABEL)
                        .color(l.c("text-secondary"))
                        .wrap(),
                ),
        )
        .child(control)
}

fn toggle_line(
    heading: &'static str,
    lede: impl Into<String>,
    checked: bool,
    disabled: bool,
    key: &'static str,
    l: &Look,
    on_toggle: impl Fn(&mut State) + 'static,
) -> El {
    let mut sw = controls::switch(l, checked, heading, disabled)
        .key(key);
    if !disabled {
        sw = sw.on_press(move |s: &mut State| on_toggle(s));
    }
    setting_line(heading, lede, sw, l)
}

fn scout_saw_section(s: &State, p: &Palette, l: &Look, tz: FixedOffset) -> El {
    let mut sec = col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .key("scout-saw")
        .child(
            text(SCOUT_SAW_HEADING)
                .font(type_::TITLE)
                .color(l.c("text-primary")),
        )
        .child(
            text(SCOUT_SAW_LEDE)
                .font(type_::LABEL)
                .color(l.c("text-secondary"))
                .wrap(),
        );

    if !s.scout.log_open {
        return sec.child(
            row().w_full().child(
                controls::button(l, "Show what Scout saw", Tone::Neutral, false)
                    .role("button")
                    .label("Show what Scout saw")
                    .key("scout-saw-open")
                    .on_press(|s: &mut State| {
                        s.scout.log_open = true;
                        s.scout.log_shown = SCOUT_SAW_PAGE;
                        scout_cards::load_log(s);
                    }),
            ),
        );
    }

    let filter = Checkbox::new(s.scout.log_aside_only)
        .label("Only what it left out")
        .on_toggle(|s: &mut State| {
            s.scout.log_aside_only = !s.scout.log_aside_only;
            s.scout.log_shown = SCOUT_SAW_PAGE;
        })
        .build(l.k)
        .key("scout-saw-aside-only");

    sec = sec.child(
        row()
            .w_full()
            .gap(8.0)
            .center_y()
            .flex_wrap()
            .child(
                controls::button(l, "Hide", Tone::Neutral, false)
                    .role("button")
                    .label("Hide")
                    .key("scout-saw-hide")
                    .on_press(|s: &mut State| {
                        s.scout.log_open = false;
                        s.scout.log_shown = SCOUT_SAW_PAGE;
                    }),
            )
            .child(
                controls::button(l, "Refresh", Tone::Neutral, s.scout.log_loading)
                    .role("button")
                    .label("Refresh")
                    .key("scout-saw-refresh")
                    .on_press(|s: &mut State| scout_cards::load_log(s)),
            )
            .child(filter),
    );

    if s.scout.log_loading && s.scout.log.is_none() {
        return sec.child(
            text("Reading what Scout saw…")
                .font(type_::LABEL)
                .color(p.muted),
        );
    }

    match &s.scout.log {
        None => sec,
        Some(Err(_)) => sec.child(
            text("What Scout saw could not be read. Try Refresh.")
                .font(type_::LABEL)
                .color(l.c("amber-text"))
                .wrap(),
        ),
        Some(Ok(snap)) => {
            let entries = view::saw_entries(&snap.entries, s.scout.log_aside_only);
            if entries.is_empty() {
                let empty = if s.scout.log_aside_only {
                    "Nothing left out in these 14 days."
                } else {
                    "No looks yet in these 14 days."
                };
                return sec.child(text(empty).font(type_::LABEL).color(p.muted));
            }
            let total = entries.len();
            let shown = s.scout.log_shown.min(total);
            let mut list = col().w_full().gap(8.0).key("scout-saw-list");
            for (idx, entry) in entries.iter().take(shown).enumerate() {
                let mut item = col()
                    .w_full()
                    .gap(3.0)
                    .padding(8.0)
                    .radius(6.0)
                    .fill(l.c("bg-raised"))
                    .border(1.0, l.c("border-card"))
                    .key(format!("scout-saw-entry-{idx}"))
                    .child(
                        text(look_name(entry, tz))
                            .font(rune::TextStyle::new(12.0, 600.0))
                            .color(l.c("text-primary")),
                    );
                for line in look_lines(entry, &snap.held) {
                    item = item.child(
                        text(line)
                            .font(type_::LABEL)
                            .color(l.c("text-secondary"))
                            .wrap(),
                    );
                }
                list = list.child(item);
            }
            sec = sec.child(list);
            if total > shown {
                let more = SCOUT_SAW_PAGE.min(total - shown);
                let label = format!("Show {more} more");
                sec = sec.child(
                    controls::ghost_button(l, &label)
                        .role("button")
                        .label(label)
                        .key("scout-saw-more")
                        .on_press(|s: &mut State| {
                            s.scout.log_shown += SCOUT_SAW_PAGE;
                        }),
                );
            }
            sec
        }
    }
}

pub(super) fn section(s: &State, p: &Palette, check_command: Option<&str>) -> El {
    section_at(s, p, check_command, chrono::Local::now().fixed_offset())
}

pub(super) fn section_at(
    s: &State,
    p: &Palette,
    check_command: Option<&str>,
    now: chrono::DateTime<FixedOffset>,
) -> El {
    let l = Look::new(p.dark);
    let st = s.scout.state.as_ref();
    let on = st.is_some_and(|st| st.settings.on);
    let busy = s.scout.setting_busy;
    let model_name = st.and_then(|st| scout_model_display(s, st));

    let lede = match st {
        Some(st) if st.settings.on => {
            view::setting_on_lede(st.settings.every_minutes, model_name.as_deref())
        }
        _ => SCOUT_OFF_LEDE.to_owned(),
    };

    let mut header_keys = row().gap(8.0).center_y();
    if on {
        let look_label = if s.scout.looking {
            "Looking…"
        } else {
            "Look now"
        };
        let mut look_btn = controls::button(l_ref(&l), look_label, Tone::Neutral, s.scout.looking || busy)
            .role("button")
            .label(look_label)
            .key("scout-setting-look-now");
        if !s.scout.looking && !busy {
            look_btn = look_btn.on_press(|s: &mut State| scout_cards::look_now(s));
        }
        header_keys = header_keys.child(look_btn);

        if let Some(tid) = st.and_then(|st| st.settings.teammate_id.clone()) {
            header_keys = header_keys.child(
                controls::button(&l, "Change its model", Tone::Neutral, false)
                    .role("button")
                    .label("Change its model")
                    .key("scout-setting-change-model")
                    .on_press(move |s: &mut State| {
                        s.settings_panel.open = false;
                        crate::people_panel::open_edit(s, &tid);
                    }),
            );
        }
    }

    let switch_disabled = st.is_none() || busy;
    let mut main_sw = controls::switch(&l, on, SCOUT_SETTING_HEADING, switch_disabled)
        .key("scout-setting-switch");
    if !switch_disabled {
        main_sw = main_sw.on_press(|s: &mut State| {
            let currently_on = s.scout.state.as_ref().is_some_and(|st| st.settings.on);
            if currently_on {
                scout_cards::apply_change(
                    s,
                    ScoutChange {
                        on: Some(false),
                        ..ScoutChange::default()
                    },
                );
            } else {
                let has_tm = s
                    .scout
                    .state
                    .as_ref()
                    .and_then(|st| st.settings.teammate_id.as_ref())
                    .is_some();
                if !has_tm && scout_cards::default_scout_route(s).is_none() {
                    s.scout.setting_message = Some(SCOUT_NO_MODEL_MESSAGE.to_owned());
                } else {
                    scout_cards::turn_on_scout(s, false);
                }
            }
        });
    }
    header_keys = header_keys.child(main_sw);

    let mut head_text = col()
        .grow(1.0)
        .shrink()
        .gap(4.0)
        .child(
            text(SCOUT_SETTING_HEADING)
                .font(type_::TITLE)
                .color(p.text),
        )
        .child(text(lede).font(type_::LABEL).color(p.secondary).wrap());

    if let Some(st) = st {
        if st.settings.on {
            if let Some(status) = view::setting_status_line(st, model_name.as_deref(), now) {
                head_text = head_text.child(text(status).font(type_::LABEL).color(p.muted).wrap());
            }
            if let Some(why) = st
                .model_said
                .as_deref()
                .map(str::trim)
                .filter(|w| !w.is_empty())
            {
                head_text = head_text.child(
                    text(format!("Last look: {why}"))
                        .font(type_::LABEL)
                        .color(l.c("amber-text"))
                        .wrap(),
                );
            }
        }
    }
    if let Some(msg) = s
        .scout
        .setting_message
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        head_text = head_text.child(
            text(msg.to_owned())
                .font(type_::LABEL)
                .color(l.c("amber-text"))
                .wrap(),
        );
    }

    let mut root = col()
        .w_full()
        .gap(8.0)
        .key("scout-setting")
        .child(row().w_full().gap(12.0).center_y().padding(10.0).child(head_text).child(header_keys));

    let Some(st) = st else {
        return root;
    };
    if !st.settings.on {
        return root;
    }

    // 1. Goal ("What to keep watch for")
    let saved_goal = s
        .scout
        .goal_pending
        .clone()
        .or_else(|| st.settings.goal.clone())
        .unwrap_or_default();
    let goal_fallback;
    let goal_edit = match &s.scout.goal_draft {
        Some(edit) => edit,
        None => {
            goal_fallback = TextEdit::new(&saved_goal);
            &goal_fallback
        }
    };
    let goal_dirty = s
        .scout
        .goal_draft
        .as_ref()
        .is_some_and(|e| e.text.trim() != saved_goal);
    let mut goal_col = col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .child(text(SCOUT_GOAL_HEADING).font(type_::TITLE).color(p.text))
        .child(
            text(SCOUT_GOAL_LEDE)
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        )
        .child(
            text_input(goal_edit, type_::BODY)
                .key("scout-goal-input")
                .w_full()
                .padding(8.0)
                .radius(6.0)
                .fill(p.field)
                .color(p.text)
                .placeholder(SCOUT_GOAL_PLACEHOLDER, p.muted)
                .role("textbox")
                .label(if saved_goal.is_empty() {
                    SCOUT_GOAL_PLACEHOLDER
                } else {
                    "What Scout keeps watch for"
                })
                .on_edit(|s: &mut State| {
                    let saved = s
                        .scout
                        .goal_pending
                        .as_deref()
                        .or_else(|| {
                            s.scout
                                .state
                                .as_ref()
                                .and_then(|st| st.settings.goal.as_deref())
                        })
                        .unwrap_or("");
                    s.scout
                        .goal_draft
                        .get_or_insert_with(|| TextEdit::new(saved))
                })
                .on_submit(|s: &mut State, _tx: &mut Tx| save_goal(s)),
        );
    if goal_dirty {
        goal_col = goal_col.child(
            row()
                .gap(8.0)
                .child(
                    controls::button(&l, "Save", Tone::Active, busy)
                        .role("button")
                        .label("Save")
                        .key("scout-goal-save")
                        .on_press(|s: &mut State| save_goal(s)),
                )
                .child(
                    controls::button(&l, "Cancel", Tone::Neutral, busy)
                        .role("button")
                        .label("Cancel")
                        .key("scout-goal-cancel")
                        .on_press(|s: &mut State| s.scout.goal_draft = None),
                ),
        );
    }
    root = root.child(goal_col);

    // 2. Notes ("Scout’s notes")
    let saved_notes = s
        .scout
        .notes_pending
        .clone()
        .unwrap_or_else(|| st.notes.join("\n"));
    let notes_fallback;
    let notes_edit = match &s.scout.notes_draft {
        Some(edit) => edit,
        None => {
            notes_fallback = TextEdit::new(&saved_notes);
            &notes_fallback
        }
    };
    let notes_dirty = s
        .scout
        .notes_draft
        .as_ref()
        .is_some_and(|e| e.text != saved_notes);
    let mut notes_col = col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .child(
            text(view::notes_heading(st.notes.len()))
                .font(type_::TITLE)
                .color(p.text),
        )
        .child(
            text(format!(
                "What Scout keeps from its looks, one line each, and reads on every look. Change or clear them here. It keeps the newest {MAX_SCOUT_NOTES} lines, each up to {MAX_SCOUT_NOTE} characters."
            ))
            .font(type_::LABEL)
            .color(p.secondary)
            .wrap(),
        )
        .child(
            text_input(notes_edit, type_::BODY)
                .key("scout-notes-input")
                .w_full()
                .padding(8.0)
                .radius(6.0)
                .fill(p.field)
                .color(p.text)
                .placeholder(SCOUT_NOTES_PLACEHOLDER, p.muted)
                .role("textbox")
                .label(if saved_notes.is_empty() {
                    SCOUT_NOTES_PLACEHOLDER
                } else {
                    "Scout’s notes"
                })
                .on_edit(|s: &mut State| {
                    let saved = s
                        .scout
                        .notes_pending
                        .clone()
                        .unwrap_or_else(|| {
                            s.scout
                                .state
                                .as_ref()
                                .map(|st| st.notes.join("\n"))
                                .unwrap_or_default()
                        });
                    s.scout
                        .notes_draft
                        .get_or_insert_with(|| TextEdit::new(&saved))
                }),
        );
    if notes_dirty {
        notes_col = notes_col.child(
            row()
                .gap(8.0)
                .child(
                    controls::button(&l, "Save", Tone::Active, busy)
                        .role("button")
                        .label("Save")
                        .key("scout-notes-save")
                        .on_press(|s: &mut State| save_notes_draft(s)),
                )
                .child(
                    controls::button(&l, "Cancel", Tone::Neutral, busy)
                        .role("button")
                        .label("Cancel")
                        .key("scout-notes-cancel")
                        .on_press(|s: &mut State| s.scout.notes_draft = None),
                ),
        );
    }
    root = root.child(notes_col);

    // 3. What Scout saw (read lazily on press)
    root = root.child(scout_saw_section(s, p, &l, *now.offset()));

    // 4. Watches (GitHub, Your /locust comments, Teammates, Folder check)
    let watches = st.settings.watches;
    root = root.child(toggle_line(
        "Pull requests and their checks",
        "Yours, the reviews asked of you, and the default branch’s checks, through the GitHub CLI you signed in.",
        watches.github,
        busy,
        "scout-watch-github",
        &l,
        |s| {
            let next = !s
                .scout
                .state
                .as_ref()
                .map_or(true, |st| st.settings.watches.github);
            scout_cards::apply_change(
                s,
                ScoutChange {
                    watches: Some(ScoutWatchesPatch {
                        github: Some(next),
                        ..ScoutWatchesPatch::default()
                    }),
                    ..ScoutChange::default()
                },
            );
        },
    ));

    root = root.child(toggle_line(
        "Your /locust comments",
        "A comment of yours on this folder's GitHub with a line starting /locust is an ask. Scout brings it to Home with the step drafted; /locust ada names the teammate.",
        watches.comments,
        busy,
        "scout-watch-comments",
        &l,
        |s| {
            let next = !s
                .scout
                .state
                .as_ref()
                .map_or(true, |st| st.settings.watches.comments);
            scout_cards::apply_change(
                s,
                ScoutChange {
                    watches: Some(ScoutWatchesPatch {
                        comments: Some(next),
                        ..ScoutWatchesPatch::default()
                    }),
                    ..ScoutChange::default()
                },
            );
        },
    ));

    root = root.child(toggle_line(
        "Your teammates’ turns",
        "One waiting on you for more than 10 minutes, and one that failed.",
        watches.teammates,
        busy,
        "scout-watch-teammates",
        &l,
        |s| {
            let next = !s
                .scout
                .state
                .as_ref()
                .map_or(true, |st| st.settings.watches.teammates);
            scout_cards::apply_change(
                s,
                ScoutChange {
                    watches: Some(ScoutWatchesPatch {
                        teammates: Some(next),
                        ..ScoutWatchesPatch::default()
                    }),
                    ..ScoutChange::default()
                },
            );
        },
    ));

    let check_lede = match check_command {
        None => "Set a check in Settings > Project folder first, such as npm test.".to_owned(),
        Some(cmd) => format!(
            "When the folder has changed, Scout runs {cmd}, never while a turn is working here. A check that starts failing needs you now."
        ),
    };
    let check_disabled = busy || (check_command.is_none() && !watches.check);
    root = root.child(toggle_line(
        "This folder’s check",
        check_lede,
        watches.check,
        check_disabled,
        "scout-watch-check",
        &l,
        |s| {
            let next = !s
                .scout
                .state
                .as_ref()
                .map_or(false, |st| st.settings.watches.check);
            scout_cards::apply_change(
                s,
                ScoutChange {
                    watches: Some(ScoutWatchesPatch {
                        check: Some(next),
                        ..ScoutWatchesPatch::default()
                    }),
                    ..ScoutChange::default()
                },
            );
        },
    ));

    // 4b. Prepare fixes before I look
    let fixes = st.settings.fixes.clone().unwrap_or_default();
    let check_on = watches.check && check_command.is_some();
    let fixes_disabled = busy || (!check_on && !fixes.on);
    let fixes_lede = if check_on {
        view::SCOUT_FIXES_LEDE.to_owned()
    } else {
        format!("{} {}", view::SCOUT_FIXES_LEDE, view::SCOUT_FIXES_NEED_CHECK)
    };
    root = root.child(toggle_line(
        view::SCOUT_FIXES_HEADING,
        fixes_lede,
        fixes.on,
        fixes_disabled,
        "scout-fixes-toggle",
        &l,
        |s| {
            let next = !s
                .scout
                .state
                .as_ref()
                .and_then(|st| st.settings.fixes.as_ref())
                .is_some_and(|f| f.on);
            scout_cards::apply_change(
                s,
                ScoutChange {
                    fixes: Some(ScoutFixSettingsPatch {
                        on: Some(next),
                        ..Default::default()
                    }),
                    ..ScoutChange::default()
                },
            );
        },
    ));
    if fixes.on {
        let scout_id = st.settings.teammate_id.as_deref();
        let candidates: Vec<&Teammate> = s
            .teammates
            .iter()
            .filter(|t| Some(t.id.as_str()) != scout_id)
            .collect();
        let mut who_row = row().gap(6.0).flex_wrap().center_y();
        if candidates.is_empty() {
            who_row = who_row.child(
                text(view::SCOUT_FIXES_ADD_TEAMMATE)
                    .font(rune::TextStyle::new(12.0, 400.0))
                    .color(l.c("text-muted")),
            );
        } else {
            if fixes.teammate_id.is_none() {
                who_row = who_row.child(
                    text(view::SCOUT_FIXES_PICK_TEAMMATE)
                        .font(rune::TextStyle::new(12.0, 400.0))
                        .color(l.c("text-muted")),
                );
            }
            for tm in candidates {
                let selected = fixes.teammate_id.as_deref() == Some(tm.id.as_str());
                let tm_id = tm.id.clone();
                let mut btn = option_chip(
                    &tm.name,
                    format!("scout-fixes-mate-{}", tm.id),
                    selected,
                    busy,
                    &l,
                );
                if !busy {
                    btn = btn.on_press(move |s: &mut State| {
                        scout_cards::apply_change(
                            s,
                            ScoutChange {
                                fixes: Some(ScoutFixSettingsPatch {
                                    teammate_id: Some(Some(tm_id.clone())),
                                    ..Default::default()
                                }),
                                ..ScoutChange::default()
                            },
                        );
                    });
                }
                who_row = who_row.child(btn);
            }
        }
        let mut days_row = row().gap(6.0).flex_wrap();
        for per_day in SCOUT_FIXES_PER_DAY {
            let selected = fixes.per_day == per_day;
            let mut btn = option_chip(
                view::fix_per_day_label(per_day),
                format!("scout-fixes-day-{per_day}"),
                selected,
                busy,
                &l,
            );
            if !busy {
                btn = btn.on_press(move |s: &mut State| {
                    scout_cards::apply_change(
                        s,
                        ScoutChange {
                            fixes: Some(ScoutFixSettingsPatch {
                                per_day: Some(per_day),
                                ..Default::default()
                            }),
                            ..ScoutChange::default()
                        },
                    );
                });
            }
            days_row = days_row.child(btn);
        }
        root = root.child(
            col()
                .w_full()
                .gap(8.0)
                .padding(8.0)
                .radius(6.0)
                .fill(l.c("bg-window"))
                .border(1.0, l.c("border-hairline"))
                .child(setting_line(
                    view::SCOUT_FIXES_WHO_HEADING,
                    "Only teammates other than Scout can edit your folder’s files.",
                    who_row,
                    &l,
                ))
                .child(days_row),
        );
    }

    // 5. How often it looks
    let mut every_row = row().gap(6.0).flex_wrap();
    for minutes in SCOUT_EVERY_MINUTES {
        let label = if minutes == 60 {
            "Every hour".to_owned()
        } else {
            format!("Every {minutes} minutes")
        };
        let mut btn = option_chip(
            label,
            format!("scout-every-{minutes}"),
            st.settings.every_minutes == minutes,
            busy,
            &l,
        );
        if !busy {
            btn = btn.on_press(move |s: &mut State| {
                scout_cards::apply_change(
                    s,
                    ScoutChange {
                        every_minutes: Some(minutes),
                        ..ScoutChange::default()
                    },
                );
            });
        }
        every_row = every_row.child(btn);
    }
    root = root.child(setting_line(
        "How often it looks",
        "Only while Locust is running. Its model is asked only when something is new.",
        every_row,
        &l,
    ));

    // 6. Morning brief
    let mut brief_row = row().gap(6.0).flex_wrap();
    let mut off_btn = option_chip(
        "Off",
        "scout-brief-off",
        st.settings.brief_at.is_none(),
        busy,
        &l,
    );
    if !busy {
        off_btn = off_btn.on_press(|s: &mut State| {
            scout_cards::apply_change(
                s,
                ScoutChange {
                    brief_at: Some(None),
                    ..ScoutChange::default()
                },
            );
        });
    }
    brief_row = brief_row.child(off_btn);
    for hour in SCOUT_BRIEF_HOURS {
        let label = format!("At {}", hour_words(hour));
        let mut btn = option_chip(
            label,
            format!("scout-brief-{hour}"),
            st.settings.brief_at == Some(hour),
            busy,
            &l,
        );
        if !busy {
            btn = btn.on_press(move |s: &mut State| {
                scout_cards::apply_change(
                    s,
                    ScoutChange {
                        brief_at: Some(Some(hour)),
                        ..ScoutChange::default()
                    },
                );
            });
        }
        brief_row = brief_row.child(btn);
    }
    root = root.child(setting_line(
        "Morning brief",
        "One card on Home each morning: who waits on you, what finished or failed since yesterday, and how the check stands. The first look from that hour on brings it.",
        brief_row,
        &l,
    ));

    // 7. Quiet hours
    let mut quiet_row = row().gap(6.0).flex_wrap();
    for (idx, (choice, label)) in view::quiet_choices_with_saved(st.settings.quiet)
        .into_iter()
        .enumerate()
    {
        let mut btn = option_chip(
            label,
            format!("scout-quiet-{idx}"),
            st.settings.quiet == choice,
            busy,
            &l,
        );
        if !busy {
            btn = btn.on_press(move |s: &mut State| {
                scout_cards::apply_change(
                    s,
                    ScoutChange {
                        quiet: Some(choice),
                        ..ScoutChange::default()
                    },
                );
            });
        }
        quiet_row = quiet_row.child(btn);
    }
    root = root.child(setting_line(
        "Quiet hours",
        "No looks on its own and no desktop notifications. The first look after them catches up on everything.",
        quiet_row,
        &l,
    ));

    // 8. Wait while you're working
    root = root.child(toggle_line(
        "Wait while you’re working",
        "A look due while your own turn runs waits for it to end, at most 30 minutes.",
        st.settings.wait_while_working,
        busy,
        "scout-wait-working",
        &l,
        |s| {
            let next = !s
                .scout
                .state
                .as_ref()
                .map_or(true, |st| st.settings.wait_while_working);
            scout_cards::apply_change(
                s,
                ScoutChange {
                    wait_while_working: Some(next),
                    ..ScoutChange::default()
                },
            );
        },
    ));

    // 9. Desktop notifications
    root = root.child(toggle_line(
        "Desktop notifications",
        view::notify_setting_lede(platform_name()),
        st.settings.notify,
        busy,
        "scout-notify",
        &l,
        |s| {
            let next = !s
                .scout
                .state
                .as_ref()
                .map_or(true, |st| st.settings.notify);
            scout_cards::apply_change(
                s,
                ScoutChange {
                    notify: Some(next),
                    ..ScoutChange::default()
                },
            );
        },
    ));

    root
}

fn l_ref(l: &Look) -> &Look {
    l
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::controls::Look;
    use crate::scout_cards::tests::{click_key, collect_rt_text, render_state};
    use locust_core::scout::ScoutLogSnapshot;
    use locust_core::scout::log::{ScoutAsideWhy, ScoutLogAside, ScoutLogEntry};
    use locust_core::scout::types::{
        ScoutCard, ScoutFact, ScoutQuietHours, ScoutSettings, ScoutState, ScoutToday, ScoutUrgency,
        ScoutWatches,
    };

    fn on_state(
        settings_fn: impl FnOnce(&mut ScoutSettings),
        state_fn: impl FnOnce(&mut ScoutState),
    ) -> ScoutState {
        let mut settings = ScoutSettings::default();
        settings.on = true;
        settings.teammate_id = Some("tm_scout".into());
        settings_fn(&mut settings);
        let mut st = ScoutState {
            settings,
            cards: vec![],
            look_ids: vec![],
            notes: vec![],
            last_look_at: None,
            model_said: None,
            today: None,
        };
        state_fn(&mut st);
        st
    }

    fn fixed_now() -> chrono::DateTime<FixedOffset> {
        chrono::DateTime::parse_from_rfc3339("2026-10-22T12:05:00.000Z").unwrap()
    }

    fn draw_no_cmd(s: &State, _cx: &mut Cx<State>) -> El {
        let p = Palette::new(true);
        section_at(s, &p, None, fixed_now())
    }

    fn draw_npm_cmd(s: &State, _cx: &mut Cx<State>) -> El {
        let p = Palette::new(true);
        col()
            .w_full()
            .h_full()
            .scroll(ScrollSpec { stick_bottom: false })
            .child(section_at(s, &p, Some("npm test"), fixed_now()))
    }

    fn draw_home_sec(s: &State, _cx: &mut Cx<State>) -> El {
        let l = Look::new(true);
        scout_cards::home_section(s, &l).expect("morning brief section")
    }

    // --- 1:1 ports of `scout-settings-say-its-goal-hours-and-day.test.tsx` (7 Part-2a cases) ---

    #[test]
    fn shows_what_it_keeps_watch_for_with_no_save_until_changed() {
        let mut s = State::default();
        s.scout.state = Some(on_state(
            |cfg| cfg.goal = Some("Keep main’s tests passing.".into()),
            |_| {},
        ));
        let rt = render_state(s, draw_no_cmd);
        let words = collect_rt_text(&rt);
        assert!(words.contains("What to keep watch for"));
        assert!(words.contains("Keep main’s tests passing."));
        assert!(rt.find("scout-goal-save").is_none());

        let mut dirty = State::default();
        dirty.scout.state = Some(on_state(
            |cfg| cfg.goal = Some("Keep main’s tests passing.".into()),
            |_| {},
        ));
        dirty.scout.goal_draft = Some(TextEdit::new("Watch auth regressions"));
        let dirty_rt = render_state(dirty, draw_no_cmd);
        assert!(dirty_rt.find("scout-goal-save").is_some());

        let mut empty = State::default();
        empty.scout.state = Some(on_state(|_| {}, |_| {}));
        let empty_rt = render_state(empty, draw_no_cmd);
        assert!(collect_rt_text(&empty_rt).contains(SCOUT_GOAL_PLACEHOLDER));
    }

    #[test]
    fn shows_scouts_notes_as_lines_and_how_many() {
        let mut s = State::default();
        s.scout.state = Some(on_state(
            |_| {},
            |st| {
                st.notes = vec![
                    "The person merges on Fridays".into(),
                    "2026-10-22: Wren owns the Mac build.".into(),
                ]
            },
        ));
        let rt = render_state(s, draw_no_cmd);
        let words = collect_rt_text(&rt);
        assert!(words.contains("Scout’s notes · 2"));
        assert!(words.contains("The person merges on Fridays\n2026-10-22: Wren owns the Mac build."));
        assert!(words.contains("It keeps the newest 40 lines, each up to 240 characters."));

        let mut empty = State::default();
        empty.scout.state = Some(on_state(|_| {}, |_| {}));
        let empty_rt = render_state(empty, draw_no_cmd);
        assert!(collect_rt_text(&empty_rt).contains(SCOUT_NOTES_PLACEHOLDER));
    }

    #[test]
    fn offers_the_folders_check_only_once_folder_has_check_command() {
        let mut s = State::default();
        s.scout.state = Some(on_state(|_| {}, |_| {}));
        let mut without_rt = render_state(s, draw_no_cmd);
        assert!(
            collect_rt_text(&without_rt)
                .contains("Set a check in Settings > Project folder first, such as npm test.")
        );
        // Pressing the disabled switch without a check command must not toggle or enter busy state.
        click_key(&mut without_rt, "scout-watch-check");
        assert!(!without_rt.state.scout.setting_busy);

        let mut with_check = State::default();
        with_check.scout.state = Some(on_state(
            |cfg| {
                cfg.watches = ScoutWatches {
                    comments: false,
                    github: true,
                    teammates: true,
                    check: true,
                };
            },
            |_| {},
        ));
        let with_rt = render_state(with_check, draw_npm_cmd);
        assert!(collect_rt_text(&with_rt).contains(
            "When the folder has changed, Scout runs npm test, never while a turn is working here."
        ));
        assert!(with_rt.find("scout-watch-check").is_some());
    }

    #[test]
    fn says_quiet_hours_as_clock_reads_them_and_offers_wait_while_working() {
        let mut s = State::default();
        s.scout.state = Some(on_state(
            |cfg| cfg.quiet = Some(ScoutQuietHours { from: 22, to: 7 }),
            |_| {},
        ));
        let rt = render_state(s, draw_no_cmd);
        let words = collect_rt_text(&rt);
        assert!(words.contains("Quiet hours"));
        assert!(words.contains("10 PM to 7 AM"));
        assert!(words.contains("Wait while you’re working"));
        assert!(rt.find("scout-wait-working").is_some());
    }

    #[test]
    fn offers_morning_brief_at_an_hour_of_the_morning() {
        let mut s = State::default();
        s.scout.state = Some(on_state(|cfg| cfg.brief_at = Some(7), |_| {}));
        let rt = render_state(s, draw_no_cmd);
        let words = collect_rt_text(&rt);
        assert!(words.contains("Morning brief"));
        assert!(words.contains(
            "One card on Home each morning: who waits on you, what finished or failed since yesterday, and how the check stands."
        ));
        assert!(words.contains("At 7 AM"));
    }

    #[test]
    fn draws_morning_brief_as_one_card_its_lines_under_its_sentence() {
        let mut s = State::default();
        let mut fact = ScoutFact::simple(
            "brief:2026-10-22",
            ScoutUrgency::Later,
            "Good morning. Here is where things stand.",
        );
        fact.lines = Some(vec![
            "Waiting on you: Ada, 30 min.".into(),
            "Since yesterday: 4 turns finished, 1 failed.".into(),
        ]);
        s.scout.now_ms = Some(fixed_now().timestamp_millis());
        s.scout.state = Some(on_state(
            |_| {},
            |st| {
                st.cards = vec![ScoutCard {
                    card_id: "card_brief".into(),
                    at: "2026-10-22T12:00:00.000Z".into(),
                    reading: false,
                    take: None,
                    fix: None,
                    fact,
                }];
            },
        ));
        let rt = render_state(s, draw_home_sec);
        let words = collect_rt_text(&rt);
        assert!(words.contains("Morning brief"));
        assert!(words.contains("Good morning. Here is where things stand."));
        assert!(words.contains("Waiting on you: Ada, 30 min."));
        assert!(words.contains("Since yesterday: 4 turns finished, 1 failed."));
    }

    #[test]
    fn says_what_todays_looks_used() {
        let mut s = State::default();
        s.teammates.push(engine::Teammate {
            id: "tm_scout".into(),
            name: "Scout".into(),
            role: "Custom".into(),
            runtime: Some("opencode".into()),
            model: Some("deepseek/v4.1:free".into()),
            account: None,
            busy: false,
            runs_in: "local".into(),
            folder: None,
        });
        s.models.insert(
            "opencode".into(),
            vec![engine::Models {
                teammate: None,
                runtime: "opencode".into(),
                model: None,
                effort: None,
                models: vec![engine::Model {
                    id: "deepseek/v4.1:free".into(),
                    name: "Deepseek V4.1 Flash Free".into(),
                    efforts: vec![],
                }],
            }],
        );
        s.scout.state = Some(on_state(
            |_| {},
            |st| {
                st.today = Some(ScoutToday {
                    day: "2026-10-22".into(),
                    looks: 14,
                    asked: 2,
                    fixes: None,
                });
            },
        ));
        let rt = render_state(s, draw_no_cmd);
        assert!(
            collect_rt_text(&rt)
                .contains("Today: 14 looks, 2 of them read by Deepseek V4.1 Flash Free.")
        );

        let mut quiet_day = State::default();
        quiet_day.scout.state = Some(on_state(
            |_| {},
            |st| {
                st.today = Some(ScoutToday {
                    day: "2026-10-22".into(),
                    looks: 1,
                    asked: 0,
                    fixes: None,
                });
            },
        ));
        let quiet_rt = render_state(quiet_day, draw_no_cmd);
        assert!(
            collect_rt_text(&quiet_rt).contains("Today: 1 look, none of them needing its model.")
        );
    }

    // --- 1:1 port of `scout-settings-show-what-it-saw.test.tsx` (1/1) ---

    #[test]
    fn what_scout_saw_is_offered_once_on_and_reads_nothing_until_opened() {
        let mut off = State::default();
        off.scout.state = Some(ScoutState {
            settings: ScoutSettings::default(),
            cards: vec![],
            look_ids: vec![],
            last_look_at: None,
            model_said: None,
            today: None,
            notes: vec![],
        });
        let off_rt = render_state(off, draw_no_cmd);
        assert!(!collect_rt_text(&off_rt).contains("What Scout saw"));

        let mut on = State::default();
        on.scout.state = Some(on_state(|_| {}, |_| {}));
        let on_rt = render_state(on, draw_no_cmd);
        let words = collect_rt_text(&on_rt);
        assert!(words.contains("What Scout saw"));
        assert!(words.contains(SCOUT_SAW_LEDE));
        assert!(words.contains("Show what Scout saw"));
        assert!(on_rt.find("scout-saw-open").is_some());
        assert!(on_rt.state.scout.log.is_none());
        assert!(!on_rt.state.scout.log_loading);
    }

    #[test]
    fn opening_what_scout_saw_pages_by_thirty_and_filters_left_out() {
        let mut entries = Vec::new();
        for i in 0..35 {
            entries.push(ScoutLogEntry {
                at: "2026-10-22T12:00:00.000Z".into(),
                until: None,
                looks: 1,
                skipped_quiet_hours: false,
                raised: vec![],
                aside: if i % 2 == 0 {
                    vec![ScoutLogAside {
                        key: format!("pr:{i}"),
                        said: "quiet draft".into(),
                        why: ScoutAsideWhy::Told,
                    }]
                } else {
                    vec![]
                },
                off: vec![],
                github_unread: false,
                github: None,
                away: false,
            });
        }
        let mut s = State::default();
        s.scout.state = Some(on_state(|_| {}, |_| {}));
        s.scout.log_open = true;
        s.scout.log = Some(Ok(ScoutLogSnapshot {
            entries,
            held: vec![],
        }));

        let mut rt = render_state(s, draw_npm_cmd);
        assert!(rt.find("scout-saw-entry-29").is_some());
        assert!(rt.find("scout-saw-entry-30").is_none());
        assert!(collect_rt_text(&rt).contains("Show 5 more"));

        rt.update(|_, tx| tx.scroll_key_to("scout-saw-more", 0.0));
        rt.frame(0.0);
        click_key(&mut rt, "scout-saw-more");
        assert_eq!(rt.state.scout.log_shown, SCOUT_SAW_PAGE * 2);
        assert!(rt.find("scout-saw-entry-34").is_some());

        // Filter to looks that set something aside (even indices -> 18 entries).
        rt.update(|_, tx| tx.scroll_key_to("scout-saw-aside-only", 0.0));
        rt.frame(0.0);
        click_key(&mut rt, "scout-saw-aside-only");
        assert!(rt.state.scout.log_aside_only);
        assert!(rt.find("scout-saw-entry-17").is_some());
        assert!(rt.find("scout-saw-entry-18").is_none());
    }

    // --- 1:1 port of `a-scout-fix-says-where-it-stands.test.tsx` (1 settings case) ---

    #[test]
    fn fixes_setting_requires_check_first_offers_non_scout_teammates_and_adds_today_fixes() {
        let mut no_check = State::default();
        no_check.scout.state = Some(on_state(|_| {}, |_| {}));
        let mut no_check_rt = render_state(no_check, draw_npm_cmd);
        let words = collect_rt_text(&no_check_rt);
        assert!(words.contains("Prepare fixes before I look"));
        assert!(words.contains("Turn on This folder’s check first."));
        click_key(&mut no_check_rt, "scout-fixes-toggle");
        assert!(!no_check_rt.state.scout.setting_busy);

        let mut on_fixes = State::default();
        on_fixes.teammates = vec![
            Teammate {
                id: "tm_scout".into(),
                name: "Scout".into(),
                role: "Watch".into(),
                runtime: None,
                model: None,
                account: None,
                busy: false,
                runs_in: "ask".into(),
                folder: None,
            },
            Teammate {
                id: "tm_wren".into(),
                name: "Wren".into(),
                role: "Builder".into(),
                runtime: None,
                model: None,
                account: None,
                busy: false,
                runs_in: "edit".into(),
                folder: None,
            },
        ];
        on_fixes.scout.state = Some(on_state(
            |cfg| {
                cfg.watches.check = true;
                cfg.fixes = Some(locust_core::scout::types::ScoutFixSettings {
                    on: true,
                    teammate_id: Some("tm_wren".into()),
                    per_day: 3,
                });
            },
            |st| {
                st.today = Some(ScoutToday {
                    day: "2026-10-22".into(),
                    looks: 4,
                    asked: 1,
                    fixes: Some(2),
                });
            },
        ));
        let rt = render_state(on_fixes, draw_npm_cmd);
        let words = collect_rt_text(&rt);
        assert!(!words.contains("Turn on This folder’s check first."));
        assert!(words.contains("Who prepares Scout’s fixes"));
        assert!(words.contains("At most 1 a day"));
        assert!(words.contains("At most 2 a day"));
        assert!(words.contains("At most 3 a day"));
        assert!(words.contains("At most 5 a day"));
        assert!(words.contains("2 of 3 fixes prepared."));
        assert!(rt.find("scout-fixes-mate-tm_wren").is_some());
        assert!(rt.find("scout-fixes-mate-tm_scout").is_none());
        assert!(rt.find("scout-fixes-day-3").is_some());

        // Without any non-Scout teammate, says to add a teammate first.
        let mut solo = State::default();
        solo.scout.state = Some(on_state(
            |cfg| {
                cfg.watches.check = true;
                cfg.fixes = Some(locust_core::scout::types::ScoutFixSettings {
                    on: true,
                    teammate_id: None,
                    per_day: 2,
                });
            },
            |_| {},
        ));
        let solo_rt = render_state(solo, draw_npm_cmd);
        assert!(collect_rt_text(&solo_rt).contains("Add a teammate first"));
    }

    // --- 1:1 port of `scout-settings-say-its-goal-hours-and-day.test.tsx` (1 Part-2b comment watch case) ---

    #[test]
    fn offers_your_own_comment_watch_on_by_default_and_says_how_named_ask_works() {
        let mut s = State::default();
        s.scout.state = Some(on_state(|_| {}, |_| {}));
        let rt = render_state(s, draw_no_cmd);
        let words = collect_rt_text(&rt);
        assert!(words.contains("Your /locust comments"));
        assert!(words.contains("/locust ada names the teammate."));
        assert!(rt.find("scout-watch-comments").is_some());

        let mut off = State::default();
        off.scout.state = Some(on_state(
            |cfg| cfg.watches.comments = false,
            |_| {},
        ));
        let off_rt = render_state(off, draw_no_cmd);
        let off_words = collect_rt_text(&off_rt);
        assert!(off_words.contains("Your /locust comments"));
        assert!(off_rt.find("scout-watch-comments").is_some());
    }
}
