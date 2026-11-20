//! Electron's `every-colour-has-a-light-one.test.ts` (3 cases), on the tokens.css Rust draws from (`TOKENS_CSS`): a
//! colour given for dark and not for light is drawn in light in its dark value.
use super::TOKENS_CSS;
use std::collections::BTreeMap;

fn block(selector: &str) -> &'static str {
    let start = TOKENS_CSS.find(&format!("{selector} {{")).unwrap_or_else(|| panic!("no {selector} block in tokens.css"));
    let end = TOKENS_CSS[start..].find("\n}").unwrap() + start;
    &TOKENS_CSS[start..end]
}

fn declared(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("--lc-")?;
            let (name, value) = rest.split_once(':')?;
            let value = value.trim().strip_suffix(';')?.trim();
            name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-').then(|| (format!("--lc-{name}"), value.to_owned()))
        })
        .collect()
}

fn colour(value: &str) -> bool {
    let lower = value.to_lowercase();
    lower.contains("rgb(") || lower.contains("rgba(") || lower.match_indices('#').any(|(at, _)| {
        let hex: String = lower[at + 1..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        (3..=8).contains(&hex.len())
    })
}

/// The same in both themes, on purpose (Electron's SHARED, with its reasons there).
fn shared(name: &str) -> bool {
    (name.starts_with("--lc-hue-") && name != "--lc-hue-pearl")
        || ["--lc-mark-", "--lc-filetype-", "--lc-boot-", "--lc-find-now"].iter().any(|p| name.starts_with(p))
        || name == "--lc-effort-thumb"
}

// case: gives every colour token a light value, or says why it is the same in both
#[test]
fn every_colour_token_has_a_light_value_or_a_reason() {
    let (dark, light) = (declared(block(":root")), declared(block(":root[data-theme='light']")));
    let colours = dark.values().filter(|v| colour(v)).count();
    assert!(colours > 50 && light.len() > 30, "the blocks were read: {colours} dark colours, {} light", light.len());
    let missing: Vec<&String> = dark.iter().filter(|(n, v)| colour(v) && !light.contains_key(*n) && !shared(n)).map(|(n, _)| n).collect();
    assert!(missing.is_empty(), "{missing:?}");
}

// case: names only tokens the dark theme has, so a typo cannot leave a colour behind
#[test]
fn the_light_theme_names_only_tokens_the_dark_theme_has() {
    let (dark, light) = (declared(block(":root")), declared(block(":root[data-theme='light']")));
    let strays: Vec<&String> = light.keys().filter(|n| !dark.contains_key(*n)).collect();
    assert!(strays.is_empty(), "{strays:?}");
}

// case: draws its readable text dark enough on its own window
#[test]
fn light_text_is_dark_enough_on_its_own_window() {
    let light = declared(block(":root[data-theme='light']"));
    let luminance = |hex: &str| -> f64 {
        let c = |at: usize| {
            let v = u8::from_str_radix(&hex[at..at + 2], 16).unwrap() as f64 / 255.0;
            if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * c(1) + 0.7152 * c(3) + 0.0722 * c(5)
    };
    let contrast = |a: &str, b: &str| {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    };
    for ground in ["--lc-bg-window", "--lc-bg-sidebar"] {
        let g = &light[ground];
        for name in ["--lc-text-primary", "--lc-text-secondary", "--lc-text-muted", "--lc-green-text", "--lc-blue-text", "--lc-amber-text", "--lc-red-text", "--lc-violet-text"] {
            let c = contrast(&light[name], g);
            assert!(c >= 4.5, "{name} on {ground}: {c:.2}");
        }
    }
}
