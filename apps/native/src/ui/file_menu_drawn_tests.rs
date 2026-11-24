//! shared/file-menu.test.ts through the window: a file opened in the viewer beside the conversation (`viewer_on`, on
//! each system with its installed editors), the choices it draws, and what Copy path and Copy relative path put on the
//! clipboard when pressed. Labels and paths are Electron's, written out.
use super::*;
use rune::{Env, Runtime};

const FOLDER: &str = "C:/work/locust";

thread_local! {
    /// The system and the installed editors the viewer is drawn for, in this test's thread.
    static ON: std::cell::Cell<(&'static str, &'static [&'static str])> = const { std::cell::Cell::new(("linux", &[])) };
}

/// The viewer open on `path` in a conversation whose folder is `folder`, drawn on `platform` with `editors`.
fn viewer_open(path: &str, folder: &str, platform: &'static str, editors: &'static [&'static str]) -> Runtime<State> {
    ON.with(|on| on.set((platform, editors)));
    let mut s = State::default();
    s.teammates.push(Teammate { id: "tm".into(), name: "Wren".into(), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: Some(folder.into()) });
    s.conversations.insert("tm".into(), Default::default());
    s.file_view = Some((FileTarget { path: path.into(), start: 1, end: 1 }, Ok("notes\n".into())));
    let mut rt = Runtime::new(
        s,
        |s, _| {
            let (platform, editors) = ON.with(|on| on.get());
            viewer_on(s, &Palette::new(true), platform, editors).unwrap()
        },
        (2400.0, 900.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn choices<T>(rt: &Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("file-reader/menu/"))).filter_map(|e| e.label).collect()
}

fn ids<T>(rt: &Runtime<T>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.key).filter_map(|k| k.strip_prefix("file-reader/menu/").map(str::to_owned)).collect()
}

/// The choice labelled `label` pressed; what it put on the clipboard.
fn copied<T>(rt: &mut Runtime<T>, label: &str) -> String {
    let key = rt.elements().into_iter().find(|e| e.label.as_deref() == Some(label) && e.key.as_deref().is_some_and(|k| k.starts_with("file-reader/menu/"))).and_then(|e| e.key).unwrap_or_else(|| panic!("{label} is offered"));
    let (x, y) = rt.find(&key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    rt.copy_request.take().as_deref().map(str::to_owned).unwrap_or_else(|| panic!("{label} copied nothing"))
}

// case: offers the editors that are installed, and the default app only for a file it shows rather than runs
#[test]
fn offers_the_editors_that_are_installed_and_the_default_app_only_for_a_file_it_shows_rather_than_runs() {
    let rt = viewer_open("C:/work/locust/docs/plan.md", FOLDER, "win32", &["vscode"]);
    assert_eq!(choices(&rt), [
        "Add to my next message",
        "Open in VS Code",
        "Open in the default app",
        "Show in Explorer",
        "Copy path",
        "Copy relative path",
        "Copy file name",
    ]);
    let script = viewer_open("C:/work/locust/tools/run.ps1", FOLDER, "win32", &["cursor"]);
    assert!(!ids(&script).contains(&"Default".to_owned()), "{:?}", ids(&script));
    assert!(ids(&script).contains(&"Cursor".to_owned()), "{:?}", ids(&script));
    // A list of what may open, not of what may not: each of these runs when its default app opens it.
    for name in ["app.ts", "page.hta", "link.lnk", "tool.jar", "keys.reg", "site.url", "panel.cpl", "run.py", "run.sh", "run.command", "app.desktop", "NOEXT"] {
        let rt = viewer_open(&format!("C:/work/locust/{name}"), FOLDER, "win32", &[]);
        assert!(!choices(&rt).iter().any(|c| c == "Open in the default app"), "{name}: {:?}", choices(&rt));
    }
    for name in ["notes.md", "shot.PNG", "report.pdf", "index.html", "clip.mp4"] {
        let rt = viewer_open(&format!("C:/work/locust/{name}"), FOLDER, "win32", &[]);
        assert!(choices(&rt).iter().any(|c| c == "Open in the default app"), "{name}: {:?}", choices(&rt));
    }
}

// case: offers Copy only when the path is outside the conversation folder
#[test]
fn offers_copy_only_when_the_path_is_outside_the_conversation_folder() {
    let copies = ["Copy path", "Copy relative path", "Copy file name"];
    assert_eq!(choices(&viewer_open("D:/other/notes.md", FOLDER, "win32", &["vscode", "cursor"])), copies);
    // A walk back up and out is outside, however it is written.
    assert_eq!(choices(&viewer_open("C:/work/locust/../secret.md", FOLDER, "win32", &["vscode"])), copies);
    let mut inside = viewer_open("C:/work/locust/src/app.ts", FOLDER, "win32", &[]);
    assert_eq!(copied(&mut inside, "Copy relative path"), "src\\app.ts");
    // A name the teammate gave relative to the folder is the folder's file.
    let mut named = viewer_open("README.md", FOLDER, "win32", &[]);
    assert!(choices(&named).iter().any(|c| c == "Add to my next message"), "{:?}", choices(&named));
    assert_eq!(copied(&mut named, "Copy path"), "C:\\work\\locust\\README.md");
    assert_eq!(choices(&viewer_open("../secret.md", FOLDER, "win32", &["vscode"])), copies);
}

// case: names the file manager for the system
#[test]
fn names_the_file_manager_for_the_system() {
    assert!(choices(&viewer_open("/work/locust/a.ts", "/work/locust", "darwin", &[])).iter().any(|c| c == "Show in Finder"));
    assert!(choices(&viewer_open("/work/locust/a.ts", "/work/locust", "linux", &[])).iter().any(|c| c == "Show in the file manager"));
}

// case: is written with the system separator, never a mix of both on Windows
#[test]
fn is_written_with_the_system_separator_never_a_mix_of_both_on_windows() {
    let mut rooted = viewer_open("src/signup.ts", "C:\\work\\app", "win32", &[]);
    assert_eq!(copied(&mut rooted, "Copy path"), "C:\\work\\app\\src\\signup.ts");
    let mut relative = viewer_open("C:/work/app/src/signup.ts", "C:/work/app", "win32", &[]);
    assert_eq!(copied(&mut relative, "Copy relative path"), "src\\signup.ts");
    let mut linux = viewer_open("/work/app/src/signup.ts", "/work/app", "linux", &[]);
    assert_eq!(copied(&mut linux, "Copy path"), "/work/app/src/signup.ts");
}
