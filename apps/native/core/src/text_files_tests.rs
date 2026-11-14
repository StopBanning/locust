//! Electron's `shared/text-files.test.ts`, one Rust test for each of its eight cases, in its order, with its values.
use super::*;

// describe('which files the viewer opens')

#[test]
fn opens_what_a_teammate_actually_writes() {
    for path in ["docs/report.md", "notes.txt", "src/index.ts", "data.json", "run.log"] {
        assert!(is_viewable_text(path), "{path}");
    }
}

#[test]
fn refuses_what_it_cannot_honestly_draw() {
    // An allowlist, not a binary sniff: guessing wrong renders a megabyte of machine code as mojibake in a panel.
    for path in ["photo.png", "app.exe", "archive.zip", "font.woff2", "video.mp4"] {
        assert!(!is_viewable_text(path), "{path}");
    }
}

#[test]
fn reads_a_dotfile_as_its_own_extension() {
    // `.gitignore` has no second dot, and treating the whole name as the extension is the only reading that opens it.
    assert_eq!(extension_of(".gitignore"), "gitignore");
    assert!(is_viewable_text(".gitignore"));
    assert_eq!(extension_of("notes"), "");
    assert!(!is_viewable_text("notes"));
}

#[test]
fn is_not_fooled_by_a_dot_in_a_folder_name() {
    assert_eq!(extension_of("my.folder/report.md"), "md");
    assert_eq!(extension_of("my.folder/README"), "");
}

#[test]
fn takes_a_windows_path_as_readily_as_a_posix_one() {
    assert_eq!(extension_of("docs\\report.md"), "md");
}

// describe('how it draws them')

#[test]
fn draws_markdown_as_prose_through_the_renderer_a_reply_already_uses() {
    assert_eq!(viewer_mode("docs/report.md"), ViewerMode::Markdown);
    assert_eq!(viewer_mode("README.markdown"), ViewerMode::Markdown);
}

#[test]
fn draws_everything_else_as_code() {
    // A `.ts` file rendered as Markdown would eat its own asterisks.
    assert_eq!(viewer_mode("src/index.ts"), ViewerMode::Code);
    assert_eq!(viewer_mode("run.log"), ViewerMode::Code);
    assert_eq!(viewer_mode("data.json"), ViewerMode::Code);
}

// describe('the bound')

#[test]
fn is_big_enough_for_anything_written_to_be_read_and_not_for_a_runaway_log() {
    assert_eq!(MAX_TEXT_BYTES, 262_144);
}

// What Electron's source does and its test is silent on. Values taken from running `shared/text-files.ts`, and the
// words and order from `main/index.ts` (the text channel) and `Thread.tsx` (a handed file's Copy).

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-text-files-{name}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn beyond_electron_the_extension_is_what_follows_the_last_dot_of_the_last_name() {
    for (path, extension) in [
        ("a.", ""), (".", ""), ("..", ""), (".env.local", "local"), ("dir/.", ""), ("x/.md", "md"), ("README.MD", "md"),
        ("dir\\.gitignore", "gitignore"), ("a/b/", ""), ("a/b.TXT/", ""), (".MD", "md"), ("a.b.c.JSON", "json"), ("", ""), ("/", ""),
        ("x.ts ", "ts "), (".ts", "ts"), ("..md", "md"), ("a/.git.ignore", "ignore"),
    ] {
        assert_eq!(extension_of(path), extension, "{path:?}");
    }
    // A name with a space after its extension is not that extension, and a dotfile with a second dot is its last part.
    assert!(!is_viewable_text("x.ts "));
    assert!(!is_viewable_text(".env.local"));
    assert!(is_viewable_text("a/.MD"));
    assert!(!is_viewable_text("a/.git.ignore"));
}

#[test]
fn beyond_electron_a_backslash_ends_a_folders_name_as_a_slash_does() {
    // Electron's own case, `docs\\report.md`, gives `md` whether or not the backslash is read as a separator (the last
    // dot is the same). A dot in a folder name is what needs it: `my.folder\\README` has no extension.
    assert_eq!(extension_of("my.folder\\README"), "");
    assert_eq!(extension_of("C:\\work.v2\\notes"), "");
    assert_eq!(extension_of("C:\\work.v2\\notes.TXT"), "txt");
    assert!(!is_viewable_text("my.md\\README"));
}

