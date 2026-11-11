//! Locust's controls, each drawn to its rule in Electron's `shell.css` (named beside each builder) from the tokens of
//! `look::tokens`. Builders return an `El` without handlers; the caller adds `.on_press(…)`. Menus, selects, tooltips,
//! toasts and segmented controls come from rune-kit, styled by `look::kit`, for their keyboard and overlay behaviour.

use super::{LocustType, Tokens, kit, locust_type, tokens};
use rune::{Color, El, Icon, Shadow, TextStyle, col, row, text};
use rune_kit::Kit;

/// One appearance's tokens, kit and type, handed to every builder.
pub struct Look {
    pub t: &'static Tokens,
    pub k: &'static Kit,
    pub ty: LocustType,
}

impl Look {
    pub fn new(dark: bool) -> Look {
        Look { t: tokens(dark), k: kit(dark), ty: locust_type() }
    }
    pub fn c(&self, name: &str) -> Color {
        self.t.color(name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    /// `.lc-button`
    Neutral,
    /// `.lc-button.is-active`: the chosen one of a set.
    Active,
    /// `.lc-button--danger`
    Danger,
}

fn ring(width: f32, color: Color) -> Shadow {
    Shadow { spread: width, color, inset: true, ..Shadow::default() }
}

fn focusable(l: &Look, e: El, label: &str, disabled: bool) -> El {
    e.focusable(!disabled).focus_ring(2.0, 2.0, l.c("focus-ring")).role("button").label(label.to_owned())
}

/// `.lc-button`: 30 high, 12 across, a 1-point border on the control fill, secondary text at 13.
pub fn button(l: &Look, label: &str, tone: Tone, disabled: bool) -> El {
    let (border, fill, ink) = match tone {
        Tone::Neutral => ("border-control", "bg-control", "text-secondary"),
        Tone::Active => ("lime-border", "lime-tint", "lime-text"),
        Tone::Danger => ("red-border", "red-tint", "red-text"),
    };
    let mut e = row()
        .h(30.0)
        .px(12.0)
        .gap(7.0)
        .center_y()
        .radius(l.t.px("radius-control"))
        .fill(l.c(fill))
        .border(1.0, l.c(border))
        .child(text(label.to_owned()).font(l.ty.body).color(l.c(ink)));
    if disabled {
        e = e.opacity(0.5);
    } else {
        e = e.hover(l.c("hover-surface")).press_scale(0.98);
    }
    focusable(l, e, label, disabled)
}

/// `.lc-primarybutton`: the one filled control in a dialog, ivory on dark and ink on light, 32 high, semibold 14.
pub fn primary_button(l: &Look, label: &str, disabled: bool) -> El {
    let mut e = row()
        .h(32.0)
        .px(16.0)
        .center_y()
        .radius(l.t.px("radius-button"))
        .fill(l.c("primary-bg"))
        .child(text(label.to_owned()).font(TextStyle { weight: 600.0, ..l.ty.control }).color(l.c("primary-fg")));
    if disabled {
        e = e.opacity(0.5);
    } else {
        e = e.hover(l.c("primary-bg-hover")).press_scale(0.98);
    }
    focusable(l, e, label, disabled)
}

/// `.lc-chip`: a pill of mono 11 with a card border; `on` takes the lime tint.
pub fn chip(l: &Look, label: &str, on: bool) -> El {
    let (border, fill, ink) =
        if on { ("lime-border", l.c("lime-tint"), "text-primary") } else { ("border-card", Color::TRANSPARENT, "text-secondary") };
    let e = row()
        .py(3.0)
        .px(10.0)
        .center_y()
        .radius(999.0)
        .fill(fill)
        .border(1.0, l.c(border))
        .hover(l.c("bg-selected"))
        .child(text(label.to_owned()).font(TextStyle::new(11.0, 400.0).mono()).color(l.c(ink)));
    focusable(l, e, label, false)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CardTone {
    Plain,
    Amber,
    Red,
    Blue,
}

/// `.lc-card` with its `__head` and `__body`: raised, a card border, radius 10; a tone tints border and fill.
pub fn card(l: &Look, title: &str, body: &str, tone: CardTone) -> El {
    let (border, fill) = match tone {
        CardTone::Plain => (l.c("border-card"), l.c("bg-raised")),
        CardTone::Amber => (l.c("amber-border"), l.c("amber-tint")),
        CardTone::Red => (l.c("red-border"), l.c("red-tint")),
        CardTone::Blue => (l.c("blue-border"), l.c("blue-tint")),
    };
    col()
        .w_full()
        .radius(l.t.px("radius-card"))
        .fill(fill)
        .border(1.0, border)
        .child(row().py(8.0).px(12.0).child(text(title.to_owned()).font(l.ty.control).color(l.c("text-primary"))))
        // A column, not a row: a row sizes to its text and never lets it wrap.
        .child(col().w_full().px(12.0).pb(12.0).child(text(body.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap()))
}

/// `.lc-switch`: a 32 by 18 track with 2 of padding and a 14 knob, off on a faint fill and on in ivory.
pub fn switch(l: &Look, on: bool, label: &str, disabled: bool) -> El {
    let knob = row()
        .w(14.0)
        .h(14.0)
        .radius(7.0)
        .fill(l.c(if on { "switch-knob-on" } else { "switch-knob" }))
        .box_shadow(l.t.shadows("switch-knob-shadow").first().copied().unwrap_or_default());
    let mut track = row()
        .w(32.0)
        .h(18.0)
        .padding(2.0)
        .center_y()
        .radius(999.0)
        .fill(l.c(if on { "switch-on" } else { "switch-off" }))
        .box_shadow(ring(0.5, l.c("border-hairline")));
    track = if on { track.justify_end() } else { track.justify_start() };
    let mut e = track.child(knob);
    if disabled {
        e = e.opacity(0.45);
    }
    focusable(l, e, label, disabled).role("switch")
}

/// `.lc-input`: 34 high, 11 across, radius 8, a card border on the raised fill, primary text at 14. `focused` draws
/// the soft treatment Electron gives a focused field: the edge lightens and a 3-point glow rings it.
pub fn field(l: &Look, value: &str, placeholder: &str, focused: bool) -> El {
    let (shown, ink) = if value.is_empty() { (placeholder, "text-faint") } else { (value, "text-primary") };
    let mut e = row()
        .h(34.0)
        .px(11.0)
        .center_y()
        .w_full()
        .radius(l.t.px("radius-button"))
        .fill(l.c("bg-raised"))
        .border(1.0, l.c(if focused { "focus-edge" } else { "border-card" }))
        .child(text(shown.to_owned()).font(l.ty.control).color(l.c(ink)));
    if focused {
        e = e.box_shadow(Shadow { spread: 3.0, color: l.c("focus-glow"), ..Shadow::default() });
    }
    e.role("textbox").label(placeholder.to_owned())
}

/// `.lc-dialog`: raised-2, a light edge, radius 14, an inner top highlight and the palette's deep shadow; a head with a
/// hairline under it, the body, then the actions right-aligned.
pub fn dialog(l: &Look, title: &str, body: &str, actions: Vec<El>) -> El {
    let mut shadows = l.t.shadows("shadow-palette");
    shadows.insert(0, Shadow { y: 1.0, color: l.c("dialog-highlight"), inset: true, ..Shadow::default() });
    let mut e = col().w(420.0).radius(14.0).fill(l.c("bg-raised-2")).border(1.0, l.c("dialog-edge"));
    for s in shadows {
        e = e.box_shadow(s);
    }
    e.child(
        row()
            .h(56.0)
            .px(20.0)
            .center_y()
            .border_sides([0.0, 0.0, 1.0, 0.0], l.c("border-hairline"))
            .child(text(title.to_owned()).font(TextStyle { weight: 600.0, ..l.ty.control }).color(l.c("text-primary"))),
    )
    .child(col().w_full().padding(20.0).child(text(body.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap()))
    .child(row().px(20.0).pb(16.0).gap(8.0).justify_end().children(actions))
    .role("dialog")
    .label(title.to_owned())
}

/// A screen or section title, set as the wordmark: bold capitals spaced wide.
pub fn title(l: &Look, words: &str) -> El {
    text(words.to_uppercase()).font(l.ty.title).color(l.c("text-primary"))
}

/// A small mono capital label over a group.
pub fn label(l: &Look, words: &str) -> El {
    text(words.to_uppercase()).font(l.ty.label).color(l.c("text-muted"))
}

/// An icon from `look::icons` (Electron's `Icon.tsx`), in the control foreground unless given a colour; decorative,
/// since every place that draws one also says what it means.
pub fn glyph(l: &Look, i: &'static Icon, size: f32, colour: Option<Color>) -> El {
    rune::icon(i, size).color(colour.unwrap_or_else(|| l.c("text-control-fg"))).a11y_hidden()
}

/// An agent's mark (`RuntimeMark.tsx`): Claude's in its own orange (`--lc-mark-claude`), Gemini's in its gradient,
/// the rest in the colour of the words beside it. Nothing for a runtime with no mark.
pub fn agent_mark(l: &Look, runtime: &str, size: f32, words: Color) -> Option<El> {
    let mark = super::icons::mark(runtime)?;
    let colour = if runtime == "claude" { l.c("mark-claude") } else { words };
    Some(rune::icon(mark, size).color(colour).a11y_hidden())
}

/// The Locust mark above the wordmark, as the boot screen and About draw them, in the primary text colour.
pub fn lockup(l: &Look, width: f32) -> El {
    // Both drawings sit centred in square boxes; negative margins take back the empty rows above and below them.
    let mark_h = width * 176.39 / 306.89;
    let word_w = width * 0.92;
    let word_h = word_w * 39.36 / 283.65;
    col()
        .center()
        .gap(width * 0.12)
        .child(rune::icon(&super::icons::LOCUST_MARK, width).color(l.c("text-primary")).my(-(width - mark_h) / 2.0))
        .child(rune::icon(&super::icons::LOCUST_WORDMARK, word_w).color(l.c("text-primary")).my(-(word_w - word_h) / 2.0))
        .role("img")
        .label("Locust")
}

/// `.lc-headicon`: a toolbar's icon button, a quiet glyph whose `help` is its tooltip and whose `label` is its name
/// for screen readers, as Electron's header and composer toolbars draw Browser, Terminal and Open in terminal.
/// Add `.on_press(..)` and `.build(l.k)`.
pub fn head_icon(i: &'static Icon, label: &str, help: &str) -> rune_kit::IconButton {
    rune_kit::IconButton::new(i).variant(rune_kit::IconButtonVariant::Ghost).label(label).help(help)
}

/// `.lc-brand__lockup`: the mark (24 high) beside the wordmark (13 high), 9 apart, as the sidebar's head draws them.
pub fn brand_row(l: &Look) -> El {
    let (mark_h, word_h) = (24.0_f32, 13.0_f32);
    let mark_w = mark_h * 306.89 / 176.39;
    let word_w = word_h * 283.65 / 39.36;
    row()
        .gap(9.0)
        .center_y()
        .child(
            rune::icon(&super::icons::LOCUST_MARK, mark_w)
                .color(l.c("text-primary").alpha(0.95))
                .my(-(mark_w - mark_h) / 2.0)
                .a11y_hidden(),
        )
        .child(rune::icon(&super::icons::LOCUST_WORDMARK, word_w).color(l.c("text-primary").alpha(0.9)).my(-(word_w - word_h) / 2.0))
        .role("img")
        .label("Locust")
}
