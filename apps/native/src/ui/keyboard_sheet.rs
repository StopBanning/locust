//! Keyboard shortcut sheet (Area 11).

use super::*;
use crate::keyboard_shortcuts::{SHEET_TITLE, keys_on, lacking_note, shortcut_groups};

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
}

fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}

pub(super) fn open(s: &mut State) {
    s.keyboard_sheet.open = true;
}

pub(super) fn view(_s: &State, p: &Palette) -> El {
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "other"
    };
    let mut body = col().w_full().gap(10.0).padding(16.0);
    body = body.child(
        row()
            .w_full()
            .center_y()
            .child(
                text(SHEET_TITLE.to_owned())
                    .font(type_::TITLE)
                    .color(p.text)
                    .grow(1.0),
            )
            .child(button("Close", p).on_press(|s: &mut State| s.keyboard_sheet.open = false)),
    );

    for (group, rows) in shortcut_groups() {
        body = body.child(
            text(group.label().to_owned())
                .font(type_::TITLE)
                .color(p.text),
        );
        for shortcut in rows {
            let keys = keys_on(shortcut.keys, platform, shortcut.control_on_mac);
            let mut line = col().w_full().gap(2.0).padding(6.0);
            line = line.child(
                row()
                    .w_full()
                    .gap(12.0)
                    .child(text(keys).font(type_::LABEL).color(p.text).w(220.0).wrap())
                    .child(
                        text(shortcut.does.to_owned())
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .grow(1.0)
                            .wrap(),
                    ),
            );
            if let Some(note) = lacking_note(shortcut.native) {
                line = line.child(
                    row().w_full().child(col().w(232.0)).child(
                        text(note.to_owned())
                            .font(type_::LABEL)
                            .color(p.muted)
                            .grow(1.0)
                            .wrap(),
                    ),
                );
            }
            body = body.child(line);
        }
    }

    col()
        .key("keyboard-sheet")
        .w(720.0)
        .h(780.0)
        .radius(10.0)
        .fill(p.raised)
        .border(1.0, p.hairline)
        .child(
            col()
                .w_full()
                .h_full()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .child(body),
        )
}

pub(super) fn scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.keyboard_sheet.open = true;
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_lists_electron_groups() {
        assert_eq!(shortcut_groups().len(), 4);
        let mut s = State::default();
        scene(&mut s);
        assert!(s.keyboard_sheet.open);
    }
}
