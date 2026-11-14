//! Electron's `shared/cursorIgnore.test.ts`, one Rust test for each of its nine cases, in its order, with its fixtures
//! (the three real files, as they were on that machine when each incident happened) and its sentences. Tests named
//! `beyond_electron_` hold what its source does and its test is silent on, from `main/cursor-visibility.ts` and from
//! running `shared/cursorIgnore.ts`.
use super::*;
use std::path::Path;

const AS_IT_WAS: &str = "# stop Cursor snapshot-indexing multi-GB agent transcripts.\n.claude/\n.codex/\n.gemini/\n.cursor/projects/\nAppData/\nnode_modules/\n**/node_modules/";

const AS_FIXED: &str = "# stop Cursor snapshot-indexing multi-GB agent transcripts.\n.claude/*\n!.claude/.locust/\n.codex/*\n!.codex/worktrees/\n.gemini/\n.cursor/projects/\nAppData/\nnode_modules/\n**/node_modules/";

fn directory(hit: Option<CursorIgnoreHit>) -> Option<String> {
    hit.map(|h| h.directory)
}

#[test]
fn finds_the_bare_directory_rule_the_shape_all_three_incidents_began_as() {
    assert_eq!(
        directory(cursor_ignore_hit("C:/Users/<home>/.claude/.locust/attachments/image-2.png", AS_IT_WAS)),
        Some(".claude".to_owned())
    );
    assert_eq!(
        directory(cursor_ignore_hit("C:/Users/<home>/.codex/worktrees/e6a8/locust-astra", AS_IT_WAS)),
        Some(".codex".to_owned())
    );
    assert_eq!(
        directory(cursor_ignore_hit("C:/Users/<home>/AppData/Local/Temp/locust-x/file.ts", AS_IT_WAS)),
        Some("AppData".to_owned())
    );
}

#[test]
fn reads_the_dir_star_shape_which_is_what_every_fix_turns_into() {
    // THE blind spot in the harness matcher this replaces: it only read rules ending in `/`, so the moment an incident
    // was fixed -- and the fix must be `dir/*`, because gitignore cannot negate inside an excluded directory -- it
    // stopped seeing the rule at all.
    assert_eq!(
        directory(cursor_ignore_hit("C:/Users/<home>/.claude/projects/x/big.jsonl", AS_FIXED)),
        Some(".claude".to_owned())
    );
}

#[test]
fn honours_the_re_inclusion_or_it_would_cry_wolf_about_the_fix_itself() {
    assert_eq!(cursor_ignore_hit("C:/Users/<home>/.claude/.locust/attachments/image-2.png", AS_FIXED), None);
    assert_eq!(cursor_ignore_hit("C:/Users/<home>/.codex/worktrees/e6a8/locust-astra", AS_FIXED), None);
}

#[test]
fn matches_a_multi_part_rule_only_where_the_parts_run_together() {
    assert_eq!(
        directory(cursor_ignore_hit("C:/Users/<home>/.cursor/projects/abc/log", AS_IT_WAS)),
        Some(".cursor/projects".to_owned())
    );
    // `.cursor` alone is not the rule, and neither is `projects` alone.
    assert_eq!(cursor_ignore_hit("C:/Users/<home>/.cursor/extensions", AS_IT_WAS), None);
    assert_eq!(cursor_ignore_hit("C:/work/projects/app", AS_IT_WAS), None);
}

#[test]
fn stays_quiet_about_a_path_nothing_names_and_about_comments() {
    assert_eq!(cursor_ignore_hit("C:/Users/<home>/Documents/Codex/app/src/index.ts", AS_IT_WAS), None);
    assert_eq!(cursor_ignore_hit("C:/Users/<home>/stop/x", "# stop Cursor snapshot-indexing"), None);
}

#[test]
fn reads_star_star_slash_name_as_the_directory_anywhere_on_the_path() {
    assert_eq!(
        directory(cursor_ignore_hit("C:/app/packages/ui/node_modules/react/index.js", AS_IT_WAS)),
        Some("node_modules".to_owned())
    );
}

