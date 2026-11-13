use super::*;
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Timelike};
fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> chrono::DateTime<Local> {
    Local
        .with_ymd_and_hms(y, m, d, h, min, 0)
        .earliest()
        .unwrap()
}
#[test]
fn schedule_validation_ts_cases() {
    for v in [
        json!({"kind":"every","hours":1}),
        json!({"kind":"every","hours":168}),
        json!({"kind":"daily","at":"09:30"}),
        json!({"kind":"weekly","days":[1,3,5],"at":"09:00"}),
        json!({"kind":"once","on":"2026-10-07T15:00"}),
        json!({"kind":"files","folder":"inbox/pdfs"}),
        json!({"kind":"hook"}),
    ] {
        assert!(schedule::valid(&v), "{v}");
    }
    for v in [
        json!({"kind":"every","hours":0}),
        json!({"kind":"every","hours":1.5}),
        json!({"kind":"every","hours":169}),
        json!({"kind":"daily","at":"24:00"}),
        json!({"kind":"daily","at":"9:30"}),
        json!({"kind":"weekly","days":[],"at":"09:00"}),
        json!({"kind":"weekly","days":[3,1],"at":"09:00"}),
        json!({"kind":"weekly","days":[1,1],"at":"09:00"}),
        json!({"kind":"weekly","days":[7],"at":"09:00"}),
        json!({"kind":"once","on":"2026-02-30T15:00"}),
        json!({"kind":"once","on":"2026-10-07 15:00"}),
        json!({"kind":"files","folder":"../inbox"}),
        json!({"kind":"files","folder":"/inbox"}),
        json!({"kind":"files","folder":"C:/inbox"}),
        json!({"kind":"files","folder":"C:inbox"}),
        json!({"kind":"hook","url":"secret"}),
        Value::Null,
    ] {
        assert!(!schedule::valid(&v), "{v}");
    }
}
#[test]
fn interval_and_closed_stretch_are_one_miss() {
    let last = at(2026, 9, 1, 10, 0);
    let now = at(2026, 9, 5, 12, 0);
    let s = json!({"kind":"every","hours":4});
    assert_eq!(schedule::next(&s, last, now), Some(now));
    let missed = schedule::missed(&s, last, now, now).unwrap();
    assert_eq!(missed, at(2026, 9, 5, 10, 0));
    assert!(schedule::missed(&s, missed, now, now).is_none());
    assert_eq!(schedule::next(&s, missed, now), Some(at(2026, 9, 5, 14, 0)));
}
#[test]
fn daily_uses_calendar_slots_and_does_not_catch_up() {
    let s = json!({"kind":"daily","at":"09:00"});
    let last = at(2026, 9, 4, 9, 0);
    let now = at(2026, 9, 5, 8, 0);
    assert_eq!(schedule::next(&s, last, now), Some(at(2026, 9, 5, 9, 0)));
    assert!(schedule::missed(&s, last, now, at(2026, 9, 5, 9, 30)).is_none());
    assert_eq!(
        schedule::next(&s, at(2026, 9, 5, 9, 1), at(2026, 9, 5, 9, 30)),
        Some(at(2026, 9, 6, 9, 0))
    );
    // DST days retain 09:00 local, independently of the machine's offset.
    let next = schedule::next(&s, at(2026, 3, 7, 9, 1), at(2026, 3, 7, 10, 0)).unwrap();
    assert_eq!((next.day(), next.hour()), (8, 9));
}
#[test]
fn daily_keeps_wall_clock_across_dst_when_host_is_new_york() {
    // Port of main/a-daily-routine-keeps-its-time.test.ts (America/New_York oracle).
    let offset = chrono::FixedOffset::west_opt(4 * 3600).unwrap();
    if Local.from_utc_datetime(
        &chrono::NaiveDate::from_ymd_opt(2026, 3, 8)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap(),
    )
    .offset()
    .local_minus_utc()
        != offset.local_minus_utc()
    {
        return;
    }
    let s = json!({"kind":"daily","at":"09:00"});
    let spring = schedule::next(
        &s,
        DateTime::parse_from_rfc3339("2026-03-07T09:00:00-05:00")
            .unwrap()
            .with_timezone(&Local),
        DateTime::parse_from_rfc3339("2026-03-07T12:00:00-05:00")
            .unwrap()
            .with_timezone(&Local),
    )
    .unwrap();
    assert_eq!(
        spring.to_rfc3339(),
        DateTime::parse_from_rfc3339("2026-03-08T09:00:00-04:00")
            .unwrap()
            .with_timezone(&Local)
            .to_rfc3339()
    );
    let fall = schedule::next(
        &s,
        DateTime::parse_from_rfc3339("2026-10-31T09:00:00-04:00")
            .unwrap()
            .with_timezone(&Local),
        DateTime::parse_from_rfc3339("2026-10-31T12:00:00-04:00")
            .unwrap()
            .with_timezone(&Local),
    )
    .unwrap();
    assert_eq!(
        fall.to_rfc3339(),
        DateTime::parse_from_rfc3339("2026-11-01T09:00:00-05:00")
            .unwrap()
            .with_timezone(&Local)
            .to_rfc3339()
    );
}
#[test]
fn weekly_weekends_missed_monday_and_made_after_slot() {
    let s = json!({"kind":"weekly","days":[1,2,3,4,5],"at":"09:00"});
    let last = at(2026, 10, 2, 9, 1);
    for day in [2, 3, 4] {
        assert_eq!(
            schedule::next(&s, last, at(2026, 10, day, 12, 0)),
            Some(at(2026, 10, 5, 9, 0))
        );
    }
    let monday = json!({"kind":"weekly","days":[1],"at":"09:00"});
    assert_eq!(
        schedule::next(&monday, at(2026, 9, 28, 9, 1), at(2026, 10, 7, 14, 0)),
        Some(at(2026, 10, 5, 9, 0))
    );
    assert_eq!(
        schedule::next(&monday, at(2026, 10, 6, 10, 0), at(2026, 10, 6, 10, 1)),
        Some(at(2026, 10, 12, 9, 0))
    );
}
#[test]
fn once_runs_only_before_receipt_and_files_have_no_clock() {
    let s = json!({"kind":"once","on":"2026-10-07T15:00"});
    let due = at(2026, 10, 7, 15, 0);
    assert_eq!(
        schedule::next(&s, at(2026, 10, 1, 12, 0), due + Duration::days(2)),
        Some(due)
    );
    assert!(schedule::next(&s, due, due).is_none());
    for s in [
        json!({"kind":"files","folder":"inbox"}),
        json!({"kind":"hook"}),
    ] {
        assert!(schedule::next(&s, due, due).is_none());
    }
}
#[test]
fn exact_card_words_and_resumed_clock() {
    for (s, expected) in [
        (Value::Null, "when you press Run"),
        (json!({"kind":"every","hours":1}), "every hour"),
        (json!({"kind":"every","hours":8}), "every 8 hours"),
        (json!({"kind":"daily","at":"07:45"}), "daily at 07:45"),
        (
            json!({"kind":"weekly","days":[1,2,3,4,5],"at":"09:00"}),
            "weekdays at 09:00",
        ),
        (
            json!({"kind":"weekly","days":[0,6],"at":"10:30"}),
            "weekends at 10:30",
        ),
        (
            json!({"kind":"weekly","days":[0,1,3],"at":"08:00"}),
            "Mon, Wed, Sun at 08:00",
        ),
        (
            json!({"kind":"once","on":"2026-10-07T15:00"}),
            "once, Wed Oct 7 at 15:00",
        ),
        // Spring-forward gap: label keeps the picked 02:30, not the shifted wall clock.
        (
            json!({"kind":"once","on":"2026-03-08T02:30"}),
            "once, Sun Mar 8 at 02:30",
        ),
    ] {
        assert_eq!(schedule::label(&s), expected);
    }
    let now = at(2026, 10, 7, 10, 0);
    assert_eq!(
        schedule::base(
            &json!({"createdAt":(now-Duration::days(7)).to_rfc3339(),"resumedAt":now.to_rfc3339()})
        ),
        Some(now)
    );
}
#[test]
fn closed_miss_uses_opened_at_not_now() {
    // Port of a-routine-due-while-closed-is-missed: open before the slot starts it; after misses it.
    let s = json!({"kind":"daily","at":"08:00"});
    let last = at(2026, 9, 4, 8, 0);
    let due_day = at(2026, 9, 5, 9, 0);
    assert!(schedule::missed(&s, last, at(2026, 9, 5, 7, 0), due_day).is_none());
    assert_eq!(
        schedule::missed(&s, last, at(2026, 9, 5, 9, 0), due_day),
        Some(at(2026, 9, 5, 8, 0))
    );
}
#[test]
fn slot_passed_during_a_long_run_is_missed() {
    let daily = json!({"kind":"daily","at":"09:00"});
    let started = at(2026, 9, 5, 8, 0);
    let ended = at(2026, 9, 5, 10, 0);
    assert_eq!(
        schedule::slot_passed_during(&daily, started, ended),
        Some(at(2026, 9, 5, 9, 0))
    );
    assert!(schedule::slot_passed_during(&daily, started, at(2026, 9, 5, 8, 30)).is_none());
    assert!(
        schedule::slot_passed_during(&json!({"kind":"every","hours":1}), started, ended).is_none()
    );
}
fn fixture() -> (std::path::PathBuf, Store) {
    let root = std::env::temp_dir().join(format!(
        "routines-{}",
        crate::native::uuid_v4().unwrap()
    ));
    let electron = root.join("electron");
    std::fs::create_dir_all(&electron).unwrap();
    std::fs::write(
        electron.join("routines.json"),
        json!({"schemaVersion":1,"routines":[]}).to_string(),
    )
    .unwrap();
    std::fs::write(electron.join("teammates.json"),json!({"schemaVersion":1,"teammates":[{"teammateId":"codec","name":"Codec","hue":"blue","role":"Custom","createdAt":"2026-11-01T00:00:00Z","folder":root.to_string_lossy(),"route":{"runtime":"codex","model":"synthetic-model","mode":"ask"}}]}).to_string()).unwrap();
    let store = Store::new(root.join("native"), electron).unwrap();
    (root, store)
}
#[test]
fn seed_once_pause_resume_edit_and_remove_leave_electron_unchanged() {
    let (root, store) = fixture();
    let source = std::fs::read(store.electron().join("routines.json")).unwrap();
    save(&store,json!({"name":" Test ","teammateId":"codec","steps":["One","Two"],"schedule":{"kind":"every","hours":4}})).unwrap();
    let r = list(&store).unwrap()["routines"][0].clone();
    let id = r["routineId"].as_str().unwrap().to_owned();
    assert_eq!(r["name"], "Test");
    pause(&store, &id, true, &now()).unwrap();
    assert_eq!(list(&store).unwrap()["routines"][0]["paused"], true);
    let resumed = "2026-10-07T15:00:00.000Z";
    pause(&store, &id, false, resumed).unwrap();
    assert_eq!(list(&store).unwrap()["routines"][0]["resumedAt"], resumed);
    let mut edited = r;
    edited["name"] = json!("Edited");
    edited["runs"] = json!(999);
    let id = id.to_owned();
    save(&store, edited).unwrap();
    assert_eq!(list(&store).unwrap()["routines"][0]["runs"], 0);
    assert_eq!(
        std::fs::read(store.electron().join("routines.json")).unwrap(),
        source
    );
    remove(&store, &id).unwrap();
    assert_eq!(list(&store).unwrap()["routines"], json!([]));
    assert_eq!(
        std::fs::read(store.electron().join("routines.json")).unwrap(),
        source
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn unreadable_file_is_not_overwritten_and_step_budget_is_utf16() {
    let (root, store) = fixture();
    let entry = json!({"name":"Test","teammateId":"codec","steps":["😀".repeat(4001)]});
    assert!(save(&store, entry).unwrap_err().contains("8,002"));
    list(&store).unwrap();
    let path = store.root().join("routines.json");
    std::fs::write(&path, "broken").unwrap();
    assert_eq!(pause(&store, "rt_a", true, &now()).unwrap_err(), UNREADABLE);
    assert_eq!(std::fs::read_to_string(path).unwrap(), "broken");
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn uncertain_progress_blocks_replay_removal_and_stale_acknowledgements() {
    let (root, store) = fixture();
    save(
        &store,
        json!({"name":"Test","teammateId":"codec","steps":["One"],"schedule":{"kind":"every","hours":4}}),
    )
    .unwrap();
    let id = list(&store).unwrap()["routines"][0]["routineId"]
        .as_str()
        .unwrap()
        .to_owned();
    mutate(&store, &id, |r| {
        r["execution"] = json!({"attemptId":"ra_a","status":"held"});
        Ok(())
    })
    .unwrap();
    assert!(awaits_review(&list(&store).unwrap()["routines"][0]));
    assert!(remove(&store, &id).unwrap_err().contains("review"));
    assert!(acknowledge(&store, &id, "stale").is_err());
    let kept_at = "2026-10-07T16:00:00.000Z";
    keep(&store, &id, "ra_a", kept_at).unwrap();
    let after_keep = list(&store).unwrap()["routines"][0].clone();
    assert!(after_keep.get("execution").is_none());
    assert_eq!(after_keep["lastRunAt"], kept_at);
    assert_eq!(after_keep["schedule"]["kind"], "every");
    mutate(&store, &id, |r| {
        r["execution"] = json!({"attemptId":"ra_b","status":"held"});
        Ok(())
    })
    .unwrap();
    acknowledge(&store, &id, "ra_b").unwrap();
    assert_eq!(
        list(&store).unwrap()["routines"][0]["execution"]["status"],
        "abandoned"
    );
    assert!(list(&store).unwrap()["routines"][0].get("schedule").is_none());
    remove(&store, &id).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn restart_holds_dispatch_and_paused_schedule_records_no_miss() {
    let (root, store) = fixture();
    save(
        &store,
        json!({"name":"Test","teammateId":"codec","steps":["One"],"schedule":{"kind":"every","hours":1}}),
    )
    .unwrap();
    let id = list(&store).unwrap()["routines"][0]["routineId"]
        .as_str()
        .unwrap()
        .to_owned();
    mutate(&store, &id, |r| {
        r["createdAt"] = json!((chrono::Utc::now() - Duration::days(7)).to_rfc3339());
        r["execution"] = json!({"attemptId":"ra_old","status":"dispatching"});
        Ok(())
    })
    .unwrap();
    record_misses(&store, Local::now()).unwrap();
    let r = list(&store).unwrap()["routines"][0].clone();
    assert_eq!(r["execution"]["status"], "held");
    assert_eq!(r["execution"]["canContinue"], false);
    assert!(r["missedAt"].is_null());
    assert!(awaits_review(&r));
    acknowledge(&store, &id, "ra_old").unwrap();
    mutate(&store, &id, |r| {
        r["schedule"] = json!({"kind":"every","hours":1});
        Ok(())
    })
    .unwrap();
    pause(&store, &id, true, &now()).unwrap();
    assert!(!record_misses(&store, Local::now()).unwrap());
    pause(&store, &id, false, &now()).unwrap();
    assert!(!record_misses(&store, Local::now()).unwrap());
    std::fs::remove_dir_all(root).unwrap();
}

// a-routine-hands-off-to-the-next-teammate.test.ts, "who takes each step, as the store keeps it", and
// a-routine-step-waits-for-your-ok.test.ts, "is kept by the store for one teammate too, from step 2 on".
fn made(store: &Store, extra: Value) -> Result<Value, String> {
    let mut entry = json!({"name": "Chain", "teammateId": "codec", "steps": ["One.", "Two.", "Three."]});
    for (k, v) in extra.as_object().unwrap() {
        entry[k] = v.clone();
    }
    save(store, entry)?;
    Ok(list(store).unwrap()["routines"].as_array().unwrap().last().unwrap().clone())
}
#[test]
fn keeps_hand_offs_that_line_up_with_the_steps_and_refuses_ones_that_do_not() {
    let (root, store) = fixture();
    let h = json!([{}, {"teammateId": "tm_atlas"}, {"teammateId": "tm_sable", "check": true}]);
    let r = made(&store, json!({"handOffs": h})).unwrap();
    assert_eq!(r["handOffs"], h);
    assert!(made(&store, json!({"handOffs": [{}, {"teammateId": "tm_atlas"}]})).unwrap_err().contains("does not line up"));
    assert!(made(&store, json!({"handOffs": [{}, {"teammateId": "../../x"}, {}]})).unwrap_err().contains("does not line up"));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn stores_nothing_when_every_step_is_the_routines_own_teammates() {
    let (root, store) = fixture();
    assert!(made(&store, json!({"handOffs": [{}, {"teammateId": "codec"}, {}]})).unwrap().get("handOffs").is_none());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn keeps_hand_offs_across_an_edit_that_does_not_name_them_only_while_the_steps_still_line_up() {
    let (root, store) = fixture();
    let h = json!([{}, {"teammateId": "tm_atlas"}, {}]);
    let r = made(&store, json!({"handOffs": h})).unwrap();
    let id = r["routineId"].clone();
    save(&store, json!({"routineId": id, "name": "Chain", "steps": ["One.", "Two!", "Three."]})).unwrap();
    assert_eq!(list(&store).unwrap()["routines"][0]["handOffs"], h);
    save(&store, json!({"routineId": id, "name": "Chain", "steps": ["One.", "Two."]})).unwrap();
    assert!(list(&store).unwrap()["routines"][0].get("handOffs").is_none());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn changes_what_a_run_may_do_on_an_edit_and_refuses_a_mode_that_is_not_one() {
    let (root, store) = fixture();
    let r = made(&store, json!({"mode": "accept-edits"})).unwrap();
    assert_eq!(r["route"]["mode"], "accept-edits");
    let id = r["routineId"].clone();
    save(&store, json!({"routineId": id, "name": "Chain", "steps": ["One.", "Two.", "Three."], "mode": "ask"})).unwrap();
    assert_eq!(list(&store).unwrap()["routines"][0]["route"]["mode"], "ask");
    // Not given: kept.
    save(&store, json!({"routineId": id, "name": "Chain", "steps": ["One.", "Two.", "Three."]})).unwrap();
    assert_eq!(list(&store).unwrap()["routines"][0]["route"]["mode"], "ask");
    assert!(save(&store, json!({"routineId": id, "name": "Chain", "steps": ["One."], "mode": "root"})).unwrap_err().contains("route is invalid"));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn a_wait_for_ok_is_kept_from_step_2_on_and_step_1_never_waits() {
    let (root, store) = fixture();
    let r = made(&store, json!({"handOffs": [{"askFirst": true}, {}, {"askFirst": true}]})).unwrap();
    assert_eq!(r["handOffs"], json!([{}, {}, {"askFirst": true}]));
    // Turned off again: nothing left to keep.
    save(&store, json!({"routineId": r["routineId"], "name": "Chain", "steps": ["One.", "Two.", "Three."], "handOffs": [{}, {}, {}]})).unwrap();
    assert!(list(&store).unwrap()["routines"][0].get("handOffs").is_none());
    let odd = save(&store, json!({"name": "Odd", "teammateId": "codec", "steps": ["One.", "Two."], "handOffs": [{}, {"askFirst": "yes"}]}));
    assert_eq!(odd.unwrap_err(), "Who takes each step does not line up with the steps");
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn a_copy_and_a_goal_are_kept_and_a_goal_that_does_not_read_is_refused() {
    let (root, store) = fixture();
    let r = made(&store, json!({"inCopy": true, "untilCheck": {"tries": 3}})).unwrap();
    assert_eq!((&r["inCopy"], &r["untilCheck"]), (&json!(true), &json!({"tries": 3})));
    assert_eq!(made(&store, json!({"untilCheck": {"tries": 99}})).unwrap_err(), "Routine goal is invalid");
    save(&store, json!({"routineId": r["routineId"], "name": "Chain", "steps": ["One."], "inCopy": false, "untilCheck": null})).unwrap();
    let after = list(&store).unwrap()["routines"][0].clone();
    assert!(after.get("inCopy").is_none() && after.get("untilCheck").is_none());
    std::fs::remove_dir_all(root).unwrap();
}
