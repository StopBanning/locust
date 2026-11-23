//! Electron's `qa-0365-concurrency.test.ts` (3 cases): a run that is over is not a run you are sharing the
//! folder with. A relayed reply, or a run started while a finished run is still reading the disk, must not be
//! marked with `host.shared_workspace`; two runs that genuinely overlap still are.

use super::*;

fn team_mate(id: &str, name: &str, role: &str) -> Value {
    json!({
        "teammateId": id,
        "name": name,
        "hue": "blue",
        "role": "Custom",
        "roleTitle": role,
        "createdAt": "2026-09-06T15:00:00.000Z",
        "avatar": {},
        "route": {"runtime": "codex", "model": "synthetic-model", "mode": "accept-edits"}
    })
}

fn teammate_in(t: &Temp, id: &str, name: &str, role: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: role.into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "accept-edits".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    }
}

fn shared_notices_in(t: &Temp) -> Vec<Value> {
    ledgers(t)
        .into_iter()
        .flat_map(|p| records(&p))
        .map(|r| r["event"].clone())
        .filter(|ev| ev["type"] == "adapter.diagnostic" && ev["payload"]["code"] == "host.shared_workspace")
        .collect()
}

// case: a relayed reply is not marked as sharing the folder with the sender whose run already ended
#[test]
fn a_relayed_reply_is_not_marked_as_sharing_the_folder_with_the_sender_whose_run_already_ended() {
    let t = Temp::new();
    let store = t.store();
    store
        .save_json(
            "teammates.json",
            &json!({
                "schemaVersion": 1,
                "teammates": [
                    team_mate("tm_wren", "Wren", "Code & Migrations"),
                    team_mate("tm_gem", "Gem", "Research & Briefs")
                ],
                "missionOwners": {},
                "settings": {"relay": true, "relayHopCap": 6}
            }),
        )
        .unwrap();
    let _g = hooks::install(store.clone(), fake("qa0365-relay"));

    let wren = teammate_in(&t, "tm_wren", "Wren", "Code & Migrations");
    let first = send(Send {
        teammate: wren,
        conversation: None,
        prompt: "do the work".into(),
        model: Some("synthetic-model".into()),
        effort: None,
    })
    .unwrap();

    wait("Wren and relayed Gem run to finish", || {
        !owns(&first.conversation_id)
            && crate::invariants::lock(&ACTIVE_TEAMMATES).is_empty()
            && crate::people::hub_of(&store, "tm_gem").is_some()
            && t.0.join("work/qa0365-wren.txt").exists()
            && t.0.join("work/qa0365-gem.txt").exists()
    });
    let gem_hub = crate::people::hub_of(&store, "tm_gem").expect("Gem hub mission recorded");
    finished(&gem_hub);

    // Premise: both Wren and Gem ran and edited files in the shared folder.
    assert!(ledgers(&t).len() >= 2, "expected at least 2 mission ledgers, got {}", ledgers(&t).len());
    // Neither run should be told the folder cannot be told apart.
    assert!(
        shared_notices_in(&t).is_empty(),
        "unexpected host.shared_workspace notice: {:?}",
        shared_notices_in(&t)
    );
}

// case: a run started while a finished run is still reading the disk does not lose its attribution
#[test]
fn a_run_started_while_a_finished_run_is_still_reading_the_disk_does_not_lose_its_attribution() {
    let t = Temp::new();
    let store = t.store();
    let _g = hooks::install(store.clone(), fake("qa0365-edit"));

    let second_id = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));
    let second_id_hook = second_id.clone();
    let gem = teammate_in(&t, "tm_gem", "Gem", "Research & Briefs");
    *crate::invariants::lock(&hooks::ON_AFTER_DISK) = Some(Box::new(move || {
        let started = send(Send {
            teammate: gem,
            conversation: None,
            prompt: "the next thing".into(),
            model: Some("synthetic-model".into()),
            effort: None,
        })
        .unwrap();
        *crate::invariants::lock(&second_id_hook) = Some(started.conversation_id);
        std::thread::sleep(Duration::from_millis(25));
    }));

    let wren = teammate_in(&t, "tm_wren", "Wren", "Code & Migrations");
    let first = send(Send {
        teammate: wren,
        conversation: None,
        prompt: "do the work".into(),
        model: Some("synthetic-model".into()),
        effort: None,
    })
    .unwrap();
    finished(&first.conversation_id);
    let second = crate::invariants::lock(&second_id)
        .clone()
        .expect("second run should have started during first run's disk observation");
    finished(&second);

    assert!(
        shared_notices_in(&t).is_empty(),
        "unexpected host.shared_workspace notice: {:?}",
        shared_notices_in(&t)
    );
}

// case: two runs that really do overlap are still marked
#[test]
fn two_runs_that_really_do_overlap_are_still_marked() {
    let t = Temp::new();
    let store = t.store();
    let _g = hooks::install(store.clone(), fake("qa0365-overlap"));

    let wren = teammate_in(&t, "tm_wren", "Wren", "Code & Migrations");
    let gem = teammate_in(&t, "tm_gem", "Gem", "Research & Briefs");
    let first = send(Send {
        teammate: wren,
        conversation: None,
        prompt: "one".into(),
        model: Some("synthetic-model".into()),
        effort: None,
    })
    .unwrap();
    wait("first run to be live", || t.0.join("work/qa0365-first-live.gate").exists());

    let second = send(Send {
        teammate: gem,
        conversation: None,
        prompt: "two".into(),
        model: Some("synthetic-model".into()),
        effort: None,
    })
    .unwrap();
    finished(&first.conversation_id);
    finished(&second.conversation_id);

    assert!(
        !shared_notices_in(&t).is_empty(),
        "expected host.shared_workspace notice for genuinely overlapping runs"
    );
}
