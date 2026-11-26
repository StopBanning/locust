//! start_conversation with cloud true, and list_conversations with cloud chats in it, over a made-up Locust whose
//! Claude's cloud is the test's own: what the tools ask of it, and what they refuse in words before asking.
use super::*;
use crate::claude_cloud::{ClaudeCloudSession, CloudSendResult};
use std::sync::Mutex;

#[derive(Default)]
struct Cloudy {
    own: bool,
    see_all: bool,
    /// Where the folder stands with GitHub; Some(None) is no GitHub remote.
    github: Option<FolderOnGitHub>,
    started: Mutex<Vec<CloudStart>>,
    /// Claude Code's own refusal, when it refuses.
    refuses: Option<&'static str>,
    chats: Mutex<Vec<CloudChat>>,
    asked_where: Mutex<Vec<String>>,
    /// What each follow-up said, to which chat; and Claude Code's answer to it.
    sent: Mutex<Vec<(String, String)>>,
    send_answer: Option<CloudSendResult>,
    /// What a read finds, and how many reads were asked for.
    found: Mutex<Option<CloudRead>>,
    reads: Mutex<u32>,
}

fn teammate(id: &str, name: &str, runtime: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Custom".into(),
        runtime: Some(runtime.into()),
        model: Some(if runtime == "claude" { "claude-opus-5-5" } else { "gpt-5.5" }.into()),
        account: None,
        busy: false,
        runs_in: "auto".into(),
        folder: (runtime == "claude").then(|| "/known/cloud".to_owned()),
    }
}

fn on_github(unpushed: Option<u64>, dirty: bool) -> FolderOnGitHub {
    FolderOnGitHub { repo: Some("locust/app".into()), branch: Some("main".into()), unpushed, dirty }
}

fn session(id: &str, folder: &str) -> ClaudeCloudSession {
    ClaudeCloudSession {
        id: id.into(),
        started_at: "2026-11-25T10:00:00.000Z".into(),
        prompt: "Fix the cart".into(),
        folder: folder.into(),
        teammate_id: Some("tm_ivy".into()),
        session_id: Some("session_0123456789abcdef".into()),
        url: Some("https://claude.ai/code/session_0123456789abcdef".into()),
        title: Some("Fix the cart".into()),
        note: None,
        ..Default::default()
    }
}

impl Locust for Cloudy {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        Ok(vec![(teammate("tm_ivy", "Ivy", "claude"), Some("high".into())), (teammate("tm_pip", "Pip", "codex"), None)])
    }
    fn busy(&self, _: &str) -> bool {
        false
    }
    fn start(&self, _: StartRequest) -> Result<Begun, String> {
        Err("a cloud chat is not a local turn".into())
    }
    fn read(&self, _: &str) -> Option<Turn> {
        None
    }
    fn newest(&self, _: &str) -> Option<String> {
        None
    }
    fn owner(&self, _: &str) -> Option<String> {
        None
    }
    fn live(&self, _: &str) -> bool {
        false
    }
    fn waiting(&self, _: &str) -> bool {
        false
    }
    fn cancel(&self, _: &str) -> bool {
        false
    }
    fn conversations(&self) -> Vec<Turn> {
        Vec::new()
    }
    fn check_folder(&self, path: &str) -> Result<PathBuf, String> {
        if path.starts_with("/known") { Ok(PathBuf::from(path)) } else { Err(format!("{path} is not a folder Locust knows.")) }
    }
    fn folder(&self) -> Option<String> {
        Some("/known/open".into())
    }
    fn own_mode(&self) -> bool {
        self.own
    }
    fn see_all(&self) -> bool {
        self.see_all
    }
    fn models(&self) -> Vec<Value> {
        vec![
            json!({"id": "claude-opus-5-5", "runtime": "claude", "displayName": "Opus 5.5", "description": "", "supportedEfforts": ["low", "high"]}),
            json!({"id": "gpt-5.5", "runtime": "codex", "displayName": "GPT-5.5", "description": "", "supportedEfforts": ["low", "high"]}),
        ]
    }
    fn carry_on(&self, _: &str, _: &str, _: Option<&str>, _: &crate::carry_on::CarryOnAlso) -> Result<(String, Option<String>), String> {
        Err("no".into())
    }
    fn cloud_where(&self, folder: &str) -> FolderOnGitHub {
        self.asked_where.lock().unwrap().push(folder.to_owned());
        self.github.clone().unwrap_or(FolderOnGitHub { repo: None, branch: None, unpushed: None, dirty: false })
    }
    fn cloud_start(&self, start: &CloudStart) -> ClaudeCloudAnswer {
        if let Some(why) = self.refuses {
            return ClaudeCloudAnswer::Err { message: why.into() };
        }
        self.started.lock().unwrap().push(start.clone());
        ClaudeCloudAnswer::Ok { session: session("cc_00000000000000000001", &start.folder) }
    }
    fn cloud_chats(&self) -> Vec<CloudChat> {
        self.chats.lock().unwrap().clone()
    }
    fn cloud_send(&self, id: &str, message: &str) -> CloudSendResult {
        self.sent.lock().unwrap().push((id.to_owned(), message.to_owned()));
        self.send_answer.clone().unwrap_or(CloudSendResult::Ok)
    }
    fn cloud_read(&self, _: &str) -> CloudRead {
        *self.reads.lock().unwrap() += 1;
        self.found.lock().unwrap().clone().unwrap_or(CloudRead::Working { note: None })
    }
}

