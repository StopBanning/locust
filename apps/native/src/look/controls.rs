//! Locust's controls, each drawn to its rule in Electron's `shell.css` (named beside each builder) from the tokens of
//! `look::tokens`. Builders return an `El` without handlers; the caller adds `.on_press(…)`. Menus, selects, tooltips,
//! toasts and segmented controls come from rune-kit, styled by `look::kit`, for their keyboard and overlay behaviour.

use super::{LocustType, Tokens, kit, locust_type, tokens};
use rune::{Color, Curve, Easing, El, Icon, Shadow, TextStyle, col, row, text};
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
    // `disabled` as the `<button disabled>` it stands for: said to assistive tech, and the arrow, not the hand
    // (`.lc-button:disabled { cursor: default }`, shell.css:710-713).
    e.focusable(!disabled).focus_ring(2.0, 2.0, l.c("focus-ring")).role("button").label(label.to_owned()).disabled(disabled)
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
        // `.lc-button:hover:not(:disabled)`: the hover surface and the border lifted to `--lc-hover-border`, both on
        // `transition: background var(--lc-motion-hover) var(--lc-ease), border-color …` (shell.css:698, 701-704), 140ms ease.
        let ease = Curve::tween(0.14, Easing::EASE);
        e = e.hover(l.c("hover-surface")).hover_border(l.c("hover-border")).hover_motion(ease, ease).press_scale(0.98);
    }
    focusable(l, e, label, disabled)
}

/// The colour that, drawn over `under`, shows `want`: Rune's hover paints over the fill, where CSS's `:hover` replaces
/// the background. Exact when the two share their hue and differ in alpha, as the switch's off and off-hover tracks do.
fn over_to(under: Color, want: Color) -> Color {
    if under.a >= 1.0 || want.a <= under.a {
        return want;
    }
    let a = (want.a - under.a) / (1.0 - under.a);
    let ch = |w: f32, u: f32| ((w * want.a - u * under.a * (1.0 - a)) / a).clamp(0.0, 1.0);
    Color { r: ch(want.r, under.r), g: ch(want.g, under.g), b: ch(want.b, under.b), a }
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
    } else if !on {
        // `button.lc-switch:hover:not(:disabled):not(.is-on) { background: var(--lc-switch-off-hover) }`
        // (shell.css:18926-18928), on `transition: background 160ms var(--lc-ease-out)` (shell.css:18923; the curve is
        // tokens.css:770).
        let ease = Curve::tween(0.16, Easing::cubic(0.22, 0.8, 0.24, 1.0));
        e = e.hover(over_to(l.c("switch-off"), l.c("switch-off-hover"))).hover_motion(ease, ease);
    }
    // Says on or off and whether it can be used, as Electron's `aria-checked` and `disabled` do: a switch that only
    // looked on was silent to a screen reader.
    focusable(l, e, label, disabled).role("switch").checked(on).disabled(disabled)
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
    runtime_mark(l, runtime, size, words, None, false)
}

