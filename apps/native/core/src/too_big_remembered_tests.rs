//! a-folder-too-big-to-keep-is-not-relisted-every-turn.test.ts (2 cases). Fixed 20 Nov: Rust listed and statted a
//! folder past Undo's limits on every turn to say the same thing; now the answer is believed for ten minutes, for
//! that folder alone. A believed refusal is shown by the folder shrinking: inside the ten minutes it is still
//! refused (nothing was listed again), after them it is kept.
use super::tests::setup;
use super::*;

fn grow_past_the_limit(ws: &Path) {
    let many = ws.join("many");
    fs::create_dir_all(&many).unwrap();
    for at in 0..20_001 {
        fs::write(many.join(format!("file-{at}.txt")), b"").unwrap();
    }
}

// case: is listed once, believed for ten minutes, then listed again
#[test]
fn listed_once_believed_for_ten_minutes_then_listed_again() {
    let (_t, s, w) = setup();
    grow_past_the_limit(&w);
    let start = std::time::Instant::now();
    let first = take_with_git_at(&s, &w, false, start).unwrap_err().to_string();
    assert!(first.contains("too many to keep"), "{first}");
    fs::remove_dir_all(w.join("many")).unwrap();
    let a_minute_on = take_with_git_at(&s, &w, false, start + std::time::Duration::from_secs(60)).unwrap_err();
    assert_eq!(a_minute_on.to_string(), first, "believed, not listed again");
    assert_eq!(TOO_BIG_REMEMBERED, std::time::Duration::from_secs(600));
    let later = take_with_git_at(&s, &w, false, start + std::time::Duration::from_secs(61) + TOO_BIG_REMEMBERED);
    assert_eq!(later.expect("listed again, and small now").files.len(), 2);
}

// case: is remembered for that folder only
#[test]
fn remembered_for_that_folder_only() {
    let (_t, s, huge) = setup();
    grow_past_the_limit(&huge);
    let now = std::time::Instant::now();
    assert!(take_with_git_at(&s, &huge, false, now).is_err());
    let small = huge.parent().unwrap().join("small");
    fs::create_dir_all(&small).unwrap();
    fs::write(small.join("notes"), b"one\n").unwrap();
    assert_eq!(take_with_git_at(&s, &small, false, now).unwrap().files.len(), 1);
    assert_eq!(take_with_git_at(&s, &small, false, now).unwrap().files.len(), 1);
}

/// The 300 MB refusal is remembered through the same `too_big`; a test folder that heavy is not made here.
#[test]
fn a_folder_too_heavy_is_believed_too() {
    let (_t, s, w) = setup();
    let now = std::time::Instant::now();
    let dir = base(&s, &w).unwrap();
    let why = too_big(&dir, "This folder holds more than 300 MB of files, too much to keep a copy of before each turn.".into(), now).to_string();
    assert_eq!(take_with_git_at(&s, &w, false, now).unwrap_err().to_string(), why);
}

#[test]
fn a_folder_listed_by_git_is_believed_too() {
    let (_t, s, w) = setup();
    grow_past_the_limit(&w);
    let now = std::time::Instant::now();
    let first = take_with_git_at(&s, &w, true, now).unwrap_err().to_string();
    assert!(first.contains("too many to keep"), "{first}");
    fs::remove_dir_all(w.join("many")).unwrap();
    assert_eq!(take_with_git_at(&s, &w, true, now).unwrap_err().to_string(), first);
}
