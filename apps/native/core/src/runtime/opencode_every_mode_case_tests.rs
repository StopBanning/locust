//! Electron's `packages/runtime-adapters/test/opencode-every-mode-through-its-server.test.ts` (0.677), case by case,
//! on the same stand-in server and caller as opencode-serve-run (the parent module): a reply streams from the
//! server's deltas, and a mode means on the server what it meant on `run`, the same config, and what OpenCode asks
//! answered as `run` answered it (Auto approves, the others refuse).
//!
//! Electron's sandboxes are Locust's modes: read-only is Ask (and Plan), workspace-write is Edit files, full-access
//! is Auto. The config the native app writes also names Locust's command guard (`plugin`), which Electron's
//! `createOpenCodeServeCommand` adds only when it is given one; the cases compare everything else.
use super::*;

const SESSION: &str = "ses_1";

fn message(id: &str, role: &str) -> String {
    json!({"type": "message.updated", "properties": {"sessionID": SESSION, "info": {"id": id, "role": role, "sessionID": SESSION}}}).to_string()
}

fn part(mut fields: Value) -> String {
    fields["sessionID"] = json!(SESSION);
    json!({"type": "message.part.updated", "properties": {"sessionID": SESSION, "part": fields}}).to_string()
}

fn delta(part_id: &str, text: &str) -> String {
    json!({"type": "message.part.delta", "properties": {"sessionID": SESSION, "messageID": "msg_a", "partID": part_id, "field": "text", "delta": text}}).to_string()
}

fn idle() -> String {
    json!({"type": "session.idle", "properties": {"sessionID": SESSION}}).to_string()
}

/// A server whose `/session` answers `ses_1`, as Electron's does here.
fn server(events: &[String]) -> Server {
    let server = Server::new(events);
    let exe = server.t.0.join("bin/opencode");
    let script = std::fs::read_to_string(&exe).unwrap().replace("ses_f27b5f958ffea6iVnmfRjD2imJ", SESSION);
    std::fs::write(&exe, script).unwrap();
    server
}

/// What the reply said, as Electron's test writes it: `operation`, `!` when final, `:`, the text.
fn said_as(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e.kind == "message.delta")
        .map(|e| {
            let p = &e.payload;
            format!(
                "{}{}:{}",
                p["operation"].as_str().unwrap_or(""),
                if p["final"] == true { "!" } else { "" },
                p["text"].as_str().unwrap_or("")
            )
        })
        .collect()
}

#[test]
fn streams_from_the_servers_deltas_and_ends_as_exactly_the_finished_text() {
    // case: streams from the server's deltas, and ends as exactly the finished text
    let ran = run_on(
        server(&[
            message("msg_a", "assistant"),
            part(json!({"id": "s1", "type": "step-start", "messageID": "msg_a"})),
            // The model's reasoning streams the same way; it is not the reply.
            part(json!({"id": "r1", "type": "reasoning", "messageID": "msg_a", "text": ""})),
            delta("r1", "The user wants a story."),
            part(json!({"id": "p1", "type": "text", "messageID": "msg_a", "text": ""})),
            delta("p1", "The keeper"),
            delta("p1", " found a bottle"),
            part(json!({"id": "p1", "type": "text", "messageID": "msg_a", "text": "The keeper found a bottle.", "time": {"start": 1, "end": 2}})),
            part(json!({"id": "s2", "type": "step-finish", "messageID": "msg_a", "reason": "stop", "tokens": {"input": 1, "output": 1}})),
            idle(),
        ]),
        Mode::Ask,
        None,
        |t, rt| {
            t.prompt = "a story".into();
            rt
        },
    );
    // At least one partial (the server's deltas come faster than the pace here); never the reasoning.
    assert!(ran.records.iter().filter(|(kind, _)| kind == "text_partial").count() >= 1, "{:?}", ran.records);
    let said = said_as(&ran.events);
    assert!(!said.iter().any(|one| one.contains("The user wants")), "{said:?}");
    assert_eq!(said.last().unwrap(), "replace!:The keeper found a bottle.");
    assert!(said[..said.len() - 1].iter().all(|one| one.starts_with("append:")), "{said:?}");
}

#[test]
fn a_step_that_ends_before_its_texts_finished_form_closes_it_with_every_word() {
    // case: a step that ends before its text's finished form closes it with every word, none held back by the pace
    let ran = run_on(
        server(&[
            message("msg_a", "assistant"),
            part(json!({"id": "p1", "type": "text", "messageID": "msg_a", "text": ""})),
            delta("p1", "The keeper"),
            delta("p1", " found a bottle."),
            // The step's end before the part's own (time.end): OpenCode sends them in either order.
            part(json!({"id": "s2", "type": "step-finish", "messageID": "msg_a", "reason": "stop", "tokens": {"input": 1, "output": 1}})),
            part(json!({"id": "p1", "type": "text", "messageID": "msg_a", "text": "The keeper found a bottle.", "time": {"start": 1, "end": 2}})),
            idle(),
        ]),
        Mode::Ask,
        None,
        |t, rt| {
            t.prompt = "a story".into();
            rt
        },
    );
    let closed: Vec<&str> = ran
        .events
        .iter()
        .filter(|e| e.kind == "message.delta" && e.payload["final"] == true)
        .filter_map(|e| e.payload["text"].as_str())
        .collect();
    // The first close is already the whole of it.
    assert_eq!(closed.first(), Some(&"The keeper found a bottle."), "{:?}", said_as(&ran.events));
}

