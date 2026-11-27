//! apps/desktop/src/main/codex-and-opencode-teammates-get-their-skills.test.ts, case by case: a run's own skills
//! folder (`prepare_run`, run-skills.ts), which agents take a skill (skill_runtimes.rs, compare.rs), the tune that
//! drafts "What it knows" as one (tune.rs), and OpenCode 2's config for that folder (runtime/opencode_config.rs).
use super::*;
use crate::tune::TuneSkill;
use std::path::{Path, PathBuf};

struct Temp(PathBuf);
impl Temp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("locust-{tag}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The test's `skill(root, name, says)`.
fn skill(root: &Path, name: &str, says: &str) {
    std::fs::create_dir_all(root.join(name)).unwrap();
    std::fs::write(root.join(name).join("SKILL.md"), format!("---\nname: {name}\ndescription: test\n---\n{says}\n")).unwrap();
}

fn candidate(name: &str, description: &str, body: &str) -> TuneSkill {
    TuneSkill { name: name.into(), description: description.into(), body: body.into() }
}

fn listed(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

fn sorted(names: &[String]) -> Vec<String> {
    let mut names = names.to_vec();
    names.sort();
    names
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// What the disk here does with a name in another case (the test's `lstat(join(root, 'LIBRARY'))`).
fn disk_folds(root: &Path) -> bool {
    std::fs::create_dir_all(root.join("library")).unwrap();
    std::fs::metadata(root.join("LIBRARY")).is_ok()
}

#[test]
fn holds_the_teammates_listed_skills_the_kept_ones_and_a_tune_columns_one_the_most_particular_winning_a_name() {
    let (scratch, teammate, library) = (Temp::new("run-skills"), Temp::new("run-skills-mate"), Temp::new("run-skills-lib"));
    skill(&teammate.0, "evidence-first", "The teammate’s own.");
    skill(&teammate.0, "not-listed", "Say hi.");
    skill(&library.0, "evidence-first", "The library’s.");
    skill(&library.0, "review", "Say hi.");
    let sources = Sources {
        library: Some(library.0.clone()),
        teammate_folder: Some(teammate.0.clone()),
        teammate_skills: vec!["evidence-first".into()],
        candidate: Some(candidate("tune-candidate", "When the tune column runs.", "Check twice.")),
    };
    let prepared = prepare_run(&scratch.0, &sources).unwrap();
    assert_eq!(sorted(&prepared.skills), ["evidence-first", "review", "tune-candidate"]);
    assert_eq!(prepared.skills_dir, prepared.dir.join("skills"));
    assert_eq!(listed(&prepared.dir), [".agents", ".cursor-plugin", ".plugin", "skills"]);
    assert_eq!(listed(&prepared.skills_dir), ["evidence-first", "review", "tune-candidate"]);
    // The teammate's own copy, not the library's of the same name.
    assert!(read(&prepared.skills_dir.join("evidence-first/SKILL.md")).contains("The teammate’s own."));
    assert!(read(&prepared.skills_dir.join("tune-candidate/SKILL.md")).contains("name: tune-candidate"));
    // Copilot's manifest points at that same folder. Antigravity gets a copy, not a link.
    let manifest: serde_json::Value = serde_json::from_str(&read(&prepared.dir.join(".plugin/plugin.json"))).unwrap();
    assert_eq!(manifest["name"], "locust-run-skills");
    assert_eq!(manifest["skills"], "skills");
    assert_eq!(listed(&prepared.dir.join(".agents/skills")), ["evidence-first", "review", "tune-candidate"]);
    assert!(read(&prepared.dir.join(".agents/skills/evidence-first/SKILL.md")).contains("The teammate’s own."));
    remove_copies(std::slice::from_ref(&prepared.dir));
    assert!(std::fs::symlink_metadata(&prepared.dir).is_err());
    remove_copies(std::slice::from_ref(&prepared.dir));
}

#[test]
fn is_nothing_at_all_when_there_is_nothing_to_hand_over_and_leaves_no_folder_behind() {
    let (scratch, teammate) = (Temp::new("run-skills"), Temp::new("run-skills-mate"));
    skill(&teammate.0, "unlisted", "Say hi.");
    let sources = Sources { library: Some(scratch.0.join("missing")), teammate_folder: Some(teammate.0.clone()), teammate_skills: vec![], candidate: None };
    assert!(prepare_run(&scratch.0, &sources).is_none());
    assert!(prepare_run(&scratch.0, &Sources::default()).is_none());
    assert!(listed(&scratch.0).is_empty());
}

#[cfg(unix)]
#[test]
fn copies_no_link_so_removing_the_copy_can_never_reach_past_it() {
    let (scratch, library, elsewhere) = (Temp::new("run-skills"), Temp::new("run-skills-lib"), Temp::new("run-skills-elsewhere"));
    std::fs::write(elsewhere.0.join("precious.txt"), "keep me").unwrap();
    skill(&library.0, "review", "Say hi.");
    std::os::unix::fs::symlink(&elsewhere.0, library.0.join("review/outside")).unwrap();
    let prepared = prepare_run(&scratch.0, &Sources { library: Some(library.0.clone()), ..Default::default() }).unwrap();
    assert_eq!(listed(&prepared.skills_dir.join("review")), ["SKILL.md"]);
    assert_eq!(listed(&prepared.dir.join(".agents/skills/review")), ["SKILL.md"]);
    remove_copies(std::slice::from_ref(&prepared.dir));
    assert_eq!(read(&elsewhere.0.join("precious.txt")), "keep me");
}

#[test]
fn keeps_the_candidate_on_windows_when_a_library_folder_differs_only_in_case() {
    let root = Temp::new("skill-case");
    let library = root.0.join("library");
    skill(&library, "Review", "The library must not win.");
    skill(&library, "another-folder", "Say hi.");
    std::fs::write(library.join("another-folder/SKILL.md"), "---\nname: ReView\ndescription: A case-only identity collision.\n---\nThis must not be offered.\n").unwrap();
    let sources = Sources { library: Some(library), candidate: Some(candidate("review", "Candidate.", "The candidate wins.")), ..Default::default() };
    let prepared = prepare_run_with(&root.0.join("runs"), &sources, true, &|from, to| copy_dir(from, to)).unwrap();
    assert_eq!(prepared.skills, ["review"]);
    assert!(read(&prepared.skills_dir.join("review/SKILL.md")).contains("The candidate wins."));
    assert_eq!(listed(&prepared.skills_dir), ["review"]);
}

#[test]
fn keeps_the_first_frontmatter_name_even_when_the_directory_names_and_yaml_spellings_differ() {
    let root = Temp::new("skill-name");
    let (teammate, library) = (root.0.join("mate"), root.0.join("library"));
    skill(&teammate, "a-different-folder", "Say hi.");
    std::fs::write(teammate.join("a-different-folder/SKILL.md"), "---\nname: \"review\" # a name, not the directory\ndescription: First.\n---\nThe teammate wins.\n").unwrap();
    skill(&library, "z-library", "Say hi.");
    std::fs::write(library.join("z-library/SKILL.md"), "---\nname: 'review'\ndescription: Later.\n---\nThe library must not win.\n").unwrap();
    let sources = Sources {
        library: Some(library),
        teammate_folder: Some(teammate),
        teammate_skills: vec!["a-different-folder".into()],
        candidate: None,
    };
    let first = prepare_run(&root.0.join("runs"), &sources).unwrap();
    assert_eq!(first.skills, ["a-different-folder"]);
    let with_candidate = Sources { candidate: Some(candidate("review", "Candidate.", "The candidate wins.")), ..sources };
    let tuned = prepare_run(&root.0.join("runs"), &with_candidate).unwrap();
    assert_eq!(tuned.skills, ["review"]);
    assert!(read(&tuned.skills_dir.join("review/SKILL.md")).contains("The candidate wins."));
}

#[test]
fn keeps_case_distinct_skills_on_a_case_sensitive_machine() {
    let root = Temp::new("skill-sensitive");
    skill(&root.0.join("library"), "Review", "Say hi.");
    let sources = Sources { library: Some(root.0.join("library")), candidate: Some(candidate("review", "Candidate.", "First.")), ..Default::default() };
    let prepared = prepare_run_with(&root.0.join("runs"), &sources, false, &|from, to| copy_dir(from, to)).unwrap();
    // Windows itself cannot hold both directories; the key's semantics can still be proved on every host.
    assert_ne!(skill_name_key("Review", false), skill_name_key("review", false));
    assert_eq!(skill_name_key("Review", true), skill_name_key("review", true));
    // Nor can a disk that folds case: there the first is kept and the other left out on purpose, as on Windows.
    let want: &[&str] = if disk_folds(&root.0) { &["review"] } else { &["review", "Review"] };
    assert_eq!(prepared.skills, want);
    assert_eq!(listed(&prepared.skills_dir), sorted(&prepared.skills));
    assert!(read(&prepared.skills_dir.join("review/SKILL.md")).contains("First."));
}

#[test]
fn asks_the_disk_not_the_platform_whether_two_names_that_differ_only_in_case_are_one() {
    let root = Temp::new("skill-disk");
    let library = root.0.join("library");
    skill(&library, "another-folder", "Say hi.");
    std::fs::write(library.join("another-folder/SKILL.md"), "---\nname: ReView\ndescription: A case-only identity collision.\n---\nOffered only where case is kept.\n").unwrap();
    // No platform named: the disk answers.
    let sources = Sources { library: Some(library), candidate: Some(candidate("review", "Candidate.", "The candidate wins.")), ..Default::default() };
    let prepared = prepare_run(&root.0.join("runs"), &sources).unwrap();
    let want: &[&str] = if disk_folds(&root.0) { &["review"] } else { &["review", "another-folder"] };
    assert_eq!(prepared.skills, want);
}

#[test]
fn publishes_no_part_of_a_skill_whose_copy_fails_after_skill_md_arrived() {
    let root = Temp::new("skill-partial");
    skill(&root.0.join("library"), "broken", "Say hi.");
    let sources = Sources { library: Some(root.0.join("library")), candidate: Some(candidate("good", "Good.", "Keep this one.")), ..Default::default() };
    let failing = |from: &Path, to: &Path| -> std::io::Result<()> {
        copy_dir(from, to)?;
        Err(std::io::Error::other("Disk read failed after SKILL.md."))
    };
    let prepared = prepare_run_with(&root.0.join("runs"), &sources, false, &failing).unwrap();
    assert_eq!(prepared.skills, ["good"]);
    assert_eq!(listed(&prepared.skills_dir), ["good"]);
    assert_eq!(listed(&prepared.dir.join(".agents/skills")), ["good"]);
    assert_eq!(listed(&prepared.dir), [".agents", ".cursor-plugin", ".plugin", "skills"]);
}

// ---- which agents take a skill --------------------------------------------------------------------------------------

#[test]
fn is_claude_code_codex_opencode_copilot_and_antigravity_and_a_comparison_says_so_for_any_other() {
    use crate::compare::{CompareRoute, compare_skill_refusal};
    use crate::skill_runtimes::takes_skills;
    assert!(["claude", "codex", "opencode", "copilot", "antigravity", "cursor"].iter().all(|r| takes_skills(Some(r))));
    assert!(![Some("gemini"), Some("muse"), Some("kilo"), Some("acp"), None].iter().any(|r| takes_skills(*r)));
    let asked = candidate("review", "When reviewing.", "Check.");
    let route = |runtime: &str, model: &str| CompareRoute { skill: Some(asked.clone()), instructions: Some("x".into()), ..CompareRoute::new(runtime, model) };
    assert_eq!(compare_skill_refusal(&[route("codex", "gpt-6-mini")]), None);
    assert_eq!(
        compare_skill_refusal(&[route("gemini", "account-default")]).as_deref(),
        Some("A skill reaches only a Claude Code, Codex, OpenCode, Copilot, Antigravity or Cursor teammate for now. Nothing was started.")
    );
}

// case: a tune names the teammate’s own agent when it drafts "What it knows" as a skill
#[test]
fn a_tune_names_the_teammates_own_agent_when_it_drafts_what_it_knows_as_a_skill() {
    use crate::tune::tune_draft_prompt;
    assert!(tune_draft_prompt("Be concise.", &[], true, Some("Codex")).contains("This teammate runs on Codex, which loads a skill only when the work matches"));
    assert!(tune_draft_prompt("Be concise.", &[], true, None).contains("This teammate runs on Claude Code,"));
    // The tune window hands it the teammate's own agent: takes_skills(runtime), and that agent's name.
    let handler = include_str!("../../src/ui/tune_teammate.rs");
    assert!(handler.contains("let skills = takes_skills(Some(&rt));"));
    assert!(handler.contains("let rt_display = locust_core::carry_on::display_name(&rt);"));
    assert!(handler.contains("tune_draft_prompt(&current_inst, &lessons, skills, Some(rt_display))"));
}

// ---- OpenCode 2 ---------------------------------------------------------------------------------------------------

/// An OpenCode 2 command for `mode`, given the run's skills folder, finished as spawn finishes it.
fn opencode_2_config(mode: crate::approval::Mode, skills_dir: Option<&str>) -> serde_json::Value {
    use crate::runtime::opencode_config::{configure_with_seal, finish_v2, rewrite_config, with_skills};
    let root = Temp::new("oc2-skills");
    let store = crate::store::Store::with_codex(root.0.join("native"), root.0.join("electron"), vec![]).unwrap();
    let mut command = std::process::Command::new("opencode");
    configure_with_seal(&mut command, mode, &store, true, false).unwrap();
    rewrite_config(&mut command, |c| with_skills(c, skills_dir, &[]));
    finish_v2(&mut command);
    let content = command.get_envs().find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT").and_then(|(_, v)| v).unwrap();
    serde_json::from_str(&content.to_string_lossy()).unwrap()
}

#[test]
fn lists_the_runs_folder_in_its_own_configs_skills_and_nothing_when_there_is_none() {
    let with = opencode_2_config(crate::approval::Mode::AcceptEdits, Some("/profile/run-skills/abc/skills"));
    assert_eq!(with["skills"], serde_json::json!(["/profile/run-skills/abc/skills"]));
    let without = opencode_2_config(crate::approval::Mode::AcceptEdits, None);
    assert!(without.get("skills").is_none(), "{without}");
}

#[test]
fn allows_the_copied_skill_subtree_through_native_rules_without_changing_edit_or_shell_denials() {
    let config = opencode_2_config(crate::approval::Mode::Ask, Some("/profile/run-skills/abc/skills"));
    let rules = config["agents"]["locust"]["permissions"].as_array().unwrap();
    for (action, resource, effect) in [
        ("external_directory", "*", "deny"),
        ("external_directory", "/profile/run-skills/abc/skills/**", "allow"),
        ("edit", "*", "deny"),
        ("shell", "*", "ask"),
    ] {
        let rule = serde_json::json!({"action": action, "resource": resource, "effect": effect});
        assert!(rules.contains(&rule), "{rule} in {rules:?}");
    }
}

// ---- Lane check: skill-discovery.ts reads the name through a YAML parser, so its escapes and quotes are YAML's -------

#[test]
fn reads_a_double_quoted_names_escapes_and_a_single_quoted_names_doubled_quote_as_yaml_does() {
    assert_eq!(skill_name_in("---\nname: \"a\\tb\\nc\"\ndescription: x\n---\nbody\n").as_deref(), Some("a\tb\nc"));
    assert_eq!(skill_name_in("---\nname: 'it''s here'\ndescription: x\n---\nbody\n").as_deref(), Some("it's here"));
}

#[test]
fn a_tune_says_a_skill_costs_nothing_on_a_turn_it_does_not_fit() {
    let prompt = crate::tune::tune_draft_prompt("Be concise.", &[], true, Some("Codex"));
    assert!(
        prompt.contains("This teammate runs on Codex, which loads a skill only when the work matches the skill's description, so a skill costs nothing on turns it does not fit."),
        "{prompt}"
    );
}

// Lane check: tune-drafts.ts writes the whole of its skill sentence, not only its first half: how to write "knows" as a
// skill, the name's and body's limits (MAX_SKILL_NAME 64, MAX_SKILL_BODY 8_000 in shared/tune.ts), and how to keep
// things as they are. Electron's words, with its reply key "knows" said as this prompt's reply format names it, "know".
#[test]
fn a_tune_says_the_whole_of_how_to_write_what_it_knows_as_a_skill_and_how_to_keep_things_as_they_are() {
    let electron = "This teammate runs on Codex, which loads a skill only when the work matches the skill's description, so a skill costs nothing on turns it does not fit. Write \"knows\" as one skill instead of an edit of the standing instructions, which stay as they are in that setup. Its name is at most 64 characters of lower-case words or digits joined by single hyphens; its description is one sentence saying when to use it, in the words of the work it fits; its body holds the checks, at most 8000 characters. To keep things as they are instead, write \"knows\" as {\"instructions\": a copy of the current instructions, \"why\": why no edit is warranted}.";
    let expected = electron.replace("\"knows\"", "\"know\"");
    let prompt = crate::tune::tune_draft_prompt("Be concise.", &[], true, Some("Codex"));
    assert!(prompt.contains(&expected), "{prompt}");
    // The reply format this prompt asks for names that alternative "know", so the sentence names it the same way.
    assert!(prompt.contains(r#""know": {"skill":"#), "{prompt}");
    // A teammate whose agent takes no skills is told none of it.
    assert!(!crate::tune::tune_draft_prompt("Be concise.", &[], false, Some("Codex")).contains("as one skill instead"));
}

#[test]
fn ends_a_plain_name_at_a_comment_after_a_space_or_a_tab_and_not_at_a_hash_inside_a_word() {
    assert_eq!(skill_name_in("---\nname: review # a comment\n---\n").as_deref(), Some("review"));
    assert_eq!(skill_name_in("---\nname: review\t# a tab before the comment\n---\n").as_deref(), Some("review"));
    assert_eq!(skill_name_in("---\nname: review#not-a-comment\n---\n").as_deref(), Some("review#not-a-comment"));
}

// case: hands the folder to Codex on its app-server, OpenCode, Copilot and Antigravity, never on a cloud computer, and removes it after
#[test]
fn codex_gets_its_run_skill_and_leaves_no_copy_when_thread_start_fails() {
    use crate::{engine::Teammate, native, store::Store};
    use serde_json::{Value, json};
    use std::time::Instant;

    let root = Temp::new("codex-failed-run-skills");
    let store = Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let work = root.0.join("work");
    std::fs::create_dir(&work).unwrap();
    let library = store.root().join("skill-library/skills");
    skill(&library, "review", "Say hi.");
    let source = library.join("review/SKILL.md");
    let original = read(&source);
    let scratch = store.root().join("run-skills");

    // Like Electron's app-server adapter, the wire names extraRoots before starting the thread. The fake
    // reads the staged skill while handling that request, then refuses thread/start before any turn exists.
    let fixture = r#"
import json
import sys
from pathlib import Path

def reply(id, **result):
    print(json.dumps({"jsonrpc": "2.0", "id": id, **result}), flush=True)

for line in sys.stdin:
    call = json.loads(line)
    method = call["method"]
    if method == "initialized":
        continue
    with Path("skill-requests.jsonl").open("a", encoding="utf-8") as log:
        log.write(json.dumps(call) + "\n")
    id = call["id"]
    if method == "initialize":
        reply(id, result={})
    elif method == "skills/extraRoots/set":
        roots = call["params"]["extraRoots"]
        assert len(roots) == 1, roots
        copied = Path(roots[0]) / "review" / "SKILL.md"
        Path("offered-skill.md").write_bytes(copied.read_bytes())
        reply(id, result={})
    elif method == "config/read":
        reply(id, result={"config": {}})
    elif method == "thread/start":
        reply(id, error={"code": -32603, "message": "The runtime connection ended."})
    else:
        raise AssertionError(method)
"#;
    let server = root.0.join("refuse-thread.py");
    std::fs::write(&server, fixture).unwrap();
    let _hook = native::hooks::install(
        store.clone(),
        Some((
            PathBuf::from("python3"),
            vec![server.to_string_lossy().into_owned()],
        )),
    );
    let started = native::send(native::Send {
        teammate: Teammate {
            folder: Some(work.to_string_lossy().into_owned()),
            ..native::nobody(Some("codex".into()), None)
        },
        conversation: None,
        prompt: "Inspect the workspace without changing it.".into(),
        model: None,
        effort: None,
    })
    .unwrap();
    let id = &started.conversation_id;
    let deadline = Instant::now() + crate::load_waits::UNDER_LOAD;
    while native::owns(id) {
        assert!(Instant::now() < deadline, "the failed start kept its slot");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let requests: Vec<Value> = read(&work.join("skill-requests.jsonl"))
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let methods: Vec<_> = requests
        .iter()
        .map(|r| r["method"].as_str().unwrap())
        .collect();
    assert_eq!(
        methods,
        [
            "initialize",
            "skills/extraRoots/set",
            "config/read",
            "thread/start"
        ]
    );
    let offered = requests[1]["params"]["extraRoots"][0].as_str().unwrap();
    let offered = Path::new(offered);
    let run_dir = offered.parent().unwrap();
    assert_eq!(run_dir.parent(), Some(scratch.as_path()));
    assert_eq!(
        requests[1]["params"],
        json!({"extraRoots": [run_dir.join("skills")]})
    );
    assert_eq!(
        read(&work.join("offered-skill.md")),
        "---\nname: review\ndescription: test\n---\nSay hi.\n"
    );
    let ledger = read(&store.write_path(id).unwrap());
    let events: Vec<Value> = ledger
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .filter_map(|r: Value| r.get("event").cloned())
        .collect();
    let terminal: Vec<_> = events
        .iter()
        .filter(|e| {
            matches!(
                e["type"].as_str(),
                Some("run.completed" | "run.failed" | "run.cancelled")
            )
        })
        .collect();
    assert_eq!(terminal.len(), 1, "{events:?}");
    assert_eq!(terminal[0]["type"], "run.failed");
    assert!(
        terminal[0]["payload"]["message"]
            .as_str()
            .unwrap()
            .contains("The runtime connection ended.")
    );
    assert!(
        listed(&scratch).is_empty(),
        "the failed start left a run's skills on disk"
    );
    assert!(
        roots(id).is_empty(),
        "the failed start left a registered Codex skill root"
    );
    assert_eq!(
        read(&source),
        original,
        "the original skill must not be removed or changed"
    );
}

// Lane check of B22 (the dot's finding): a start refused after the run's skills were copied, but before the worker
// began (here, a ledger folder that cannot be written), takes that copy away too, as a run that ends does.
#[test]
fn a_start_refused_before_its_run_begins_leaves_no_copy_of_its_skills() {
    use crate::{engine::Teammate, native, store::Store};

    let root = Temp::new("refused-run-skills");
    let store = Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let work = root.0.join("work");
    std::fs::create_dir(&work).unwrap();
    skill(&store.root().join("skill-library/skills"), "review", "Say hi.");
    let scratch = store.root().join("run-skills");
    // A plain file where the ledger folder goes: the record cannot be made, so the run must not start.
    std::fs::write(store.root().join("mission-ledger"), "not a folder").unwrap();
    let _hook = native::hooks::install(store.clone(), Some((PathBuf::from("python3"), vec!["-c".into(), "raise SystemExit(1)".into()])));
    let refused = native::send(native::Send {
        teammate: Teammate { folder: Some(work.to_string_lossy().into_owned()), ..native::nobody(Some("codex".into()), None) },
        conversation: None,
        prompt: "Inspect the workspace without changing it.".into(),
        model: None,
        effort: None,
    });
    assert!(refused.is_err(), "a run with nowhere to record itself started: {:?}", refused.map(|s| s.conversation_id));
    assert!(listed(&scratch).is_empty(), "the refused start left a run's skills on disk: {:?}", listed(&scratch));
}