#[test]
fn splits_a_real_windows_path_which_every_fixture_here_used_to_dodge() {
    // The test that would have caught it. The separator was a character class a shell had halved, so it matched forward
    // slash only -- and every fixture above is written with forward slashes, so seven tests passed over a matcher that
    // could not read a single path this app actually handles. The paths below are built from the character code so
    // nothing can halve them on the way to disk either.
    let sep = char::from(92u8).to_string();
    let windows_path = ["C:", "Users", "<home>", ".claude", ".locust", "attachments", "a.png"].join(&sep);
    assert_eq!(directory(cursor_ignore_hit(&windows_path, AS_IT_WAS)), Some(".claude".to_owned()));
    assert_eq!(cursor_ignore_hit(&windows_path, AS_FIXED), None);
    let mixed = format!("C:{sep}Users/<home>{sep}AppData/Local/Temp/x");
    assert_eq!(directory(cursor_ignore_hit(&mixed, AS_IT_WAS)), Some("AppData".to_owned()));
}

#[test]
fn does_not_tell_you_to_change_a_rule_into_itself() {
    // 0.87.0 shipped advice that read "Change that rule to \".claude/*\"" when the rule ALREADY was `.claude/*`. A
    // message that tells a person to make a change they have made is how they stop believing the rest of it.
    //
    // The two cases need different advice: a bare `dir/` cannot be negated at all, so the rule itself must change; a
    // `dir/*` already can be, so what is missing is the line that lets this folder back in.
    let narrowed = cursor_ignore_hit("C:/Users/<home>/.claude/projects/x", AS_FIXED).unwrap();
    assert!(narrowed.already_narrowed);
    let advice = cursor_ignore_sentence(&narrowed, "C:/Users/<home>/.cursorignore");
    assert!(!advice.contains("Change that rule"));
    assert!(advice.contains("Add \"!.claude/"));

    let bare = cursor_ignore_hit("C:/Users/<home>/.claude/projects/x", AS_IT_WAS).unwrap();
    assert!(!bare.already_narrowed);
    assert!(cursor_ignore_sentence(&bare, "C:/x/.cursorignore").contains("Change that rule to \".claude/*\""));
}

#[test]
fn says_what_to_do_naming_the_rule_and_the_file() {
    let hit = cursor_ignore_hit("C:/Users/<home>/.claude/.locust/attachments/a.png", AS_IT_WAS).unwrap();
    let said = cursor_ignore_sentence(&hit, "C:/Users/<home>/.cursorignore");
    // The rule and where it lives, because "Cursor cannot read this folder" on its own is the same dead end the runtime
    // already gives.
    assert!(said.contains(".cursorignore"));
    assert!(said.contains("\".claude/\""));
    assert!(said.contains("\".claude/*\""));
    assert!(said.contains("permission denied"));
}

// What Electron's source does and its test is silent on. Values taken from running `shared/cursorIgnore.ts`.

#[test]
fn beyond_electron_the_whole_sentence_is_electrons_word_for_word() {
    let bare = cursor_ignore_hit("/p/.claude/x", AS_IT_WAS).unwrap();
    assert_eq!(
        cursor_ignore_sentence(&bare, "/p/.cursorignore"),
        "Cursor cannot read files here: /p/.cursorignore hides this folder with the rule \".claude/\". Its tools will answer \"permission denied\" and give no reason. Change that rule to \".claude/*\" and add \"!.claude/<the folder you need>\" beneath it, or remove it."
    );
    let narrowed = cursor_ignore_hit("/p/.claude/x", AS_FIXED).unwrap();
    assert_eq!(
        cursor_ignore_sentence(&narrowed, "/p/.cursorignore"),
        "Cursor cannot read files here: /p/.cursorignore hides this folder with the rule \".claude/*\". Its tools will answer \"permission denied\" and give no reason. Add \"!.claude/<the folder you need>\" beneath that rule, or remove it."
    );
}

