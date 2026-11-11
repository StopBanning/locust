//! Every Locust control in one scene (`--scene kit`, `--scene kit-light`): the exit test of the kit round (C1).
//! Static: overlays are drawn open and nothing answers a press.

use super::controls::{CardTone, Look, Tone, button, card, chip, dialog, field, label, primary_button, switch, title};
use rune::{El, col, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Menu, MenuItem, SegmentedControl, Select, Toast, Tooltip};

const MODELS: [&str; 3] = ["gpt-6.1-sol", "Opus 5.5", "deepseek-v4.1-flash-free"];

fn section(l: &Look, name: &str, body: El) -> El {
    col().gap(10.0).child(label(l, name)).child(body)
}

pub fn view(dark: bool) -> El {
    let l = Look::new(dark);
    let k = l.k;
    let mut layer: Vec<El> = Vec::new();

    let buttons = row()
        .gap(10.0)
        .center_y()
        .flex_wrap()
        .child(button(&l, "Open folder", Tone::Neutral, false))
        .child(button(&l, "Ask", Tone::Active, false))
        .child(button(&l, "Forget everything", Tone::Danger, false))
        .child(button(&l, "Not now", Tone::Neutral, true))
        .child(primary_button(&l, "Save", false))
        .child(primary_button(&l, "Save", true));

    let icons = row()
        .gap(6.0)
        .center_y()
        .child(IconButton::new(k.glyph.copy).variant(IconButtonVariant::Ghost).control_size(ControlSize::Small).label("Copy").build(k))
        .child(IconButton::new(k.glyph.search).variant(IconButtonVariant::Ghost).label("Search").build(k))
        .child(IconButton::new(k.glyph.close).label("Close").build(k));

    let chips = row()
        .gap(6.0)
        .center_y()
        .child(chip(&l, "~/work/locust", false))
        .child(chip(&l, "gpt-6.1-sol", true))
        .child(chip(&l, "high", false));

    let switches = row()
        .gap(14.0)
        .center_y()
        .child(switch(&l, false, "Off", false))
        .child(switch(&l, true, "On", false))
        .child(switch(&l, true, "Disabled", true))
        .child(SegmentedControl::new(&["Ask", "Accept edits", "Auto"], Some(0)).fit().build(k));

    let fields = col()
        .gap(8.0)
        .w(300.0)
        .child(field(&l, "", "Name this teammate", false))
        .child(field(&l, "Codec", "Name this teammate", true));

    let cards = col()
        .gap(8.0)
        .w(300.0)
        .child(card(&l, "Folder", "Everything Codec does happens in ~/work/locust.", CardTone::Plain))
        .child(card(&l, "Needs you", "Codec wants to run pnpm test.", CardTone::Amber))
        .child(card(&l, "Stopped", "The turn ended before it finished.", CardTone::Red))
        .child(card(&l, "Carried on", "Argon picked up where Codec stopped.", CardTone::Blue));

    let menu = Menu::new("gallery-menu")
        .min_width(230.0)
        .trigger(button(&l, "Menu", Tone::Neutral, false))
        .item(MenuItem::new("New conversation").shortcut("⌘ N"))
        .item(MenuItem::new("Open folder…"))
        .separator()
        .item(MenuItem::new("Forget everything").disabled(true))
        .open(true)
        .build(k, &mut layer);
    let select = Select::new("gallery-select", &MODELS).current(Some("Opus 5.5")).open(true).build(k, &mut layer);
    let tip = Tooltip::new("gallery-tip").trigger(button(&l, "Tooltip", Tone::Neutral, false)).open(true);
    let tip = tip.text(k, "Copy the reply").build(k, &mut layer);
    // Each open overlay gets its own column wide enough for its popup, so none covers another.
    let overlays = row()
        .gap(16.0)
        .h(170.0)
        .items_start()
        .child(col().w(230.0).child(menu))
        .child(col().w(220.0).child(select))
        .child(col().w(150.0).child(tip));

    let toast = Toast::new("gallery-toast", "Locust 0.872 is ready").description("Restart to update.").closable().build(k);
    let sheet = dialog(
        &l,
        "Forget everything?",
        "Codec forgets every memory it kept for this folder.",
        vec![button(&l, "Cancel", Tone::Neutral, false), primary_button(&l, "Forget", false)],
    );

    let left = col()
        .gap(22.0)
        .child(section(&l, "Buttons", buttons))
        .child(section(&l, "Icon buttons", icons))
        .child(section(&l, "Chips", chips))
        .child(section(&l, "Switches and modes", switches))
        .child(section(&l, "Menu, select, tooltip", overlays))
        .child(section(&l, "Dialog", sheet));
    let right = col()
        .gap(22.0)
        .child(section(&l, "Fields", fields))
        .child(section(&l, "Cards", cards))
        .child(section(&l, "Toast", toast))
        .child(section(
            &l,
            "A reply",
            col().w(300.0).child(
                text("The window is taking shape. Teammates live on the left, and your conversation stays here.")
                    .font(l.ty.prose)
                    .color(l.c("text-primary"))
                    .wrap(),
            ),
        ));

    let body = col()
        .w_full()
        .h_full()
        .fill(l.c("bg-window"))
        .padding(28.0)
        .gap(20.0)
        .child(
            row()
                .gap(12.0)
                .center_y()
                .child(title(&l, "Locust kit"))
                .child(text(if dark { "dark" } else { "light" }).font(l.ty.mono).color(l.c("text-muted"))),
        )
        .child(row().gap(40.0).items_start().child(left).child(right));
    rune_kit::root(body, layer)
}

/// Every icon of `Icon.tsx`, every agent's mark and the Locust lockup (`--scene icons`, `icons-light`): round C2's exit.
pub fn icons_view(dark: bool) -> El {
    use super::controls::{agent_mark, glyph, lockup};
    use super::icons;
    let l = Look::new(dark);
    let cell = |e: El, name: &str| {
        col()
            .w(100.0)
            .gap(6.0)
            .center()
            .child(row().h(28.0).center().child(e))
            .child(text(name.to_owned()).font(l.ty.mono).color(l.c("text-muted")))
    };
    let mut grid = row().gap(4.0).flex_wrap().w(1040.0);
    for i in icons::ALL {
        grid = grid.child(cell(glyph(&l, i, 18.0, None), i.name));
    }
    let mut marks = row().gap(4.0).flex_wrap().w(1040.0);
    for runtime in icons::RUNTIMES {
        if let Some(m) = agent_mark(&l, runtime, 22.0, l.c("text-secondary")) {
            marks = marks.child(cell(m, runtime));
        }
    }
    col()
        .w_full()
        .h_full()
        .fill(l.c("bg-window"))
        .padding(28.0)
        .gap(22.0)
        .child(
            row()
                .gap(12.0)
                .center_y()
                .child(title(&l, "Locust icons"))
                .child(text(if dark { "dark" } else { "light" }).font(l.ty.mono).color(l.c("text-muted"))),
        )
        .child(row().gap(40.0).items_start().child(section(&l, "Icon.tsx", grid)))
        .child(section(&l, "Agents", marks))
        .child(section(&l, "Locust", row().pt(8.0).child(lockup(&l, 140.0))))
}
