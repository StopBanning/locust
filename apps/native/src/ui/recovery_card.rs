//! The interrupted-run card (Electron `ResumeCard.tsx`). A turn Locust's quit cut short, whose checkpoint the record can
//! vouch for, offers Electron's control: "Resume from checkpoint", or "Resume anyway" beside the caution when an action
//! never reported back. The press asks the engine to reconcile a fresh checkpoint and start the next turn from its
//! brief. The card shows the engine's refusal inline if the press could not start a turn.
use super::*;
use crate::recovery::Offer;
use std::sync::OnceLock;

pub(super) struct Attempt {
    mission: String,
    refusal: Arc<OnceLock<String>>,
}

fn resume(s: &mut State, mission: String, runtime: String) {
    if s.flight.is_some() || s.sending.is_pending() {
        return;
    }
    let Some(pane) = s.ledger.as_ref().filter(|pane| {
        pane.snapshot.tip == mission && matches!(pane.snapshot.recovery, Some(Offer::Resume { .. }))
    }) else {
        return;
    };
    let header = pane.header.clone();
    let conversation_id = pane.conversation.conversation_id.clone();
    let refusal = Arc::new(OnceLock::new());
    s.turn_undo.resume_attempt = Some(Attempt { mission: mission.clone(), refusal: refusal.clone() });

    // Use the window's normal send completion and ledger tracking. Keep the raw refusal with this card's mission.
    s.own_turn = None;
    s.picks.remove(&header.id);
    s.picks.remove(LEDGER_TARGET);
    s.ledger_job.cancel();
    s.model_menu = None;
    set_status(s, &header.id, "Sending…", StatusTone::Muted);
    s.flight = Some(Flight {
        teammate_id: header.id.clone(),
        conversation_id,
        header: Some(header),
        after_tip: Some(mission.clone()),
        message: None,
    });
    s.sending.request((mission, runtime), move |(mission, runtime)| {
        locust_core::engine_api::api().carry(&mission, &runtime, None)
            .map(|(_, _, mode)| engine::Started { conversation_id: mission, mode })
            .inspect_err(|why| { let _ = refusal.set(why.clone()); })
    });
}

pub(super) fn card(s: &State, p: &Palette) -> Option<El> {
    if s.flight.is_some() {
        return None;
    }
    let pane = s.ledger.as_ref()?;
    let tip = &pane.snapshot.tip;
    let offer = pane.snapshot.recovery.as_ref();
    let refusal = s.turn_undo.resume_attempt.as_ref()
        .filter(|attempt| &attempt.mission == tip)
        .and_then(|attempt| attempt.refusal.get());
    if offer.is_none() && refusal.is_none() {
        return None;
    }
    let mut card = col()
        .w_full()
        .gap(8.0)
        .padding(12.0)
        .radius(9.0)
        .fill(p.raised);
    match offer {
        Some(Offer::Refused(note)) => {
            card = card
                .child(text("Cannot be resumed from here").color(p.text))
                .child(text(note.clone()).color(p.secondary).wrap_anywhere())
        }
        Some(offer @ Offer::Resume {
            epoch,
            note,
            unverified,
        }) => {
            card = card
                .child(text("Interrupted — the last checkpoint was kept").color(p.text))
                .child(
                    text(format!("checkpoint {epoch}"))
                        .font(type_::LABEL)
                        .color(p.muted),
                )
                .child(text(note.clone()).color(p.secondary).wrap_anywhere());
            for name in unverified {
                card = card.child(text(name.clone()).color(p.amber).wrap_anywhere());
            }
            // The turn's own agent: a resume here is a follow-up on it, never a switch.
            let runtime = pane.snapshot.turn_metas.iter().find(|m| &m.mission_id == tip).map(|m| m.runtime.clone()).filter(|r| !r.is_empty());
            match (s.engine == Some(Kind::Native), runtime, locust_core::recovery::resume_control(offer)) {
                (true, Some(runtime), Some(label)) if !tip.is_empty() => {
                    let tip = tip.clone();
                    let l = crate::look::controls::Look::new(p.dark);
                    let mut actions = row().w_full().gap(10.0).center_y().child(
                        crate::look::controls::button(&l, label, crate::look::controls::Tone::Neutral, false)
                            .key("resume-from-checkpoint")
                            .on_press(move |s: &mut State| resume(s, tip.clone(), runtime.clone())),
                    );
                    if !unverified.is_empty() {
                        actions = actions.child(
                            text(locust_core::recovery::RESUME_CAUTION)
                                .font(type_::LABEL)
                                .color(p.muted)
                                .wrap_anywhere()
                                .grow(1.0),
                        );
                    }
                    card = card.child(actions);
                }
                _ => {
                    // Another engine's turn: this window cannot pick it up.
                    card = card.child(text("Cannot be resumed from here").color(p.secondary));
                }
            }
        }
        None => {}
    }
    if let Some(why) = refusal {
        card = card.child(text(format!("Not resumed: {why}")).key("resume-refusal").role("alert").color(p.text).wrap_anywhere());
    }
    Some(card)
}