/// Gemini greyed with its words (`RuntimeMark muted`): its star in the words' colour, no gradient.
static MARK_GEMINI_PLAIN: rune::Icon = Icon { name: "mark-gemini-plain", view: 28.0, body: r##"<path d="M14 28C14 26.0633 13.6267 24.2433 12.88 22.54C12.1567 20.8367 11.165 19.355 9.905 18.095C8.645 16.835 7.16333 15.8433 5.46 15.12C3.75667 14.3733 1.93667 14 0 14C1.93667 14 3.75667 13.6383 5.46 12.915C7.16333 12.1683 8.645 11.165 9.905 9.905C11.165 8.645 12.1567 7.16333 12.88 5.46C13.6267 3.75667 14 1.93667 14 0C14 1.93667 14.3617 3.75667 15.085 5.46C15.8317 7.16333 16.835 8.645 18.095 9.905C19.355 11.165 20.8367 12.1683 22.54 12.915C24.2433 13.6383 26.0633 14 28 14C26.0633 14 24.2433 14.3733 22.54 15.12C20.8367 15.8433 19.355 16.835 18.095 18.095C16.835 19.355 15.8317 20.8367 15.085 22.54C14.3617 24.2433 14 26.0633 14 28Z" fill="currentColor"/>"## };

/// Apple Intelligence on this Mac (runtime::apple): Apple's logo (Simple Icons "Apple", source apple.com), as every other
/// agent wears its maker's mark. Colin, 22 Nov: a chip of our own was the one made-up mark in the row. Drawn here, not in
/// icons.rs, which marks.py writes from Electron's drawings.
static MARK_APPLE_ON_DEVICE: rune::Icon = Icon { name: "mark-apple", view: 24.0, body: r##"<path d="M12.152 6.896c-.948 0-2.415-1.078-3.96-1.04-2.04.027-3.91 1.183-4.961 3.014-2.117 3.675-.546 9.103 1.519 12.09 1.013 1.454 2.208 3.09 3.792 3.039 1.52-.065 2.09-.987 3.935-.987 1.831 0 2.35.987 3.96.948 1.637-.026 2.676-1.48 3.676-2.948 1.156-1.688 1.636-3.325 1.662-3.415-.039-.013-3.182-1.221-3.22-4.857-.026-3.04 2.48-4.494 2.597-4.559-1.429-2.09-3.623-2.324-4.39-2.376-2-.156-3.675 1.09-4.61 1.09zM15.53 3.83c.843-1.012 1.4-2.427 1.245-3.83-1.207.052-2.662.805-3.532 1.818-.78.896-1.454 2.338-1.273 3.714 1.338.104 2.715-.688 3.559-1.701" fill="currentColor"/>"## };

/// The mark to draw: `icons::mark`, but Gemini's plain star when muted, as Electron greys it with its words, and the
/// apple runtime's logo. Kept here, not in icons.rs, which `scripts/marks.py` writes from Electron's drawings.
pub fn mark_drawn(runtime: &str, muted: bool) -> Option<&'static rune::Icon> {
    match runtime {
        "gemini" if muted => Some(&MARK_GEMINI_PLAIN),
        "apple" => Some(&MARK_APPLE_ON_DEVICE),
        _ => super::icons::mark(runtime),
    }
}

/// `RuntimeMark`: decorative beside a name (hidden from a screen reader), or, given a label, named where it stands in
/// for one (an image with that name). Muted, it greys with its words: Claude's orange and Gemini's gradient give way to
/// the words' colour. Nothing for a runtime with no mark.
pub fn runtime_mark(l: &Look, runtime: &str, size: f32, words: Color, label: Option<&str>, muted: bool) -> Option<El> {
    let mark = mark_drawn(runtime, muted)?;
    let colour = if runtime == "claude" && !muted { l.c("mark-claude") } else { words };
    let icon = rune::icon(mark, size).color(colour);
    // shell.css `.lc-runtimemark.is-muted[data-runtime]`: the words' colour, at half strength.
    let icon = if muted { icon.opacity(0.5) } else { icon };
    Some(match label {
        Some(name) => icon.role("img").label(name.to_owned()),
        None => icon.a11y_hidden(),
    })
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

/// A header icon that opens a panel beside the conversation (Electron's BrowserToggle and TerminalToggle): pressed
/// while its panel is open (`aria-pressed`, `.is-active`), and, turned off by the organization, disabled, named
/// "<name>, set by your organization" and saying the refusal where its tooltip was. Add `.on_press(..)`, then
/// `.build(l.k)` and `.checked(open)`.
pub fn head_toggle(i: &'static Icon, name: &str, help: &str, open: bool, refusal: Option<&str>) -> rune_kit::IconButton {
    let label = match refusal {
        Some(_) => format!("{name}, set by your organization"),
        None => name.to_owned(),
    };
    let button = head_icon(i, &label, refusal.unwrap_or(help));
    let button = if open && refusal.is_none() { button.state(rune_kit::State::Pressed) } else { button };
    button.disabled(refusal.is_some())
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

/// `.lc-control`: a chip under the message box (the mode, the folder, the model): no box, secondary words at the
/// control size, an optional lead (an icon or an agent's mark) and a chevron when it opens a menu.
pub fn composer_chip(l: &Look, lead: Option<El>, words: &str, chevron: bool) -> El {
    composer_route_chip(l, lead, words, false, chevron)
}

/// A composer chip with an optional non-truncating `Free` tag drawn outside the truncated model text.
pub fn composer_route_chip(l: &Look, lead: Option<El>, words: &str, free: bool, chevron: bool) -> El {
    let mut e = row().py(4.0).px(6.0).gap(6.0).center_y().radius(l.t.px("radius-control")).hover(l.c("hover-surface"));
    if let Some(lead) = lead {
        e = e.child(lead);
    }
    e = e.child(text(words.to_owned()).font(TextStyle::new(13.0, 400.0)).color(l.c("text-secondary")).truncate().shrink());
    if free {
        e = e.child(
            row()
                .key("model-chip-free")
                .px(5.0)
                .py(1.0)
                .radius(4.0)
                .fill(l.c("hover-surface"))
                .child(text("Free").font(TextStyle::new(10.5, 500.0)).color(l.c("text-muted"))),
        );
    }
    if chevron {
        e = e.child(glyph(l, &super::icons::CHEVRON_DOWN, 13.0, Some(l.c("text-muted"))));
    }
    let accessible = if free && !words.is_empty() { format!("{words} Free") } else if free { "Free".to_owned() } else { words.to_owned() };
    e.role("button").label(accessible)
}

/// What the round send button says and does.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SendState {
    /// Nothing to send: the dim circle.
    Idle,
    /// Something to send (or to queue): ivory, an arrow.
    Ready,
    /// A reply is running and nothing waits: a square stops it.
    Stop,
}

/// The composer's round send button: 40 across, the circle's fill and border; ivory with a 16-point arrow once there is
/// something to send, or a borderless disc with a card-border inner ring, stop-glow and 14-point square while a reply
/// runs, as Electron's metal composer draws it (`.lc-composer__controls .lc-send`, `.is-stop`).
/// The send circle's size; a one-line message field is as tall, so they share a centre.
pub const SEND_SIZE: f32 = 40.0;
pub const SEND_BORDER: f32 = 1.0;
pub const SEND_ARROW_SIZE: f32 = 16.0;
pub const STOP_SQUARE_SIZE: f32 = 14.0;
pub const STOP_SQUARE_RADIUS: f32 = 3.0;
pub const STOP_GLOW_BLUR: f32 = 50.0;

pub fn send_circle(l: &Look, state: SendState, label: &str) -> El {
    let (fill, ink) = match state {
        SendState::Ready => (l.c("primary-bg"), l.c("primary-fg")),
        SendState::Stop => (l.c("circle-bg"), l.c("text-secondary")),
        SendState::Idle => (l.c("circle-bg"), l.c("circle-fg").alpha(0.35)),
    };
    let inner = match state {
        SendState::Stop => row().key("stop-square").w(STOP_SQUARE_SIZE).h(STOP_SQUARE_SIZE).radius(STOP_SQUARE_RADIUS).fill(ink),
        _ => glyph(l, &super::icons::ARROW_UP, SEND_ARROW_SIZE, Some(ink)).key("send-arrow"),
    };
    let mut e = row().w(SEND_SIZE).h(SEND_SIZE).center().fill(fill).child(inner);
    e = match state {
        SendState::Stop => e
            .radius(999.0)
            .box_shadow(ring(1.0, l.c("border-card")))
            .box_shadow(Shadow { blur: STOP_GLOW_BLUR, color: l.c("stop-glow"), inset: true, ..Shadow::default() })
            .hover(l.c("red-tint")),
        _ => e.radius(SEND_SIZE / 2.0).border(SEND_BORDER, l.c("circle-border")),
    };
    if state == SendState::Ready {
        e = e.hover(l.c("hover-surface"));
    }
    if state != SendState::Idle {
        e = e.press_scale(0.96);
    }
    focusable(l, e, label, state == SendState::Idle)
}

/// `.lc-composer__box`: the rounded box a message is written in, radius 20 on the chat fill. One row, as Claude Code's:
/// the + and the buttons sit on the field's last line as it grows (`align-items: end`), and while the box has focus its
/// edge is `--lc-focus-edge-box`, 1 point outside (`box-shadow: 0 0 0 1px`), which meets 3:1 against the page and the box.
pub fn composer_box(l: &Look) -> El {
    row()
        .w_full()
        .items_end()
        .gap(10.0)
        .pl(16.0)
        .pr(10.0)
        .py(10.0)
        .radius(20.0)
        .fill(l.c("chat-bg"))
        .focus_within_ring(1.0, 0.0, l.c("focus-edge-box"))
}

/// `.lc-ghostbutton`: an outlined choice beside the primary one, 32 high, the strong border, secondary words at 14.
pub fn ghost_button(l: &Look, label: &str) -> El {
    let e = row()
        .h(32.0)
        .px(15.0)
        .center()
        .radius(l.t.px("radius-control"))
        .border(1.0, l.c("border-strong"))
        .hover(l.c("hover-surface"))
        .press_scale(0.98)
        .child(text(label.to_owned()).font(l.ty.control).color(l.c("text-secondary")));
    focusable(l, e, label, false)
}

/// `.lc-denybutton`: the same, outlined and worded in red.
pub fn deny_button(l: &Look, label: &str) -> El {
    let e = row()
        .h(32.0)
        .px(15.0)
        .center()
        .radius(l.t.px("radius-control"))
        .border(1.0, l.c("red-border"))
        .hover(l.c("red-tint"))
        .press_scale(0.98)
        .child(text(label.to_owned()).font(l.ty.control).color(l.c("red-text")));
    focusable(l, e, label, false)
}

/// The thread header's one worded action (Commit): an icon and a word in an outlined box, 28 high, as Electron's.
pub fn header_button(l: &Look, i: &'static Icon, words: &str) -> El {
    let e = row()
        .h(28.0)
        .px(10.0)
        .gap(6.0)
        .center_y()
        .radius(l.t.px("radius-control"))
        .fill(l.c("bg-control"))
        .border(1.0, l.c("border-control"))
        .hover(l.c("hover-surface"))
        .child(glyph(l, i, 13.0, Some(l.c("text-secondary"))))
        .child(text(words.to_owned()).font(TextStyle::new(13.0, 400.0)).color(l.c("text-secondary")));
    focusable(l, e, words, false)
}

#[cfg(test)]
#[path = "runtime_mark_colour_tests.rs"]
mod runtime_mark_colour_tests;

#[cfg(test)]
#[path = "control_hover_tests.rs"]
mod control_hover_tests;