fn cloudy() -> Cloudy {
    Cloudy { github: Some(on_github(Some(0), false)), ..Default::default() }
}

fn say(r: &ToolResult) -> Value {
    serde_json::from_str(&r.text).unwrap_or(Value::Null)
}

fn start(tools: &Tools<Cloudy>, args: Value) -> ToolResult {
    let mut all = json!({"teammate": "Ivy", "message": "Fix the cart", "cloud": true});
    for (k, v) in args.as_object().into_iter().flatten() {
        all[k] = v.clone();
    }
    tools.call("start_conversation", &all)
}

#[test]
fn cloud_true_with_a_claude_teammate_starts_the_task_in_claudes_cloud_in_the_teammates_folder() {
    let tools = Tools::new(cloudy());
    let r = start(&tools, json!({}));
    assert!(!r.is_error, "{}", r.text);
    let said = say(&r);
    assert_eq!(said["conversation_id"], "cc_00000000000000000001");
    assert_eq!((said["runtime"].clone(), said["cloud"].clone()), (json!("claude"), json!(true)));
    assert_eq!((said["folder"].clone(), said["repo"].clone(), said["branch"].clone()), (json!("/known/cloud"), json!("locust/app"), json!("main")));
    assert_eq!(said["url"], "https://claude.ai/code/session_0123456789abcdef");
    assert_eq!(said["session_known"], true);
    assert!(said.get("mode").is_none(), "a cloud chat has no Locust mode to report");
    assert!(said.get("warnings").is_none());
    let asked = tools.locust.started.lock().unwrap();
    assert_eq!(
        *asked,
        vec![CloudStart {
            folder: "/known/cloud".into(),
            prompt: "Fix the cart".into(),
            teammate_id: "tm_ivy".into(),
            model: Some("claude-opus-5-5".into()),
            effort: None,
        }]
    );
    assert_eq!(*tools.locust.asked_where.lock().unwrap(), vec!["/known/cloud".to_owned()]);
}

