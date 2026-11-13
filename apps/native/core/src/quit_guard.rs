//! Closing the window while a teammate works asks first (`main/quit-guard.ts`).

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseMoment {
    pub live_runs: usize,
    pub app_quitting: bool,
    pub session_ending: bool,
    pub confirmed: bool,
}

pub fn should_ask_before_closing(moment: &CloseMoment) -> bool {
    moment.live_runs > 0 && !moment.app_quitting && !moment.session_ending && !moment.confirmed
}

pub fn hide_to_tray_on_close(moment: &CloseMoment, keep_running: bool) -> bool {
    keep_running && !moment.app_quitting && !moment.session_ending && !moment.confirmed
}

fn unique_names<'a>(working_names: &'a [&'a str]) -> Vec<&'a str> {
    let mut seen = Vec::new();
    for &name in working_names {
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    seen
}

fn who_is_working(working_names: &[&str], live_runs: usize) -> String {
    let names = unique_names(working_names);
    match names.len() {
        0 => {
            if live_runs == 1 {
                "A teammate is".into()
            } else {
                format!("{live_runs} teammates are")
            }
        }
        1 => format!("{} is", names[0]),
        2 => format!("{} and {} are", names[0], names[1]),
        n => format!("{} and {} are", names[..n - 1].join(", "), names[n - 1]),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseQuestion {
    pub message: String,
    pub detail: String,
}

pub fn close_question(working_names: &[&str], live_runs: usize) -> CloseQuestion {
    let names = unique_names(working_names);
    let single = names.len() == 1 || (names.is_empty() && live_runs == 1);
    let them = if single {
        "the run stops"
    } else {
        "their runs stop"
    };
    CloseQuestion {
        message: format!("{} still working.", who_is_working(working_names, live_runs)),
        detail: format!(
            "Keep them working and Locust closes to the tray until you open it again, with a notification when they finish. If you quit now, {them}; you can resume from the conversation when you're back."
        ),
    }
}

pub const CLOSE_BUTTONS: [&str; 3] = [
    "Keep working in the background",
    "Quit anyway",
    "Cancel",
];

pub fn tray_menu_labels(next_due: &str) -> [String; 3] {
    ["Open Locust".into(), next_due.into(), "Quit".into()]
}

pub fn tray_line(working_names: &[&str], live_runs: usize) -> String {
    if live_runs == 0 {
        return "Locust: nobody is working. Click to open.".into();
    }
    let subject = who_is_working(working_names, live_runs);
    let trimmed = if let Some(prefix) = subject.strip_suffix(" is") {
        format!("{prefix} is working")
    } else if let Some(prefix) = subject.strip_suffix(" are") {
        format!("{prefix} are working")
    } else {
        subject
    };
    format!("Locust: {trimmed}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moment() -> CloseMoment {
        CloseMoment {
            live_runs: 1,
            app_quitting: false,
            session_ending: false,
            confirmed: false,
        }
    }

    #[test]
    fn asks_only_when_someone_is_working_and_person_is_closing() {
        assert!(should_ask_before_closing(&moment()));
        assert!(!should_ask_before_closing(&CloseMoment {
            live_runs: 0,
            ..moment()
        }));
    }

    #[test]
    fn never_stands_in_way_of_quit_or_session_end_or_confirmed() {
        assert!(!should_ask_before_closing(&CloseMoment {
            app_quitting: true,
            ..moment()
        }));
        assert!(!should_ask_before_closing(&CloseMoment {
            session_ending: true,
            ..moment()
        }));
        assert!(!should_ask_before_closing(&CloseMoment {
            confirmed: true,
            ..moment()
        }));
    }

    #[test]
    fn names_who_is_working_and_what_each_answer_does() {
        assert_eq!(
            close_question(&["Wren"], 1),
            CloseQuestion {
                message: "Wren is still working.".into(),
                detail: "Keep them working and Locust closes to the tray until you open it again, with a notification when they finish. If you quit now, the run stops; you can resume from the conversation when you're back.".into(),
            }
        );
        assert_eq!(
            close_question(&["Wren", "Sable"], 2).message,
            "Wren and Sable are still working."
        );
        assert_eq!(
            close_question(&["Wren", "Sable", "Juno"], 3).message,
            "Wren, Sable and Juno are still working."
        );
        assert_eq!(
            close_question(&["Wren", "Wren"], 2).message,
            "Wren is still working."
        );
        let unnamed = close_question(&[], 2);
        assert_eq!(unnamed.message, "2 teammates are still working.");
        assert!(unnamed.detail.contains("their runs stop"));
    }

    #[test]
    fn close_buttons_and_tray_behavior_match_electron() {
        assert_eq!(
            CLOSE_BUTTONS,
            ["Keep working in the background", "Quit anyway", "Cancel"]
        );
        assert!(hide_to_tray_on_close(&moment(), true));
        assert!(hide_to_tray_on_close(
            &CloseMoment {
                live_runs: 0,
                ..moment()
            },
            true
        ));
        assert!(!hide_to_tray_on_close(&moment(), false));
        assert!(!hide_to_tray_on_close(
            &CloseMoment {
                app_quitting: true,
                ..moment()
            },
            true
        ));
        assert_eq!(
            tray_menu_labels("Next: Morning digest at 08:00"),
            ["Open Locust", "Next: Morning digest at 08:00", "Quit"]
        );
        assert_eq!(tray_line(&["Wren"], 1), "Locust: Wren is working.");
        assert_eq!(
            tray_line(&["Wren", "Sable"], 2),
            "Locust: Wren and Sable are working."
        );
        assert_eq!(tray_line(&[], 3), "Locust: 3 teammates are working.");
        assert_eq!(
            tray_line(&[], 0),
            "Locust: nobody is working. Click to open."
        );
    }
}
