//! Electron's `main/attention.test.ts`, with its exact values: what earns a notification, its words, and a room's
//! changes gathered into one toast per two minutes. The window's side (shown only while it is elsewhere, a click opening
//! the conversation) is in the native app's `attention_watch_tests` and `room_toast_tests`.
use super::*;
use std::time::{Duration, Instant};

fn at(start: Instant, ms: u64) -> Instant {
    start + Duration::from_millis(ms)
}

// case: when an approval earns a notification > only when the window is not in front, and only where the OS can show one
#[test]
fn only_when_the_window_is_not_in_front_and_only_where_the_os_can_show_one() {
    assert!(should_notify(false, true));
    assert!(!should_notify(true, true));
    assert!(!should_notify(false, false));
}

// case: when an approval earns a notification > names the teammate and the action, never the exact command
#[test]
fn names_the_teammate_and_the_action_never_the_exact_command() {
    let (title, body) = approval_text(false, "Run a command", Some("Wren"), None);
    assert_eq!(title, "Wren needs your approval");
    assert_eq!(body, "Run a command · the run is paused until you answer.");
    assert_eq!(approval_text(true, "Answer a question", None, None).0, "A teammate has a question");
}

// case: when a room changes while the person is elsewhere > says one change by the room’s name, after the window, not before
#[test]
fn says_one_change_by_the_rooms_name_after_the_window_not_before() {
    let start = Instant::now();
    let mut rooms = RoomToasts::default();
    rooms.changed("room_r", "Release", "Wren finished \"Write the notes\".", start);
    assert!(rooms.due(start).is_empty());
    assert!(rooms.due(at(start, ROOM_TOAST_WINDOW_MS - 1)).is_empty());
    assert_eq!(rooms.due(at(start, ROOM_TOAST_WINDOW_MS)), vec![("Release".to_owned(), "Wren finished \"Write the notes\".".to_owned())]);
    // Said once, then let go.
    assert!(rooms.due(at(start, 10 * ROOM_TOAST_WINDOW_MS)).is_empty());
    assert!(rooms.pending().is_empty());
}

// case: when a room changes while the person is elsewhere > gathers a burst into ONE toast: three teammates finishing together are one thing to read
#[test]
fn gathers_a_burst_into_one_toast() {
    let start = Instant::now();
    let mut rooms = RoomToasts::default();
    let mut now = 0;
    let mut shown = Vec::new();
    for who in ["Wren", "Booty", "Atlas"] {
        rooms.changed("room_r", "Release", &format!("{who} took on \"Ship it\"."), at(start, now));
        now += 1_000;
        shown.extend(rooms.due(at(start, now)));
    }
    shown.extend(rooms.due(at(start, now + ROOM_TOAST_WINDOW_MS)));
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].0, "Release · 3 changes");
    assert_eq!(shown[0].1.split('\n').count(), 3);
    // The window runs from the room's FIRST change: a later one does not push it back.
    let mut rooms = RoomToasts::default();
    rooms.changed("room_r", "Release", "a", start);
    rooms.changed("room_r", "Release", "b", at(start, ROOM_TOAST_WINDOW_MS - 1));
    assert_eq!(rooms.pending(), vec![("room_r".to_owned(), vec!["a".to_owned(), "b".to_owned()])]);
    assert_eq!(rooms.due(at(start, ROOM_TOAST_WINDOW_MS)).len(), 1);
}

// case: when a room changes while the person is elsewhere > keeps a long burst readable: the newest lines and a count of the rest
#[test]
fn keeps_a_long_burst_readable_the_newest_lines_and_a_count_of_the_rest() {
    let changes: Vec<String> = (1..=12).map(|i| format!("change {i}")).collect();
    let (title, body) = room_text("Release", &changes);
    assert_eq!(title, "Release · 12 changes");
    let lines: Vec<&str> = body.split('\n').collect();
    assert_eq!(lines.len(), ROOM_TOAST_LINES + 1);
    assert_eq!(lines[0], "… and 9 more");
    assert_eq!(*lines.last().unwrap(), "change 12");
    assert_eq!(lines[1], "change 10");
    // Exactly as many as it shows: no count line.
    let three: Vec<String> = (1..=3).map(|i| format!("change {i}")).collect();
    assert_eq!(room_text("Release", &three).1, "change 1\nchange 2\nchange 3");
    // Two is already "changes"; one is the room's name alone.
    assert_eq!(room_text("Release", &three[..2]).0, "Release · 2 changes");
    assert_eq!(room_text("Release", &three[..1]), ("Release".to_owned(), "change 1".to_owned()));
}

