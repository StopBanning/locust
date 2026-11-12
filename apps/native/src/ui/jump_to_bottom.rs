//! The way back down, offered only while the person is away from the bottom.
//! Port of `apps/desktop/src/renderer/src/components/JumpToBottom.tsx`.

use super::*;

pub(super) const LABEL_IDLE: &str = "Go to the newest message";
pub(super) const LABEL_WORKING: &str = "Still working. Go to the newest message";

pub(super) fn aria_label(working: bool) -> &'static str {
    if working { LABEL_WORKING } else { LABEL_IDLE }
}

pub(super) fn shows_dots(working: bool, hovered: bool) -> bool {
    working && !hovered
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
    let content = if shows_dots(working, hovered) {
        thinking_dots(icon_color)
    } else {
        crate::look::controls::glyph(&l, &crate::look::icons::CHEVRON_DOWN, 14.0, Some(icon_color))
            .key("jump-to-bottom-chevron")
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
            .child(content)
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
mod tests {
    use super::*;

    #[test]
    fn wears_thinking_dots_while_working_and_says_so() {
        let p = Palette::new(true);
        assert_eq!(aria_label(true), "Still working. Go to the newest message");
        assert!(shows_dots(true, false));
        let el = view(true, true, false, &p);
        assert!(el.is_some());
    }

    #[test]
    fn plain_chevron_when_idle_and_absent_at_bottom() {
        let p = Palette::new(true);
        assert_eq!(aria_label(false), "Go to the newest message");
        assert!(!shows_dots(false, false));
        assert!(view(true, false, false, &p).is_some());
        assert!(view(false, true, false, &p).is_none());
    }

    #[test]
    fn brings_chevron_back_on_hover() {
        assert!(!shows_dots(true, true));
    }
}