#[test]
fn beyond_electron_every_extension_on_the_list_opens_and_a_near_miss_does_not() {
    for extension in [
        "md", "markdown", "txt", "text", "log", "json", "jsonl", "yaml", "yml", "toml", "ini", "cfg", "conf", "env", "ts", "tsx", "js", "jsx",
        "mjs", "cjs", "py", "rb", "go", "rs", "java", "c", "h", "cpp", "hpp", "cs", "swift", "kt", "sh", "bash", "ps1", "bat", "css", "scss",
        "html", "htm", "xml", "svg", "sql", "graphql", "diff", "patch", "gitignore", "dockerignore", "editorconfig",
    ] {
        assert!(is_viewable_text(&format!("a.{extension}")), "{extension}");
    }
    assert_eq!(TEXT_EXTENSIONS.len(), 49);
    // The ones a person might expect and Electron's list leaves out (a spreadsheet and a Word file have a reader of
    // their own; the rest is not on the list).
    for extension in ["csv", "tsv", "docx", "pdf", "rtf", "zsh", "lock", "tsconfig", "vue", "php", "lua", "r", "ipynb"] {
        assert!(!is_viewable_text(&format!("a.{extension}")), "{extension}");
    }
}

#[test]
fn beyond_electron_a_file_over_the_bound_is_refused_whole_in_kilobytes_rounded() {
    // `Math.round(size / 1024)`: a half rounds up.
    assert_eq!(too_big_to_show(MAX_TEXT_BYTES + 1), "That file is 256KB, too big to show here. Save a copy and open it in an editor.");
    assert_eq!(too_big_to_show(300 * 1024 + 511), "That file is 300KB, too big to show here. Save a copy and open it in an editor.");
    assert_eq!(too_big_to_show(300 * 1024 + 512), "That file is 301KB, too big to show here. Save a copy and open it in an editor.");
    assert_eq!(too_big_to_show(5 * 1024 * 1024), "That file is 5120KB, too big to show here. Save a copy and open it in an editor.");
}

#[test]
fn beyond_electron_reading_a_file_judges_its_kind_first_then_the_place_then_the_size() {
    let dir = scratch("read");
    let note = dir.join("notes.md");
    std::fs::write(&note, "# Rollup\n").unwrap();
    assert_eq!(read_viewable_text("notes.md", &note).unwrap(), "# Rollup\n");
    // A kind not on the list is refused before the disk is touched, whether or not the file is there.
    assert_eq!(read_viewable_text("photo.png", &dir.join("photo.png")).unwrap_err(), "Locust does not open that kind of file here.");
    assert_eq!(read_viewable_text("notes", &dir.join("nowhere")).unwrap_err(), NOT_OPENED);
    // A file that is not there says the teammate named it and did not write it.
    assert_eq!(
        read_viewable_text("gone.md", &dir.join("gone.md")).unwrap_err(),
        "That file is not there. The teammate named it but did not write it."
    );
    // A folder with a listed name is a folder, not a file.
    let folder = dir.join("docs.md");
    std::fs::create_dir_all(&folder).unwrap();
    assert_eq!(read_viewable_text("docs.md", &folder).unwrap_err(), "That is a folder, not a file.");
    // A quarter of a megabyte is shown; one byte more is refused, never cut.
    let exact = dir.join("exact.log");
    std::fs::write(&exact, vec![b'x'; MAX_TEXT_BYTES as usize]).unwrap();
    assert_eq!(read_viewable_text("exact.log", &exact).unwrap().len(), MAX_TEXT_BYTES as usize);
    let over = dir.join("over.log");
    std::fs::write(&over, vec![b'x'; MAX_TEXT_BYTES as usize + 1]).unwrap();
    assert_eq!(
        read_viewable_text("over.log", &over).unwrap_err(),
        "That file is 256KB, too big to show here. Save a copy and open it in an editor."
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn beyond_electron_bytes_that_are_not_utf8_are_read_as_node_reads_them() {
    // `readFile(path, 'utf8')` puts U+FFFD where a byte is not text; it does not refuse.
    let dir = scratch("lossy");
    let file = dir.join("odd.txt");
    std::fs::write(&file, b"caf\xe9 ok").unwrap();
    assert_eq!(read_viewable_text("odd.txt", &file).unwrap(), "caf\u{fffd} ok");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn beyond_electron_a_handed_files_copy_refuses_a_sheet_or_a_document_and_reads_the_rest_as_the_viewer_does() {
    let dir = scratch("copy");
    for name in ["a.csv", "a.tsv", "a.xlsx", "a.docx", "a.pptx", "A.XLSX"] {
        let file = dir.join(name);
        std::fs::write(&file, "x").unwrap();
        assert_eq!(read_for_copy(name, &file).unwrap_err(), "Only a text file can be copied. Save a copy instead.", "{name}");
    }
    // A kind the viewer does not open says so in the viewer's words, not the Copy sentence.
    let pdf = dir.join("a.pdf");
    std::fs::write(&pdf, "x").unwrap();
    assert_eq!(read_for_copy("a.pdf", &pdf).unwrap_err(), "Locust does not open that kind of file here.");
    let text = dir.join("a.txt");
    std::fs::write(&text, "words").unwrap();
    assert_eq!(read_for_copy("a.txt", &text).unwrap(), "words");
    let _ = std::fs::remove_dir_all(&dir);
}