#[test]
fn beyond_electron_a_rule_is_read_as_javascript_trims_and_matches_it() {
    let hit = |text: &str| cursor_ignore_hit("/x/.claude/y", text).map(|h| (h.rule, h.directory, h.already_narrowed));
    // A byte-order mark at the head of the file is white space to `trim`; U+0085 is not.
    assert_eq!(hit("\u{feff}.claude/"), Some((".claude/".to_owned(), ".claude".to_owned(), false)));
    assert_eq!(hit(".claude/\u{85}"), None);
    // The no-break space and the other Unicode spaces are trimmed as JavaScript trims them.
    assert_eq!(hit("\u{a0}.claude/\u{a0}"), Some((".claude/".to_owned(), ".claude".to_owned(), false)));
    // `.` does not cross a carriage return, a line separator or a paragraph separator.
    assert_eq!(cursor_ignore_hit("/x/a/y", "a\r/").map(|h| h.rule), None);
    assert_eq!(cursor_ignore_hit("/x/a/y", "a\u{2028}b/").map(|h| h.rule), None);
    // A rule with a carriage return at its end (a file saved on Windows) is trimmed, and still a rule.
    assert_eq!(hit(".claude/\r\n"), Some((".claude/".to_owned(), ".claude".to_owned(), false)));
}

#[test]
fn beyond_electron_a_rule_is_a_directory_and_nothing_else() {
    let hit = |path: &str, text: &str| cursor_ignore_hit(path, text).map(|h| (h.rule, h.directory, h.already_narrowed));
    // A multi-part directory in the `dir/*` shape; a leading `/` is kept in the name (and splits to nothing); `a/**` is
    // not a shape it reads; `**/` is dropped once; a doubled slash keeps one in the name; a bare `//` names nothing.
    assert_eq!(hit("/x/a/b/y", "a/b/*\n"), Some(("a/b/*".to_owned(), "a/b".to_owned(), true)));
    assert_eq!(hit("/x/a/y", "/a/"), Some(("/a/".to_owned(), "/a".to_owned(), false)));
    assert_eq!(hit("/x/a/y", "a/**"), None);
    assert_eq!(hit("/x/a/y", "**/a/*"), Some(("**/a/*".to_owned(), "a".to_owned(), true)));
    assert_eq!(hit("/x/a/y", "a//"), Some(("a//".to_owned(), "a/".to_owned(), false)));
    assert_eq!(hit("/x/a/y", "//"), None);
    // A bang with a space after it is a negation of ` a/`, a name that is not on the path, so the rule below stands.
    assert_eq!(hit("/x/a/y", "! a/\na/"), Some(("a/".to_owned(), "a".to_owned(), false)));
}

#[test]
fn beyond_electron_the_later_rule_wins_and_a_re_inclusion_names_the_path_it_brings_back() {
    // The exclusion, then its negation: nothing. The negation of a deeper folder does not cancel a shallower hit's
    // being listed first; it cancels only when its own parts are on the path.
    assert_eq!(cursor_ignore_hit("/x/a/y", "a/\n!a/\n"), None);
    assert_eq!(directory(cursor_ignore_hit("/x/a/y", "a/\n!a/z/")), Some("a".to_owned()));
    assert_eq!(directory(cursor_ignore_hit("/x/a/z/y", "a/\n!a/z/")), None);
    // The negation first, the exclusion after it: the exclusion wins.
    assert_eq!(directory(cursor_ignore_hit("/x/a/y", "!a/\na/")), Some("a".to_owned()));
}

#[test]
fn beyond_electron_the_rules_files_are_the_folders_own_then_the_home_folders() {
    // cursor-visibility.ts: `.cursorignore` in the folder, nearest first, then in the home folder.
    let paths = cursor_ignore_paths(Path::new("/work/app"), Some(Path::new("/Users/pat")));
    assert_eq!(paths, vec![Path::new("/work/app/.cursorignore").to_path_buf(), Path::new("/Users/pat/.cursorignore").to_path_buf()]);
}

