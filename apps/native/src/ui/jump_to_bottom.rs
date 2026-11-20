//! The way back down, offered only while the person is away from the bottom.
//! Port of `apps/desktop/src/renderer/src/components/JumpToBottom.tsx`.

use super::*;

pub(super) const LABEL_IDLE: &str = "Go to the newest message";
pub(super) const LABEL_WORKING: &str = "Still working. Go to the newest message";

pub(super) fn aria_label(working: bool) -> &'static str {
    if working { LABEL_WORKING } else { LABEL_IDLE }
}

fn thinking_dots(color: Color) -> El {
    let dot = |phase: f32| {
        el().size(4.0, 4.0)
            .radius(2.0)
            .fill(color)
            .cycle(Cycle {
                period: 1.4,
                phase,
                stops: &[(0.0, 0.3), (0.5, 1.0), (1.0, 0.3)],
                still: 0.7,
                fps: 30.0,
            })
    };
    row()
        .key("jump-to-bottom-dots")
        .gap(3.0)
        .center()
        .child(dot(0.0))
        .child(dot(0.2))
        .child(dot(0.4))
}

pub(super) fn view(shown: bool, working: bool, hovered: bool, p: &Palette) -> Option<El> {
    if !shown {
        return None;
    }
    let l = crate::look::controls::Look::new(p.dark);
    let icon_color = l.c(if hovered { "text-primary" } else { "text-muted" });
    let border_color = l.c(if hovered { "border-hover" } else { "border-button" });
    // Electron's shell.css draws both while a run works and lets `:hover` and `:focus-visible` swap them: the chevron
    // comes back where the button is about to be pressed, by pointer or by Tab.
    let chevron = crate::look::controls::glyph(&l, &crate::look::icons::CHEVRON_DOWN, 14.0, Some(icon_color))
        .key("jump-to-bottom-chevron");
    let content: Vec<El> = if working {
        vec![
            chevron.group_opacity(0.0, 1.0),
            row().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).center().child(thinking_dots(icon_color).group_opacity(1.0, 0.0)),
        ]
    } else {
        vec![chevron]
    };
    Some(
        el().key("jump-to-bottom")
            .pin(None, None, None, Some(16.0))
            .size(30.0, 30.0)
            .radius(15.0)
            .center()
            .fill(l.c("bg-raised-2"))
            .border(1.0, border_color)
            .role("button")
            .label(aria_label(working))
            .focusable(true)
            .focus_ring(2.0, 2.0, l.c("focus-ring"))
            .hover_group_focus()
            .children(content)
            .on_hover_change(|s: &mut State, over, _| {
                s.jump_hover = over;
            })
            .on_press_tx(|s: &mut State, tx: &mut Tx| {
                s.thread_away = false;
                s.jump_hover = false;
                tx.scroll_to_bottom("messages");
            }),
    )
}

#[cfg(test)]
#[path = "jump_way_down_tests.rs"]
mod jump_way_down_tests;
