//! Every answer Electron's own AI agents screen gives (parity/agents.json, from scripts/agents-goldens.mjs).
use super::*;
use serde_json::Value;

fn goldens() -> Value {
    serde_json::from_str(include_str!("../../parity/agents.json")).expect("parity/agents.json")
}

fn found(id: &str, display_name: &str, state: &Value) -> Found {
    let text = |k: &str| state[k].as_str().map(str::to_owned);
    Found {
        id: id.into(),
        display_name: display_name.into(),
        installed: state["installed"] == true,
        version: text("version"),
        auth: match state["auth"].as_str().unwrap() {
            "authenticated" => Auth::Authenticated,
            "unauthenticated" => Auth::Unauthenticated,
            "not-applicable" => Auth::NotApplicable,
            _ => Auth::Unknown,
        },
        ready: state["ready"] == true,
        status: match state["status"].as_str().unwrap() {
            "ready" => Probe::Ready,
            "auth-required" => Probe::AuthRequired,
            "unsupported" => Probe::Unsupported,
            "offline" => Probe::Offline,
            "probe-failed" => Probe::ProbeFailed,
            "missing" => Probe::Missing,
            _ => Probe::Unknown,
        },
        unsupported_note: text("unsupportedNote"),
        through_cli: state["throughCli"] == true,
        checking: state["checking"] == true,
    }
}

#[test]
fn every_row_says_what_electrons_agents_screen_says() {
    let g = goldens();
    let rows = g["rows"].as_array().unwrap();
    assert!(rows.len() > 300, "the grid is whole");
    let mut differ = Vec::new();
    for r in rows {
        let (id, name) = (r["id"].as_str().unwrap(), r["displayName"].as_str().unwrap());
        let state = &g["states"][r["state"].as_str().unwrap()];
        let integration = integration_of(id);
        assert_eq!(integration.as_str(), r["integration"], "{id}'s integration");
        let got = row_status(&found(id, name, state), integration, r["active"] == true, r["limited"].as_str());
        let want = &r["status"];
        if got.tag != want["tag"] || got.selectable != want["selectable"].as_bool().unwrap() || got.detail != want["detail"] {
            differ.push(format!("{id}/{}/active={}/limited={}: got {got:?}, Electron {want}", r["state"], r["active"], !r["limited"].is_null()));
        }
    }
    assert!(differ.is_empty(), "{} of {} rows differ:\n{}", differ.len(), rows.len(), differ.join("\n"));
}

#[test]
fn install_and_sign_in_lines_are_electrons() {
    let g = goldens();
    for (id, want) in g["install"].as_object().unwrap() {
        let name = g["rows"].as_array().unwrap().iter().find(|r| r["id"] == *id).unwrap()["displayName"].as_str().unwrap();
        assert_eq!(install_command(id).as_deref(), want["command"].as_str(), "{id} install command");
        assert_eq!(install_sentence(id, name), want["sentence"].as_str().unwrap(), "{id} install sentence");
        assert_eq!(sign_in_command(id, false).as_deref(), want["signIn"].as_str(), "{id} sign in");
        assert_eq!(sign_in_command(id, true).as_deref(), want["signInAgain"].as_str(), "{id} sign in again");
    }
}
