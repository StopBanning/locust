//! Locust's look: the design tokens of `renderer/src/tokens.css`, read from that file at build time, mapped onto
//! rune-kit's semantic `Kit`, plus Locust's own controls (`controls`) and a gallery of them (`gallery`).
//!
//! The CSS stays the one source of truth while Electron ships: a colour changed there changes here on the next build,
//! and `tokens_match_the_css` fails if a token this module names is missing. Components read `kit(dark)` (rune-kit's
//! builders) or `tokens(dark)` (a named Locust token); a screen never writes a colour, size or radius of its own.
//!
//! Fonts: Electron vendors Figtree (chrome), Geist Mono (machine text) and Anthropic Sans (replies) as `.woff2`, which
//! Rune's text stack (parley and swash) cannot read. Until `.ttf` copies are added under `assets/fonts/`, `FONTS` is
//! empty and text uses the system faces at Locust's sizes and weights.

pub mod controls;
pub mod gallery;

use rune::{Color, Shadow, TextStyle};
use rune_kit::{Kit, PaletteSeed, StateColors, Type, css};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Electron's token file, compiled in.
pub const TOKENS_CSS: &str = include_str!("../../../desktop/src/renderer/src/tokens.css");

/// `AppSpec::fonts` once the `.ttf` faces are added (see the module note); nothing reads it until then.
#[allow(dead_code)]
pub const FONTS: &[&[u8]] = &[];

/// Every `--lc-*` token for one appearance, as the CSS writes it (without `--lc-`).
pub struct Tokens {
    values: HashMap<String, String>,
}

fn without_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        out.push_str(&rest[..start]);
        rest = rest[start + 2..].find("*/").map_or("", |end| &rest[start + 2 + end + 2..]);
    }
    out.push_str(rest);
    out
}

/// The declarations of the block that opens with `selector {` at the start of a line.
fn block(css: &str, selector: &str) -> Vec<(String, String)> {
    let opener = format!("\n{selector} {{");
    let Some(start) = css.find(&opener) else { return Vec::new() };
    let body = &css[start + opener.len()..];
    let body = &body[..body.find("\n}").unwrap_or(body.len())];
    body.split(';')
        .filter_map(|d| {
            let (name, value) = d.trim().split_once(':')?;
            Some((name.trim().strip_prefix("--lc-")?.to_owned(), value.trim().to_owned()))
        })
        .collect()
}

/// The tokens for one appearance: `:root` for dark, with `:root[data-theme='light']` laid over it for light.
pub fn tokens(dark: bool) -> &'static Tokens {
    static DARK: OnceLock<Tokens> = OnceLock::new();
    static LIGHT: OnceLock<Tokens> = OnceLock::new();
    let load = || {
        let css = without_comments(TOKENS_CSS);
        let mut values: HashMap<String, String> = block(&css, ":root").into_iter().collect();
        if !dark {
            values.extend(block(&css, ":root[data-theme='light']"));
        }
        Tokens { values }
    };
    if dark { DARK.get_or_init(load) } else { LIGHT.get_or_init(load) }
}

impl Tokens {
    /// The token's value with any `var(--lc-…)` resolved. Panics on a name the CSS does not define: a typo here is a
    /// bug, and `tokens_match_the_css` runs every name this module uses.
    #[track_caller]
    pub fn raw(&self, name: &str) -> String {
        let mut value = self.values.get(name).unwrap_or_else(|| panic!("tokens.css has no --lc-{name}")).clone();
        for _ in 0..8 {
            let Some(start) = value.find("var(--lc-") else { break };
            let end = start + value[start..].find(')').expect("closed var()");
            let inner = &value[start + "var(--lc-".len()..end];
            let replacement = self.raw(inner);
            value.replace_range(start..=end, &replacement);
        }
        value
    }

    #[track_caller]
    pub fn color(&self, name: &str) -> Color {
        parse_color(&self.raw(name)).unwrap_or_else(|| panic!("--lc-{name} is not a colour"))
    }

    /// A length in points (`17px`, `0.14em` at `em` points).
    #[track_caller]
    pub fn px(&self, name: &str) -> f32 {
        let v = self.raw(name);
        v.strip_suffix("px").and_then(|n| n.trim().parse().ok()).unwrap_or_else(|| panic!("--lc-{name} is not px"))
    }

