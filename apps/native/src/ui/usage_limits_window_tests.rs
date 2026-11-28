use super::*;
use rune::{Env, Runtime};
use serde_json::{Value, json};
use std::{fs, path::PathBuf, time::{Duration, Instant}};

const WORK: &str = "account_11111111111111111111111111111111";

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

fn has_text(rt: &Runtime<State>, words: &str) -> bool {
    rt.elements().iter().any(|e| !e.exiting && e.text.as_deref() == Some(words))
}

fn a11y_meter<'a>(tree: &'a Value, label: &str) -> Vec<&'a Value> {
    if tree["name"] == label && tree["role"] == "meter" {
        return vec![tree];
    }
    tree["children"].as_array().into_iter().flatten().flat_map(|n| a11y_meter(n, label)).collect()
}

// Window integration for UsageDialog.tsx's Limits section, additional to its existing parser and mark cases.
#[test]
fn usage_details_shows_the_latest_short_window_for_the_right_account() {
    let _accounts = crate::agent_accounts_card::test_guard();
    let _dir = Temp(std::env::temp_dir().join(format!("locust-usage-details-{}", locust_core::invariants::unique_nanos())));
    let store = locust_core::store::Store::new(_dir.0.join("native"), _dir.0.join("electron")).unwrap();
    let native = store.ledger_dir().unwrap();
    fs::create_dir_all(&native).unwrap();
    fs::create_dir_all(store.electron()).unwrap();
    let account_dir = store.root().join(locust_core::agent_accounts::FOLDER);
    fs::create_dir_all(&account_dir).unwrap();
    let accounts_file = account_dir.join("accounts.json");
    let accounts = json!([{"id":WORK,"runtime":"claude","name":"Work","state":"ready"}]).to_string();
    fs::write(&accounts_file, &accounts).unwrap();
    let at = chrono::Utc::now() - chrono::Duration::minutes(5);
    let occurred = at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let before = (at - chrono::Duration::hours(1)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let held = json!({
        format!("claude:{WORK}"): format!("5-hour window 90% used · from a run at {before}"),
        "claude": "5-hour window 12% used",
    }).to_string();
    let electron_file = store.electron().join("usage-readings.json");
    fs::write(&electron_file, &held).unwrap();

    let rows = [
        json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"usage_fixture","runtime":"claude",
            "model":"sonnet","createdAt":occurred,"resolvedRouteId": format!("claude:sonnet-account:{WORK}")}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"type":"adapter.diagnostic","sourceAdapter":"claude","occurredAt":occurred,
            "payload":{"code":"claude.usage_window","message":"5-hour window 35% used · resets 2099-09-27T22:10:00.000Z · 7-day window 20% used · resets 2000-09-27T05:30:00.000Z"}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"type":"run.completed","occurredAt":occurred,
            "payload":{"usage":{"billing":"subscription","inputTokens":5000,"outputTokens":500}}}}),
    ];
    fs::write(native.join("usage_fixture.jsonl"), rows.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();

    let mut s = State::default();
    home_panel::home_scene(&mut s);
    s.demo = true;
    s.home.splash_open = false;
    s.data = Some(store.clone());
    crate::usage_line::request(&mut s);
    let mut rt = Runtime::new(s, crate::view, (1400., 1200.), Env::new(true, 1.));
    let stop = Instant::now() + Duration::from_secs(4);
    while rt.find("usage-line-open").is_none() {
        assert!(Instant::now() < stop, "Home did not show Details from its ledger summary");
        rt.update(|s, _| { crate::usage_line::poll(s); });
        rt.frame(0.05);
        std::thread::sleep(Duration::from_millis(5));
    }
    for _ in 0..8 { rt.frame(0.05); }
    assert!(has_text(&rt, "1 turn · 5.5k tokens"));
    let (x, y) = rt.find("usage-line-open").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..8 { rt.frame(0.05); }
    assert!(rt.state.usage_panel.open && rt.find("usage-dialog").is_some(), "Details opens Usage");

    assert!(has_text(&rt, "Claude Code · Work"), "the named account owns the newer reading");
    assert!(has_text(&rt, "Claude Code"));
    assert!(has_text(&rt, "65% left") && has_text(&rt, "88% left"), "shown figures are quota left");
    assert!(!has_text(&rt, "10% left"), "the older remembered reading must not win");
    assert!(has_text(&rt, "reset since") && !has_text(&rt, "80% left"), "expired readings do not show a live quota");

    let tree = rt.accessibility_json();
    let mut used: Vec<_> = a11y_meter(&tree, "5-hour window used").into_iter()
        .map(|meter| meter["range"][0].as_f64().unwrap() as u32).collect();
    used.sort_unstable();
    assert_eq!(used, [12, 35], "meter values are usage, not quota left");
    assert!(a11y_meter(&tree, "7-day window used").is_empty(), "a reset window has no meter");
    let rows = rt.elements();
    for (key, fraction) in [("usage-limit-claude-5-hour window".to_owned(), 0.12),
        (format!("usage-limit-claude:{WORK}-5-hour window"), 0.35)] {
        let track = rows.iter().find(|e| e.key.as_deref() == Some(key.as_str())).expect("that account has its own track");
        let fill = rows.iter().find(|e| e.key.as_deref() == Some(format!("{key}-fill").as_str())).expect("that account has its own fill");
        assert!(track.rect[2] > 0. && (fill.rect[2] / track.rect[2] - fraction).abs() < 0.01,
            "the visible fill is the reported fraction used by this account");
    }
    assert!(rows.iter().filter_map(|e| e.text.as_deref()).any(|v| v.starts_with("Resets ") && !v.contains("2099")));
    assert!(rows.iter().filter_map(|e| e.text.as_deref()).any(|v| v.starts_with("From Locust's last run on it,") && v.ends_with("Use outside Locust since then isn't counted.")));
    assert_eq!(fs::read_to_string(electron_file).unwrap(), held, "Electron's reading was read-only");
    assert_eq!(fs::read_to_string(accounts_file).unwrap(), accounts, "Usage does not change account settings");
    assert!(fs::read_to_string(store.root().join("usage-readings.json")).unwrap().contains("35% used"));
}