// case: when a room changes while the person is elsewhere > MEASURED: three teammates unattended for ten minutes are a readable set of toasts, not a storm
#[test]
fn measured_three_teammates_unattended_for_ten_minutes_are_a_readable_set_of_toasts_not_a_storm() {
    let start = Instant::now();
    let mut rooms = RoomToasts::default();
    let mut shown = Vec::new();
    let mut changes = 0;
    for second in 0..600u64 {
        for (index, who) in ["Wren", "Booty", "Atlas"].into_iter().enumerate() {
            if (second + index as u64 * 30) % 90 == 0 {
                rooms.changed("room_r", "Release", &format!("{who} moved the board ({second}s)."), at(start, second * 1_000));
                changes += 1;
            }
        }
        shown.extend(rooms.due(at(start, (second + 1) * 1_000)));
    }
    shown.extend(rooms.due(at(start, 600_000 + ROOM_TOAST_WINDOW_MS)));
    assert_eq!(changes, 20);
    assert!(shown.len() as u64 <= 600 / (ROOM_TOAST_WINDOW_MS / 1_000));
    assert!(shown.len() <= 5, "{}", shown.len());
    assert!(shown.len() < changes);
    for (_, body) in &shown {
        assert!(body.split('\n').count() <= ROOM_TOAST_LINES + 1);
    }
    // Every change was said somewhere, none twice.
    let said: usize = shown
        .iter()
        .map(|(title, _)| title.split(" · ").nth(1).and_then(|n| n.strip_suffix(" changes")).map_or(1, |n| n.parse().unwrap()))
        .sum();
    assert_eq!(said, changes);
}

// case: when a room changes while the person is elsewhere > keeps rooms apart
#[test]
fn keeps_rooms_apart() {
    let start = Instant::now();
    let mut rooms = RoomToasts::default();
    rooms.changed("room_a", "Alpha", "a", start);
    rooms.changed("room_b", "Beta", "b", start);
    let mut titles: Vec<String> = rooms.due(at(start, ROOM_TOAST_WINDOW_MS)).into_iter().map(|(t, _)| t).collect();
    titles.sort();
    assert_eq!(titles, ["Alpha", "Beta"]);
}

// case: the two other ways a run ends waiting for a person > a question card is said as the teammate asking, with the question and that the run waits
#[test]
fn a_question_card_is_said_as_the_teammate_asking_with_the_question_and_that_the_run_waits() {
    assert_eq!(
        decision_text(Some("Wren"), "Which  database?", None),
        ("Wren is asking you something".to_owned(), "Which database? · the run waits for your answer.".to_owned())
    );
    assert!(decision_text(None, &"x".repeat(200), None).1.chars().count() < 180);
}

// case: the two other ways a run ends waiting for a person > a limit is said with the runtime and its own words, or a plain line when it had none
#[test]
fn a_limit_is_said_with_the_runtime_and_its_own_words_or_a_plain_line_when_it_had_none() {
    assert_eq!(
        limit_text(Some("Booty"), "Claude Code", Some("You have hit your weekly limit.")),
        ("Booty hit the Claude Code limit".to_owned(), "You have hit your weekly limit. · pick where it continues.".to_owned())
    );
    assert_eq!(limit_text(None, "Codex CLI", None).1, "The run stopped at a checkpoint · pick where it continues.");
}

// case: a finished run, told once > gathers finishes that land together into one toast, and a click opens the newest
#[test]
fn gathers_finishes_that_land_together_into_one_toast() {
    let finishes = vec![
        ("Wren finished".to_owned(), "Moved the port. · took 4 min".to_owned()),
        ("Juno stopped".to_owned(), "Not signed in. · after 2 min".to_owned()),
    ];
    assert_eq!(
        finished_text(&finishes),
        ("2 teammates are done".to_owned(), "Wren finished: Moved the port. · took 4 min\nJuno stopped: Not signed in. · after 2 min".to_owned())
    );
}

// case: a finished run, told once > one finish is said as itself
#[test]
fn one_finish_is_said_as_itself() {
    let one = vec![("Wren finished".to_owned(), "Done. · took 3 min".to_owned())];
    assert_eq!(finished_text(&one), one[0]);
}

// case: a finished run, told once > believes only a bounded title and body from the renderer
#[test]
fn believes_only_a_bounded_title_and_body() {
    // There is no renderer here: the toast is built from the record in one process. Its bounds are finishFrom's: a title
    // of one line and 120 characters at most, whatever the teammate is called, and a body well inside 400.
    let name = format!("Wren\n{}", "t".repeat(500));
    let (title, body) = finished_toast(true, false, true, 120_000, Some(&name), Some(&"b".repeat(900)), None).unwrap();
    assert_eq!(title.chars().count(), 120);
    assert!(title.starts_with("Wren t") && !title.contains('\n'), "{title}");
    assert!(body.chars().count() <= 400, "{}", body.chars().count());
    let (title, body) = finished_toast(false, true, true, 120_000, Some(&name), None, Some(&"e".repeat(900))).unwrap();
    assert_eq!(title.chars().count(), 120);
    assert!(body.chars().count() <= 400);
    assert_eq!(finished_toast(true, false, true, 120_000, Some("Wren"), Some("Done."), None).unwrap().0, "Wren finished");
}
