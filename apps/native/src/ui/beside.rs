//! Beside conversation panel — port of `BesideConversation.tsx`.

use super::*;

/// Words copied from Electron `BesideConversation.tsx`.
pub const BESIDE_OPEN_HERE: &str = "Open it here";
/// Electron `title` on the open-here control (tooltip); kept for exact-word tests.
#[allow(dead_code)]
pub const BESIDE_OPEN_HERE_TITLE: &str = "Open it here, to answer it";
pub const BESIDE_CLOSE: &str = "Close";
pub const BESIDE_FALLBACK_NAME: &str = "Conversation";

pub fn beside_aria_label(title: &str) -> String {
    format!("Beside: {title}")
}

pub(super) fn beside_panel(s: &State, p: &Palette) -> Option<El> {
    let beside = s.beside.as_ref()?;
    let name = beside
        .owner_name
        .clone()
        .unwrap_or_else(|| BESIDE_FALLBACK_NAME.into());
    let title = beside.title.clone();
    let aria = beside_aria_label(&title);

    let mut thread = col().w_full().gap(8.0).padding(12.0);
    if !beside.prompt.is_empty() {
        thread = thread.child(
            col()
                .w_full()
                .padding(12.0)
                .radius(8.0)
                .fill(p.your_bubble)
                .child(
                    text(beside.prompt.clone())
                        .font(type_::BODY)
                        .color(p.text)
                        .w_full()
                        .wrap(),
                ),
        );
    }
    for line in &beside.reply_lines {
        thread = thread.child(
            text(line.clone())
                .font(type_::BODY)
                .color(p.text)
                .w_full()
                .wrap(),
        );
    }

    Some(
        col()
            .key("beside-panel")
            .w(320.0)
            .h_full()
            .fill(p.sidebar)
            .border_sides([0.0, 0.0, 0.0, 1.0], p.hairline)
            .child(
                row()
                    .w_full()
                    .center_y()
                    .gap(8.0)
                    .px(12.0)
                    .py(10.0)
                    .child(
                        col()
                            .grow(1.0)
                            .shrink()
                            .gap(2.0)
                            .child(text(name).font(type_::TITLE).color(p.text).truncate())
                            .child(
                                text(title.clone())
                                    .font(type_::LABEL)
                                    .color(p.muted)
                                    .truncate(),
                            ),
                    )
                    .child(
                        text(BESIDE_OPEN_HERE)
                            .key("beside-open-here")
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .on_press_tx(|s: &mut State, tx| {
                                open_beside_here(s, tx);
                            }),
                    )
                    .child(
                        text(BESIDE_CLOSE)
                            .key("beside-close")
                            .font(type_::LABEL)
                            .color(p.muted)
                            .on_press(|s: &mut State| {
                                s.beside = None;
                            }),
                    ),
            )
            .child(
                text(aria)
                    .key("beside-aria")
                    .font(type_::LABEL)
                    .color(p.muted)
                    .w_full()
                    .px(12.0)
                    .truncate(),
            )
            .child(
                col()
                    .w_full()
                    .grow(1.0)
                    .min_h(0.0)
                    .scroll(ScrollSpec {
                        stick_bottom: false,
                    })
                    .child(thread),
            ),
    )
}

pub(super) fn open_beside_here(s: &mut State, tx: &mut Tx) {
    let Some(beside) = s.beside.take() else {
        return;
    };
    if let Some(item) = s.recent.iter().find(|r| r.id == beside.mission_id).cloned() {
        open_recent(s, item, tx);
    }
}

#[derive(Clone, Debug)]
pub struct BesideState {
    pub mission_id: String,
    pub title: String,
    pub owner_name: Option<String>,
    pub prompt: String,
    pub reply_lines: Vec<String>,
    #[allow(dead_code)] // Electron run.phase; streams when live beside is wired
    pub phase: String,
}

/// Never draw the conversation already in the middle a second time (Electron App.tsx).
pub fn beside_allowed(middle_mission_id: Option<&str>, beside_mission_id: &str) -> bool {
    middle_mission_id != Some(beside_mission_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    // a-conversation-opens-beside-another.test.tsx (words + guards)
    #[test]
    fn a_conversation_opens_beside_another_words() {
        assert_eq!(
            beside_aria_label("Count from 1 to 150."),
            "Beside: Count from 1 to 150."
        );
        assert_eq!(BESIDE_OPEN_HERE, "Open it here");
        assert_eq!(BESIDE_OPEN_HERE_TITLE, "Open it here, to answer it");
        assert_eq!(BESIDE_CLOSE, "Close");
        assert_eq!(BESIDE_FALLBACK_NAME, "Conversation");
        assert!(!beside_allowed(Some("mission_sable"), "mission_sable"));
        assert!(beside_allowed(Some("other"), "mission_sable"));
        assert!(beside_allowed(None, "mission_sable"));
    }
}
