//! a-turns-steps-read-like-claude-code.test.ts: a group of steps in one line, as `stepsLine` says it.
use crate::activity_entries::Patch;
use crate::ledger::{Item, Status, ToolDetail};
use crate::steps_line::{short_name, steps_line};

struct Shell<'a> {
    title: Option<&'a str>,
    failed: bool,
    exit: Option<i64>,
    settled: bool,
    status: &'a str,
}
const OK: Shell<'static> = Shell { title: None, failed: false, exit: Some(0), settled: true, status: "" };

fn shell_with(command: &str, over: Shell) -> Item {
    Item::Tool {
        name: "bash".into(),
        status: if !over.settled { Status::Running } else if over.failed { Status::Failed } else { Status::Done },
        summary: over.title.unwrap_or(command).into(),
        detail: Box::new(ToolDetail { arguments: command.into(), exit_code: over.exit, runtime_status: over.status.into(), ..Default::default() }),
    }
}
fn shell(command: &str) -> Item {
    shell_with(command, OK)
}
fn tool(tool: &str, name: &str) -> Item {
    Item::Tool {
        name: tool.into(),
        status: Status::Done,
        summary: name.into(),
        detail: Box::new(ToolDetail { arguments: name.into(), ..Default::default() }),
    }
}
fn thought(text: &str, ms: Option<u64>) -> Item {
    Item::Thought { id: "t".into(), text: text.into(), duration_ms: ms }
}
fn written(name: &str, text: String) -> Item {
    Item::Tool {
        name: "write".into(),
        status: Status::Done,
        summary: name.into(),
        detail: Box::new(ToolDetail { arguments: name.into(), patch: Some(Patch { text, added: 1, removed: 1, truncated: false }), ..Default::default() }),
    }
}
fn created(name: &str) -> Item {
    written(name, format!("diff --git a/{name} b/{name}\nnew file mode 100644\n--- /dev/null\n+++ b/{name}\n@@ -0,0 +1,2 @@\n+a\n+b\n"))
}
fn edited(name: &str) -> Item {
    written(name, format!("diff --git a/{name} b/{name}\n--- a/{name}\n+++ b/{name}\n@@ -1 +1 @@\n-a\n+b\n"))
}
fn line(items: &[Item]) -> String {
    steps_line(items, true, None, false)
        .segments
        .iter()
        .map(|s| if s.amber { format!("[{}]", s.text) } else { s.text.clone() })
        .collect::<Vec<_>>()
        .join(" · ")
}
const LONG: &str = "a-full-rule-store-keeps-every-rule.test.ts";

// case: names a lone command by what the model said it was for (Claude Code)
#[test]
fn names_a_lone_command_by_what_the_model_said_it_was_for_claude_code() {
    let item = shell_with("pnpm esbuild x.ts", Shell { title: Some("Bundle with pnpm esbuild and run benchmark"), ..OK });
    assert_eq!(line(&[item]), "Bundle with pnpm esbuild and run benchmark");
}

// case: says a lone described command that failed as the thing not done
#[test]
fn says_a_lone_described_command_that_failed_as_the_thing_not_done() {
    let item = shell_with("python ce3.py", Shell { title: Some("Add per-column effort chip and CSS"), failed: true, exit: Some(2), ..OK });
    assert_eq!(line(&[item]), "[Failed to add per-column effort chip and CSS]");
}

// case: counts reads and commands the way Claude Code does, naming nothing when it mixes kinds (0.610)
#[test]
fn counts_reads_and_commands_the_way_claude_code_does_naming_nothing_when_it_mixes_kinds_0_610() {
    assert_eq!(line(&[tool("Read", "src/a.ts"), tool("Read", "src/b.ts"), shell("git status")]), "Read 2 files, ran a command");
    assert_eq!(line(&[shell("npm test"), shell("npm run build"), tool("Read", "Composer.tsx")]), "Ran 2 commands, read a file");
    assert_eq!(line(&[shell("git status")]), "Ran git status");
}

