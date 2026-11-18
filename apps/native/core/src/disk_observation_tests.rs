use crate::{activity_entries, activity_summary, ledger::{Item, Transcript}};
use serde_json::json;
const FIRST: &str = "--- a/README.md\n+++ b/README.md\n@@ -3 +3 @@\n-Colour: blue\n+Colour: green\n";
const SECOND: &str = "--- a/README.md\n+++ b/README.md\n@@ -4 +4 @@\n-Status: draft\n+Status: ready\n";
fn fold(rows: &[(&str, &str, &str, Option<&str>)]) -> Vec<Item> {
    let mut events = vec![];
    for (id, kind, path, patch) in rows {
        for phase in ["started", "completed"] {
            let mut p = json!({"itemId":id,"toolKind":kind,"name":if *kind == "file_change" {"file_change"} else {"edit"},"command":path,"phase":phase});
            if *kind == "observed_edit" { p["status"] = json!("reported by the runtime, read from disk"); }
            if phase == "completed" && let Some(text) = patch {
                let counts = crate::changes::parse(text).iter().fold((0,0), |(a,d),f| {let c=f.counts();(a+c.0,d+c.1)});
                p["patch"] = json!({"text":text,"added":counts.0,"removed":counts.1,"truncated":false});
            }
            events.push(json!({"recordType":"mission.event","event":{"type":format!("tool.{phase}"),"sequence":events.len()+1,"payload":p}}));
        }
    }
    let mut t=Transcript::default();t.apply(events);t.items
}
#[test]
fn attaches_the_patch_to_the_row_the_runtime_drew_and_draws_a_row_only_for_a_path_it_never_named() {
    let patch="--- /dev/null\n+++ b/NOTES.md\n@@ -0,0 +1,2 @@\n+# Notes\n+First entry.\n";
    let items=fold(&[("fc","file_change","NOTES.md",None),("disk-observed-1","observed_edit","NOTES.md",Some(patch)),("disk-observed-2","observed_edit","other.txt",None)]);
    let notes=items.iter().filter(|i|matches!(i,Item::Tool{summary,..} if summary.contains("NOTES"))).collect::<Vec<_>>();
    assert_eq!(notes.len(),1);
    let Item::Tool{detail,..}=notes[0] else {panic!()};assert_eq!(detail.patch.as_ref().unwrap().added,2);
    assert!(items.iter().any(|i|matches!(i,Item::Tool{summary,..} if summary=="other.txt")));
    assert_eq!(activity_summary::activity_summary(&items),"Edited 2 files");
}
#[test]
fn still_attaches_when_the_runtime_already_sent_its_own_diff_for_that_path() {
    let patch="--- /dev/null\n+++ b/report.md\n@@ -0,0 +1 @@\n+hello\n";
    let items=fold(&[("w","write","report.md",Some(patch)),("obs","observed_edit","report.md",Some(patch))]);
    assert_eq!(activity_entries::net_entries(&items,None).len(),1);
    assert_eq!(activity_summary::activity_summary(&items),"Edited 1 file");
}
#[test]
fn supersedes_every_other_edit_row_for_that_file_so_the_card_counts_the_file_once() {
    let net="--- a/README.md\n+++ b/README.md\n@@ -3,2 +3,2 @@\n-Colour: blue\n-Status: draft\n+Colour: green\n+Status: ready\n";
    let items=fold(&[("e1","edit","C:\\work\\README.md",Some(FIRST)),("e2","edit","C:\\work\\README.md",Some(SECOND)),("obs","observed_edit","README.md",Some(net))]);
    let card=activity_entries::net_entries(&items,Some("C:\\work"));assert_eq!(card.len(),1);assert_eq!(card[0].1.counts,(2,2));
    assert_eq!(activity_summary::activity_summary(&items),"Edited 1 file");
}
#[test]
fn leaves_two_edits_to_one_file_alone_when_no_observation_arrives() {
    let items=fold(&[("e1","edit","README.md",Some(FIRST)),("e2","edit","README.md",Some(SECOND))]);
    assert_eq!(activity_entries::counts(&items),(2,2));
}

// ---- main/disk-observation.test.ts ----

mod electron_disk_observation {
    use crate::checkpoints;
    use crate::disk_observation::*;
    use crate::store::Store;
    use serde_json::{Value, json};
    use std::cell::RefCell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::io;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static TMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(prefix: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "{prefix}-{}-{}",
                std::process::id(),
                TMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct FakeHost<'a> {
        git: Box<dyn Fn(&[&str], &Path) -> Result<String, GitOutputError> + 'a>,
        stat: Box<dyn Fn(&Path) -> Option<FileStat> + 'a>,
        read: Box<dyn Fn(&Path) -> Option<String> + 'a>,
        list: Box<dyn Fn(&Path) -> io::Result<Vec<FolderEntry>> + 'a>,
        now: u64,
        before: Box<dyn Fn(&str) -> Option<String> + 'a>,
    }