#[test]
fn beyond_electron_it_says_the_first_file_that_hides_the_folder_and_a_file_it_cannot_read_says_nothing() {
    let cwd = Path::new("/Users/pat/.claude/work");
    let home = Path::new("/Users/pat");
    let read = |files: Vec<(&'static str, &'static str)>| {
        move |path: &Path| files.iter().find(|(p, _)| Path::new(p) == path).map(|(_, text)| (*text).to_owned())
    };
    // Neither file there: nothing to say.
    assert_eq!(cursor_cannot_see(cwd, Some(home), &read(vec![])), None);
    // Only the home folder's, and it hides the folder: its path is named in the sentence.
    let said = cursor_cannot_see(cwd, Some(home), &read(vec![("/Users/pat/.cursorignore", ".claude/")])).unwrap();
    assert!(said.starts_with("Cursor cannot read files here: /Users/pat/.cursorignore hides this folder with the rule \".claude/\"."));
    // The folder's own file does not hide it, so the home folder's is asked next.
    let both = read(vec![("/Users/pat/.claude/work/.cursorignore", "node_modules/"), ("/Users/pat/.cursorignore", ".claude/")]);
    assert!(cursor_cannot_see(cwd, Some(home), &both).unwrap().contains("/Users/pat/.cursorignore"));
    // The folder's own file hides it first.
    let own = read(vec![("/Users/pat/.claude/work/.cursorignore", ".claude/*"), ("/Users/pat/.cursorignore", ".claude/")]);
    let said = cursor_cannot_see(cwd, Some(home), &own).unwrap();
    assert!(said.contains("/Users/pat/.claude/work/.cursorignore") && said.contains("\".claude/*\""));
}

#[test]
fn beyond_electron_a_failed_tool_says_permission_denied_in_any_case() {
    // codex-mission.ts explainADeniedRead: the raw record of a failed tool carries the message, unredacted.
    assert!(says_permission_denied(&serde_json::json!({"errorMessage": "Permission denied"})));
    assert!(says_permission_denied(&serde_json::json!({"nested": {"e": "PERMISSION DENIED reading x"}})));
    assert!(!says_permission_denied(&serde_json::json!({"errorMessage": "No such file"})));
    assert!(!says_permission_denied(&serde_json::Value::Null));
}

// The wrapper that says it in the run, around a Cursor runtime that is not one: a stub that hands back what it is given.

struct Scripted {
    events: std::collections::VecDeque<crate::runtime::Event>,
}

impl crate::runtime::Runtime for Scripted {
    fn start(&mut self, _turn: &crate::runtime::Turn) -> std::io::Result<()> {
        Ok(())
    }
    fn next_event(&mut self) -> std::io::Result<Option<crate::runtime::Event>> {
        Ok(self.events.pop_front())
    }
    fn stop(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn event(kind: &str, payload: serde_json::Value) -> crate::runtime::Event {
    crate::runtime::Event { kind: kind.into(), payload, thread: Some("th_1".into()) }
}

fn denied(item: &str) -> crate::runtime::Event {
    event("tool.failed", serde_json::json!({"itemId": item, "evidence": {"runtimeEventType": "tool_call", "raw": {"errorMessage": "Permission denied"}}}))
}

fn scratch_folder(name: &str, rules: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-cursor-ignore-{name}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(".cursorignore"), rules).unwrap();
    dir
}

fn drain(wrapped: &mut ExplainsDeniedReads) -> Vec<(String, serde_json::Value)> {
    use crate::runtime::Runtime;
    let mut out = Vec::new();
    while let Some(e) = wrapped.next_event().unwrap() {
        out.push((e.kind, e.payload));
    }
    out
}

#[test]
fn beyond_electron_a_denied_read_is_explained_once_beside_the_failure_it_explains() {
    // codex-mission.ts explainADeniedRead: only after a failed tool whose record says "permission denied", never
    // before, once per run, and the sentence follows the failure.
    let name = scratch_folder("denied", "scratch/\n");
    let folder = name.join("scratch");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(".cursorignore"), "scratch/\n").unwrap();
    let mut inner = Scripted {
        events: vec![
            event("tool.completed", serde_json::json!({"itemId": "a", "evidence": {"raw": {"note": "Permission denied was a word in the file"}}})),
            denied("b"),
            denied("c"),
            event("run.completed", serde_json::json!({})),
        ]
        .into(),
    };
    let mut wrapped = ExplainsDeniedReads::new(&mut inner, &folder, None);
    let seen = drain(&mut wrapped);
    let kinds: Vec<&str> = seen.iter().map(|(k, _)| k.as_str()).collect();
    // A completed tool that merely mentions the words is not a refusal; the first refusal is followed by the sentence,
    // the second is not explained again.
    assert_eq!(kinds, ["tool.completed", "tool.failed", "adapter.diagnostic", "tool.failed", "run.completed"]);
    let notice = &seen[2].1;
    assert_eq!(notice["level"], "warning");
    assert_eq!(notice["code"], "host.cursorignore.runtime_error");
    assert_eq!(notice["terminal"], false);
    assert_eq!(notice["evidence"], serde_json::json!({"redacted": true}));
    let said = notice["message"].as_str().unwrap();
    assert!(said.starts_with("Cursor cannot read files here: "), "{said}");
    assert!(said.contains("hides this folder with the rule \"scratch/\""), "{said}");
    let _ = std::fs::remove_dir_all(&name);
}

