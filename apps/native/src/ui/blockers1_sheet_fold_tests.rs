use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
fn setup() -> (Temp, State) {
    let dir = Temp(std::env::temp_dir().join(format!("locust-sheet-fold-{}", locust_core::invariants::unique_nanos())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let mut s = State::default();
    crate::scenes::composer_base_scene(&mut s, false);
    let teammate = &mut s.teammates[s.selected];
    teammate.folder = Some(dir.0.to_string_lossy().into_owned());
    teammate.runtime = Some("opencode".into());
    (dir, s)
}
fn edit(path: &str, name: &str, status: &str, patch: Option<&str>) -> ledger::Item {
    ledger::Item::Tool {
        name: name.into(), summary: path.into(), status: ledger::Status::Done,
        detail: Box::new(ledger::ToolDetail {
            id: path.into(), finished: true, runtime_status: status.into(),
            observed_edit: status.contains("on disk"),
            patch: patch.map(|text| locust_core::activity_entries::Patch { text: text.into(), ..Default::default() }),
            ..Default::default()
        }),
    }
}
fn words(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

// case: previews a new CSV as its first rows
#[test]
fn previews_a_new_csv_as_its_first_rows() {
    let (_dir, s) = setup();
    const PATCH: &str = "--- /dev/null\n+++ b/budget.csv\n@@ -0,0 +1,3 @@\n+Category,Budgeted\n+Rent,\"1,400\"\n+Food,500";
    let mut rt = rune::Runtime::new(s, |s, _| {
        let item = edit("budget.csv", "edit", "", Some(PATCH));
        activity_item(&item, &Palette::new(false), s, "fold")
    },
        (1000.0, 1200.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    assert!(rt.find("activity-sheet-preview").is_some());
    let elements = rt.elements();
    assert!(elements.iter().any(|e| e.role == "cell" && e.key.as_deref() == Some("sheet-number/1") && e.text.as_deref() == Some("1,400")));
    assert!(elements.iter().any(|e| e.label.as_deref() == Some("Open budget.csv")));
    assert!(!words(&rt).contains("Rent,\"1,400\""));
}

// case: names a workbook a command made as a changed file, and opens it
#[test]
fn names_a_workbook_a_command_made_as_a_changed_file_and_opens_it() {
    let (dir, s) = setup();
    const PATH: &str = "monthly_budget.xlsx";
    std::fs::write(dir.0.join(PATH), include_bytes!("../../../desktop/test/workbooks/budget-openpyxl.xlsx")).unwrap();
    let turn = vec![edit(PATH, "observed_edit", "observed on disk", None)];
    assert_eq!(locust_core::activity_entries::changed_paths(&turn), [PATH.to_owned()]);
    let mut rt = rune::Runtime::new(s, |s, _| {
        if let Some(viewer) = crate::reply_actions::viewer_on(s, &Palette::new(false), "darwin", &[]) {
            viewer
        } else {
            let turn = [edit(PATH, "observed_edit", "observed on disk", None)];
            turn_foot_card(&turn, &Palette::new(false), s, "fold", None).unwrap()
        }
    }, (1200.0, 1800.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    assert!(words(&rt).contains("changed · seen on disk"));
    assert!(!words(&rt).contains("did not report"));
    let elements = rt.elements();
    assert!(elements.iter().any(|e| e.label.as_deref() == Some("Open monthly_budget.xlsx")));
    let (x, y) = rt.find("fold/seen/monthly_budget.xlsx/view").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.state.file_view.as_ref().is_some_and(|(target, body)| target.path == PATH && body.is_ok()));
    assert!(rt.find("sheet/grid").is_some());
}

// case: keeps the teammate's own word on a changed file the host could not read (0.597)
#[test]
fn keeps_the_teammates_own_word_when_the_host_cannot_read_the_changed_file() {
    let (_dir, s) = setup();
    let mut rt = rune::Runtime::new(s, |s, _| {
        let turn = [
            edit("report.md", "write_to_file", "reported by the runtime, changed on disk", None),
            edit("NOTES.md", "file_change", "reported by the runtime, changed on disk", None),
        ];
        turn_foot_card(&turn, &Palette::new(false), s, "fold", None).unwrap()
    },
        (1000.0, 1000.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    let said = words(&rt);
    assert!(said.contains("report.md\nWrite\nchanged · seen on disk"), "{said}");
    assert!(said.contains("NOTES.md\nchanged · seen on disk"), "{said}");
    assert!(!said.lines().any(|line| line == "changed") && !said.contains("did not report"), "{said}");
}

// case: still says a runtime did not report a change it did not see on disk
#[test]
fn still_says_a_runtime_did_not_report_a_change_it_did_not_see_on_disk() {
    let (_dir, s) = setup();
    let mut rt = rune::Runtime::new(s, |s, _| {
        let item = edit("notes.md", "edit", "", None);
        activity_item(&item, &Palette::new(false), s, "fold")
    },
        (900.0, 900.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    assert!(words(&rt).contains("OpenCode did not report the change"));
}
