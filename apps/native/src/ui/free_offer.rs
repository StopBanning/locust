//! Busy and quiet free-model offers in the thread (`a-busy-free-model-offers-the-next`, `a-free-model-that-says-nothing-is-offered-another`).
use super::*;

fn is_busy_note(level: &str, text: &str) -> bool {
    level == "warning"
        && (text.contains("provider answered") || text.contains("trying again on its own"))
}

fn parse_rfc3339_ms(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp_millis())
}

fn opencode_models(s: &State, teammate_id: &str) -> Vec<locust_core::engine::Model> {
    s.models
        .get(teammate_id)
        .into_iter()
        .flatten()
        .find(|a| a.runtime == "opencode")
        .map(|a| a.models.clone())
        .unwrap_or_default()
}

fn apply_switch(s: &mut State, teammate_id: &str, next_runtime: &str, next_model: &str, prompt: &str) {
    stop(s);
    s.picks.insert(
        teammate_id.to_owned(),
        TurnPick {
            runtime: Some(next_runtime.to_owned()),
            model: next_model.to_owned(),
            effort: None,
            account: None,
        },
    );
    if !prompt.is_empty() {
        s.draft.replace_text(prompt);
        s.draft.focused = true;
    }
}

fn offer_button(
    key: &'static str,
    label: String,
    teammate_id: String,
    next_runtime: String,
    next_model: String,
    prompt: String,
    p: &Palette,
) -> El {
    el().key(key)
        .px(10.0)
        .py(6.0)
        .radius(6.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(text(label.clone()).font(type_::LABEL).color(p.text))
        .role("button")
        .label(label)
        .on_press(move |s: &mut State| {
            apply_switch(s, &teammate_id, &next_runtime, &next_model, &prompt);
        })
}

pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    view_at(s, p, chrono::Utc::now().timestamp_millis())
}