#[test]
fn names_its_session_so_opencode_does_not_spend_a_model_call_naming_it() {
    // case: names its session, so OpenCode does not spend a model call naming it
    let ran = run_on(server(&[idle()]), Mode::AcceptEdits, None, |t, rt| {
        t.prompt = "hi".into();
        rt
    });
    let made = ran.server.calls().into_iter().find(|c| c["path"] == "/session").unwrap();
    assert_eq!(made["body"], json!({"title": "Locust"}));
}

/// The `OPENCODE_*` environment the app gives OpenCode for `mode`, its config parsed and without the guard plugin.
fn told(mode: Mode) -> serde_json::Map<String, Value> {
    let server = Server::new(&[]);
    let command = server.command(mode);
    command
        .get_envs()
        .filter_map(|(k, v)| {
            let (k, v) = (k.to_str()?.to_owned(), v?.to_str()?.to_owned());
            if !k.starts_with("OPENCODE_") {
                return None;
            }
            let value = if k == "OPENCODE_CONFIG_CONTENT" {
                let mut config: Value = serde_json::from_str(&v).unwrap();
                config.as_object_mut().unwrap().remove("plugin");
                config
            } else {
                json!(v)
            };
            Some((k, value))
        })
        .collect()
}

#[test]
fn read_only_is_read_only_loads_no_plugins_workspace_write_stays_in_the_folder_auto_may_leave_it() {
    // case: read-only is read-only, loads no plugins; workspace-write stays in the folder; Auto may leave it
    // commands.ts: OPENCODE_READ_ONLY_CONFIG (OPENCODE_READ_ONLY_PERMISSIONS, external_directory deny, keep going),
    // OPENCODE_CONFINED_CONFIG and OPENCODE_AUTO_CONFIG, each through withoutOpenCodeQuestionTool.
    let read_only = json!({
        "permission": {
            "*": "ask",
            "read": {"*": "allow", "*.env": "deny", "*.env.*": "deny", "*.env.example": "allow"},
            "glob": "allow", "grep": "allow", "list": "allow", "lsp": "allow", "task": "allow",
            "todowrite": "allow", "todoread": "allow", "skill": "allow",
            "webfetch": "allow", "websearch": "allow", "codesearch": "allow",
            "edit": "deny", "write": "deny", "bash": "ask", "patch": "deny",
            "external_directory": "deny",
        },
        "experimental": {"continue_loop_on_deny": true},
        "tools": {"question": false},
    });
    for mode in [Mode::Ask, Mode::Plan] {
        assert_eq!(
            Value::Object(told(mode)),
            json!({"OPENCODE_CONFIG_CONTENT": read_only, "OPENCODE_PURE": "1"}),
            "{mode:?}"
        );
    }
    assert_eq!(
        Value::Object(told(Mode::AcceptEdits)),
        json!({"OPENCODE_CONFIG_CONTENT": {"permission": {"external_directory": "deny"}, "tools": {"question": false}}})
    );
    assert_eq!(
        Value::Object(told(Mode::Auto)),
        json!({"OPENCODE_CONFIG_CONTENT": {"permission": {"external_directory": "allow"}, "tools": {"question": false}}})
    );
}

/// What the run answered when OpenCode asked to run `ls`.
fn asks(mode: Mode) -> Option<Value> {
    let ran = run_on(
        server(&[
            json!({"type": "permission.asked", "properties": {"sessionID": SESSION, "id": "per_1", "permission": "bash", "patterns": ["ls"], "always": [], "metadata": {}}}).to_string(),
            idle(),
        ]),
        mode,
        None,
        |t, rt| {
            t.prompt = "hi".into();
            rt
        },
    );
    ran.server.sent("/permission/").map(|body| body["reply"].clone())
}

#[test]
fn what_opencode_asks_auto_approves_any_other_mode_refuses() {
    // case: what OpenCode asks: Auto approves it, as run --auto did; any other mode refuses it, as run did
    assert_eq!(asks(Mode::Auto), Some(json!("once")));
    for mode in [Mode::Ask, Mode::Plan, Mode::AcceptEdits] {
        assert_eq!(asks(mode), Some(json!("reject")), "{mode:?}");
    }
}