    /// A CSS box-shadow list as Rune shadows.
    #[track_caller]
    pub fn shadows(&self, name: &str) -> Vec<Shadow> {
        css::box_shadows(&hex_colours(&self.raw(name)), |c| c)
    }
}

/// `rgb()` and `rgba()` written as `#rrggbbaa`, which rune-kit's box-shadow parser reads (it takes hex only).
pub fn hex_colours(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut rest = css;
    while let Some(start) = rest.find("rgb") {
        out.push_str(&rest[..start]);
        let Some(end) = rest[start..].find(')') else { break };
        let call = &rest[start..=start + end];
        match rgba_hex(call) {
            Some(hex) => out.push_str(&hex),
            None => out.push_str(call),
        }
        rest = &rest[start + end + 1..];
    }
    out.push_str(rest);
    out
}

fn rgba_hex(v: &str) -> Option<String> {
    let inner = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb("))?.strip_suffix(')')?;
    let parts: Vec<f32> = inner.split(',').map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let (r, g, b, a) = match parts[..] {
        [r, g, b] => (r, g, b, 1.0),
        [r, g, b, a] => (r, g, b, a),
        _ => return None,
    };
    let byte = |x: f32| x.round().clamp(0.0, 255.0) as u8;
    Some(format!("#{:02x}{:02x}{:02x}{:02x}", byte(r), byte(g), byte(b), byte(a * 255.0)))
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()` and `transparent`, as a browser reads them.
pub fn parse_color(v: &str) -> Option<Color> {
    let v = v.trim();
    if v == "transparent" {
        return Some(Color::TRANSPARENT);
    }
    if v.starts_with('#') {
        return rune::color::hex(v);
    }
    rune::color::hex(&rgba_hex(v)?)
}

/// Locust's own type roles beside the kit's: the wordmark-style title, the mono label, the reply's prose.
pub struct LocustType {
    /// Screen and section titles, set as the wordmark: bold capitals spaced wide (`--lc-title-*`).
    pub title: TextStyle,
    /// Small capital labels in mono, tracked (`--lc-text-mono-label`, `--lc-tracking-label`).
    pub label: TextStyle,
    /// Paths, ids, commands (`--lc-text-mono-caption`).
    pub mono: TextStyle,
    /// A teammate's reply (`--lc-text-prose`, `--lc-leading-prose`).
    pub prose: TextStyle,
    /// Controls and body (`--lc-text-description`).
    pub body: TextStyle,
    /// A field's text and card heads (`--lc-text-control`).
    pub control: TextStyle,
}

pub fn locust_type() -> LocustType {
    let t = tokens(true);
    let em = |name: &str, size: f32| t.raw(name).strip_suffix("em").and_then(|n| n.parse::<f32>().ok()).unwrap_or(0.0) * size;
    let title = t.px("title-size");
    let label = t.px("text-mono-label");
    let prose = t.px("text-prose");
    LocustType {
        title: TextStyle::new(title, t.raw("title-weight").parse().unwrap_or(700.0)).tracking(em("title-tracking", title)),
        label: TextStyle::new(label, 500.0).mono().tracking(em("tracking-label", label)),
        mono: TextStyle::new(t.px("text-mono-caption"), 400.0).mono(),
        prose: TextStyle::new(prose, 400.0).lh(prose * 1.55),
        body: TextStyle::new(t.px("text-description"), 400.0).lh(18.0),
        control: TextStyle::new(t.px("text-control"), 400.0).lh(20.0),
    }
}

/// The kit for one appearance, resolved for the blend mode of the window building the view, as rune-kit's own themes
/// are (`rune::theme::blends_srgb`).
pub fn kit(dark: bool) -> &'static Kit {
    static KITS: [[OnceLock<Kit>; 2]; 2] = [const { [const { OnceLock::new() }; 2] }; 2];
    let srgb = rune::theme::blends_srgb();
    KITS[dark as usize][srgb as usize].get_or_init(|| rune::theme::with_blend(srgb, || build(dark)))
}

fn build(dark: bool) -> Kit {
    let t = tokens(dark);
    let c = |n: &str| t.color(n);
    let mut k = Kit::from_palette(PaletteSeed { bg: c("bg-window"), fg: c("text-primary"), accent: c("lime") });
    let state = |hue: &str, icon: Color| StateColors {
        bg: c(&format!("{hue}-tint")),
        fg: c(&format!("{hue}-text")),
        border: c(&format!("{hue}-border")),
        icon,
    };
    let p = &mut k.c;
    p.page = c("bg-window");
    p.bg_base = c("bg-window");
    p.bg_deep = c("bg-app");
    p.layer_01 = c("bg-raised");
    p.layer_02 = c("bg-raised-2");
    p.layer_03 = c("bg-selected");
    p.layer_04 = c("bg-control");
    // Locust's one filled control: ivory on dark, ink on light (`.lc-primarybutton`).
    p.bg_inverse = c("primary-bg");
    p.bg_contrast = c("primary-bg");
    p.bg_icon_button_contrast = c("primary-bg");
    p.bg_button_neutral = c("bg-control");
    p.bg_accent = c("lime");
    p.submit_fg = c("primary-fg");
    p.submit_icon = c("primary-fg");
    p.submit_highlight = Color::TRANSPARENT;
    p.text_base = c("text-primary");
    p.text_muted = c("text-muted");
    p.text_faint = c("text-faint");
    p.text_inverse = c("primary-fg");
    p.text_contrast = c("primary-fg");
    p.text_accent = c("lime-text");
    p.icon_base = c("text-control-fg");
    p.icon_muted = c("text-muted");
    p.icon_faint = c("text-faint");
    p.icon_inverse = c("primary-fg");
    p.icon_contrast = c("primary-fg");
    p.icon_accent = c("lime");
    p.border_muted = c("border-hairline");
    p.border_base = c("border-control");
    p.border_strong = c("border-strong");
    p.border_focus = c("focus-ring");
    p.hover = c("hover-surface");
    p.pressed = c("hover-surface-strong");
    p.scrim = c("overlay-scrim");
    p.knob = c("switch-knob");
    p.knob_on = c("switch-knob-on");
    p.success = state("green", c("green"));
    p.warning = state("amber", c("amber"));
    p.danger = state("red", c("red"));
    p.info = state("blue", c("blue"));
    p.surface_base = c("bg-raised");
    p.surface_weak = c("bg-control");
    p.surface_raised = c("bg-raised-2");
    p.surface_raised_base = c("bg-raised");
    p.surface_float = c("bg-raised-2");
    p.border_weak = c("border-hairline");
    p.border_hover = c("border-hover");
    p.text_strong = c("text-name");
    p.text_weak = c("text-muted");
    p.std_border = c("border-control");
    p.std_text = c("text-primary");

    k.elevation.overlay = t.shadows("shadow-popover");
    k.elevation.floating = t.shadows("shadow-dropdown");
    k.elevation.md = t.shadows("shadow-dropdown");
    k.elevation.knob = t.shadows("switch-knob-shadow");
    k.elevation.button_neutral = css::box_shadows(&format!("inset 0 0 0 1px {}", t.raw("border-control")), |c| c);
    k.elevation.button_contrast = Vec::new();
    k.elevation.raised = Vec::new();

    k.radius.xs = 4.0;
    k.radius.sm = t.px("radius-control");
    k.radius.md = t.px("radius-button");
    k.radius.lg = t.px("radius-card");
    k.radius.xl = t.px("radius-popover");

    let lt = locust_type();
    let body = lt.body;
    k.ty = Type {
        ui: body,
        ui_medium: TextStyle { weight: 500.0, ..body },
        ui_compact: body.lh(16.0),
        small: TextStyle::new(11.5, 400.0).lh(14.0),
        small_medium: TextStyle::new(11.5, 500.0).lh(14.0),
        body: lt.control,
        body_medium: TextStyle { weight: 500.0, ..lt.control },
        text_small: body,
        text_small_medium: TextStyle { weight: 500.0, ..body },
        text_xsmall: TextStyle::new(t.px("text-ask"), 400.0).lh(16.0),
        text_xsmall_medium: TextStyle::new(t.px("text-ask"), 500.0).lh(16.0),
        mono: lt.mono,
    };
    k
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every token this module and `controls` name, in both themes, read as the kind it is used as.
    #[test]
    fn tokens_match_the_css() {
        let colours = [
            "bg-window", "bg-app", "bg-raised", "bg-raised-2", "bg-selected", "bg-control", "bg-field", "primary-bg",
            "primary-bg-hover", "primary-fg", "lime", "lime-text", "lime-tint", "lime-border", "text-primary",
            "text-name", "text-secondary", "text-muted", "text-faint", "text-control-fg", "border-hairline",
            "border-control", "border-card", "border-strong", "border-hover", "hover-surface", "hover-surface-strong",
            "hover-border", "focus-ring", "focus-edge", "focus-glow", "overlay-scrim", "switch-off", "switch-off-hover",
            "switch-on", "switch-knob", "switch-knob-on", "green", "green-text", "green-tint", "green-border",
            "amber", "amber-text", "amber-tint", "amber-border", "red", "red-text", "red-tint", "red-border", "blue",
            "blue-text", "blue-tint", "blue-border", "dialog-edge", "dialog-highlight", "segment-track",
            "segment-thumb", "chip-bg", "chip-bg-hover", "chip-text",
        ];
        for dark in [true, false] {
            let t = tokens(dark);
            for name in colours {
                let _ = t.color(name);
            }
            for name in ["radius-control", "radius-button", "radius-card", "radius-popover", "title-size"] {
                assert!(t.px(name) > 0.0, "{name}");
            }
            for name in ["shadow-popover", "shadow-dropdown", "switch-knob-shadow", "shadow-palette"] {
                assert!(!t.shadows(name).is_empty(), "{name}");
            }
        }
    }

    #[test]
    fn the_two_themes_differ_where_the_css_says() {
        let (d, l) = (tokens(true), tokens(false));
        assert_eq!(d.raw("bg-window"), "#121415");
        assert_eq!(l.raw("bg-window"), "#faf9f6");
        // Not overridden for light: the same value in both.
        assert_eq!(d.raw("radius-card"), l.raw("radius-card"));
        // var() resolves through the theme: the focus ring is lime, ivory on dark and ink on light.
        assert_eq!(d.raw("focus-ring"), d.raw("lime"));
        assert_eq!(l.raw("focus-ring"), "#1c1e1d");
    }

    #[test]
    fn colours_read_as_a_browser_reads_them() {
        let white = parse_color("#ffffff").unwrap();
        assert_eq!(parse_color("rgba(255, 255, 255, 0.06)").unwrap(), white.alpha(15.0 / 255.0));
        assert_eq!(parse_color("rgb(255, 255, 255)").unwrap(), white);
        assert_eq!(parse_color("transparent").unwrap(), Color::TRANSPARENT);
        assert!(parse_color("12px").is_none());
        assert_eq!(hex_colours("0 1px 2px rgba(0, 0, 0, 0.45), 0 0 0 1px rgb(28, 30, 29)"), "0 1px 2px #00000073, 0 0 0 1px #1c1e1dff");
    }

    #[test]
    fn the_old_hand_copied_palette_agrees_with_the_css() {
        // ui/app_state.rs Palette::new copied twelve colours by hand; until every screen reads the kit, hold it to them.
        for (dark, values) in [
            (true, ["#121415", "#17191a", "#191c1d", "#1e2223", "#23282a", "#1a1d1e", "#f2efe9", "#c7c3bc", "#8b9190", "#7fc97f", "#e9b949", "#e4685d"]),
            (false, ["#faf9f6", "#f1f0ec", "#ffffff", "#e7e6e1", "#ebeae5", "#ffffff", "#1c1e1d", "#3a3e3d", "#5f6563", "#2e8540", "#b8790a", "#c9443a"]),
        ] {
            let names = ["bg-window", "bg-sidebar", "bg-raised", "bg-selected", "bg-bubble-user", "bg-field", "text-primary", "text-secondary", "text-muted", "green", "amber", "red"];
            for (name, value) in names.iter().zip(values) {
                assert_eq!(tokens(dark).raw(name), value, "{name} dark={dark}");
            }
        }
    }

    #[test]
    fn the_kit_carries_locust_values() {
        let k = kit(true);
        assert_eq!(k.radius.sm, 7.0);
        assert_eq!(k.radius.xl, 12.0);
        assert_eq!(k.c.bg_contrast, tokens(true).color("primary-bg"));
        assert_eq!(kit(false).c.text_base, tokens(false).color("text-primary"));
    }
}