/// Electron `ResumeCard.tsx` and `App.tsx` `resumeMission`, in the window with no screen: a turn the quit cut short
/// offers "Resume from checkpoint"; the press picks it up on its own agent from the reconciled checkpoint, told what was
/// done rather than sent its prompt again; a doubt says "Resume anyway" beside the caution; a refusal has no button.
#[cfg(test)]
mod tests {
    use crate::*;
    use rune::{Env, Key, Runtime};
    use std::path::PathBuf;
    use std::time::Duration;

    struct Dirs(PathBuf);
    impl Drop for Dirs {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn settle(rt: &mut Runtime<State>) {
        for _ in 0..40 {
            rt.frame(0.05);
        }
    }

    fn click(rt: &mut Runtime<State>, key: &str) {
        settle(rt);
        let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        settle(rt);
    }

    fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(60);
        let mut now = 0.0;
        while !done(&rt.state) {
            assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}: status {:?}", rt.state.status);
            rt.update(move |s, tx| {
                poll(s, now, tx);
            });
            rt.frame(0.3);
            now += 0.3;
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    fn texts(rt: &Runtime<State>) -> Vec<String> {
        rt.elements().into_iter().filter_map(|e| e.text).collect()
    }

    fn fake(scenario: &str) -> Option<(PathBuf, Vec<String>)> {
        Some((
            PathBuf::from("python3"),
            vec![format!("{}/scripts/fixtures/m1-app-server.py", env!("CARGO_MANIFEST_DIR")), scenario.into()],
        ))
    }

    fn first_record(store: &locust_core::store::Store, id: &str) -> serde_json::Value {
        let path = store.write_path(id).unwrap();
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        serde_json::from_str(text.lines().next().unwrap()).unwrap()
    }

    fn interrupted() -> (Dirs, locust_core::store::Store, Runtime<State>, String, native::hooks::Guard) {
        let root = std::env::temp_dir().join(format!("locust-resume-card-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
        std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
        std::fs::create_dir_all(root.join("work")).unwrap();
        let _dirs = Dirs(root.clone());
        let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        store
            .save_json(
                "teammates.json",
                &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "role": "Code & Migrations", "hue": "lime",
                    "createdAt": "2026-11-26T00:00:00.000Z", "folder": root.join("work").to_string_lossy(), "route": {"runtime": "codex", "model": "synthetic-model", "mode": "ask"}}],
                    "missionOwners": {}, "settings": {}}),
            )
            .unwrap();
        let hook = native::hooks::install(store.clone(), fake("interrupt"));
        let mut s = State::default();
        s.engine = Some(Kind::Native);
        s.data = Some(store.clone());
        apply_team(
            &mut s,
            vec![Teammate {
                id: "tm_codec".into(),
                name: "Codec".into(),
                role: "Code & Migrations".into(),
                runtime: Some("codex".into()),
                model: Some("synthetic-model".into()),
                account: None,
                busy: false,
                runs_in: "ask".into(),
                folder: Some(root.join("work").to_string_lossy().into()),
            }],
        );
        let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
        rt.frame(0.0);
        click(&mut rt, "composer");
        rt.paste("Synthetic prompt");
        rt.frame(0.016);
        rt.key(Key::Enter.into());
        rt.frame(0.016);
        pump(&mut rt, "the turn to run", |s| s.ledger.as_ref().is_some_and(|p| p.snapshot.running && !p.snapshot.tip.is_empty()));
        let cut_short = rt.state.ledger.as_ref().map(|p| p.snapshot.tip.clone()).unwrap();
        assert!(rt.find("resume-from-checkpoint").is_none(), "nothing to resume while it runs");