#[test]
fn beyond_electron_a_denied_read_with_no_rule_hiding_the_folder_says_nothing_and_is_not_asked_again() {
    let folder = scratch_folder("quiet", "node_modules/\n");
    let mut inner = Scripted { events: vec![denied("a"), denied("b")].into() };
    let mut wrapped = ExplainsDeniedReads::new(&mut inner, &folder, None);
    let kinds: Vec<String> = drain(&mut wrapped).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, ["tool.failed", "tool.failed"]);
    let _ = std::fs::remove_dir_all(&folder);
}

#[test]
fn beyond_electron_the_notice_is_a_diagnostic_the_thread_does_not_drop() {
    // missionView.ts keeps a diagnostic raised before the first tool call only when its code ends in `runtime_error`
    // or `notification`; this one is raised before anything at all, so it must be in the family that gets through.
    assert!(NOTICE_CODE.ends_with("runtime_error"));
    assert_eq!(notice_payload("x")["message"], "x");
}

#[test]
fn beyond_electron_a_refused_read_in_the_real_cursor_stream_is_explained_in_the_run() {
    // Through the real Cursor reader, not a stub: a read whose result is Cursor's own refusal ("Permission denied", as
    // its failed tool's record carries it) comes out `tool.failed` with the record in its evidence, and the wrapper
    // names the rule right after it.
    use crate::runtime::cursor_events::CursorNormalizer;
    let folder = scratch_folder("stream", "app/\n");
    let work = folder.join("app");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(work.join(".cursorignore"), "app/\n").unwrap();
    let lines = [
        r#"{"type":"system","subtype":"init","apiKeySource":"login","cwd":"/w","session_id":"s1","model":"Composer","permissionMode":"default"}"#,
        r#"{"type":"tool_call","subtype":"started","call_id":"c1","tool_call":{"readToolCall":{"args":{"path":"/w/app/a.ts"}},"toolCallId":"c1"},"session_id":"s1"}"#,
        r#"{"type":"tool_call","subtype":"completed","call_id":"c1","tool_call":{"readToolCall":{"args":{"path":"/w/app/a.ts"},"result":{"error":{"errorMessage":"Permission denied"}}},"toolCallId":"c1"},"session_id":"s1"}"#,
    ];
    let mut normal = CursorNormalizer::default();
    let events: std::collections::VecDeque<_> = lines.iter().flat_map(|l| normal.accept_line(l)).collect();
    let mut inner = Scripted { events };
    let mut wrapped = ExplainsDeniedReads::new(&mut inner, &work, None);
    let seen = drain(&mut wrapped);
    let kinds: Vec<&str> = seen.iter().map(|(k, _)| k.as_str()).collect();
    let failed_at = kinds.iter().position(|k| *k == "tool.failed").expect("a failed tool");
    assert_eq!(kinds[failed_at + 1], "adapter.diagnostic", "{kinds:?}");
    assert!(seen[failed_at + 1].1["message"].as_str().unwrap().contains("hides this folder with the rule \"app/\""));
    let _ = std::fs::remove_dir_all(&folder);
}
