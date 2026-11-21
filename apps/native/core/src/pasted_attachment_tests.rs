//! Electron's `main/pasted-attachment.test.ts` (4 cases): a name that arrived from the clipboard, or from anywhere a
//! window can send one, still lands inside the one folder Locust owns. Fixed 21 Nov: Rust's copy into a run folder
//! named a file by whatever its path ended in, `.` and `..` included, and never refused a separator or a colon.
use super::*;
use std::collections::HashSet;

fn none() -> HashSet<String> {
    HashSet::new()
}

// case: lands inside the one folder Locust owns
#[test]
fn lands_inside_the_one_folder_locust_owns() {
    assert_eq!(attachment_destination("shot.png", &none()).as_deref(), Ok(".locust/attachments/shot.png"));
}

// case: cannot climb out of it, however it is spelled
#[test]
fn cannot_climb_out_of_it_however_it_is_spelled() {
    let sep = '\\';
    let hostile = [
        "..".to_owned(),
        ".".to_owned(),
        String::new(),
        format!("..{sep}..{sep}settings.json"),
        "../../settings.json".to_owned(),
        format!("C:{sep}Windows{sep}system32{sep}drivers{sep}etc{sep}hosts"),
        "/etc/passwd".to_owned(),
        // On Windows this names an alternate data stream rather than a file.
        "notes.txt:stream".to_owned(),
        format!("{sep}{sep}server{sep}share{sep}thing.txt"),
    ];
    let depth = ATTACHMENT_DIR.split('/').count() + 1;
    for name in hostile {
        // Refused outright is fine; what must never happen is a destination that leaves the folder.
        let Ok(destination) = attachment_destination(&name, &none()) else { continue };
        assert!(destination.starts_with(&format!("{ATTACHMENT_DIR}/")), "{name} -> {destination}");
        assert!(!destination.split('/').any(|part| part == ".."), "{name} -> {destination}");
        assert_eq!(destination.split('/').count(), depth, "{name} -> {destination}");
    }
    // The ones that cannot be a name at all are refused, in Electron's words.
    assert_eq!(attachment_destination("..", &none()), Err("That file cannot be attached: \"..\" is not a file name.".into()));
    assert!(attachment_destination("", &none()).is_err());
    assert!(attachment_destination("notes.txt:stream", &none()).is_err());
    // A plain path reduces to its name, which lands in the folder like any other file.
    assert_eq!(attachment_destination("/etc/passwd", &none()).as_deref(), Ok(".locust/attachments/passwd"));
}

// case: never overwrites something already attached
#[test]
fn never_overwrites_something_already_attached() {
    // Two screenshots pasted in a row are both called image.png.
    let taken: HashSet<String> = ["image.png", "image-2.png"].map(str::to_owned).into();
    assert_eq!(attachment_destination("image.png", &taken).as_deref(), Ok(".locust/attachments/image-3.png"));
}

// case: keeps the extension where there is one, and copes where there is not
#[test]
fn keeps_the_extension_where_there_is_one_and_copes_where_there_is_not() {
    let taken: HashSet<String> = ["report.tar.gz", "LICENSE"].map(str::to_owned).into();
    assert_eq!(attachment_destination("report.tar.gz", &taken).as_deref(), Ok(".locust/attachments/report.tar-2.gz"));
    assert_eq!(attachment_destination("LICENSE", &taken).as_deref(), Ok(".locust/attachments/LICENSE-2"));
}