    impl<'a> Default for FakeHost<'a> {
        fn default() -> Self {
            Self {
                git: Box::new(|_, _| Err(GitOutputError { stdout: String::new(), message: "no git".into() })),
                stat: Box::new(|_| None),
                read: Box::new(|_| None),
                list: Box::new(|_| Err(io::Error::new(io::ErrorKind::NotFound, "no dir"))),
                now: 0,
                before: Box::new(|_| None),
            }
        }
    }

    impl DiskObservationHost for FakeHost<'_> {
        fn run_git(&self, args: &[&str], cwd: &Path) -> Result<String, GitOutputError> {
            (self.git)(args, cwd)
        }
        fn stat_of(&self, path: &Path) -> Option<FileStat> {
            (self.stat)(path)
        }
        fn read_text(&self, path: &Path) -> Option<String> {
            (self.read)(path)
        }
        fn read_text_up_to(&self, path: &Path, _max: usize) -> Option<String> {
            (self.read)(path)
        }
        fn list_directory(&self, dir: &Path) -> io::Result<Vec<FolderEntry>> {
            (self.list)(dir)
        }
        fn now_ms(&self) -> u64 {
            self.now
        }
        fn before_text(&self, rel: &str) -> Option<String> {
            (self.before)(rel)
        }
    }

    fn tool_event(ty: &str, name: &str, command: Option<&str>) -> Value {
        let mut payload = json!({
            "itemId": format!("i-{name}"),
            "toolKind": "tool",
            "name": name,
            "phase": if ty == "tool.started" { "started" } else { "completed" },
            "evidence": { "redacted": true }
        });
        if let Some(cmd) = command {
            payload["command"] = json!(cmd);
        }
        json!({
            "id": format!("e-{name}"),
            "runId": "run_1",
            "missionId": "mission_1",
            "sequence": 1,
            "type": ty,
            "occurredAt": "2026-09-05T00:00:00.000Z",
            "sourceAdapter": "opencode",
            "payload": payload
        })
    }

    #[test]
    fn parses_the_nul_separated_porcelain_form_including_a_rename_with_its_source() {
        let output = " M src/notes.ts\0?? scratch/new.md\0R  old.ts\0new.ts\0";
        let snap = parse_porcelain(output);
        let got: Vec<(&str, &str)> = snap.entries.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert_eq!(got, vec![("old.ts", "R "), ("scratch/new.md", "??"), ("src/notes.ts", " M")]);
    }

    #[test]
    fn looks_at_the_folder_itself_when_git_cannot_answer_and_is_nothing_when_that_cannot_be_read_either() {
        let host = FakeHost::default();
        assert!(snapshot_workspace(Path::new("/no-such-folder-0365"), &host).is_none());
    }

    #[test]
    fn asks_for_untracked_files_one_by_one() {
        let asked: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
        let host = FakeHost {
            git: Box::new(|args, _| {
                asked.borrow_mut().push(args.iter().map(|s| (*s).to_owned()).collect());
                Ok(String::new())
            }),
            ..Default::default()
        };
        snapshot_workspace(Path::new("C:/work/pebble"), &host);
        assert_eq!(
            asked.borrow()[0],
            vec!["status", "--porcelain", "-z", "--untracked-files=all"]
        );
    }

    #[test]
    fn names_a_file_that_became_dirty_one_that_changed_kind_and_one_that_was_cleaned() {
        let before = parse_porcelain("?? a.txt\0 M b.ts\0");
        let after = parse_porcelain("?? a.txt\0A  b.ts\0 M c.ts\0");
        assert_eq!(changed_paths(&before, &after), vec!["b.ts", "c.ts"]);
        assert_eq!(
            changed_paths(&parse_porcelain(" M z.ts\0"), &parse_porcelain("")),
            vec!["z.ts"]
        );
    }

    #[test]
    fn is_empty_when_nothing_moved() {
        let same = parse_porcelain(" M b.ts\0");
        assert!(changed_paths(&same, &same).is_empty());
    }

    #[test]
    fn keeps_a_changed_path_only_a_read_or_a_command_named_and_drops_one_an_edit_named() {
        let events = vec![
            tool_event("tool.started", "task", None),
            tool_event("tool.started", "read", Some("src/notes.ts")),
            tool_event("tool.started", "bash", Some("python3 -c \"import openpyxl; wb.save('monthly_budget.xlsx')\"")),
            tool_event("tool.started", "write", Some("src/written.ts")),
        ];
        assert_eq!(
            unreported_paths(
                &[
                    "src/notes.ts".into(),
                    "src/other.ts".into(),
                    "monthly_budget.xlsx".into(),
                    "src/written.ts".into(),
                ],
                &events
            ),
            vec!["src/notes.ts", "src/other.ts", "monthly_budget.xlsx"]
        );
    }

    #[test]
    fn counts_a_command_whose_own_words_are_an_edit_as_reporting_it() {
        assert!(unreported_paths(
            &["notes.md".into()],
            &[tool_event("tool.started", "bash", Some("sed -i 's/a/b/' notes.md"))]
        )
        .is_empty());
        assert_eq!(
            unreported_paths(
                &["notes.md".into()],
                &[tool_event("tool.started", "bash", Some("sed -n '1,4p' notes.md"))]
            ),
            vec!["notes.md"]
        );
    }

    #[test]
    fn matches_on_the_file_name_whatever_the_runtime_wrote_before_it_case_blind() {
        let events = vec![tool_event("tool.completed", "Edit", Some("C:\\Work\\Pebble\\SRC\\NOTES.TS"))];
        assert!(unreported_paths(&["src/notes.ts".into()], &events).is_empty());
    }

    #[test]
    fn is_a_started_completed_pair_per_path_contiguous_in_sequence_labelled_as_observed() {
        let events = observed_edit_events(
            "run_1",
            "mission_1",
            "opencode",
            12,
            &["src/notes.ts".into(), "README.md".into()],
            "2026-09-05T01:00:00.000Z",
            &BTreeMap::new(),
            &BTreeSet::new(),
        );
        let seqs: Vec<(u64, &str)> = events
            .iter()
            .map(|e| (e["sequence"].as_u64().unwrap(), e["type"].as_str().unwrap()))
            .collect();
        assert_eq!(seqs, vec![(12, "tool.started"), (13, "tool.completed"), (14, "tool.started"), (15, "tool.completed")]);
        assert_eq!(events[0]["sourceAdapter"], "opencode");
        assert_eq!(events[0]["id"], "run_1:disk:12");
        assert_eq!(events[0]["payload"]["itemId"], "disk-observed-1");
        assert_eq!(events[0]["payload"]["name"], "edit");
        assert_eq!(events[0]["payload"]["command"], "src/notes.ts");
        assert_eq!(events[0]["payload"]["status"], "observed on disk");
        assert_eq!(events[1]["payload"]["phase"], "completed");
        assert_eq!(events[3]["payload"]["itemId"], "disk-observed-2");
        assert_eq!(events[3]["payload"]["command"], "README.md");
    }

    #[test]
    fn adds_nothing_for_nothing() {
        assert!(observed_edit_events(
            "run_1",
            "mission_1",
            "codex",
            1,
            &[],
            "2026-09-05T01:00:00.000Z",
            &BTreeMap::new(),
            &BTreeSet::new(),
        )
        .is_empty());
    }

    #[test]
    fn turns_a_new_untracked_text_file_into_an_add_from_its_own_contents() {
        let after = WorkspaceSnapshot {
            entries: BTreeMap::from([("NOTES.md".into(), "??".into())]),
            ..Default::default()
        };
        let host = FakeHost {
            read: Box::new(|p| p.to_string_lossy().ends_with("NOTES.md").then(|| "# Notes\nFirst entry.\n".into())),
            git: Box::new(|_, _| panic!("git must not be asked about an untracked file")),
            ..Default::default()
        };
        let patches = observed_patches(Path::new("C:\\w"), &after, &["NOTES.md".into()], &host, None);
        let patch = &patches["NOTES.md"];
        assert_eq!(patch["added"], 2);
        assert_eq!(patch["removed"], 0);
        assert!(patch["text"].as_str().unwrap().contains("+# Notes"));
        assert!(patch["text"].as_str().unwrap().contains("+First entry."));
    }

    #[test]
    fn asks_git_for_a_tracked_files_diff_and_the_index_when_the_working_tree_is_clean() {
        let after = WorkspaceSnapshot {
            entries: BTreeMap::from([("src/app.ts".into(), " M".into()), ("staged.ts".into(), "M ".into())]),
            ..Default::default()
        };
        let asked: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
        let host = FakeHost {
            git: Box::new(|args, _| {
                asked.borrow_mut().push(args.iter().map(|s| (*s).to_owned()).collect());
                if args.contains(&"--cached") {
                    Ok(if args.contains(&"staged.ts") {
                        "diff --git a/staged.ts b/staged.ts\n--- a/staged.ts\n+++ b/staged.ts\n@@ -1 +1 @@\n-old\n+new\n".into()
                    } else {
                        String::new()
                    })
                } else {
                    Ok(if args.contains(&"src/app.ts") {
                        "diff --git a/src/app.ts b/src/app.ts\n--- a/src/app.ts\n+++ b/src/app.ts\n@@ -1,2 +1,3 @@\n a\n+b\n c\n".into()
                    } else {
                        String::new()
                    })
                }
            }),
            ..Default::default()
        };
        let patches = observed_patches(
            Path::new("C:\\w"),
            &after,
            &["src/app.ts".into(), "staged.ts".into()],
            &host,
            None,
        );
        assert_eq!(patches["src/app.ts"]["added"], 1);
        assert!(patches["staged.ts"]["text"].as_str().unwrap().contains("+new"));
        assert!(asked.borrow().iter().any(|a| a.contains(&"--cached".into()) && a.contains(&"staged.ts".into())));
    }

    #[test]
    fn gives_no_patch_for_a_file_it_cannot_read_as_text_and_the_row_keeps_its_path() {
        let after = WorkspaceSnapshot {
            entries: BTreeMap::from([("pic.png".into(), "??".into())]),
            ..Default::default()
        };
        let patches = observed_patches(Path::new("C:\\w"), &after, &["pic.png".into()], &FakeHost::default(), None);
        assert!(!patches.contains_key("pic.png"));
    }

    #[test]
    fn never_throws_a_git_that_fails_for_one_path_costs_only_that_path() {
        let after = WorkspaceSnapshot {
            entries: BTreeMap::from([("src/app.ts".into(), " M".into()), ("NOTES.md".into(), "??".into())]),
            ..Default::default()
        };
        let host = FakeHost {
            read: Box::new(|_| Some("x\n".into())),
            git: Box::new(|_, _| Err(GitOutputError { stdout: String::new(), message: "boom".into() })),
            ..Default::default()
        };
        let patches = observed_patches(
            Path::new("C:\\w"),
            &after,
            &["src/app.ts".into(), "NOTES.md".into()],
            &host,
            None,
        );
        assert!(!patches.contains_key("src/app.ts"));
        assert_eq!(patches["NOTES.md"]["added"], 1);
    }

    #[test]
    fn carries_the_patch_on_the_completed_event_and_says_when_the_runtime_had_named_the_path() {
        let patches = BTreeMap::from([(
            "NOTES.md".into(),
            json!({"text": "--- /dev/null\n+++ b/NOTES.md\n@@ -0,0 +1 @@\n+hi\n", "added": 1, "removed": 0, "truncated": false}),
        )]);
        let reported = BTreeSet::from(["NOTES.md".into(), "big.bin".into()]);
        let events = observed_edit_events(
            "run_1",
            "mission_1",
            "codex",
            10,
            &["NOTES.md".into(), "other.txt".into(), "big.bin".into()],
            "2026-09-05T00:00:00.000Z",
            &patches,
            &reported,
        );
        let completed: Vec<&Value> = events.iter().filter(|e| e["type"] == "tool.completed").map(|e| &e["payload"]).collect();
        assert_eq!(completed[0]["command"], "NOTES.md");
        assert_eq!(completed[0]["status"], "reported by the runtime, read from disk");
        assert_eq!(completed[0]["patch"]["added"], 1);
        assert_eq!(completed[1]["command"], "other.txt");
        assert_eq!(completed[1]["status"], "observed on disk");
        assert!(completed[1]["patch"].is_null());
        assert_eq!(completed[2]["command"], "big.bin");
        assert_eq!(completed[2]["status"], "reported by the runtime, changed on disk");
        assert!(completed[2]["patch"].is_null());
    }

    #[test]
    fn carries_its_text_on_the_snapshot_so_a_later_edit_is_a_change_the_snapshot_can_see() {
        let host = FakeHost {
            git: Box::new(|_, _| Ok("?? NOTES.md\0 M src/app.ts\0".into())),
            read: Box::new(|p| p.to_string_lossy().ends_with("NOTES.md").then(|| "# Notes\n".into())),
            ..Default::default()
        };
        let snap = snapshot_workspace(Path::new("C:\\w"), &host).unwrap();
        assert_eq!(status_of(snap.entries.get("NOTES.md").map(String::as_str)), Some("??"));
        assert_eq!(text_of(snap.entries.get("NOTES.md").map(String::as_str)), Some("# Notes\n"));
        assert_eq!(snap.entries.get("src/app.ts").map(String::as_str), Some(" M"));
        let mut later = snap.clone();
        later.entries.insert("NOTES.md".into(), "??\x01# Notes\nSecond entry.\n".into());
        assert_eq!(changed_paths(&snap, &later), vec!["NOTES.md"]);
        assert!(changed_paths(&snap, &snap).is_empty());
    }

    #[test]
    fn diffs_the_earlier_text_against_the_later_one_headed_with_the_path_a_person_knows() {
        let before = WorkspaceSnapshot {
            entries: BTreeMap::from([("NOTES.md".into(), "??\x01# Notes\nFirst entry.\n".into())]),
            ..Default::default()
        };
        let after = WorkspaceSnapshot {
            entries: BTreeMap::from([("NOTES.md".into(), "??\x01# Notes\nFirst entry.\nSecond entry.\n".into())]),
            ..Default::default()
        };
        let host = FakeHost {
            git: Box::new(|args, _| {
                assert!(args.contains(&"--no-index"));
                Err(GitOutputError {
                    stdout: "diff --git a/tmp/a b/tmp/b\nindex 1..2 100644\n--- a/tmp/a\n+++ b/tmp/b\n@@ -1,2 +1,3 @@\n # Notes\n First entry.\n+Second entry.\n".into(),
                    message: "exit 1".into(),
                })
            }),
            ..Default::default()
        };
        let patches = observed_patches(Path::new("C:\\w"), &after, &["NOTES.md".into()], &host, Some(&before));
        let patch = &patches["NOTES.md"];
        assert_eq!(patch["added"], 1);
        assert_eq!(patch["removed"], 0);
        assert!(patch["text"].as_str().unwrap().starts_with("--- a/NOTES.md\n+++ b/NOTES.md\n@@"));
        assert!(patch["text"].as_str().unwrap().contains("+Second entry."));
    }

    #[test]
    fn never_reads_a_file_past_the_cap_and_still_sees_it_change() {
        let reads: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let look = |size: u64| {
            let host = FakeHost {
                git: Box::new(|_, _| Ok("?? data.csv\0".into())),
                stat: Box::new(move |_| Some(FileStat { size, mtime_ms: 1000, ctime_ms: None })),
                read: Box::new(|p| {
                    reads.borrow_mut().push(p.to_string_lossy().into_owned());
                    Some("x".into())
                }),
                ..Default::default()
            };
            snapshot_workspace(Path::new("C:/w"), &host).unwrap()
        };
        let before = look(200 * 1024 * 1024);
        let after = look(200 * 1024 * 1024 + 10);
        assert!(reads.borrow().is_empty());
        assert_eq!(status_of(before.entries.get("data.csv").map(String::as_str)), Some("??"));
        assert!(text_of(before.entries.get("data.csv").map(String::as_str)).is_none());
        assert_eq!(changed_paths(&before, &after), vec!["data.csv"]);
    }

    #[test]
    fn carries_the_text_of_only_so_many_files_and_a_size_for_the_rest() {
        let status_all: String = (0..MAX_UNTRACKED_TEXTS + 50)
            .map(|n| format!("?? file{n}.txt\0"))
            .collect();
        let reads: RefCell<usize> = RefCell::new(0);
        let host = FakeHost {
            git: Box::new(move |_, _| Ok(status_all.clone())),
            stat: Box::new(|_| Some(FileStat { size: 10, mtime_ms: 1, ctime_ms: None })),
            read: Box::new(|_| {
                *reads.borrow_mut() += 1;
                Some("hello".into())
            }),
            ..Default::default()
        };
        let snap = snapshot_workspace(Path::new("C:/w"), &host).unwrap();
        assert_eq!(*reads.borrow(), MAX_UNTRACKED_TEXTS);
        assert_eq!(text_of(snap.entries.get("file0.txt").map(String::as_str)), Some("hello"));
        let beyond = format!("file{}.txt", MAX_UNTRACKED_TEXTS + 10);
        assert!(text_of(snap.entries.get(&beyond).map(String::as_str)).is_none());
        assert_eq!(status_of(snap.entries.get(&beyond).map(String::as_str)), Some("??"));
    }

    // ---- main/a-plain-folder-is-watched-too.test.ts ----

    #[test]
    fn is_looked_at_directly_and_a_file_a_command_made_is_a_change() {
        let root = TempDir::new("locust-plain-folder");
        fs::write(root.0.join("notes.md"), "# Notes\n").unwrap();
        fs::create_dir(root.0.join("reports")).unwrap();
        fs::write(root.0.join("reports/march.md"), "March\n").unwrap();
        let host = DefaultDiskHost::new();
        let before = snapshot_folder(&root.0, &host).unwrap();
        assert!(!before.partial);

        fs::write(root.0.join("monthly_budget.xlsx"), [0x50, 0x4b, 0x03, 0x04, 0, 0, 0, 1]).unwrap();
        fs::write(root.0.join("reports/march.md"), "March, revised\n").unwrap();
        let after = snapshot_folder(&root.0, &host).unwrap();

        let changed = changed_paths(&before, &after);
        assert_eq!(changed, vec!["monthly_budget.xlsx", "reports/march.md"]);
        let patches = observed_patches(&root.0, &after, &changed, &host, Some(&before));
        let rows = observed_edit_events(
            "r",
            "m",
            "opencode",
            1,
            &changed,
            "2026-09-26T12:00:00.000Z",
            &patches,
            &BTreeSet::new(),
        );
        let cmds: Vec<&str> = rows
            .iter()
            .filter(|r| r["type"] == "tool.completed")
            .map(|r| r["payload"]["command"].as_str().unwrap())
            .collect();
        assert_eq!(cmds, vec!["monthly_budget.xlsx", "reports/march.md"]);
        assert!(!patches.contains_key("monthly_budget.xlsx"));
        assert!(patches.contains_key("reports/march.md"));
    }

    #[test]
    fn never_walks_into_a_programs_own_folders_and_ignores_the_files_the_system_rewrites() {
        let root = TempDir::new("locust-plain-skip");
        for inside in ["node_modules", ".git", ".venv", ".locust", "__pycache__"] {
            fs::create_dir(root.0.join(inside)).unwrap();
            fs::write(root.0.join(inside).join("x.txt"), "x").unwrap();
        }
        for noise in ["desktop.ini", "Thumbs.db", ".DS_Store", "~$budget.xlsx"] {
            fs::write(root.0.join(noise), "x").unwrap();
        }
        fs::write(root.0.join("kept.txt"), "kept").unwrap();
        let snap = snapshot_folder(&root.0, &DefaultDiskHost::new()).unwrap();
        let keys: Vec<&str> = snap.entries.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["kept.txt"]);
    }

    #[test]
    fn says_when_it_stopped_short_and_then_reports_no_disappearances() {
        let root = TempDir::new("locust-plain-deep");
        let mut deep = root.0.clone();
        for level in 0..=MAX_FOLDER_DEPTH {
            deep = deep.join(format!("d{level}"));
            fs::create_dir(&deep).unwrap();
        }
        fs::write(deep.join("too-deep.txt"), "x").unwrap();
        fs::write(root.0.join("top.txt"), "x").unwrap();
        let before = snapshot_folder(&root.0, &DefaultDiskHost::new()).unwrap();
        assert!(before.partial);
        assert!(before.entries.contains_key("top.txt"));
        let mut after = before.clone();
        after.entries.remove("top.txt");
        assert!(changed_paths(&before, &after).is_empty());
    }

    #[test]
    fn is_nothing_when_the_folder_itself_cannot_be_read() {
        let missing = std::env::temp_dir().join("locust-no-such-folder-0365");
        assert!(snapshot_workspace(&missing, &DefaultDiskHost::new()).is_none());
    }

    #[test]
    fn never_follows_a_link_out_of_the_folder() {
        let host = FakeHost {
            list: Box::new(|dir| {
                if dir == Path::new("C:/work") {
                    Ok(vec![
                        FolderEntry { name: "elsewhere".into(), kind: EntryKind::Link },
                        FolderEntry { name: "real.txt".into(), kind: EntryKind::File },
                    ])
                } else {
                    Ok(vec![FolderEntry { name: "secret.txt".into(), kind: EntryKind::File }])
                }
            }),
            stat: Box::new(|_| Some(FileStat { size: 1, mtime_ms: 1, ctime_ms: None })),
            read: Box::new(|_| Some("x".into())),
            ..Default::default()
        };
        let listed = snapshot_folder(Path::new("C:/work"), &host).unwrap();
        let keys: Vec<&str> = listed.entries.keys().map(String::as_str).collect();
        assert_eq!(keys, vec!["real.txt"]);
    }

    // ---- main/a-second-edit-to-a-changed-file-is-seen.test.ts ----

    fn tracked_and_modified(mtime_ms: u64, size: u64) -> WorkspaceSnapshot {
        let host = FakeHost {
            git: Box::new(|_, _| Ok(" M notes.txt\0".into())),
            stat: Box::new(move |_| Some(FileStat { size, mtime_ms, ctime_ms: None })),
            ..Default::default()
        };
        snapshot_workspace(Path::new("C:/work"), &host).unwrap()
    }

    #[test]
    fn is_seen_as_changed_when_it_was_written_again() {
        let before = tracked_and_modified(1_000, 5);
        let after = tracked_and_modified(2_000, 6);
        assert_eq!(changed_paths(&before, &after), vec!["notes.txt"]);
    }

    #[test]
    fn is_not_seen_as_changed_when_nothing_touched_it() {
        let before = tracked_and_modified(1_000, 5);
        let after = tracked_and_modified(1_000, 5);
        assert!(changed_paths(&before, &after).is_empty());
    }

    // ---- main/a-large-file-in-a-plain-folder-shows-its-change.test.ts ----

    fn big_text(second: &str) -> String {
        let filler: String = (0..3000)
            .map(|at| format!("filler {at}, there only to make the file large.\n"))
            .collect();
        format!("line one\n{second}\nline three\n{filler}")
    }

    #[test]
    fn reads_as_one_line_changed_when_the_checkpoint_has_it_from_before() {
        let ws = TempDir::new("locust-plain-large");
        let original = big_text("line two");
        assert!(original.len() > MAX_OBSERVED_FILE_BYTES);
        fs::write(ws.0.join("notes.txt"), &original).unwrap();
        let host = DefaultDiskHost::new();
        let before = snapshot_folder(&ws.0, &host).unwrap();
        fs::write(ws.0.join("notes.txt"), big_text("line 2")).unwrap();
        let after = snapshot_folder(&ws.0, &host).unwrap();

        let without = observed_patches(&ws.0, &after, &["notes.txt".into()], &host, Some(&before));
        assert!(!without.contains_key("notes.txt"));

        let asked: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let lookup = |p: &str| {
            asked.borrow_mut().push(p.to_owned());
            Some(original.clone())
        };
        let with_ckpt = observed_patches(
            &ws.0,
            &after,
            &["notes.txt".into()],
            &DefaultDiskHost::with_before_text(&lookup),
            Some(&before),
        );
        assert_eq!(*asked.borrow(), vec!["notes.txt"]);
        assert_eq!(with_ckpt["notes.txt"]["added"], 1);
        assert_eq!(with_ckpt["notes.txt"]["removed"], 1);
    }

    #[test]
    fn is_read_back_from_a_real_checkpoint_as_it_was_after_the_file_changed() {
        let t = TempDir::new("locust-plain-ckpt");
        let ws = t.0.join("ws");
        fs::create_dir_all(&ws).unwrap();
        let store = Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
        let original = big_text("line two");
        fs::write(ws.join("notes.txt"), &original).unwrap();
        let turn = checkpoints::Turn::begin(&store, &ws);
        fs::write(ws.join("notes.txt"), big_text("line 2")).unwrap();
        turn.finish(&store, "turn_1").unwrap();
        assert_eq!(checkpoints::file_at(&store, &ws, "turn_1", "notes.txt").as_deref(), Some(original.as_str()));
        assert!(checkpoints::file_at(&store, &ws, "turn_1", "../outside.txt").is_none());
        assert!(checkpoints::file_at(&store, &ws, "HEAD", "notes.txt").is_none());
    }

    // ---- main/a-big-folder-does-not-invent-changes.test.ts ----

    const DAY: u64 = 24 * 60 * 60 * 1000;
    const STARTED: u64 = 1_791_500_400_000; // 2026-10-08T23:00:00Z

    fn many(count: usize) -> Vec<String> {
        (0..count).map(|i| format!("f{i:05}")).collect()
    }

    fn big_folder_host<'a>(names: &'a [String], times: &'a BTreeMap<String, u64>, now: u64) -> FakeHost<'a> {
        FakeHost {
            list: Box::new(move |dir| {
                if dir.to_string_lossy().ends_with("root") {
                    Ok(names
                        .iter()
                        .map(|n| FolderEntry { name: n.clone(), kind: EntryKind::File })
                        .collect())
                } else {
                    Ok(Vec::new())
                }
            }),
            stat: Box::new(move |abs| {
                let name = abs.file_name()?.to_string_lossy().into_owned();
                let at = times.get(&name).copied()?;
                Some(FileStat { size: 10, mtime_ms: at, ctime_ms: Some(at) })
            }),
            read: Box::new(|abs| {
                let name = abs.file_name()?.to_string_lossy();
                Some(format!("text of {name}"))
            }),
            now,
            ..Default::default()
        }
    }

    #[test]
    fn a_file_that_only_moved_into_view_is_not_a_change_one_written_during_the_run_is() {
        let mut names = many(MAX_UNTRACKED_LOOKED_AT + 3);
        let mut times: BTreeMap<String, u64> = names.iter().map(|n| (n.clone(), STARTED - 21 * DAY)).collect();
        let before = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED)).unwrap();
        assert!(before.partial);
        assert!(!before.entries.contains_key(&format!("f{MAX_UNTRACKED_LOOKED_AT:05}")));

        names.retain(|n| n != "f00010");
        names.push("zz-made-by-the-run".into());
        times.insert("zz-made-by-the-run".into(), STARTED + 30_000);
        let after = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED + 60_000)).unwrap();
        assert!(changed_paths(&before, &after).is_empty());

        names.retain(|n| n != "f00020");
        names.insert(5, "a-new-report".into());
        times.insert("a-new-report".into(), STARTED + 40_000);
        let later = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED + 90_000)).unwrap();
        assert_eq!(changed_paths(&before, &later), vec!["a-new-report"]);
    }

    #[test]
    fn a_file_that_crossed_the_text_bound_is_compared_by_its_time_not_by_how_it_was_looked_at() {
        let mut names = many(MAX_UNTRACKED_LOOKED_AT + 1);
        let mut times: BTreeMap<String, u64> = names.iter().map(|n| (n.clone(), STARTED - DAY)).collect();
        let before = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED)).unwrap();

        names.retain(|n| n != "f00001");
        let after = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED + 5_000)).unwrap();
        let crossed = format!("f{MAX_UNTRACKED_TEXTS:05}");
        assert!(changed_paths(&before, &after).is_empty());

        times.insert(crossed.clone(), STARTED + 2_000);
        let rewritten = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED + 6_000)).unwrap();
        assert_eq!(changed_paths(&before, &rewritten), vec![crossed]);
    }

    #[test]
    fn a_folder_looked_at_whole_still_reports_every_new_file_however_old_its_time() {
        let mut names = many(20);
        let mut times: BTreeMap<String, u64> = names.iter().map(|n| (n.clone(), STARTED - DAY)).collect();
        let before = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED)).unwrap();
        assert!(!before.partial);

        names.push("copied-in.xlsx".into());
        times.insert("copied-in.xlsx".into(), STARTED - 300 * DAY);
        let after = snapshot_folder(Path::new("C:/root"), &big_folder_host(&names, &times, STARTED + 1_000)).unwrap();
        assert_eq!(changed_paths(&before, &after), vec!["copied-in.xlsx"]);
    }

    #[test]
    fn a_file_written_a_moment_before_the_first_look_is_still_a_change() {
        // A clock that runs a little apart from the disk's must not hide a file the run wrote as it started.
        let before = WorkspaceSnapshot { partial: true, taken_at: Some(STARTED), ..Default::default() };
        let after = WorkspaceSnapshot {
            entries: [("just-now.txt", "??"), ("long-ago.txt", "??")]
                .into_iter()
                .map(|(p, s)| (p.to_owned(), s.to_owned()))
                .collect(),
            written_at: [("just-now.txt", STARTED - 1_000), ("long-ago.txt", STARTED - 3_000)]
                .into_iter()
                .map(|(p, t)| (p.to_owned(), t))
                .collect(),
            ..Default::default()
        };
        assert_eq!(changed_paths(&before, &after), vec!["just-now.txt"]);
    }

    #[test]
    fn never_reads_a_new_file_that_may_hold_secrets_and_still_sees_it_change() {
        let read_from: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let host = FakeHost {
            git: Box::new(|_, _| Ok("?? .env\0?? notes.txt\0".into())),
            stat: Box::new(|_| Some(FileStat { size: 12, mtime_ms: STARTED, ctime_ms: None })),
            read: Box::new(|abs| {
                let name = abs.file_name()?.to_string_lossy().into_owned();
                read_from.borrow_mut().push(name);
                Some("its words".into())
            }),
            ..Default::default()
        };
        let snapshot = snapshot_workspace(Path::new("C:/root"), &host).unwrap();
        assert_eq!(*read_from.borrow(), vec!["notes.txt"]);
        let secret = &snapshot.entries[".env"];
        assert!(!secret.contains("its words"), "{secret}");
        assert_ne!(secret, "??", "a size and time are kept, so a later write is seen");
    }

    #[test]
    fn carries_no_more_than_five_thousand_new_files_and_leaves_the_rest_bare() {
        let names = many(MAX_UNTRACKED_LOOKED_AT + 1);
        let listing: String = names.iter().map(|n| format!("?? {n}\0")).collect();
        let host = FakeHost {
            git: Box::new(move |_, _| Ok(listing.clone())),
            stat: Box::new(|_| Some(FileStat { size: 999_999_999, mtime_ms: STARTED, ctime_ms: None })),
            ..Default::default()
        };
        let snapshot = snapshot_workspace(Path::new("C:/root"), &host).unwrap();
        let carried = snapshot.entries.values().filter(|s| s.as_str() != "??").count();
        assert_eq!(carried, MAX_UNTRACKED_LOOKED_AT);
        assert_eq!(snapshot.entries[names.last().unwrap()], "??");
    }
}

/// A changed file that is a link is never read through: a link in the project can point at a key file anywhere, and its
/// text would land in the conversation's record (this port's rule; Electron's readFile follows it).
#[cfg(unix)]
#[test]
fn never_reads_a_changed_file_through_a_link() {
    use crate::disk_observation::{DefaultDiskHost, DiskObservationHost};
    let root = std::env::temp_dir().join(format!("locust-link-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("project")).unwrap();
    std::fs::write(root.join("private.txt"), "private words").unwrap();
    std::fs::write(root.join("project").join("plain.txt"), "plain words").unwrap();
    std::os::unix::fs::symlink(root.join("private.txt"), root.join("project").join("linked.txt")).unwrap();
    let host = DefaultDiskHost::new();
    assert_eq!(host.read_text(&root.join("project").join("plain.txt")).as_deref(), Some("plain words"));
    assert_eq!(host.read_text(&root.join("project").join("linked.txt")), None);
    let _ = std::fs::remove_dir_all(&root);
}