// case: reads a command that only looks as looking (Codex and Cursor send no descriptions)
#[test]
fn reads_a_command_that_only_looks_as_looking_codex_and_cursor_send_no_descriptions() {
    assert_eq!(line(&[shell("sed -n '1,80p' src/app.ts"), shell("rg effort src"), shell("ls")]), "Read a file, ran a search, listed a folder");
    assert_eq!(line(&[shell("sed -n '1,80p' src/app.ts")]), "Read app.ts");
    assert_eq!(crate::steps_line::command_looks_at("cd repo && cat README.md"), Some("read"));
    assert_eq!(line(&[shell(r#"Get-Content "C:\tmp\drive.log" -Tail 80; Write-Output "----TERMINAL----"; Get-Content "C:\x\""#)]), "Read drive.log");
    assert_eq!(crate::steps_line::command_looks_at("pnpm test"), None);
}

// case: names what it made when that is all it did, and counts it beside other steps (0.610)
#[test]
fn names_what_it_made_when_that_is_all_it_did_and_counts_it_beside_other_steps_0_610() {
    assert_eq!(line(&[created("ce3.py")]), "Created ce3.py");
    assert_eq!(
        line(&[created("ce3.py"), shell("python ce3.py"), shell("ls -la"), shell("pnpm tsc")]),
        "Created a file, ran 2 commands, listed a folder"
    );
}

// case: says the thought only when nothing else was done: Claude Code's lines never lead with it (0.610)
#[test]
fn says_the_thought_only_when_nothing_else_was_done_claude_codes_lines_never_lead_with_it_0_610() {
    assert_eq!(line(&[thought("", Some(12_000)), shell("npm test")]), "Ran npm test");
    assert_eq!(line(&[thought("", Some(12_000))]), "Thought for 12s");
}

// case: never hides what went wrong: failures, silence, refusals in amber after the words
#[test]
fn never_hides_what_went_wrong_failures_silence_refusals_in_amber_after_the_words() {
    assert_eq!(line(&[shell_with("npm test", Shell { exit: Some(1), failed: true, ..OK }), shell("npm run lint")]), "Ran 2 commands · [1 failed]");
    assert_eq!(line(&[shell_with("npm test", Shell { settled: false, ..OK })]), "Ran npm test · [1 did not report]");
    assert_eq!(line(&[shell_with("rm -rf dist", Shell { status: "refused", ..OK }), shell("ls")]), "Listed a folder · [1 refused]");
}

// case: says searches, waits and a phrase-named read the way a person would
#[test]
fn says_searches_waits_and_a_phrase_named_read_the_way_a_person_would() {
    assert_eq!(line(&[tool("Grep", "effort"), tool("Grep", "slot")]), "Ran 2 searches");
    assert_eq!(line(&[tool("await", "shell-3"), tool("await", "shell-3")]), "Waited 2 times");
    assert_eq!(line(&[tool("list_dir", "Listing orb user session dirs")]), "Listed a folder");
    assert_eq!(line(&[tool("view_file", "\"C:/work/src/app.ts\"")]), "Read app.ts");
}

// case: reads a script, a phrase-named command and a folder searched as a person would
#[test]
fn reads_a_script_a_phrase_named_command_and_a_folder_searched_as_a_person_would() {
    assert_eq!(line(&[shell("@'\nconsole.log(1)\n'@ | node -")]), "Ran a script");
    assert_eq!(line(&[shell("Running probe-picker-contents.mjs --dev")]), "Running probe-picker-contents.mjs --dev");
    assert_eq!(line(&[tool("grep", r"C:\Users\me\project\src")]), "Searched src");
    assert_eq!(line(&[shell("node --input-type=module -e \"import { x } from './a.mjs'; x()\"")]), "Ran a node script");
    assert_eq!(line(&[shell("node -e \"console.log(6*7)\"")]), "Ran node -e \"console.log(6*7)\"");
    assert_eq!(line(&[shell(r"rg C:\Users\me\AppData\Local\Temp\run -n")]), "Searched run");
    assert_eq!(line(&[tool("node_repl", "node_repl.js"), tool("node_repl", "node_repl.js")]), "Ran JavaScript 2 times");
    assert_eq!(line(&[tool("python_repl", "x")]), "Ran Python");
    assert_eq!(line(&[tool("glob", "**/*")]), "Listed every file");
    assert_eq!(line(&[tool("Glob", "src/**/*.ts")]), "Listed files matching src/**/*.ts");
    assert_eq!(line(&[shell("Get-Content src/a.mjs; Get-Content src/b.mjs, src/c.mjs; Get-Content README.md | Select-Object -First 5")]), "Read 4 files");
    assert_eq!(line(&[shell("rg --files -g '**'")]), "Listed a folder");
    assert_eq!(line(&[shell("rg -n -g '**' validateMonth src")]), "Searched for validateMonth");
    assert_eq!(line(&[shell("cat README.md LOCUST.md")]), "Read 2 files");
    assert_eq!(
        line(&[thought("I will read README.md, then run the two commands.", None)]),
        "Thought: I will read README.md, then run the two commands."
    );
    assert_eq!(line(&[thought("x", None), tool("Read", "a.ts")]), "Read a.ts");
}

// case: names a connector and a tool it does not know by their own names
#[test]
fn names_a_connector_and_a_tool_it_does_not_know_by_their_own_names() {
    assert_eq!(line(&[tool("robinhood", "get_watchlists")]), "Used robinhood");
    assert_eq!(line(&[tool("WebSearch", "claude code app"), tool("WebFetch", "https://example.com")]), "Searched the web, fetched a page");
}

// case: cuts a long file name in the middle and keeps its extension
#[test]
fn cuts_a_long_file_name_in_the_middle_and_keeps_its_extension() {
    assert_eq!(line(&[edited(LONG)]), "Edited a-full-rule-stor\u{2026}very-rule.test.ts");
    assert_eq!(short_name(LONG).chars().count(), 34);
    assert_eq!(short_name("short.ts"), "short.ts");
    assert_eq!(short_name("exactly-thirty-four-characters.ts"), "exactly-thirty-four-characters.ts");
    let bare = short_name(&"a".repeat(40));
    assert_eq!(bare.chars().count(), 34);
    let (head, tail) = bare.split_once('\u{2026}').expect("cut in the middle");
    assert!(!head.is_empty() && !tail.is_empty() && head.chars().chain(tail.chars()).all(|c| c == 'a'), "{bare}");
}

// case: offers shorter forms: names give way to counts, then the last phrases to "and N more"
#[test]
fn offers_shorter_forms_names_give_way_to_counts_then_the_last_phrases_to_and_n_more() {
    let steps = [edited(LONG), shell("cat NOTES.md"), shell("pnpm test"), shell("pnpm tsc"), shell("rg approval src"), shell("ls -la")];
    let built = steps_line(&steps, true, None, false);
    assert_eq!(built.segments[0].text, "Edited a file, read a file, ran 2 commands, ran a search, listed a folder");
    assert_eq!(
        built.shorter,
        [
            "Edited a file, read a file, ran 2 commands, ran a search and 1 more",
            "Edited a file, read a file, ran 2 commands and 2 more",
            "Edited a file, read a file and 3 more",
            "Edited a file and 4 more",
        ]
    );
    assert_eq!(built.title, format!("Edited {LONG}, read NOTES.md, ran 2 commands, searched for approval, listed a folder"));
    assert_eq!(steps_line(&[shell("cat NOTES.md")], true, None, false).title, "Read NOTES.md");
}

// case: leaves a thought out of a line with other steps, and has nothing shorter for a lone failure
#[test]
fn leaves_a_thought_out_of_a_line_with_other_steps_and_has_nothing_shorter_for_a_lone_failure() {
    let built = steps_line(&[thought("Checking the store first.", Some(4_000)), edited(LONG), shell("pnpm test")], true, None, false);
    assert_eq!(built.segments[0].text, "Edited a file, ran a command");
    assert_eq!(built.shorter, ["Edited a file and 1 more"]);
    let failed = shell_with("pnpm test", Shell { title: Some("Run the tests"), exit: Some(1), failed: true, ..OK });
    assert!(steps_line(&[failed], true, None, false).shorter.is_empty());
    assert!(steps_line(&[], true, None, false).shorter.is_empty());
}

// From the runtime's events through the ledger, the fold and the line (a-long-silent-stretch-reads-as-its-moves.test.ts,
// a-wordless-thought-does-not-start-a-line.test.ts), as `buildThread` + `stepsLine` read them.
mod from_events {
    use crate::steps_fold::fold_steps;
    use crate::steps_line::steps_line;
    use serde_json::{json, Value};

    fn transcript(events: &[(&str, Value)]) -> crate::ledger::Transcript {
        let mut records = vec![json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {"missionId": "m", "prompt": "go", "createdAt": "2026-09-30T08:00:00.000Z"}})];
        for (n, (kind, payload)) in events.iter().enumerate() {
            let mut payload = payload.clone();
            payload["evidence"] = json!({"redacted": true});
            records.push(json!({"recordType": "mission.event", "event": {"id": format!("e{n}"), "runId": "r", "missionId": "m", "sequence": n + 1,
                "occurredAt": format!("2026-09-30T08:00:{:02}.000Z", n % 60), "sourceAdapter": "antigravity", "type": kind, "payload": payload}}));
        }
        let mut t = crate::ledger::Transcript::default();
        t.apply(records);
        t
    }
    fn think(id: &str, message: Option<&str>) -> Vec<(&'static str, Value)> {
        let mut done = json!({"stepKind": "reasoning", "itemId": id, "durationMs": 3800});
        if let Some(m) = message {
            done["message"] = json!(m);
        }
        vec![("step.started", json!({"stepKind": "reasoning", "itemId": id})), ("step.completed", done)]
    }
    fn shell(id: &str, command: &str) -> Vec<(&'static str, Value)> {
        vec![
            ("tool.started", json!({"itemId": id, "toolKind": "command_execution", "name": "run_command", "command": command, "phase": "started"})),
            ("tool.completed", json!({"itemId": id, "toolKind": "command_execution", "name": "run_command", "command": command, "phase": "completed", "exitCode": 0})),
        ]
    }
    fn lines(events: Vec<(&'static str, Value)>) -> Vec<(String, usize)> {
        let t = transcript(&events);
        fold_steps(&t.items, false)
            .iter()
            .map(|g| {
                let items: Vec<_> = g.items.iter().map(|&i| t.items[i].clone()).collect();
                let thoughts = items.iter().filter(|i| matches!(i, crate::ledger::Item::Thought { .. })).count();
                (steps_line(&items, true, None, false).segments[0].text.clone(), thoughts)
            })
            .collect()
    }

    // case: starts a new line each time the model thinks, led by what it thought about
    #[test]
    fn a_silent_stretch_starts_a_line_at_each_thought_and_reads_as_its_headline() {
        let mut events = vec![("run.started", json!({}))];
        events.extend(think("r1", Some("**Inspecting the config**\n\nLooking for where the port is set.")));
        events.extend(shell("c1", "cat config.json"));
        events.extend(shell("c2", "rg port src"));
        events.extend(think("r2", Some("**Running the tests**")));
        events.extend(shell("c3", "npm test"));
        events.extend(think("r3", None));
        events.extend(shell("c4", "npm run build"));
        events.push(("message.delta", json!({"itemId": "m1", "operation": "append", "text": "Done.", "final": true})));
        events.push(("run.completed", json!({})));
        let said: Vec<String> = lines(events).into_iter().map(|(text, _)| text).collect();
        assert_eq!(said, ["Inspecting the config", "Running the tests"]);
    }

    // case: leaves a turn with no thinking reported as one line between what was said (Claude Code without thinking)
    #[test]
    fn a_turn_with_no_thinking_reported_is_one_line_of_its_moves() {
        let mut events = vec![("run.started", json!({}))];
        events.extend(shell("c1", "ls"));
        events.extend(shell("c2", "pwd"));
        events.push(("run.completed", json!({})));
        let said = lines(events);
        assert_eq!(said.len(), 1);
        assert_eq!(said[0].0, "Listed a folder, ran a command");
    }

    // case: is one line when the thoughts say nothing
    #[test]
    fn wordless_thoughts_before_every_call_are_one_line_that_still_holds_them() {
        let mut events = vec![("run.started", json!({}))];
        for (n, command) in ["Get-ChildItem", "Get-Content a.md", "git status"].iter().enumerate() {
            events.extend(think(&format!("t{n}"), None));
            events.extend(shell(&format!("c{n}"), command));
        }
        events.push(("run.completed", json!({})));
        let said = lines(events);
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(said[0].1, 3, "the thinking is still in the line it folded into");
        assert_eq!(said[0].0, "Listed a folder, read a file, ran a command");
    }

    // case: still starts a line at a thought that says something
    #[test]
    fn a_thought_with_words_still_starts_its_own_line() {
        let mut events = vec![("run.started", json!({}))];
        events.extend(think("t1", Some("Listing the folder")));
        events.extend(shell("c1", "Get-ChildItem"));
        events.extend(think("t2", Some("Reading the plan")));
        events.extend(shell("c2", "Get-Content a.md"));
        events.push(("run.completed", json!({})));
        let said = lines(events);
        assert_eq!(said.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), ["Listed a folder", "Read a.md"]);
    }
}

// case: carries the summary Codex shows, and never the raw reasoning
#[test]
fn a_codex_thought_carries_the_summary_codex_shows_and_never_the_raw_reasoning() {
    let mut codex = crate::runtime::events::Normalizer::default();
    codex.accept("item/started", &serde_json::json!({"threadId": "t", "turnId": "u", "item": {"type": "reasoning", "id": "rs_1", "summary": [], "content": []}}));
    let item = serde_json::json!({"type": "reasoning", "id": "rs_1", "summary": ["**Inspecting the config**\n\nThe port is read from config.json."], "content": ["RAW PRIVATE REASONING"]});
    let events = codex.accept("item/completed", &serde_json::json!({"threadId": "t", "turnId": "u", "item": item}));
    let done = events.iter().find(|e| e.kind == "step.completed").expect("the thought completes");
    assert!(done.payload["message"].as_str().is_some_and(|m| m.contains("Inspecting the config")), "{}", done.payload);
    assert!(!done.payload.to_string().contains("RAW PRIVATE REASONING"));
}
