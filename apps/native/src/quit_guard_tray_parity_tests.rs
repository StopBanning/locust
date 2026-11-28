use super::*;

#[derive(Default)]
struct RecordingTray {
    line: String,
    labels: Vec<String>,
}

impl TrayHost for RecordingTray {
    fn set_menu(&mut self, items: &[MenuItem]) {
        self.labels = items.iter().map(|item| item.label.clone()).collect();
    }

    fn set_tooltip(&mut self, line: &str) {
        self.line = line.to_owned();
    }
}

// case: the tray says who is working while the window is closed, and when nobody is
#[test]
fn the_tray_updates_live_names_even_when_the_routine_menu_does_not_change() {
    let mut tray = MenuBar::new(RecordingTray::default());
    for (i, (names, count, label)) in [
        (&["Wren"][..], 1, "Locust: Wren is working."),
        (
            &["Wren", "Sable"][..],
            2,
            "Locust: Wren and Sable are working.",
        ),
        (&[][..], 3, "Locust: 3 teammates are working."),
        (&[][..], 0, "Locust: nobody is working. Click to open."),
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            tray.say(names, count, "Next: Morning digest at 08:00", None),
            i == 0
        );
        assert_eq!(tray.host.line, label);
        assert_eq!(
            tray.host.labels,
            ["Open Locust", "Next: Morning digest at 08:00", "Quit"]
        );
    }
}