        // Locust quits under the turn, as the window closes it (through the engine); the card reads the checkpoint the quit kept.
        locust_core::engine_api::api().shut_down();
        locust_core::engine_api::api().wait_for_shutdown();
        drop(hook);
        let hook = native::hooks::install(store.clone(), fake("cards-share"));
        pump(&mut rt, "the card", |s| s.flight.is_none() && s.ledger.as_ref().is_some_and(|p| !p.snapshot.running && p.snapshot.recovery.is_some()));
        settle(&mut rt);
        let shown = texts(&rt);
        assert!(shown.iter().any(|t| t == "Interrupted — the last checkpoint was kept"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Resume from checkpoint"), "{shown:?}");
        assert!(!shown.iter().any(|t| t == "Cannot be resumed from here"), "{shown:?}");
        (_dirs, store, rt, cut_short, hook)
    }

    // case: continues the conversation from the checkpoint the quit kept, on the same agent, with Electron's control
    #[test]
    fn resume_from_checkpoint_picks_up_a_turn_the_quit_cut_short_on_its_own_agent() {
        let (dirs, store, mut rt, cut_short, _hook) = interrupted();
        click(&mut rt, "resume-from-checkpoint");
        assert!(rt.state.flight.is_some(), "the press starts the resume");
        pump(&mut rt, "the resumed turn", |s| {
            s.flight.is_none() && s.ledger.as_ref().is_some_and(|p| !p.snapshot.running && !p.snapshot.tip.is_empty() && p.snapshot.tip != cut_short)
        });
        settle(&mut rt);
        let next = rt.state.ledger.as_ref().map(|p| p.snapshot.tip.clone()).unwrap();
        let metadata = first_record(&store, &next)["metadata"].clone();
        assert_eq!(metadata["continuesFrom"]["missionId"], serde_json::json!(cut_short));
        assert_eq!(metadata["continuesFrom"]["reason"], "follow-up", "on the same agent");
        assert_eq!(metadata["runtime"], "codex");
        assert_eq!(metadata["startedBy"], serde_json::json!({"kind": "resume", "epoch": 2}));
        let told = std::fs::read_to_string(dirs.0.join("last-prompt.txt")).unwrap();
        assert!(told.contains("Synthetic prompt") && told.trim() != "Synthetic prompt", "told from the checkpoint: {told}");
        assert!(rt.find("resume-from-checkpoint").is_none(), "the resumed turn finished: nothing left to resume");

        // A doubt: "Resume anyway", the caution beside it. A refusal: words, no button.
        rt.update(|s, _| {
            if let Some(pane) = s.ledger.as_mut() {
                pane.snapshot.recovery = Some(locust_core::recovery::Offer::Resume { epoch: 3, note: "One action started and never reported an outcome, so whether it took effect is unknown.".into(), unverified: vec!["npm publish".into()] });
            }
        });
        settle(&mut rt);
        let shown = texts(&rt);
        assert!(shown.iter().any(|t| t == "Resume anyway"), "{shown:?}");
        assert!(shown.iter().any(|t| t == "Check those first — the next run is told to verify them before building on them."), "{shown:?}");
        rt.update(|s, _| {
            if let Some(pane) = s.ledger.as_mut() {
                pane.snapshot.recovery = Some(locust_core::recovery::Offer::Refused("The mission ledger stopped short of its end during recovery.".into()));
            }
        });
        settle(&mut rt);
        assert!(rt.find("resume-from-checkpoint").is_none(), "a refusal has no button");
        assert!(texts(&rt).iter().any(|t| t == "Cannot be resumed from here"));
    }

    #[test]
    fn resume_press_shows_the_refusal_on_the_pressed_card_only() {
        let (dirs, store, mut rt, cut_short, _hook) = interrupted();
        let path = store.write_path(&cut_short).unwrap();
        let before = std::fs::read(&path).unwrap();
        let last_prompt = dirs.0.join("last-prompt.txt");
        let prompt_before = std::fs::read(&last_prompt).ok();
        let original = std::fs::metadata(&path).unwrap().permissions();
        let mut read_only = original.clone();
        read_only.set_readonly(true);
        std::fs::set_permissions(&path, read_only).unwrap();
        // No alert until the press. The ledger is still readable, but reconciling cannot append a checkpoint.
        assert!(rt.find("resume-refusal").is_none());
        click(&mut rt, "resume-from-checkpoint");
        assert!(rt.state.flight.is_some(), "the press asks the engine to resume");
        pump(&mut rt, "the refused resume", |s| s.flight.is_none());
        std::fs::set_permissions(&path, original).unwrap();
        settle(&mut rt);
        assert!(rt.find("resume-refusal").is_some());
        assert!(texts(&rt).iter().any(|t| t == "Not resumed: The mission could not be reconciled, so nothing was resumed."));
        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::read(last_prompt).ok(), prompt_before);
        assert_eq!(rt.state.ledger.as_ref().unwrap().snapshot.tip, cut_short);

        rt.update(|s, _| s.ledger.as_mut().unwrap().snapshot.tip = "another_mission".into());
        settle(&mut rt);
        assert!(rt.find("resume-refusal").is_none(), "the refusal belongs to the pressed mission");
    }
}
#[cfg(test)] #[path = "recovery_card_refusal_without_offer_tests.rs"] mod refusal_without_offer_tests;