fn view_at(s: &State, p: &Palette, now_ms: i64) -> Option<El> {
    let pane = s.ledger.as_ref()?;
    if !turn_running(s) {
        return None;
    }
    let teammate_id = pane.header.id.clone();
    let last_meta = pane.snapshot.turn_metas.last();
    let runtime = last_meta
        .map(|m| m.runtime.clone())
        .or_else(|| s.picks.get(&teammate_id).and_then(|pk| pk.runtime.clone()))
        .or_else(|| pane.header.runtime.clone())
        .unwrap_or_default();
    let model = last_meta
        .map(|m| m.model.clone())
        .or_else(|| s.picks.get(&teammate_id).map(|pk| pk.model.clone()))
        .or_else(|| pane.header.model.clone())
        .unwrap_or_default();
    let prompt = last_meta
        .map(|m| m.prompt.clone())
        .unwrap_or_default();

    let started_at = last_meta
        .map(|m| m.created_at.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(pane.snapshot.started_at.as_str());
    let turn_start_idx = pane
        .snapshot
        .items
        .iter()
        .rposition(|i| matches!(i, ledger::Item::User(_)))
        .map_or(0, |at| at + 1);
    let turn_items = &pane.snapshot.items[turn_start_idx..];
    let has_running_tool = turn_items
        .iter()
        .any(|i| matches!(i, ledger::Item::Tool { status: ledger::Status::Running, .. }));

    let models = opencode_models(s, &teammate_id);
    let next = locust_core::route_rows::next_free_model(&runtime, Some(&model), &models, &[]);

    let has_busy_note = turn_items.iter().any(|i| match i {
        ledger::Item::Note { level, text } => is_busy_note(level, text),
        _ => false,
    });
    if has_busy_note && let Some(next) = next {
        let label = format!("Stop and switch to {}", locust_core::teammate_tooltip::route_model_name("opencode", &next.id));
        return Some(
            row()
                .w_full()
                .max_w(720.0)
                .px(24.0)
                .pb(12.0)
                .child(offer_button(
                    "busy-free-offer",
                    label,
                    teammate_id,
                    "opencode".into(),
                    next.id.clone(),
                    prompt,
                    p,
                )),
        );
    }

    let has_work = turn_items.iter().any(|i| {
        matches!(
            i,
            ledger::Item::Assistant { .. }
                | ledger::Item::Tool { .. }
                | ledger::Item::Plan { .. }
                | ledger::Item::Thought { .. }
                | ledger::Item::Trace(_)
                | ledger::Item::Limit { .. }
        )
    });
    if !has_work && let Some(next) = next {
        let began_ms = parse_rfc3339_ms(started_at);
        let mut events = vec![locust_core::route_rows::QuietEvent {
            kind: "run.started".into(),
            at_ms: began_ms,
            code: None,
        }];
        for item in turn_items {
            if let ledger::Item::Note { .. } = item {
                events.push(locust_core::route_rows::QuietEvent {
                    kind: "adapter.diagnostic".into(),
                    at_ms: began_ms,
                    code: None,
                });
            }
        }
        if locust_core::route_rows::quiet_free_run(Some(&runtime), Some(&model), &events, now_ms) {
            let label = format!("Stop and switch to {}", locust_core::teammate_tooltip::route_model_name("opencode", &next.id));
            let current_name = locust_core::teammate_tooltip::route_model_name(&runtime, &model);
            let sentence = format!(
                "{current_name} has said nothing in two minutes. Free models get busy; your message goes back in the chat box, to send there."
            );
            return Some(
                row()
                    .key("quiet-free-offer")
                    .w_full()
                    .max_w(720.0)
                    .px(24.0)
                    .pb(12.0)
                    .gap(10.0)
                    .center_y()
                    .child(
                        text(sentence)
                            .font(type_::LABEL)
                            .color(p.muted)
                            .grow(1.0)
                            .shrink()
                            .wrap(),
                    )
                    .child(offer_button(
                        "quiet-free-offer-action",
                        label,
                        teammate_id,
                        "opencode".into(),
                        next.id.clone(),
                        prompt,
                        p,
                    )),
            );
        }
    }

    if !has_running_tool
        && let Some(mins) = locust_core::route_rows::quiet_minutes((!started_at.is_empty()).then_some(started_at), now_ms)
    {
        return Some(
            row()
                .key("quiet-turn-row")
                .w_full()
                .max_w(720.0)
                .px(24.0)
                .pb(12.0)
                .gap(8.0)
                .center_y()
                .child(text(format!("no word for {mins} min")).font(type_::LABEL).color(p.muted))
                .child(
                    el().key("quiet-turn-stop")
                        .px(6.0)
                        .py(2.0)
                        .child(text("Stop").font(type_::LABEL).color(p.text))
                        .role("button")
                        .label("Stop")
                        .on_press(stop),
                ),
        );
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rune::{Env, Runtime};

    fn text_in(rt: &Runtime<State>, key: &str) -> Vec<String> {
        let ([x, y, w, h], _) = rt.presented(key).unwrap_or_else(|| panic!("no element {key} on screen"));
        rt.elements()
            .into_iter()
            .filter_map(|e| {
                let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
                (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
            })
            .collect()
    }

    fn render_offer(
        runtime: &str,
        model: &str,
        started_at: &str,
        items: Vec<ledger::Item>,
        now_ms: i64,
    ) -> Runtime<State> {
        let mut s = State::default();
        s.models.insert(
            "tm_1".into(),
            vec![engine::Models {
                runtime: "opencode".into(),
                model: None,
                effort: None,
                teammate: None,
                models: vec![
                    engine::Model { id: "opencode/glm-5.3-flash-free".into(), name: "GLM 5.3 Flash Free".into(), efforts: vec![] },
                    engine::Model { id: "opencode/beam-free".into(), name: "Beam Free".into(), efforts: vec![] },
                    engine::Model { id: "opencode/deepseek-v4.1-flash-free".into(), name: "DeepSeek V4.1 Flash Free".into(), efforts: vec![] },
                    engine::Model { id: "opencode/big-pickle-free".into(), name: "Big Pickle Free".into(), efforts: vec![] },
                ],
            }],
        );
        let mut snap = ledger::Snapshot::default();
        snap.running = true;
        snap.started_at = started_at.into();
        snap.items = items;
        snap.turn_metas.push(ledger::Metadata {
            mission_id: "m1".into(),
            prompt: "Where can I keep savings for three years?".into(),
            runtime: runtime.into(),
            model: model.into(),
            created_at: started_at.into(),
            ..Default::default()
        });
        s.ledger = Some(LedgerPane {
            header: Teammate {
                id: "tm_1".into(),
                name: "Wren".into(),
                role: "Code".into(),
                runtime: Some(runtime.into()),
                model: Some(model.into()),
                account: None,
                busy: true,
                runs_in: "local".into(),
                folder: None,
            },
            conversation: Conversation::default(),
            items: snap.items.clone(),
            dir: std::path::PathBuf::new(),
            worker: None,
            snapshot: snap,
            due: f64::INFINITY,
            loading: false,
            after_tip: None,
            after_tip_deadline: None,
            backoff: 0.0,
        });
        thread_local! {
            static NOW_MS: std::cell::Cell<i64> = const { std::cell::Cell::new(0) };
        }
        NOW_MS.with(|c| c.set(now_ms));
        fn test_view(s: &State, _cx: &mut Cx<State>) -> El {
            let p = Palette::new(true);
            let now = NOW_MS.with(|c| c.get());
            col().w_full().h_full().children(view_at(s, &p, now))
        }
        let mut rt = Runtime::new(s, test_view, (800.0, 200.0), Env::new(true, 1.0));
        rt.frame(0.0);
        rt
    }

    // a-busy-free-model-offers-the-next.test.tsx
    #[test]
    fn carries_the_offer_as_a_button_under_its_words_and_only_the_busy_one_does() {
        let rt_busy = render_offer(
            "opencode",
            "opencode/glm-5.3-flash-free",
            "2026-09-26T19:00:00.000Z",
            vec![ledger::Item::Note {
                level: "warning".into(),
                text: "The model's provider answered \"Rate limit exceeded\", and OpenCode is trying again on its own.".into(),
            }],
            0,
        );
        assert!(rt_busy.find("busy-free-offer").is_some());
        assert_eq!(text_in(&rt_busy, "busy-free-offer"), vec!["Stop and switch to Beam Free"]);

        let rt_other = render_offer(
            "opencode",
            "opencode/glm-5.3-flash-free",
            "2026-09-26T19:00:00.000Z",
            vec![ledger::Item::Note {
                level: "warning".into(),
                text: "Something else happened.".into(),
            }],
            0,
        );
        assert!(rt_other.find("busy-free-offer").is_none());
    }

    #[test]
    fn clicking_the_offer_stops_the_run_switches_the_route_and_hands_the_message_back() {
        let mut rt = render_offer(
            "opencode",
            "opencode/glm-5.3-flash-free",
            "2026-09-26T19:00:00.000Z",
            vec![ledger::Item::Note {
                level: "warning".into(),
                text: "The model's provider answered \"Rate limit exceeded\", and OpenCode is trying again on its own.".into(),
            }],
            0,
        );
        let (x, y) = rt.find("busy-free-offer").expect("busy offer button");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
        let pick = rt.state.picks.get("tm_1").expect("route switched");
        assert_eq!(pick.runtime.as_deref(), Some("opencode"));
        assert_eq!(pick.model, "opencode/beam-free");
        assert_eq!(rt.state.draft.text, "Where can I keep savings for three years?");
    }

    // a-free-model-that-says-nothing-is-offered-another.test.tsx
    #[test]
    fn names_the_quiet_model_says_where_the_message_goes_and_offers_the_next_one() {
        let began = chrono::DateTime::parse_from_rfc3339("2026-11-01T05:00:00.000Z").unwrap().timestamp_millis();
        let rt_quiet = render_offer(
            "opencode",
            "opencode/deepseek-v4.1-flash-free",
            "2026-11-01T05:00:00.000Z",
            vec![],
            began + locust_core::route_rows::QUIET_FREE_MS,
        );
        assert!(rt_quiet.find("quiet-free-offer").is_some());
        let words = text_in(&rt_quiet, "quiet-free-offer").join(" ");
        assert!(words.contains("DeepSeek V4.1 Flash Free has said nothing in two minutes."));
        assert!(words.contains("your message goes back in the chat box"));
        assert!(words.contains("Stop and switch to GLM 5.3 Flash Free"));

        let rt_early = render_offer(
            "opencode",
            "opencode/deepseek-v4.1-flash-free",
            "2026-11-01T05:00:00.000Z",
            vec![],
            began + locust_core::route_rows::QUIET_FREE_MS - 1,
        );
        assert!(rt_early.find("quiet-free-offer").is_none());
    }

    // a-turn-that-goes-quiet-says-so.test.tsx
    #[test]
    fn says_so_after_five_minutes_without_a_word_with_stop_beside_it() {
        let began = chrono::DateTime::parse_from_rfc3339("2026-11-01T05:00:00.000Z").unwrap().timestamp_millis();
        let rt_quiet = render_offer(
            "claude",
            "opus",
            "2026-11-01T05:00:00.000Z",
            vec![ledger::Item::Assistant { item_id: "m1".into(), text: "ONE".into(), r#final: true }],
            began + 7 * 60_000,
        );
        assert!(rt_quiet.find("quiet-turn-row").is_some());
        let words = text_in(&rt_quiet, "quiet-turn-row").join(" ");
        assert!(words.contains("no word for 7 min"), "got: {words}");
        assert!(words.contains("Stop"), "got: {words}");

        // While a command it named runs, it says nothing.
        let rt_tool = render_offer(
            "claude",
            "opus",
            "2026-11-01T05:00:00.000Z",
            vec![ledger::Item::Tool {
                name: "Bash".into(),
                status: ledger::Status::Running,
                summary: "Running".into(),
                detail: Box::new(ledger::ToolDetail::default()),
            }],
            began + 7 * 60_000,
        );
        assert!(rt_tool.find("quiet-turn-row").is_none());
    }
}