#[test]
fn a_model_and_effort_the_teammates_agent_offers_go_with_the_task_and_others_are_refused() {
    let tools = Tools::new(cloudy());
    let r = start(&tools, json!({"model": "Opus 5.5", "effort": "high"}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!((say(&r)["model"].clone(), say(&r)["effort"].clone()), (json!("claude-opus-5-5"), json!("high")));
    let asked = tools.locust.started.lock().unwrap();
    assert_eq!((asked[0].model.clone(), asked[0].effort.clone()), (Some("claude-opus-5-5".into()), Some("high".into())));
    drop(asked);
    let bad = start(&tools, json!({"effort": "extreme"}));
    assert!(bad.is_error && bad.text.ends_with("Nothing was started."), "{}", bad.text);
    assert_eq!(tools.locust.started.lock().unwrap().len(), 1);
}

#[test]
fn only_a_claude_teammate_can_start_a_cloud_chat_and_the_refusal_says_why() {
    let tools = Tools::new(cloudy());
    let r = start(&tools, json!({"teammate": "Pip"}));
    assert!(r.is_error);
    assert!(r.text.contains("Claude's cloud is Claude Code's: Pip runs on codex"), "{}", r.text);
    assert!(r.text.ends_with("Nothing was started."));
    let none = start(&tools, json!({"teammate": null, "runtime": "claude"}));
    assert!(none.is_error && none.text.contains("belongs to a Claude teammate"), "{}", none.text);
    let unknown = start(&tools, json!({"teammate": "Nobody Here"}));
    assert!(unknown.is_error && unknown.text.contains("Nothing was started"), "{}", unknown.text);
    assert!(tools.locust.started.lock().unwrap().is_empty());
    assert!(tools.locust.asked_where.lock().unwrap().is_empty(), "refused before anything was asked of GitHub or Claude");
}

#[test]
fn a_cloud_chat_takes_no_mode_and_no_read_only_and_a_task_that_fits_what_claude_code_takes() {
    let tools = Tools::new(Cloudy { own: true, ..cloudy() });
    for extra in [json!({"mode": "auto"}), json!({"read_only": true}), json!({"read_only": false})] {
        let r = start(&tools, extra);
        assert!(r.is_error && r.text.contains("no Locust mode") && r.text.ends_with("Nothing was started."), "{}", r.text);
    }
    let long = start(&tools, json!({"message": "x".repeat(4001)}));
    assert!(long.is_error && long.text.contains("up to 4000 characters") && long.text.contains("will not send it cut short"), "{}", long.text);
    assert!(!start(&tools, json!({"message": "x".repeat(4000)})).is_error);
    let odd = start(&tools, json!({"cloud": "yes"}));
    assert!(odd.is_error && odd.text == "cloud is true or false. Nothing was started.", "{}", odd.text);
    assert_eq!(tools.locust.started.lock().unwrap().len(), 1);
}

#[test]
fn cloud_false_or_left_out_is_an_ordinary_conversation() {
    let tools = Tools::new(cloudy());
    for args in [json!({"cloud": false}), json!({})] {
        let mut all = json!({"teammate": "Ivy", "message": "hi"});
        for (k, v) in args.as_object().unwrap() {
            all[k] = v.clone();
        }
        let r = tools.call("start_conversation", &all);
        assert!(r.is_error && r.text == "a cloud chat is not a local turn", "{}", r.text);
    }
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn a_named_folder_needs_the_switch_and_a_folder_locust_knows_and_the_open_folder_stands_in_for_none() {
    let off = Tools::new(cloudy());
    let r = start(&off, json!({"folder": "/known/other"}));
    assert!(r.is_error && r.text.contains("Choosing the mode or folder is off"), "{}", r.text);
    let on = Tools::new(Cloudy { own: true, ..cloudy() });
    let r = start(&on, json!({"folder": "/known/other"}));
    assert_eq!(say(&r)["folder"], "/known/other");
    assert_eq!(on.locust.started.lock().unwrap()[0].folder, "/known/other");
    let stranger = start(&on, json!({"folder": "/etc"}));
    assert!(stranger.is_error && stranger.text == "/etc is not a folder Locust knows. Nothing was started.", "{}", stranger.text);
    assert_eq!(on.locust.started.lock().unwrap().len(), 1);
}

#[test]
fn a_folder_with_no_github_remote_is_refused_because_the_task_would_start_with_no_project() {
    let tools = Tools::new(Cloudy { github: None, ..Default::default() });
    let r = start(&tools, json!({}));
    assert!(r.is_error);
    assert!(r.text.contains("/known/cloud is not a git checkout with a GitHub remote"), "{}", r.text);
    assert!(r.text.ends_with("Nothing was started."));
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn what_claudes_cloud_will_not_see_is_said_with_the_start() {
    let tools = Tools::new(Cloudy { github: Some(on_github(Some(2), true)), ..Default::default() });
    let said = say(&start(&tools, json!({})));
    let warnings: Vec<String> = said["warnings"].as_array().unwrap().iter().map(|w| w.as_str().unwrap().to_owned()).collect();
    assert_eq!(
        warnings,
        vec![
            "2 commits on this branch are not pushed, and Claude's cloud starts from the newest pushed commit, so it will not see them.".to_owned(),
            "The folder has uncommitted changes; Claude's cloud will not see them.".to_owned(),
        ]
    );
    let unknown = Tools::new(Cloudy { github: Some(on_github(None, false)), ..Default::default() });
    assert!(say(&start(&unknown, json!({})))["warnings"][0].as_str().unwrap().contains("could not tell whether this branch is pushed"));
}

#[test]
fn one_unpushed_commit_is_said_in_the_singular_and_a_pushed_clean_folder_says_nothing() {
    let one = Tools::new(Cloudy { github: Some(on_github(Some(1), false)), ..Default::default() });
    let said = say(&start(&one, json!({})));
    assert_eq!(
        said["warnings"][0],
        "1 commit on this branch is not pushed, and Claude's cloud starts from the newest pushed commit, so it will not see it."
    );
    assert_eq!(said["warnings"].as_array().unwrap().len(), 1);
    let pushed = Tools::new(Cloudy { github: Some(on_github(Some(0), false)), ..Default::default() });
    assert!(say(&start(&pushed, json!({}))).get("warnings").is_none(), "nothing for Claude's cloud to miss, nothing said");
}

#[test]
fn claude_codes_own_refusal_is_passed_on_word_for_word_with_nothing_started_said_once() {
    let said = |why| {
        let tools = Tools::new(Cloudy { refuses: Some(why), ..cloudy() });
        start(&tools, json!({}))
    };
    let r = said("Claude Code is not installed here. Settings > AI agents shows how to add it.");
    assert!(r.is_error);
    assert_eq!(r.text, "Claude Code is not installed here. Settings > AI agents shows how to add it. Nothing was started.");
    let r = said("Describe the task first. Nothing was sent.");
    assert_eq!(r.text, "Describe the task first. Nothing was sent.");
}

#[test]
fn list_conversations_lists_cloud_chats_with_their_claude_ai_link_and_who_started_them() {
    let tools = Tools::new(Cloudy { see_all: true, ..cloudy() });
    let mut theirs = session("cc_00000000000000000002", "/known/other");
    theirs.started_at = "2026-11-25T11:00:00.000Z".into();
    theirs.session_id = None;
    theirs.url = None;
    theirs.title = None;
    theirs.teammate_id = None;
    tools.locust.chats.lock().unwrap().extend([
        CloudChat { session: session("cc_00000000000000000001", "/known/cloud"), from_app: true },
        CloudChat { session: theirs, from_app: false },
    ]);
    let r = tools.call("list_conversations", &json!({}));
    assert!(!r.is_error, "{}", r.text);
    let rows = say(&r);
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["conversation_id"], "cc_00000000000000000002", "newest first");
    assert_eq!((rows[0]["started_by"].clone(), rows[0]["teammate"].clone(), rows[0]["cloud"].clone()), (json!("the person"), Value::Null, json!(true)));
    assert_eq!(rows[0]["url"], "https://claude.ai/code");
    assert_eq!(rows[0]["title"], "Fix the cart");
    assert!(rows[0]["state"].as_str().unwrap().contains("follow it on claude.ai"));
    assert_eq!(rows[1]["conversation_id"], "cc_00000000000000000001");
    assert_eq!((rows[1]["started_by"].clone(), rows[1]["teammate"].clone(), rows[1]["runtime"].clone()), (json!("an app"), json!("Ivy"), json!("claude")));
    assert_eq!((rows[1]["url"].clone(), rows[1]["folder"].clone(), rows[1]["state"].clone()), (json!("https://claude.ai/code/session_0123456789abcdef"), json!("/known/cloud"), json!("in Claude's cloud")));
    // Off, the list says so and shows nothing, cloud chats included.
    let off = Tools::new(cloudy());
    assert!(off.call("list_conversations", &json!({})).text.starts_with("Off:"));
}

#[test]
fn a_voice_call_cannot_start_a_cloud_chat() {
    let scope = std::sync::Arc::new(VoiceScope::new(PathBuf::from("/known/cloud"), None));
    let tools = Tools::new(cloudy()).with_voice(scope);
    let r = start(&tools, json!({}));
    assert!(r.is_error && r.text.contains("Nothing was started"), "{}", r.text);
    assert!(tools.locust.started.lock().unwrap().is_empty());
}

#[test]
fn the_tools_schema_and_description_say_what_cloud_does() {
    let list = crate::door::tool_list();
    let tool = list.as_array().unwrap().iter().find(|t| t["name"] == "start_conversation").unwrap();
    assert_eq!(tool["inputSchema"]["properties"]["cloud"], json!({"type": "boolean"}));
    let said = tool["description"].as_str().unwrap();
    for words in ["cloud true", "newest pushed commit", "pushed to GitHub", "Claude teammate", "no mode or read_only", "4,000 characters"] {
        assert!(said.contains(words), "the description lacks \"{words}\"");
    }
}

// ---- follow up, read, stop -------------------------------------------------------------------------------------------

const ID: &str = "cc_00000000000000000001";

/// A Locust that knows one cloud chat, started by an app unless said otherwise.
fn with_chat(from_app: bool, see_all: bool) -> Tools<Cloudy> {
    let cloudy = Cloudy { see_all, ..cloudy() };
    cloudy.chats.lock().unwrap().push(CloudChat { session: session(ID, "/known/cloud"), from_app });
    Tools::new(cloudy)
}

fn send(tools: &Tools<Cloudy>, extra: Value) -> ToolResult {
    let mut all = json!({"conversation_id": ID, "message": "Also add a test"});
    for (k, v) in extra.as_object().into_iter().flatten() {
        all[k] = v.clone();
    }
    tools.call("send_message", &all)
}

#[test]
fn send_message_on_a_cloud_chat_sends_the_follow_up_to_the_session_through_the_engine() {
    let tools = with_chat(true, false);
    let r = send(&tools, json!({}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(*tools.locust.sent.lock().unwrap(), vec![(ID.to_owned(), "Also add a test".to_owned())]);
    let said = say(&r);
    assert_eq!((said["sent"].clone(), said["cloud"].clone(), said["conversation_id"].clone()), (json!(true), json!(true), json!(ID)));
    let structured = r.structured.as_ref().unwrap();
    assert_eq!((structured["status"].clone(), structured["runtime"].clone(), structured["can_carry_on"].clone()), (json!("working"), json!("claude"), json!(false)));
    assert_eq!(structured["url"], "https://claude.ai/code/session_0123456789abcdef");
}

#[test]
fn a_follow_up_claude_code_could_not_confirm_is_passed_on_so_it_is_not_sent_twice() {
    let unconfirmed = Tools::new(Cloudy {
        send_answer: Some(CloudSendResult::Err { message: crate::claude_cloud::UNCONFIRMED_SEND_MESSAGE.into() }),
        ..cloudy()
    });
    unconfirmed.locust.chats.lock().unwrap().push(CloudChat { session: session(ID, "/known/cloud"), from_app: true });
    let r = send(&unconfirmed, json!({}));
    assert!(r.is_error);
    assert!(r.text.starts_with(crate::claude_cloud::UNCONFIRMED_SEND_MESSAGE), "{}", r.text);
    assert!(r.text.contains("before sending it again"));
    assert!(r.text.ends_with("Its page: https://claude.ai/code/session_0123456789abcdef"), "{}", r.text);
    assert_eq!(unconfirmed.locust.sent.lock().unwrap().len(), 1, "asked once, never retried");
}

#[test]
fn a_follow_up_to_a_cloud_chat_that_does_not_fit_is_refused_before_anything_is_sent() {
    let tools = with_chat(true, false);
    let long = send(&tools, json!({"message": "x".repeat(4001)}));
    assert!(long.is_error && long.text.contains("will not send it cut short") && long.text.ends_with("Nothing was sent."), "{}", long.text);
    for extra in [json!({"mode": "auto"}), json!({"folder": "/known/x"}), json!({"model": "claude-opus-5-5"}), json!({"effort": "high"}), json!({"read_only": true})] {
        let r = send(&tools, extra);
        assert!(r.is_error && r.text.contains("goes to the session as it is") && r.text.ends_with("Nothing was sent."), "{}", r.text);
    }
    assert!(send(&tools, json!({"message": ""})).is_error);
    assert!(tools.locust.sent.lock().unwrap().is_empty());
}

#[test]
fn an_app_reaches_only_the_cloud_chats_it_started_unless_the_person_lets_apps_see_theirs() {
    let theirs = with_chat(false, false);
    for r in [send(&theirs, json!({})), theirs.call("read_reply", &json!({"conversation_id": ID})), theirs.call("stop_turn", &json!({"conversation_id": ID}))] {
        assert!(r.is_error && r.text.starts_with("That conversation could not be found"), "{}", r.text);
    }
    assert!(theirs.locust.sent.lock().unwrap().is_empty());
    assert_eq!(*theirs.locust.reads.lock().unwrap(), 0);
    let seen = with_chat(false, true);
    assert!(!send(&seen, json!({})).is_error);
    let unknown = send(&seen, json!({"conversation_id": "cc_00000000000000000042"}));
    assert!(unknown.is_error && unknown.text.starts_with("That conversation could not be found"));
    // A voice call never reaches one, whatever the person allowed.
    let scope = std::sync::Arc::new(VoiceScope::new(PathBuf::from("/known/cloud"), Some(ID)));
    let voice = Tools::new(Cloudy { see_all: true, ..cloudy() }).with_voice(scope);
    voice.locust.chats.lock().unwrap().push(CloudChat { session: session(ID, "/known/cloud"), from_app: true });
    assert!(send(&voice, json!({})).is_error);
    assert!(voice.locust.sent.lock().unwrap().is_empty());
}

#[test]
fn read_reply_on_a_cloud_chat_says_still_working_until_the_session_has_a_reply() {
    let tools = with_chat(true, false);
    let r = tools.call("read_reply", &json!({"conversation_id": ID, "wait_seconds": 20}));
    assert!(!r.is_error, "{}", r.text);
    assert_eq!(r.text, "Still working: waiting for Claude's cloud to answer.");
    assert_eq!(r.structured.as_ref().unwrap()["status"], "working");
    assert_eq!(*tools.locust.reads.lock().unwrap(), 1, "one look per read: wait_seconds does not make a cloud read loop");
    *tools.locust.found.lock().unwrap() = Some(CloudRead::Working { note: Some("Claude Code brought it in but did not save its conversation this time.".into()) });
    assert!(tools.call("read_reply", &json!({"conversation_id": ID})).text.ends_with("did not save its conversation this time."));
}

#[test]
fn read_reply_on_a_cloud_chat_gives_the_answer_the_files_it_changed_and_its_page() {
    let tools = with_chat(true, false);
    *tools.locust.found.lock().unwrap() = Some(CloudRead::Done {
        answer: "Fixed: total now sums prices.".into(),
        model: Some("claude-opus-5-5".into()),
        changed: vec![("cart.js".into(), 3, 1), ("cart.test.js".into(), 12, 0)],
        change_too_big: false,
        note: None,
    });
    let r = tools.call("read_reply", &json!({"conversation_id": ID}));
    assert!(!r.is_error, "{}", r.text);
    assert!(r.text.starts_with("Fixed: total now sums prices.\n\n"));
    assert!(r.text.contains("cart.js (+3 -1), cart.test.js (+12 -0)"), "{}", r.text);
    assert!(r.text.contains("not in your folder"));
    assert!(r.text.ends_with("Its page: https://claude.ai/code/session_0123456789abcdef"));
    let structured = r.structured.as_ref().unwrap();
    assert_eq!((structured["status"].clone(), structured["model"].clone(), structured["folder"].clone()), (json!("done"), json!("claude-opus-5-5"), json!("/known/cloud")));
    assert_eq!(structured["message"], r.text);
    *tools.locust.found.lock().unwrap() =
        Some(CloudRead::Done { answer: "Done.".into(), model: None, changed: vec![], change_too_big: true, note: Some("A note.".into()) });
    let big = tools.call("read_reply", &json!({"conversation_id": ID}));
    assert!(big.text.contains("too big to read here") && big.text.contains("Locust: A note."), "{}", big.text);
}

#[test]
fn a_cloud_read_that_fails_says_why_and_where_to_look() {
    let tools = with_chat(true, false);
    *tools.locust.found.lock().unwrap() = Some(CloudRead::Failed("Locust does not know this session\u{2019}s id, so it cannot read it here. See it on claude.ai.".into()));
    let r = tools.call("read_reply", &json!({"conversation_id": ID}));
    assert!(r.is_error);
    assert!(r.text.contains("cannot read it here") && r.text.ends_with("Its page: https://claude.ai/code/session_0123456789abcdef"), "{}", r.text);
    assert_eq!(r.structured.as_ref().unwrap()["status"], "failed");
}

#[test]
fn stop_turn_on_a_cloud_chat_says_it_cannot_be_stopped_from_here_and_gives_its_link() {
    let tools = with_chat(true, false);
    let r = tools.call("stop_turn", &json!({"conversation_id": ID}));
    assert!(r.is_error);
    assert_eq!(
        r.text,
        "A Claude cloud chat cannot be stopped from here: Claude Code gives Locust no way to stop a session. Stop it on claude.ai: https://claude.ai/code/session_0123456789abcdef Nothing was stopped."
    );
    // Without a page of its own, the list of sessions.
    let cloudy = Cloudy { see_all: false, ..cloudy() };
    let mut bare = session(ID, "/known/cloud");
    bare.url = None;
    cloudy.chats.lock().unwrap().push(CloudChat { session: bare, from_app: true });
    assert!(Tools::new(cloudy).call("stop_turn", &json!({"conversation_id": ID})).text.contains("Stop it on claude.ai: https://claude.ai/code Nothing"));
}

#[test]
fn a_cloud_chat_is_not_carried_on_to_another_agent_and_has_no_question_cards() {
    let tools = with_chat(true, true);
    let carried = tools.call("carry_on", &json!({"conversation_id": ID, "runtime": "codex"}));
    assert!(carried.is_error && carried.text.contains("cannot be carried on") && carried.text.ends_with("Nothing was started."), "{}", carried.text);
    let answered = tools.call("answer_question", &json!({"conversation_id": ID, "choice": 1}));
    assert!(answered.is_error && answered.text.contains("no question cards") && answered.text.ends_with("Nothing was sent."), "{}", answered.text);
    assert!(tools.locust.sent.lock().unwrap().is_empty());
}

#[test]
fn the_tools_descriptions_say_what_a_cloud_chat_does_when_followed_up_read_and_stopped() {
    let list = crate::door::tool_list();
    let said = |name: &str| list.as_array().unwrap().iter().find(|t| t["name"] == name).unwrap()["description"].as_str().unwrap().to_owned();
    assert!(said("send_message").contains("goes to the session itself") && said("send_message").contains("look at the session before sending again"));
    assert!(said("read_reply").contains("up to two minutes") && said("read_reply").contains("wait_seconds is ignored"));
    assert!(said("stop_turn").contains("cannot be stopped from here") && said("stop_turn").contains("claude.ai link"));
}
