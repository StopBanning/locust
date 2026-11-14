//! Pure bot specifications, geometry, preset outlines, colors, and anchors.
//!
//! Mirrors `shared/avatar.ts`, `botSizes.ts`, `botAnchors.ts`, `locustBots.ts`,
//! the preset geometry of `bot-avatars@0.2.2`, and the pure screen/glyph/visor
//! math from `components/Bot.tsx`. Zero UI toolkit or image-decoding dependency.

use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::{Mutex, OnceLock};

pub const HEADWEAR_COUNT: u8 = 6;
pub const ACCESSORY_COUNT: u8 = 3;
pub const MOUTH_COUNT: u8 = 4;

/// All 23 bot shapes (the 18 bot-avatars presets, 2 Locust derived insects, and 3 picked-only bots).
pub const BOT_SHAPES: [&str; 23] = [
    "clover", "flower", "triangle", "square", "blob", "ghost", "circle", "drop", "star", "droid",
    "mech", "alien", "hexagon", "cat", "cloud", "pill", "pebble", "puddle", "hopper", "swarm",
    "critter", "prompt", "spark",
];

/// The first twenty shapes from which unpicked seeded teammate looks are derived.
pub const DERIVED_SHAPES: [&str; 20] = [
    "clover", "flower", "triangle", "square", "blob", "ghost", "circle", "drop", "star", "droid",
    "mech", "alien", "hexagon", "cat", "cloud", "pill", "pebble", "puddle", "hopper", "swarm",
];

/// The 12 shapes suited to wear a terminal screen face.
pub const SCREEN_SHAPES: [&str; 12] = [
    "square", "ghost", "circle", "droid", "mech", "hexagon", "cat", "pill", "pebble", "critter",
    "prompt", "spark",
];

/// The eleven hues a teammate can wear, in `tokens.css` order.
pub const HUES: [&str; 11] = [
    "lime", "blue", "violet", "clay", "teal", "butter", "rose", "slate", "pearl", "indigo",
    "coral",
];

/// Sizes across every surface (`botSizes.ts`).
pub mod bot_size {
    pub const THREAD_LIVE: f32 = 34.0;
    pub const WORKROOM_HEADER: f32 = 40.0;
    pub const ROSTER_CARD: f32 = 44.0;
    pub const SIDEBAR_FACES: f32 = 32.0;
    pub const RAIL_ROW: f32 = 34.0;
    pub const ROOM_ANSWER: f32 = 30.0;
    pub const POST_FACE: f32 = 26.0;
    pub const ROOM_SAID: f32 = 28.0;
    pub const PEER_ORIGIN: f32 = 26.0;
    pub const PEER_TOGGLE: f32 = 20.0;
    pub const CONVERSATION_OWNER: f32 = 20.0;
    pub const PICKER_MARK: f32 = 20.0;
    pub const TILE_MARK: f32 = 18.0;
    pub const MEMORY_AUTHOR: f32 = 18.0;
    pub const ROUTINE_ROW: f32 = 22.0;
    pub const SAVABLE_ROW: f32 = 20.0;
    pub const BESIDE_HEAD: f32 = 22.0;
}

/// Below or at this size in logical points, bots were rasterized at 2x (`Bot.tsx`).
pub const SUPERSAMPLE_AT_OR_BELOW: f32 = 32.0;

/// Returns the drawing size (twice when `size <= SUPERSAMPLE_AT_OR_BELOW`).
pub fn draw_size_for(size: f32) -> f32 {
    if size <= SUPERSAMPLE_AT_OR_BELOW {
        size * 2.0
    } else {
        size
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BotFace {
    Eyes,
    Mouth,
}

impl BotFace {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Eyes => "eyes",
            Self::Mouth => "mouth",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "eyes" => Some(Self::Eyes),
            "mouth" => Some(Self::Mouth),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BotSpec {
    pub shape: String,
    pub face: BotFace,
    pub screen: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetSource {
    Gallery,
    Codex,
    Bundled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetRef {
    pub source: PetSource,
    pub id: String,
    pub screen: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarSpec {
    pub headwear: u8,
    pub accessory: u8,
    pub mouth: u8,
    pub bot: Option<BotSpec>,
    pub pet: Option<PetRef>,
}

pub fn screen_suits(shape: &str) -> bool {
    SCREEN_SHAPES.contains(&shape)
}

pub fn is_valid_bot_shape(shape: &str) -> bool {
    BOT_SHAPES.contains(&shape)
}

pub fn is_locust_bot(shape: &str) -> bool {
    matches!(shape, "hopper" | "swarm" | "critter" | "prompt" | "spark")
}

pub fn is_pet_id(value: &str) -> bool {
    if value == "builtin" || value.is_empty() || value.len() > 64 {
        return false;
    }
    let mut bytes = value.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    bytes.all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

pub fn same_pet(a: Option<&PetRef>, b: Option<&PetRef>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.source == b.source && a.id == b.id,
        _ => false,
    }
}

/// FNV-1a over the id's UTF-16 first code unit per scalar, matching JavaScript's
/// `for (const ch of id) ch.charCodeAt(0)`, followed by the Murmur finalizer avalanche.
pub fn seed_parts(id: &str) -> [u8; 3] {
    let mut hash: u32 = 2_166_136_261;
    for ch in id.chars() {
        let mut units = [0u16; 2];
        hash ^= u32::from(ch.encode_utf16(&mut units)[0]);
        hash = hash.wrapping_mul(16_777_619);
    }
    hash ^= hash >> 16;
    hash = hash.wrapping_mul(0x85eb_ca6b);
    hash ^= hash >> 13;
    let part = |shift: u32, modulus: u32| (((hash >> shift) & 0xff) % modulus) as u8;
    [
        part(4, u32::from(HEADWEAR_COUNT)),
        part(12, u32::from(ACCESSORY_COUNT)),
        part(20, u32::from(MOUTH_COUNT)),
    ]
}

pub fn seed_avatar(id: &str) -> AvatarSpec {
    let [headwear, accessory, mouth] = seed_parts(id);
    AvatarSpec {
        headwear,
        accessory,
        mouth,
        bot: None,
        pet: None,
    }
}

pub fn bot_for_parts(parts: [u8; 3], picked: Option<&(String, String)>) -> (String, BotFace) {
    if let Some((shape, face)) = picked {
        if is_valid_bot_shape(shape) {
            return (
                shape.clone(),
                if face == "mouth" {
                    BotFace::Mouth
                } else {
                    BotFace::Eyes
                },
            );
        }
    }
    let [headwear, accessory, mouth] = parts.map(usize::from);
    let index = (headwear * usize::from(ACCESSORY_COUNT) * usize::from(MOUTH_COUNT)
        + accessory * usize::from(MOUTH_COUNT)
        + mouth)
        % DERIVED_SHAPES.len();
    let face = if mouth == 2 {
        BotFace::Mouth
    } else {
        BotFace::Eyes
    };
    (DERIVED_SHAPES[index].to_owned(), face)
}

pub fn bot_for(avatar: &AvatarSpec) -> BotSpec {
    if let Some(bot) = &avatar.bot {
        if is_valid_bot_shape(&bot.shape) {
            return bot.clone();
        }
    }
    let (shape, face) = bot_for_parts([avatar.headwear, avatar.accessory, avatar.mouth], None);
    BotSpec {
        shape,
        face,
        screen: None,
    }
}

pub fn shuffled_avatar(current: &AvatarSpec) -> AvatarSpec {
    AvatarSpec {
        headwear: (current.headwear + 1) % HEADWEAR_COUNT,
        accessory: (current.accessory + 1) % ACCESSORY_COUNT,
        mouth: (current.mouth + 1) % MOUTH_COUNT,
        bot: None,
        pet: None,
    }
}

pub fn clean_avatar(avatar: &AvatarSpec) -> AvatarSpec {
    AvatarSpec {
        headwear: avatar.headwear,
        accessory: avatar.accessory,
        mouth: avatar.mouth,
        bot: avatar.bot.as_ref().map(|b| BotSpec {
            shape: b.shape.clone(),
            face: b.face,
            screen: b.screen,
        }),
        pet: avatar.pet.as_ref().map(|p| PetRef {
            source: p.source,
            id: p.id.clone(),
            screen: p.screen,
        }),
    }
}

/// Deterministic fallback hue for teammates without a recorded look.
pub fn fallback_hue_for(id: &str) -> &'static str {
    let [h, a, m] = seed_parts(&format!("{id}#hue"));
    HUES[(usize::from(h) * 12 + usize::from(a) * 4 + usize::from(m)) % HUES.len()]
}

/// 0-1 FNV-1a seed used by bot choreography and idle rhythms (`TeammateBot.tsx`'s `seedOf`).
pub fn seed_of(text: &str) -> f64 {
    let mut hash: u32 = 2_166_136_261;
    for ch in text.chars() {
        let mut units = [0u16; 2];
        hash ^= u32::from(ch.encode_utf16(&mut units)[0]);
        hash = hash.wrapping_mul(16_777_619);
    }
    f64::from(hash) / 4_294_967_296.0
}

// ---------------------------------------------------------------------------
// Colors (`tokens.css`, `bot-avatars`, and `Bot.tsx`)
// ---------------------------------------------------------------------------

pub fn hue_hex(hue: &str, light_theme: bool) -> Option<&'static str> {
    match hue {
        "lime" => Some("#a9d93f"),
        "blue" => Some("#5e9bf0"),
        "violet" => Some("#a98be8"),
        "clay" => Some("#e4857a"),
        "teal" => Some("#54c7b5"),
        "butter" => Some("#e6c85c"),
        "rose" => Some("#e78fb3"),
        "slate" => Some("#93a4bd"),
        "pearl" => Some(if light_theme { "#dedde6" } else { "#ecebf2" }),
        "indigo" => Some("#3f5be8"),
        "coral" => Some("#e0603c"),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phosphor {
    Cyan,
    Green,
    Amber,
    Red,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hsla {
    /// Hue in degrees [0, 360).
    pub h: f64,
    /// Saturation in [0, 1].
    pub s: f64,
    /// Lightness in [0, 1].
    pub l: f64,
    /// Alpha in [0, 1].
    pub a: f64,
}

impl Phosphor {
    pub const fn lit_hsla(self) -> Hsla {
        match self {
            Self::Cyan => Hsla { h: 190.0, s: 1.0, l: 0.80, a: 1.0 },
            Self::Green => Hsla { h: 140.0, s: 0.90, l: 0.74, a: 1.0 },
            Self::Amber => Hsla { h: 40.0, s: 1.0, l: 0.72, a: 1.0 },
            Self::Red => Hsla { h: 356.0, s: 1.0, l: 0.77, a: 1.0 },
        }
    }

    pub const fn glow_hsla(self) -> Hsla {
        match self {
            Self::Cyan => Hsla { h: 192.0, s: 1.0, l: 0.58, a: 0.85 },
            Self::Green => Hsla { h: 140.0, s: 0.95, l: 0.48, a: 0.85 },
            Self::Amber => Hsla { h: 36.0, s: 1.0, l: 0.52, a: 0.85 },
            Self::Red => Hsla { h: 356.0, s: 1.0, l: 0.58, a: 0.85 },
        }
    }

    pub const fn lit_css(self) -> &'static str {
        match self {
            Self::Cyan => "hsl(190, 100%, 80%)",
            Self::Green => "hsl(140, 90%, 74%)",
            Self::Amber => "hsl(40, 100%, 72%)",
            Self::Red => "hsl(356, 100%, 77%)",
        }
    }

    pub const fn glow_css(self) -> &'static str {
        match self {
            Self::Cyan => "hsla(192, 100%, 58%, 0.85)",
            Self::Green => "hsla(140, 95%, 48%, 0.85)",
            Self::Amber => "hsla(36, 100%, 52%, 0.85)",
            Self::Red => "hsla(356, 100%, 58%, 0.85)",
        }
    }
}

pub fn hsl_to_rgb(h_norm: f64, s: f64, l: f64) -> [f64; 3] {
    if s <= 0.0 {
        return [l * 255.0, l * 255.0, l * 255.0];
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f64| -> f64 {
        let u = t.rem_euclid(1.0);
        let v = if u < 1.0 / 6.0 {
            p + (q - p) * 6.0 * u
        } else if u < 0.5 {
            q
        } else if u < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - u) * 6.0
        } else {
            p
        };
        v * 255.0
    };
    [
        channel(h_norm + 1.0 / 3.0),
        channel(h_norm),
        channel(h_norm - 1.0 / 3.0),
    ]
}

pub fn rgb_to_hsl([r, g, b]: [f64; 3]) -> [f64; 3] {
    let r = r / 255.0;
    let g = g / 255.0;
    let b = b / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() <= f64::EPSILON {
        return [0.0, 0.0, l];
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let mut h = if (max - r).abs() <= f64::EPSILON {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() <= f64::EPSILON {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    h /= 6.0;
    [h, s, l]
}

pub fn parse_css_rgba(color: &str) -> Option<[f64; 4]> {
    let trimmed = color.trim();
    if let Some(hex) = trimmed.strip_prefix('#') {
        let expanded = match hex.len() {
            3 => {
                let b = hex.as_bytes();
                let chars = [b[0], b[0], b[1], b[1], b[2], b[2]];
                std::str::from_utf8(&chars).ok()?.to_owned()
            }
            6 => hex.to_owned(),
            _ => hex.get(..6)?.to_owned(),
        };
        let n = u32::from_str_radix(&expanded, 16).ok()?;
        return Some([
            f64::from((n >> 16) & 255),
            f64::from((n >> 8) & 255),
            f64::from(n & 255),
            1.0,
        ]);
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(inner) = lower
        .strip_prefix("hsla(")
        .or_else(|| lower.strip_prefix("hsl("))
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|s| !s.is_empty() && *s != "/")
            .collect();
        if parts.len() >= 3 {
            let h: f64 = parts[0].trim_end_matches("deg").parse().ok()?;
            let s: f64 = parts[1].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
            let l: f64 = parts[2].trim_end_matches('%').parse::<f64>().ok()? / 100.0;
            let a: f64 = if let Some(ap) = parts.get(3) {
                ap.parse().ok()?
            } else {
                1.0
            };
            let [r, g, b] = hsl_to_rgb((h / 360.0).rem_euclid(1.0), s, l);
            return Some([r, g, b, a]);
        }
    }
    if let Some(inner) = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))
        .and_then(|s| s.strip_suffix(')'))
    {
        let parts: Vec<&str> = inner
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|s| !s.is_empty() && *s != "/")
            .collect();
        if parts.len() >= 3 {
            let r: f64 = parts[0].parse().ok()?;
            let g: f64 = parts[1].parse().ok()?;
            let b: f64 = parts[2].parse().ok()?;
            let a: f64 = if let Some(ap) = parts.get(3) {
                ap.parse().ok()?
            } else {
                1.0
            };
            return Some([r, g, b, a]);
        }
    }
    None
}

pub fn parse_css_rgb(color: &str) -> Option<[f64; 3]> {
    let [r, g, b, _] = parse_css_rgba(color)?;
    Some([r, g, b])
}

pub fn luminance(color: &str) -> f64 {
    let Some([r, g, b]) = parse_css_rgb(color) else {
        return 0.5;
    };
    let lin = |s: f64| -> f64 {
        let a = s / 255.0;
        if a <= 0.03928 {
            a / 12.92
        } else {
            ((a + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
}

pub const DARK_INK: &str = "#1E1A33";
pub const LIGHT_INK: &str = "#F7F5F2";
pub const GLYPH_INK: &str = "#010203";

pub fn auto_ink(color: &str) -> &'static str {
    if luminance(color) < 0.13 {
        LIGHT_INK
    } else {
        DARK_INK
    }
}

pub fn shade_rgb(rgb: [f64; 3], dl: f64, ds: f64) -> [f64; 3] {
    let [h, s, l] = rgb_to_hsl(rgb);
    let ns = (s + ds + if dl < 0.0 { -dl * 0.25 } else { 0.0 }).clamp(0.0, 1.0);
    let nl = (l + dl).clamp(0.0, 1.0);
    hsl_to_rgb(h, ns, nl)
}

pub fn shade_css(color: &str, dl: f64, ds: f64) -> String {
    let Some(rgb) = parse_css_rgb(color) else {
        return color.to_owned();
    };
    let [h, s, l] = rgb_to_hsl(rgb);
    let ns = (s + ds + if dl < 0.0 { -dl * 0.25 } else { 0.0 }).clamp(0.0, 1.0);
    let nl = (l + dl).clamp(0.0, 1.0);
    format!("hsl({:.1} {:.1}% {:.1}%)", h * 360.0, ns * 100.0, nl * 100.0)
}

pub fn vivify_rgb(rgb: [f64; 3], boost: f64) -> [f64; 3] {
    if !(boost > 0.0) {
        return rgb;
    }
    let srgb_to_lin = |c: f64| -> f64 {
        let a = c / 255.0;
        if a <= 0.04045 {
            a / 12.92
        } else {
            ((a + 0.055) / 1.055).powf(2.4)
        }
    };
    let lin_to_srgb = |c: f64| -> f64 {
        if c <= 0.0031308 {
            12.92 * c
        } else {
            1.055 * c.powf(1.0 / 2.4) - 0.055
        }
    };
    let r = srgb_to_lin(rgb[0]);
    let g = srgb_to_lin(rgb[1]);
    let b = srgb_to_lin(rgb[2]);
    let l_ = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m_ = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s_ = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    let ok_l = 0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_;
    let ok_a = 1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_;
    let ok_b = 0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_;
    let chroma = ok_a.hypot(ok_b);
    if chroma < 1e-4 {
        return rgb;
    }
    let ca = ok_a / chroma;
    let cb = ok_b / chroma;
    let target_c = chroma * (1.0 + 0.5 * boost);
    let oklab_to_lin = |l: f64, a: f64, b: f64| -> [f64; 3] {
        let lp = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
        let mp = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
        let sp = (l - 0.0894841775 * a - 1.291485548 * b).powi(3);
        [
            4.0767416621 * lp - 3.3077115913 * mp + 0.2309699292 * sp,
            -1.2684380046 * lp + 2.6097574011 * mp - 0.3413193965 * sp,
            -0.0041960863 * lp - 0.7034186147 * mp + 1.707614701 * sp,
        ]
    };
    let in_gamut = |lin: [f64; 3]| -> bool {
        lin.iter().all(|&c| c >= -1e-4 && c <= 1.0 + 1e-4)
    };
    let mut best: Option<[f64; 3]> = None;
    let mut best_c = chroma;
    for f in 0..=6 {
        let bl = ok_l * (1.0 - 0.15 * boost * f64::from(f) / 6.0);
        let mut lo = 0.0;
        let mut hi = target_c;
        for _ in 0..18 {
            let mid = (lo + hi) / 2.0;
            if in_gamut(oklab_to_lin(bl, ca * mid, cb * mid)) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if lo > best_c + 4e-3 * f64::from(f) {
            best_c = lo;
            best = Some(oklab_to_lin(bl, ca * lo, cb * lo));
        }
        if lo >= target_c - 1e-4 {
            break;
        }
    }
    match best {
        Some(lin) => [
            (255.0 * lin_to_srgb(lin[0].clamp(0.0, 1.0))).round(),
            (255.0 * lin_to_srgb(lin[1].clamp(0.0, 1.0))).round(),
            (255.0 * lin_to_srgb(lin[2].clamp(0.0, 1.0))).round(),
        ],
        None => rgb,
    }
}

pub fn hue_of(color: &str) -> f64 {
    let lower = color.trim().to_ascii_lowercase();
    if let Some(inner) = lower
        .strip_prefix("hsla(")
        .or_else(|| lower.strip_prefix("hsl("))
        .and_then(|s| s.strip_suffix(')'))
    {
        if let Some(first) = inner
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .find(|s| !s.is_empty())
        {
            if let Ok(deg) = first.trim_end_matches("deg").parse::<f64>() {
                return deg.rem_euclid(360.0);
            }
        }
    }
    let Some(rgb) = parse_css_rgb(color) else {
        return 0.0;
    };
    let [h, _, _] = rgb_to_hsl(rgb);
    (h * 360.0).rem_euclid(360.0)
}

pub fn same_hue(color: &str, saturation: f64, lightness: f64, alpha: f64) -> String {
    format!(
        "hsla({}, {}%, {}%, {})",
        hue_of(color).round() as i64,
        (saturation * 100.0).round() as i64,
        (lightness * 100.0).round() as i64,
        alpha
    )
}

pub fn same_hue_rgba(color: &str, saturation: f64, lightness: f64, alpha: f64) -> [u8; 4] {
    let deg = hue_of(color).round().rem_euclid(360.0);
    let s = (saturation * 100.0).round() / 100.0;
    let l = (lightness * 100.0).round() / 100.0;
    let [r, g, b] = hsl_to_rgb(deg / 360.0, s, l);
    [
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8,
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

pub fn phosphor_rgba(color: &str) -> [f64; 4] {
    let Some([r, g, b, a]) = parse_css_rgba(color) else {
        return [255.0, 255.0, 255.0, 1.0];
    };
    [r.round(), g.round(), b.round(), a]
}

pub fn mix_phosphor_rgba(from: [f64; 4], to: [f64; 4], k: f64) -> [u8; 4] {
    let mix = |i: usize| from[i] + (to[i] - from[i]) * k;
    [
        mix(0).round().clamp(0.0, 255.0) as u8,
        mix(1).round().clamp(0.0, 255.0) as u8,
        mix(2).round().clamp(0.0, 255.0) as u8,
        (mix(3).clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

pub fn mix_phosphor_css(from: &str, to: &str, k: f64) -> String {
    let a = phosphor_rgba(from);
    let b = phosphor_rgba(to);
    let mix = |i: usize| a[i] + (b[i] - a[i]) * k;
    let alpha = ((mix(3) * 1000.0).round() / 1000.0)
        .to_string()
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned();
    let alpha_str = if alpha.is_empty() { "0".to_owned() } else { alpha };
    format!(
        "rgba({}, {}, {}, {})",
        mix(0).round() as i64,
        mix(1).round() as i64,
        mix(2).round() as i64,
        alpha_str
    )
}

// ---------------------------------------------------------------------------
// Presets (`bot-avatars@0.2.2`) & Locust's own mascots (`locustBots.ts`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BotPreset {
    pub id: &'static str,
    pub label: &'static str,
    pub color: &'static str,
    pub face: BotFace,
    pub face_x: f64,
    pub face_y: f64,
    pub face_scale: f64,
    pub brightness: Option<f64>,
    pub saturation: Option<f64>,
    pub body: &'static str,
    pub parts: &'static str,
}

pub const BOT_AVATAR_PRESETS: [BotPreset; 18] = [
    BotPreset {
        id: "clover",
        label: "Clover",
        color: "#41C4FF",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 50.0,
        face_scale: 1.0,
        brightness: Some(1.2),
        saturation: Some(1.59),
        body: "M26.53 22.38A25 25 0 0 1 73.47 22.38A7 7 0 0 0 77.62 26.53A25 25 0 0 1 77.62 73.47A7 7 0 0 0 73.47 77.62A25 25 0 0 1 26.53 77.62A7 7 0 0 0 22.38 73.47A25 25 0 0 1 22.38 26.53A7 7 0 0 0 26.53 22.38Z",
        parts: "",
    },
    BotPreset {
        id: "flower",
        label: "Flower",
        color: "#2FCB7A",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 51.0,
        face_scale: 0.95,
        brightness: None,
        saturation: None,
        body: "M31.2 19.82A20.5 20.5 0 0 1 68.8 19.82A5 5 0 0 0 72.9 22.8A20.5 20.5 0 0 1 84.51 58.55A5 5 0 0 0 82.95 63.37A20.5 20.5 0 0 1 52.54 85.47A5 5 0 0 0 47.46 85.47A20.5 20.5 0 0 1 17.05 63.37A5 5 0 0 0 15.49 58.55A20.5 20.5 0 0 1 27.1 22.8A5 5 0 0 0 31.2 19.82Z",
        parts: "",
    },
    BotPreset {
        id: "triangle",
        label: "Triangle",
        color: "#DC48FF",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 61.0,
        face_scale: 0.9,
        brightness: None,
        saturation: None,
        body: "M38.75 27.43A13 13 0 0 1 61.25 27.43L82.7 64.49A13 13 0 0 1 71.45 84L28.55 84A13 13 0 0 1 17.3 64.49L38.75 27.43Z",
        parts: "",
    },
    BotPreset {
        id: "square",
        label: "Square",
        color: "#35B8FF",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 50.0,
        face_scale: 1.0,
        brightness: None,
        saturation: None,
        body: "M93 50C93 53.66 92.96 58.23 92.88 60.97C92.8 63.71 92.67 64.81 92.51 66.44C92.35 68.08 92.15 69.44 91.9 70.77C91.66 72.1 91.37 73.3 91.04 74.44C90.72 75.58 90.35 76.63 89.94 77.63C89.53 78.63 89.07 79.55 88.58 80.43C88.08 81.31 87.54 82.13 86.96 82.9C86.37 83.67 85.75 84.39 85.07 85.07C84.39 85.75 83.67 86.37 82.9 86.96C82.13 87.54 81.31 88.08 80.43 88.58C79.55 89.07 78.63 89.53 77.63 89.94C76.63 90.35 75.58 90.72 74.44 91.04C73.3 91.37 72.1 91.66 70.77 91.9C69.44 92.15 68.08 92.35 66.44 92.51C64.81 92.67 63.71 92.8 60.97 92.88C58.23 92.96 53.66 93 50 93C46.34 93 41.77 92.96 39.03 92.88C36.29 92.8 35.19 92.67 33.56 92.51C31.92 92.35 30.56 92.15 29.23 91.9C27.9 91.66 26.7 91.37 25.56 91.04C24.42 90.72 23.37 90.35 22.37 89.94C21.37 89.53 20.45 89.07 19.57 88.58C18.69 88.08 17.87 87.54 17.1 86.96C16.33 86.37 15.61 85.75 14.93 85.07C14.25 84.39 13.63 83.67 13.04 82.9C12.46 82.13 11.92 81.31 11.42 80.43C10.93 79.55 10.47 78.63 10.06 77.63C9.65 76.63 9.28 75.58 8.96 74.44C8.63 73.3 8.34 72.1 8.1 70.77C7.85 69.44 7.65 68.08 7.49 66.44C7.33 64.81 7.2 63.71 7.12 60.97C7.04 58.23 7 53.66 7 50C7 46.34 7.04 41.77 7.12 39.03C7.2 36.29 7.33 35.19 7.49 33.56C7.65 31.92 7.85 30.56 8.1 29.23C8.34 27.9 8.63 26.7 8.96 25.56C9.28 24.42 9.65 23.37 10.06 22.37C10.47 21.37 10.93 20.45 11.42 19.57C11.92 18.69 12.46 17.87 13.04 17.1C13.63 16.33 14.25 15.61 14.93 14.93C15.61 14.25 16.33 13.63 17.1 13.04C17.87 12.46 18.69 11.92 19.57 11.42C20.45 10.93 21.37 10.47 22.37 10.06C23.37 9.65 24.42 9.28 25.56 8.96C26.7 8.63 27.9 8.34 29.23 8.1C30.56 7.85 31.92 7.65 33.56 7.49C35.19 7.33 36.29 7.2 39.03 7.12C41.77 7.04 46.34 7 50 7C53.66 7 58.23 7.04 60.97 7.12C63.71 7.2 64.81 7.33 66.44 7.49C68.08 7.65 69.44 7.85 70.77 8.1C72.1 8.34 73.3 8.63 74.44 8.96C75.58 9.28 76.63 9.65 77.63 10.06C78.63 10.47 79.55 10.93 80.43 11.42C81.31 11.92 82.13 12.46 82.9 13.04C83.67 13.63 84.39 14.25 85.07 14.93C85.75 15.61 86.37 16.33 86.96 17.1C87.54 17.87 88.08 18.69 88.58 19.57C89.07 20.45 89.53 21.37 89.94 22.37C90.35 23.37 90.72 24.42 91.04 25.56C91.37 26.7 91.66 27.9 91.9 29.23C92.15 30.56 92.35 31.92 92.51 33.56C92.67 35.19 92.8 36.29 92.88 39.03C92.96 41.77 93 46.34 93 50Z",
        parts: "",
    },
    BotPreset {
        id: "blob",
        label: "Blob",
        color: "#2FCB7A",
        face: BotFace::Eyes,
        face_x: 49.5,
        face_y: 50.0,
        face_scale: 1.0,
        brightness: None,
        saturation: None,
        body: "M93.92 50C94.18 51.9 94.24 53.9 93.99 55.79C93.73 57.68 93.18 59.61 92.39 61.36C91.6 63.1 90.48 64.78 89.27 66.27C88.07 67.76 86.58 69.08 85.14 70.29C83.7 71.49 82.1 72.5 80.62 73.5C79.14 74.49 77.64 75.34 76.24 76.24C74.84 77.15 73.51 78 72.2 78.93C70.88 79.86 69.66 80.83 68.36 81.8C67.06 82.76 65.78 83.82 64.39 84.74C63 85.67 61.55 86.63 60.01 87.35C58.47 88.08 56.82 88.73 55.15 89.1C53.48 89.47 51.71 89.64 50 89.57C48.29 89.5 46.55 89.16 44.91 88.68C43.27 88.21 41.67 87.47 40.16 86.72C38.65 85.96 37.24 85.02 35.86 84.13C34.48 83.24 33.19 82.29 31.89 81.37C30.58 80.46 29.32 79.56 28.03 78.63C26.74 77.71 25.44 76.82 24.16 75.84C22.88 74.86 21.57 73.87 20.36 72.74C19.15 71.62 17.95 70.42 16.91 69.1C15.88 67.79 14.92 66.35 14.15 64.85C13.38 63.35 12.76 61.73 12.29 60.1C11.81 58.48 11.52 56.78 11.29 55.1C11.06 53.41 10.98 51.71 10.91 50C10.83 48.29 10.83 46.59 10.83 44.84C10.84 43.1 10.85 41.34 10.93 39.53C11.02 37.72 11.09 35.86 11.34 33.99C11.59 32.12 11.88 30.16 12.43 28.31C12.99 26.46 13.69 24.56 14.66 22.88C15.64 21.21 16.86 19.58 18.27 18.27C19.69 16.96 21.4 15.84 23.17 15.03C24.93 14.22 26.95 13.71 28.89 13.43C30.83 13.15 32.9 13.21 34.82 13.36C36.74 13.5 38.66 13.92 40.43 14.28C42.2 14.65 43.87 15.16 45.46 15.54C47.06 15.93 48.52 16.31 50 16.58C51.48 16.85 52.87 17.02 54.32 17.17C55.77 17.32 57.22 17.36 58.71 17.49C60.2 17.62 61.74 17.7 63.28 17.95C64.81 18.19 66.39 18.5 67.91 18.98C69.43 19.46 70.95 20.08 72.38 20.83C73.81 21.58 75.19 22.5 76.5 23.5C77.81 24.5 79.03 25.63 80.22 26.81C81.41 27.99 82.54 29.25 83.65 30.57C84.76 31.89 85.85 33.26 86.89 34.72C87.93 36.18 88.98 37.69 89.9 39.31C90.82 40.92 91.73 42.64 92.4 44.42C93.07 46.2 93.66 48.1 93.92 50Z",
        parts: "",
    },
    BotPreset {
        id: "ghost",
        label: "Ghost",
        color: "#F4F2FA",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 48.0,
        face_scale: 0.95,
        brightness: None,
        saturation: None,
        body: "M17 50C17 31.78 31.78 17 50 17C68.22 17 83 31.78 83 50V81.5Q72 93.5 61 81.5Q50 93.5 39 81.5Q28 93.5 17 81.5Z",
        parts: "",
    },
    BotPreset {
        id: "circle",
        label: "Circle",
        color: "#9A62FF",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 50.0,
        face_scale: 1.0,
        brightness: None,
        saturation: None,
        body: "M50 8A42 42 0 1 1 50 92A42 42 0 1 1 50 8Z",
        parts: "",
    },
    BotPreset {
        id: "drop",
        label: "Drop",
        color: "#1ED3C6",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 62.0,
        face_scale: 0.9,
        brightness: None,
        saturation: None,
        body: "M50 8.5C52.2 8.5 53.4 10.3 56.4 15.2C62.9 25.5 84 44.6 84 61.5C84 80.3 68.8 92 50 92C31.2 92 16 80.3 16 61.5C16 44.6 37.1 25.5 43.6 15.2C46.6 10.3 47.8 8.5 50 8.5Z",
        parts: "",
    },
    BotPreset {
        id: "star",
        label: "Star",
        color: "#EFDB8E",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 52.0,
        face_scale: 0.82,
        brightness: Some(1.1),
        saturation: Some(1.84),
        body: "M45.77 9.7A5 5 0 0 1 54.23 9.7L65.02 26.81A4 4 0 0 0 67.42 28.55L87.02 33.53A5 5 0 0 1 89.63 41.57L76.7 57.12A4 4 0 0 0 75.78 59.94L77.11 80.12A5 5 0 0 1 70.26 85.09L51.48 77.59A4 4 0 0 0 48.52 77.59L29.74 85.09A5 5 0 0 1 22.89 80.12L24.22 59.94A4 4 0 0 0 23.3 57.12L10.37 41.57A5 5 0 0 1 12.98 33.53L32.58 28.55A4 4 0 0 0 34.98 26.81L45.77 9.7Z",
        parts: "",
    },
    BotPreset {
        id: "droid",
        label: "Droid",
        color: "#D5DBEA",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 60.0,
        face_scale: 0.95,
        brightness: None,
        saturation: None,
        body: "M16 50C16 38.95 24.95 30 36 30H64C75.05 30 84 38.95 84 50V70C84 81.05 75.05 90 64 90H36C24.95 90 16 81.05 16 70ZM4 62A7 7 0 1 1 18 62A7 7 0 1 1 4 62ZM82 62A7 7 0 1 1 96 62A7 7 0 1 1 82 62Z",
        parts: "M47.5 14H52.5V32H47.5ZM43 11A7 7 0 1 1 57 11A7 7 0 1 1 43 11Z",
    },
    BotPreset {
        id: "mech",
        label: "Mech",
        color: "#95A6C4",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 59.0,
        face_scale: 1.0,
        brightness: None,
        saturation: None,
        body: "M10 48C10 38.06 18.06 30 28 30H72C81.94 30 90 38.06 90 48V70C90 79.94 81.94 88 72 88H28C18.06 88 10 79.94 10 70ZM3 54C3 51.79 4.79 50 7 50H11V72H7C4.79 72 3 70.21 3 68ZM89 50H93C95.21 50 97 51.79 97 54V68C97 70.21 95.21 72 93 72H89Z",
        parts: "M19.5 32L24.5 32L17 13L12 13ZM75.5 32L80.5 32L88 13L83 13ZM10 11.5A4.5 4.5 0 1 1 19 11.5A4.5 4.5 0 1 1 10 11.5ZM81 11.5A4.5 4.5 0 1 1 90 11.5A4.5 4.5 0 1 1 81 11.5Z",
    },
    BotPreset {
        id: "alien",
        label: "Alien",
        color: "#9CD66A",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 45.0,
        face_scale: 1.05,
        brightness: None,
        saturation: None,
        body: "M50 10C70 10 83 27 83 46C83 65 64 92 50 92C36 92 17 65 17 46C17 27 30 10 50 10Z",
        parts: "",
    },
    BotPreset {
        id: "hexagon",
        label: "Hexagon",
        color: "#EB7575",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 50.0,
        face_scale: 0.95,
        brightness: None,
        saturation: None,
        body: "M91.4 45.5A9 9 0 0 1 91.4 54.5L74.6 83.61A9 9 0 0 1 66.8 88.11L33.2 88.11A9 9 0 0 1 25.4 83.61L8.6 54.5A9 9 0 0 1 8.6 45.5L25.4 16.39A9 9 0 0 1 33.2 11.89L66.8 11.89A9 9 0 0 1 74.6 16.39L91.4 45.5Z",
        parts: "",
    },
    BotPreset {
        id: "cat",
        label: "Cat",
        color: "#EAA06F",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 58.0,
        face_scale: 1.0,
        brightness: Some(1.22),
        saturation: Some(1.88),
        body: "M50 20A36 36 0 1 1 50 92A36 36 0 1 1 50 20ZM24.72 44.64A4.5 4.5 0 0 1 17.35 40.67L20.21 16.66A4.5 4.5 0 0 1 26.86 13.26L42.3 21.83A4.5 4.5 0 0 1 43.02 29.21L24.72 44.64ZM82.65 40.67A4.5 4.5 0 0 1 75.28 44.64L56.98 29.21A4.5 4.5 0 0 1 57.7 21.83L73.14 13.26A4.5 4.5 0 0 1 79.79 16.66L82.65 40.67Z",
        parts: "",
    },
    BotPreset {
        id: "cloud",
        label: "Cloud",
        color: "#CFD5E4",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 58.0,
        face_scale: 0.95,
        brightness: None,
        saturation: None,
        body: "M19 44A25 25 0 1 1 69 44A25 25 0 1 1 19 44ZM47 50A21 21 0 1 1 89 50A21 21 0 1 1 47 50ZM7 68A17 17 0 1 1 41 68A17 17 0 1 1 7 68ZM32 73A18 18 0 1 1 68 73A18 18 0 1 1 32 73ZM60 70A16 16 0 1 1 92 70A16 16 0 1 1 60 70Z",
        parts: "",
    },
    BotPreset {
        id: "pill",
        label: "Pill",
        color: "#ACAAF3",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 50.0,
        face_scale: 0.9,
        brightness: None,
        saturation: None,
        body: "M28 28H72C84.15 28 94 37.85 94 50C94 62.15 84.15 72 72 72H28C15.85 72 6 62.15 6 50C6 37.85 15.85 28 28 28Z",
        parts: "",
    },
    BotPreset {
        id: "pebble",
        label: "Pebble",
        color: "#ABC793",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 50.0,
        face_scale: 0.95,
        brightness: None,
        saturation: None,
        body: "M94.6 50C94.71 51.92 94.56 53.93 94.17 55.82C93.79 57.7 93.13 59.6 92.28 61.33C91.44 63.06 90.32 64.72 89.1 66.19C87.87 67.67 86.41 69.01 84.92 70.16C83.42 71.31 81.75 72.29 80.11 73.1C78.47 73.92 76.73 74.55 75.06 75.06C73.39 75.58 71.7 75.91 70.1 76.2C68.5 76.48 66.93 76.62 65.45 76.76C63.96 76.9 62.55 76.94 61.2 77.03C59.84 77.11 58.57 77.16 57.31 77.26C56.05 77.36 54.86 77.48 53.64 77.63C52.42 77.79 51.24 77.98 50 78.18C48.76 78.39 47.52 78.64 46.2 78.85C44.89 79.06 43.53 79.3 42.11 79.46C40.69 79.61 39.2 79.75 37.67 79.77C36.14 79.78 34.54 79.74 32.95 79.54C31.35 79.33 29.7 79.03 28.1 78.55C26.49 78.07 24.86 77.45 23.33 76.67C21.8 75.88 20.29 74.94 18.92 73.85C17.54 72.77 16.23 71.51 15.08 70.16C13.94 68.8 12.9 67.29 12.04 65.72C11.17 64.16 10.45 62.46 9.91 60.74C9.36 59.03 8.98 57.22 8.76 55.43C8.55 53.64 8.5 51.8 8.6 50C8.71 48.2 8.98 46.39 9.38 44.65C9.79 42.91 10.35 41.19 11.02 39.56C11.7 37.92 12.52 36.34 13.43 34.85C14.34 33.37 15.39 31.96 16.5 30.66C17.61 29.36 18.84 28.15 20.11 27.07C21.39 25.98 22.76 25 24.15 24.15C25.54 23.3 27.01 22.56 28.48 21.95C29.94 21.33 31.46 20.85 32.95 20.46C34.44 20.08 35.95 19.83 37.43 19.66C38.91 19.48 40.38 19.43 41.81 19.43C43.24 19.43 44.64 19.53 46.01 19.66C47.37 19.78 48.7 19.98 50 20.18C51.3 20.39 52.57 20.63 53.83 20.87C55.1 21.11 56.34 21.37 57.6 21.62C58.87 21.87 60.13 22.12 61.43 22.39C62.74 22.66 64.07 22.93 65.45 23.24C66.83 23.56 68.25 23.88 69.72 24.3C71.19 24.72 72.71 25.17 74.25 25.75C75.78 26.34 77.37 27 78.91 27.81C80.46 28.63 82.04 29.56 83.5 30.66C84.97 31.75 86.43 33 87.7 34.38C88.98 35.77 90.19 37.32 91.17 38.97C92.14 40.62 92.98 42.43 93.56 44.27C94.13 46.1 94.5 48.08 94.6 50Z",
        parts: "",
    },
    BotPreset {
        id: "puddle",
        label: "Puddle",
        color: "#EE8BDB",
        face: BotFace::Eyes,
        face_x: 50.0,
        face_y: 50.0,
        face_scale: 0.95,
        brightness: None,
        saturation: None,
        body: "M85.34 50C85.65 51.54 85.9 53.13 86.01 54.74C86.11 56.35 86.11 58 85.95 59.63C85.8 61.26 85.51 62.93 85.07 64.52C84.62 66.12 84.03 67.72 83.3 69.23C82.58 70.74 81.7 72.21 80.72 73.57C79.74 74.94 78.62 76.23 77.43 77.43C76.24 78.62 74.93 79.72 73.58 80.73C72.23 81.74 70.79 82.65 69.33 83.48C67.86 84.31 66.34 85.04 64.79 85.71C63.24 86.37 61.65 86.95 60.04 87.46C58.42 87.96 56.77 88.4 55.1 88.75C53.43 89.09 51.72 89.37 50 89.54C48.28 89.71 46.52 89.81 44.76 89.76C43.01 89.72 41.22 89.58 39.47 89.29C37.72 89 35.95 88.58 34.26 88C32.57 87.42 30.89 86.69 29.33 85.8C27.77 84.92 26.26 83.87 24.91 82.7C23.56 81.53 22.3 80.19 21.23 78.77C20.16 77.35 19.22 75.79 18.47 74.2C17.71 72.61 17.13 70.91 16.7 69.23C16.26 67.55 16.02 65.81 15.88 64.13C15.75 62.45 15.78 60.76 15.88 59.14C15.97 57.52 16.21 55.94 16.45 54.42C16.7 52.89 17.03 51.43 17.34 50C17.65 48.57 18 47.2 18.32 45.83C18.63 44.46 18.94 43.13 19.24 41.76C19.53 40.39 19.79 39.03 20.08 37.61C20.36 36.18 20.62 34.74 20.95 33.23C21.28 31.72 21.61 30.16 22.07 28.57C22.52 26.98 23.02 25.32 23.69 23.69C24.35 22.06 25.11 20.38 26.05 18.79C26.99 17.21 28.08 15.62 29.33 14.2C30.58 12.78 32 11.41 33.54 10.26C35.08 9.12 36.8 8.11 38.57 7.35C40.34 6.6 42.27 6.04 44.17 5.73C46.08 5.43 48.08 5.37 50 5.54C51.92 5.71 53.87 6.15 55.69 6.75C57.52 7.36 59.3 8.22 60.94 9.19C62.57 10.15 64.11 11.33 65.51 12.56C66.91 13.78 68.17 15.15 69.33 16.52C70.48 17.89 71.5 19.34 72.44 20.76C73.38 22.18 74.19 23.62 74.97 25.03C75.75 26.43 76.44 27.83 77.12 29.19C77.8 30.56 78.42 31.89 79.05 33.23C79.67 34.57 80.28 35.87 80.87 37.21C81.46 38.55 82.05 39.88 82.6 41.27C83.14 42.65 83.68 44.05 84.14 45.51C84.6 46.96 85.03 48.46 85.34 50Z",
        parts: "",
    },
];

pub fn preset_of(id: &str) -> Option<&'static BotPreset> {
    BOT_AVATAR_PRESETS.iter().find(|p| p.id == id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChestMarkKind {
    Prompt,
    Spark,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChestMark {
    pub mark: ChestMarkKind,
    pub x: f64,
    pub y: f64,
    pub size: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LocustBotShape {
    pub key: &'static str,
    pub name: &'static str,
    pub body: String,
    pub parts: String,
    pub parts_depth: f64,
    pub turn: f64,
    pub face_x: f64,
    pub face_y: f64,
    pub face_scale: f64,
    pub screen: bool,
    pub chest: Option<ChestMark>,
}

fn round_2(n: f64) -> f64 {
    (n * 100.0).round() / 100.0
}

fn fmt_2(n: f64) -> String {
    let v = round_2(n);
    if v == 0.0 {
        return "0".to_owned();
    }
    let s = format!("{v:.2}");
    if let Some(stripped) = s.strip_suffix(".00") {
        stripped.to_owned()
    } else if s.contains('.') {
        s.trim_end_matches('0').to_owned()
    } else {
        s
    }
}

pub fn ellipse_path(cx: f64, cy: f64, rx: f64, ry: f64, deg: f64) -> String {
    let t = deg * PI / 180.0;
    let x0 = cx + rx * t.cos();
    let y0 = cy + rx * t.sin();
    let x1 = cx - rx * t.cos();
    let y1 = cy - rx * t.sin();
    format!(
        "M{} {}A{} {} {} 1 1 {} {}A{} {} {} 1 1 {} {}Z",
        fmt_2(x0),
        fmt_2(y0),
        fmt_2(rx),
        fmt_2(ry),
        fmt_2(deg),
        fmt_2(x1),
        fmt_2(y1),
        fmt_2(rx),
        fmt_2(ry),
        fmt_2(deg),
        fmt_2(x0),
        fmt_2(y0)
    )
}

pub fn rod_path(ax: f64, ay: f64, bx: f64, by: f64, r: f64) -> String {
    let length = (bx - ax).hypot(by - ay);
    if length <= 1e-9 {
        return String::new();
    }
    let nx = (-(by - ay) / length) * r;
    let ny = ((bx - ax) / length) * r;
    format!(
        "M{} {}L{} {}A{} {} 0 0 0 {} {}L{} {}A{} {} 0 0 0 {} {}Z",
        fmt_2(ax + nx),
        fmt_2(ay + ny),
        fmt_2(bx + nx),
        fmt_2(by + ny),
        fmt_2(r),
        fmt_2(r),
        fmt_2(bx - nx),
        fmt_2(by - ny),
        fmt_2(ax - nx),
        fmt_2(ay - ny),
        fmt_2(r),
        fmt_2(r),
        fmt_2(ax + nx),
        fmt_2(ay + ny)
    )
}

pub fn cloud_path(cx: f64, cy: f64, core: f64, reach: f64, lobe: f64, lobes: usize, turn: f64) -> String {
    let mut path = ellipse_path(cx, cy, core, core, 0.0);
    for i in 0..lobes {
        let angle = (i as f64 / lobes as f64) * PI * 2.0 + turn;
        path.push_str(&ellipse_path(
            round_2(cx + angle.cos() * reach),
            round_2(cy + angle.sin() * reach),
            lobe,
            lobe,
            0.0,
        ));
    }
    path
}

pub fn round_rect_path(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    format!(
        "M{} {}H{}A{} {} 0 0 1 {} {}V{}A{} {} 0 0 1 {} {}H{}A{} {} 0 0 1 {} {}V{}A{} {} 0 0 1 {} {}Z",
        fmt_2(x + r),
        fmt_2(y),
        fmt_2(x + w - r),
        fmt_2(r),
        fmt_2(r),
        fmt_2(x + w),
        fmt_2(y + r),
        fmt_2(y + h - r),
        fmt_2(r),
        fmt_2(r),
        fmt_2(x + w - r),
        fmt_2(y + h),
        fmt_2(x + r),
        fmt_2(r),
        fmt_2(r),
        fmt_2(x),
        fmt_2(y + h - r),
        fmt_2(y + r),
        fmt_2(r),
        fmt_2(r),
        fmt_2(x + r),
        fmt_2(y),
    )
}

fn thigh_path(ax: f64, ay: f64, bx: f64, by: f64, width: f64) -> String {
    let deg = round_2((by - ay).atan2(bx - ax) * 180.0 / PI);
    ellipse_path(
        (ax + bx) / 2.0,
        (ay + by) / 2.0,
        (bx - ax).hypot(by - ay) / 2.0 + 2.0,
        width,
        deg,
    )
}

pub fn union_outline(centre: [f64; 2], rounds: &[[f64; 5]], steps: usize) -> String {
    let [cx, cy] = centre;
    let mut out = String::new();
    for i in 0..steps {
        let angle = -PI / 2.0 + (i as f64 / steps as f64) * PI * 2.0;
        let dx = angle.cos();
        let dy = angle.sin();
        let mut reach = 0.0_f64;
        for &[x, y, rx, ry, turn] in rounds {
            let cos = (-turn).cos();
            let sin = (-turn).sin();
            let px = cx - x;
            let py = cy - y;
            let ox = (px * cos - py * sin) / rx;
            let oy = (px * sin + py * cos) / ry;
            let ux = (dx * cos - dy * sin) / rx;
            let uy = (dx * sin + dy * cos) / ry;
            let a = ux * ux + uy * uy;
            let b = 2.0 * (ox * ux + oy * uy);
            let c = ox * ox + oy * oy - 1.0;
            let disc = b * b - 4.0 * a * c;
            if disc >= 0.0 && a.abs() > 1e-12 {
                reach = reach.max((-b + disc.sqrt()) / (2.0 * a));
            }
        }
        out.push(if i == 0 { 'M' } else { 'L' });
        out.push_str(&fmt_2(cx + dx * reach));
        out.push(' ');
        out.push_str(&fmt_2(cy + dy * reach));
    }
    out.push('Z');
    out
}

pub fn grown_path(path: &str, k: f64, ox: f64, oy: f64) -> String {
    let mut tokens: Vec<&str> = Vec::new();
    let bytes = path.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let b = bytes[idx];
        if b.is_ascii_alphabetic() && matches!(b.to_ascii_uppercase(), b'M' | b'L' | b'A' | b'Z') {
            tokens.push(&path[idx..idx + 1]);
            idx += 1;
        } else if b == b'-' || b == b'.' || b.is_ascii_digit() {
            let start = idx;
            if b == b'-' {
                idx += 1;
            }
            while idx < bytes.len()
                && (bytes[idx].is_ascii_digit() || bytes[idx] == b'.')
            {
                idx += 1;
            }
            if idx < bytes.len() && (bytes[idx] == b'e' || bytes[idx] == b'E') {
                idx += 1;
                if idx < bytes.len() && (bytes[idx] == b'-' || bytes[idx] == b'+') {
                    idx += 1;
                }
                while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                    idx += 1;
                }
            }
            tokens.push(&path[start..idx]);
        } else {
            idx += 1;
        }
    }
    let mut out = String::new();
    let mut ti = 0;
    let next_num = |tokens: &[&str], ti: &mut usize| -> Option<f64> {
        let s = tokens.get(*ti)?;
        *ti += 1;
        s.parse().ok()
    };
    while ti < tokens.len() {
        let cmd = tokens[ti];
        ti += 1;
        match cmd {
            "M" | "L" => {
                if let (Some(vx), Some(vy)) = (next_num(&tokens, &mut ti), next_num(&tokens, &mut ti))
                {
                    out.push_str(cmd);
                    out.push_str(&fmt_2(ox + (vx - ox) * k));
                    out.push(' ');
                    out.push_str(&fmt_2(oy + (vy - oy) * k));
                }
            }
            "A" => {
                if let (
                    Some(rx),
                    Some(ry),
                    Some(turn),
                    Some(large),
                    Some(sweep),
                    Some(vx),
                    Some(vy),
                ) = (
                    next_num(&tokens, &mut ti),
                    next_num(&tokens, &mut ti),
                    next_num(&tokens, &mut ti),
                    next_num(&tokens, &mut ti),
                    next_num(&tokens, &mut ti),
                    next_num(&tokens, &mut ti),
                    next_num(&tokens, &mut ti),
                ) {
                    out.push('A');
                    out.push_str(&fmt_2(rx * k));
                    out.push(' ');
                    out.push_str(&fmt_2(ry * k));
                    out.push(' ');
                    out.push_str(&fmt_2(turn));
                    out.push(' ');
                    out.push_str(&fmt_2(large));
                    out.push(' ');
                    out.push_str(&fmt_2(sweep));
                    out.push(' ');
                    out.push_str(&fmt_2(ox + (vx - ox) * k));
                    out.push(' ');
                    out.push_str(&fmt_2(oy + (vy - oy) * k));
                }
            }
            "Z" => out.push('Z'),
            _ => {}
        }
    }
    out
}

fn grow_locust_shape(shape: LocustBotShape, k: f64, ox: f64, oy: f64) -> LocustBotShape {
    LocustBotShape {
        key: shape.key,
        name: shape.name,
        body: grown_path(&shape.body, k, ox, oy),
        parts: if shape.parts.is_empty() {
            String::new()
        } else {
            grown_path(&shape.parts, k, ox, oy)
        },
        parts_depth: shape.parts_depth,
        turn: shape.turn,
        face_x: ox + (shape.face_x - ox) * k,
        face_y: oy + (shape.face_y - oy) * k,
        face_scale: shape.face_scale * k,
        screen: shape.screen,
        chest: shape.chest.map(|c| ChestMark {
            mark: c.mark,
            x: ox + (c.x - ox) * k,
            y: oy + (c.y - oy) * k,
            size: c.size * k,
        }),
    }
}

pub fn locust_bot(key: &str) -> Option<&'static LocustBotShape> {
    static SHAPES: OnceLock<[LocustBotShape; 5]> = OnceLock::new();
    let list = SHAPES.get_or_init(|| {
        let hopper = LocustBotShape {
            key: "hopper",
            name: "Hopper",
            body: ellipse_path(50.0, 62.0, 28.0, 27.0, 0.0),
            parts: format!(
                "{}{}{}{}{}{}",
                rod_path(43.0, 38.0, 31.0, 7.0, 2.2),
                rod_path(57.0, 38.0, 69.0, 7.0, 2.2),
                thigh_path(33.0, 74.0, 13.0, 37.0, 6.2),
                rod_path(13.0, 37.0, 16.0, 91.0, 2.5),
                thigh_path(67.0, 74.0, 87.0, 37.0, 6.2),
                rod_path(87.0, 37.0, 84.0, 91.0, 2.5),
            ),
            parts_depth: 0.5,
            turn: 1.0,
            face_x: 50.0,
            face_y: 59.0,
            face_scale: 0.92,
            screen: false,
            chest: None,
        };
        let wings = format!(
            "{}{}{}{}",
            ellipse_path(27.0, 44.0, 25.0, 8.5, 12.0),
            ellipse_path(73.0, 44.0, 25.0, 8.5, -12.0),
            ellipse_path(31.0, 60.0, 20.0, 7.5, -12.0),
            ellipse_path(69.0, 60.0, 20.0, 7.5, 12.0),
        );
        let swarm = LocustBotShape {
            key: "swarm",
            name: "Swarm",
            body: format!(
                "{}{}",
                ellipse_path(50.0, 61.0, 13.0, 31.0, 0.0),
                ellipse_path(50.0, 31.0, 18.0, 15.0, 0.0),
            ),
            parts: format!(
                "{}{}{}",
                wings,
                rod_path(46.0, 22.0, 35.0, 3.0, 2.0),
                rod_path(54.0, 22.0, 65.0, 3.0, 2.0),
            ),
            parts_depth: 0.55,
            turn: 0.7,
            face_x: 50.0,
            face_y: 31.0,
            face_scale: 0.74,
            screen: false,
            chest: None,
        };
        let critter = LocustBotShape {
            key: "critter",
            name: "Critter",
            body: format!(
                "{}{}{}",
                round_rect_path(15.0, 30.0, 70.0, 44.0, 13.0),
                round_rect_path(5.0, 45.0, 14.0, 13.0, 5.0),
                round_rect_path(81.0, 45.0, 14.0, 13.0, 5.0),
            ),
            parts: format!(
                "{}{}{}{}",
                rod_path(27.0, 70.0, 27.0, 89.0, 3.6),
                rod_path(41.0, 70.0, 41.0, 89.0, 3.6),
                rod_path(59.0, 70.0, 59.0, 89.0, 3.6),
                rod_path(73.0, 70.0, 73.0, 89.0, 3.6),
            ),
            parts_depth: 0.55,
            turn: 1.0,
            face_x: 50.0,
            face_y: 51.0,
            face_scale: 0.9,
            screen: false,
            chest: None,
        };
        let codex_head = union_outline(
            [50.0, 38.0],
            &[
                [37.0, 28.0, 14.0, 13.0, 0.0],
                [63.0, 28.0, 14.0, 13.0, 0.0],
                [37.0, 48.0, 14.0, 13.0, 0.0],
                [63.0, 48.0, 14.0, 13.0, 0.0],
                [50.0, 38.0, 25.8, 22.4, 0.0],
            ],
            240,
        );
        let codex_body = union_outline(
            [50.0, 68.0],
            &[
                [50.0, 66.5, 14.5, 11.0, 0.0],
                [50.0, 71.0, 13.0, 9.5, 0.0],
                [43.5, 80.0, 5.8, 6.2, 0.0],
                [56.5, 80.0, 5.8, 6.2, 0.0],
            ],
            180,
        );
        let codex_mitts = format!(
            "{}{}",
            ellipse_path(31.4, 67.5, 4.3, 6.8, 12.0),
            ellipse_path(68.6, 67.5, 4.3, 6.8, -12.0),
        );
        let prompt = grow_locust_shape(
            LocustBotShape {
                key: "prompt",
                name: "Prompt",
                body: format!("{codex_head}{codex_body}{codex_mitts}"),
                parts: String::new(),
                parts_depth: 0.5,
                turn: 1.0,
                face_x: 50.0,
                face_y: 41.0,
                face_scale: 0.76,
                screen: true,
                chest: Some(ChestMark {
                    mark: ChestMarkKind::Prompt,
                    x: 50.0,
                    y: 67.5,
                    size: 11.0,
                }),
            },
            1.26,
            50.0,
            50.5,
        );
        let claude_burst = union_outline(
            [50.0, 46.0],
            &[
                [50.0, 45.0, 22.0, 22.0, 0.0],
                [50.0, 60.0, 17.0, 15.0, 0.0],
                [50.0, 21.0, 9.5, 9.5, 0.0],
                [35.2, 26.4, 9.5, 9.5, 0.0],
                [64.8, 26.4, 9.5, 9.5, 0.0],
                [27.3, 40.0, 9.5, 9.5, 0.0],
                [72.7, 40.0, 9.5, 9.5, 0.0],
                [30.1, 55.5, 9.5, 9.5, 0.0],
                [69.9, 55.5, 9.5, 9.5, 0.0],
                [27.5, 67.5, 5.5, 6.5, 0.6],
                [72.5, 67.5, 5.5, 6.5, -0.6],
            ],
            240,
        );
        let spark = grow_locust_shape(
            LocustBotShape {
                key: "spark",
                name: "Spark",
                body: format!(
                    "{}{}{}",
                    claude_burst,
                    ellipse_path(42.5, 79.0, 5.4, 6.2, 0.0),
                    ellipse_path(57.5, 79.0, 5.4, 6.2, 0.0),
                ),
                parts: String::new(),
                parts_depth: 0.5,
                turn: 1.0,
                face_x: 50.0,
                face_y: 43.0,
                face_scale: 0.7,
                screen: true,
                chest: Some(ChestMark {
                    mark: ChestMarkKind::Spark,
                    x: 50.0,
                    y: 66.0,
                    size: 18.0,
                }),
            },
            1.22,
            50.0,
            48.3,
        );
        [hopper, swarm, critter, prompt, spark]
    });
    list.iter().find(|s| s.key == key)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub key: String,
    pub body: String,
    pub parts: Option<String>,
    pub parts_depth: Option<f64>,
    pub turn: f64,
    pub face: BotFace,
    pub face_x: f64,
    pub face_y: f64,
    pub face_scale: f64,
    pub screen: bool,
    pub chest: Option<ChestMark>,
}

pub fn outline_of(bot_type: &str) -> Outline {
    if let Some(shape) = locust_bot(bot_type) {
        return Outline {
            key: format!("locust-{}", shape.key),
            body: shape.body.clone(),
            parts: Some(shape.parts.clone()),
            parts_depth: Some(shape.parts_depth),
            turn: shape.turn,
            face: BotFace::Eyes,
            face_x: shape.face_x,
            face_y: shape.face_y,
            face_scale: shape.face_scale,
            screen: shape.screen,
            chest: shape.chest,
        };
    }
    let preset = preset_of(bot_type).unwrap_or(&BOT_AVATAR_PRESETS[0]);
    Outline {
        key: preset.id.to_owned(),
        body: preset.body.to_owned(),
        parts: if preset.parts.is_empty() {
            None
        } else {
            Some(preset.parts.to_owned())
        },
        parts_depth: None,
        turn: 1.0,
        face: preset.face,
        face_x: preset.face_x,
        face_y: preset.face_y,
        face_scale: preset.face_scale,
        screen: false,
        chest: None,
    }
}

/// Splits multi-piece `parts` paths on every `M` command so each piece extrudes separately.
pub fn part_pieces(parts: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut start = None;
    for (idx, ch) in parts.char_indices() {
        if ch == 'M' {
            if let Some(s) = start {
                let slice = parts[s..idx].trim();
                if !slice.is_empty() {
                    result.push(slice);
                }
            }
            start = Some(idx);
        }
    }
    if let Some(s) = start {
        let slice = parts[s..].trim();
        if !slice.is_empty() {
            result.push(slice);
        }
    }
    result
}

/// Body color adjusted identically to `Bot.tsx`'s `bodyColorOf` (`SATURATION = 1.15`).
pub const BOT_SATURATION: f64 = 1.15;
pub const PALETTE_SATURATION: f64 = 1.1;

pub fn body_color_of(bot_type: &str, color: Option<&str>) -> String {
    if is_locust_bot(bot_type) {
        return color.unwrap_or("#9CD66A").to_owned();
    }
    let saturation = if color.is_none() {
        PALETTE_SATURATION
    } else {
        BOT_SATURATION
    };
    let base = color
        .or_else(|| preset_of(bot_type).map(|p| p.color))
        .unwrap_or("#41C4FF");
    shade_css(base, 0.0, (saturation - 1.0) * 0.5)
}

// ---------------------------------------------------------------------------
// SVG path parser & polygon raster/containment
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum SvgToken {
    Cmd(char),
    Num(f64),
}

fn tokenize_svg_path(d: &str) -> Vec<SvgToken> {
    let mut tokens = Vec::new();
    let bytes = d.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b.is_ascii_whitespace() || b == b',' {
            i += 1;
        } else if b.is_ascii_alphabetic() {
            tokens.push(SvgToken::Cmd(b as char));
            i += 1;
        } else if b == b'-' || b == b'+' || b == b'.' || b.is_ascii_digit() {
            let start = i;
            if b == b'-' || b == b'+' {
                i += 1;
            }
            let mut has_dot = false;
            while i < bytes.len() {
                let c = bytes[i];
                if c.is_ascii_digit() {
                    i += 1;
                } else if c == b'.' && !has_dot {
                    has_dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                i += 1;
                if i < bytes.len() && (bytes[i] == b'-' || bytes[i] == b'+') {
                    i += 1;
                }
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
            }
            if let Ok(n) = d[start..i].parse::<f64>() {
                tokens.push(SvgToken::Num(n));
            }
        } else {
            i += 1;
        }
    }
    tokens
}

fn push_cubic(
    pts: &mut Vec<[f64; 2]>,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
    x3: f64,
    y3: f64,
) {
    const STEPS: usize = 8;
    for s in 1..=STEPS {
        let t = s as f64 / STEPS as f64;
        let u = 1.0 - t;
        let x = u * u * u * x0 + 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * x3;
        let y = u * u * u * y0 + 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * y3;
        pts.push([x, y]);
    }
}

fn push_quad(pts: &mut Vec<[f64; 2]>, x0: f64, y0: f64, x1: f64, y1: f64, x2: f64, y2: f64) {
    const STEPS: usize = 8;
    for s in 1..=STEPS {
        let t = s as f64 / STEPS as f64;
        let u = 1.0 - t;
        let x = u * u * x0 + 2.0 * u * t * x1 + t * t * x2;
        let y = u * u * y0 + 2.0 * u * t * y1 + t * t * y2;
        pts.push([x, y]);
    }
}

/// Converts an SVG elliptical arc `(x0, y0) -> (x, y)` to flattened polyline points using W3C F.6.5/6.6.
fn push_svg_arc(
    pts: &mut Vec<[f64; 2]>,
    x0: f64,
    y0: f64,
    mut rx: f64,
    mut ry: f64,
    angle_deg: f64,
    large_arc: bool,
    sweep: bool,
    x: f64,
    y: f64,
) {
    if (x - x0).hypot(y - y0) < 1e-6 {
        return;
    }
    rx = rx.abs();
    ry = ry.abs();
    if rx < 1e-6 || ry < 1e-6 {
        pts.push([x, y]);
        return;
    }
    let phi = angle_deg * PI / 180.0;
    let cos_p = phi.cos();
    let sin_p = phi.sin();
    let dx2 = (x0 - x) / 2.0;
    let dy2 = (y0 - y) / 2.0;
    let x1p = cos_p * dx2 + sin_p * dy2;
    let y1p = -sin_p * dx2 + cos_p * dy2;

    let rx_sq = rx * rx;
    let ry_sq = ry * ry;
    let x1p_sq = x1p * x1p;
    let y1p_sq = y1p * y1p;

    let lam = x1p_sq / rx_sq + y1p_sq / ry_sq;
    if lam > 1.0 {
        let scale = lam.sqrt();
        rx *= scale;
        ry *= scale;
    }
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let num = (rx2 * ry2 - rx2 * y1p_sq - ry2 * x1p_sq).max(0.0);
    let den = rx2 * y1p_sq + ry2 * x1p_sq;
    let factor = if den > 1e-12 {
        (num / den).sqrt() * if large_arc == sweep { -1.0 } else { 1.0 }
    } else {
        0.0
    };
    let cxp = factor * ((rx * y1p) / ry);
    let cyp = factor * (-(ry * x1p) / rx);
    let cx = cos_p * cxp - sin_p * cyp + (x0 + x) / 2.0;
    let cy = sin_p * cxp + cos_p * cyp + (y0 + y) / 2.0;

    let vec_angle = |ux: f64, uy: f64, vx: f64, vy: f64| -> f64 {
        let dot = ux * vx + uy * vy;
        let len = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let ratio = if len > 1e-12 {
            (dot / len).clamp(-1.0, 1.0)
        } else {
            1.0
        };
        let sign = if ux * vy - uy * vx < 0.0 { -1.0 } else { 1.0 };
        sign * ratio.acos()
    };

    let theta1 = vec_angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut dtheta = vec_angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );
    if !sweep && dtheta > 0.0 {
        dtheta -= 2.0 * PI;
    } else if sweep && dtheta < 0.0 {
        dtheta += 2.0 * PI;
    }

    let steps = ((dtheta.abs() / (PI / 16.0)).ceil() as usize).max(4);
    for s in 1..=steps {
        let theta = theta1 + dtheta * (s as f64 / steps as f64);
        let px = rx * theta.cos();
        let py = ry * theta.sin();
        pts.push([cx + cos_p * px - sin_p * py, cy + sin_p * px + cos_p * py]);
    }
}

/// Parses an SVG path string into closed polygon subpaths.
pub fn parse_svg_subpaths(d: &str) -> Vec<Vec<[f64; 2]>> {
    let tokens = tokenize_svg_path(d);
    let mut subpaths: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut cur: Vec<[f64; 2]> = Vec::new();
    let mut cx = 0.0_f64;
    let mut cy = 0.0_f64;
    let mut sx = 0.0_f64;
    let mut sy = 0.0_f64;
    let mut last_ctrl: Option<[f64; 2]> = None;
    let mut last_cmd = ' ';
    let mut i = 0;

    let take_num = |tokens: &[SvgToken], i: &mut usize| -> Option<f64> {
        match tokens.get(*i) {
            Some(SvgToken::Num(v)) => {
                *i += 1;
                Some(*v)
            }
            _ => None,
        }
    };

    let mut active_cmd = ' ';
    while i < tokens.len() {
        match tokens[i] {
            SvgToken::Cmd(c) => {
                active_cmd = c;
                i += 1;
                if active_cmd == 'Z' || active_cmd == 'z' {
                    if !cur.is_empty() {
                        subpaths.push(std::mem::take(&mut cur));
                    }
                    cx = sx;
                    cy = sy;
                    last_ctrl = None;
                    last_cmd = active_cmd;
                    continue;
                }
            }
            SvgToken::Num(_) => {
                if active_cmd == 'M' {
                    active_cmd = 'L';
                } else if active_cmd == 'm' {
                    active_cmd = 'l';
                }
            }
        }
        let rel = active_cmd.is_ascii_lowercase();
        let op = active_cmd.to_ascii_uppercase();
        match op {
            'M' => {
                let (Some(x), Some(y)) = (take_num(&tokens, &mut i), take_num(&tokens, &mut i))
                else {
                    break;
                };
                if !cur.is_empty() {
                    subpaths.push(std::mem::take(&mut cur));
                }
                cx = if rel { cx + x } else { x };
                cy = if rel { cy + y } else { y };
                sx = cx;
                sy = cy;
                cur.push([cx, cy]);
                last_ctrl = None;
            }
            'L' => {
                let (Some(x), Some(y)) = (take_num(&tokens, &mut i), take_num(&tokens, &mut i))
                else {
                    break;
                };
                cx = if rel { cx + x } else { x };
                cy = if rel { cy + y } else { y };
                cur.push([cx, cy]);
                last_ctrl = None;
            }
            'H' => {
                let Some(x) = take_num(&tokens, &mut i) else {
                    break;
                };
                cx = if rel { cx + x } else { x };
                cur.push([cx, cy]);
                last_ctrl = None;
            }
            'V' => {
                let Some(y) = take_num(&tokens, &mut i) else {
                    break;
                };
                cy = if rel { cy + y } else { y };
                cur.push([cx, cy]);
                last_ctrl = None;
            }
            'C' => {
                let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                ) else {
                    break;
                };
                let (ax1, ay1) = if rel { (cx + x1, cy + y1) } else { (x1, y1) };
                let (ax2, ay2) = if rel { (cx + x2, cy + y2) } else { (x2, y2) };
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                push_cubic(&mut cur, cx, cy, ax1, ay1, ax2, ay2, ax, ay);
                cx = ax;
                cy = ay;
                last_ctrl = Some([ax2, ay2]);
            }
            'S' => {
                let (Some(x2), Some(y2), Some(x), Some(y)) = (
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                ) else {
                    break;
                };
                let [ax1, ay1] = match (last_cmd.to_ascii_uppercase(), last_ctrl) {
                    ('C' | 'S', Some([lx, ly])) => [2.0 * cx - lx, 2.0 * cy - ly],
                    _ => [cx, cy],
                };
                let (ax2, ay2) = if rel { (cx + x2, cy + y2) } else { (x2, y2) };
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                push_cubic(&mut cur, cx, cy, ax1, ay1, ax2, ay2, ax, ay);
                cx = ax;
                cy = ay;
                last_ctrl = Some([ax2, ay2]);
            }
            'Q' => {
                let (Some(x1), Some(y1), Some(x), Some(y)) = (
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                ) else {
                    break;
                };
                let (ax1, ay1) = if rel { (cx + x1, cy + y1) } else { (x1, y1) };
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                push_quad(&mut cur, cx, cy, ax1, ay1, ax, ay);
                cx = ax;
                cy = ay;
                last_ctrl = Some([ax1, ay1]);
            }
            'T' => {
                let (Some(x), Some(y)) = (take_num(&tokens, &mut i), take_num(&tokens, &mut i))
                else {
                    break;
                };
                let [ax1, ay1] = match (last_cmd.to_ascii_uppercase(), last_ctrl) {
                    ('Q' | 'T', Some([lx, ly])) => [2.0 * cx - lx, 2.0 * cy - ly],
                    _ => [cx, cy],
                };
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                push_quad(&mut cur, cx, cy, ax1, ay1, ax, ay);
                cx = ax;
                cy = ay;
                last_ctrl = Some([ax1, ay1]);
            }
            'A' => {
                let (
                    Some(rx),
                    Some(ry),
                    Some(rot),
                    Some(large),
                    Some(sweep),
                    Some(x),
                    Some(y),
                ) = (
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                    take_num(&tokens, &mut i),
                )
                else {
                    break;
                };
                let (ax, ay) = if rel { (cx + x, cy + y) } else { (x, y) };
                push_svg_arc(
                    &mut cur,
                    cx,
                    cy,
                    rx,
                    ry,
                    rot,
                    large.abs() > 0.5,
                    sweep.abs() > 0.5,
                    ax,
                    ay,
                );
                cx = ax;
                cy = ay;
                last_ctrl = None;
            }
            _ => break,
        }
        last_cmd = active_cmd;
    }
    if !cur.is_empty() {
        subpaths.push(cur);
    }
    subpaths
}

/// Nonzero winding rule test across closed subpaths (matches Canvas `isPointInPath`).
pub fn point_in_subpaths(subpaths: &[Vec<[f64; 2]>], x: f64, y: f64) -> bool {
    let mut winding: i32 = 0;
    for poly in subpaths {
        if poly.len() < 2 {
            continue;
        }
        for i in 0..poly.len() {
            let [x0, y0] = poly[i];
            let [x1, y1] = poly[(i + 1) % poly.len()];
            if y0 <= y {
                if y1 > y {
                    let cross = (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0);
                    if cross > 0.0 {
                        winding += 1;
                    }
                }
            } else if y1 <= y {
                let cross = (x1 - x0) * (y - y0) - (x - x0) * (y1 - y0);
                if cross < 0.0 {
                    winding -= 1;
                }
            }
        }
    }
    winding != 0
}

// ---------------------------------------------------------------------------
// Anchors (`botAnchors.ts`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyBox {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingAnchor {
    pub left: f64,
    pub top: f64,
    pub side: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DotAnchor {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BotAnchors {
    pub ring: RingAnchor,
    pub dot: DotAnchor,
    /// Mirrored horizontally about the ring centre for the runtime mark.
    pub mark_left: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelBounds {
    pub left: usize,
    pub top: usize,
    pub right: usize,
    pub bottom: usize,
}

pub const RING_MARGIN: f64 = 0.06;
const OPAQUE: u8 = 40;

pub fn painted_bounds(data: &[u8], width: usize, height: usize) -> Option<PixelBounds> {
    let mut left = width;
    let mut top = height;
    let mut right: Option<usize> = None;
    let mut bottom: usize = 0;
    for y in 0..height {
        let row = y * width * 4;
        for x in 0..width {
            let alpha = data.get(row + x * 4 + 3).copied().unwrap_or(0);
            if alpha > OPAQUE {
                if x < left {
                    left = x;
                }
                if right.map_or(true, |r| x > r) {
                    right = Some(x);
                }
                if y < top {
                    top = y;
                }
                if y > bottom {
                    bottom = y;
                }
            }
        }
    }
    right.map(|r| PixelBounds {
        left,
        top,
        right: r + 1,
        bottom: bottom + 1,
    })
}

pub fn anchors_of(body: BodyBox) -> BotAnchors {
    let width = body.right - body.left;
    let height = body.bottom - body.top;
    let centre_x = (body.left + body.right) / 2.0;
    let centre_y = (body.top + body.bottom) / 2.0;
    let side = width.max(height) + 2.0 * RING_MARGIN;
    let reach = std::f64::consts::FRAC_1_SQRT_2 / 2.0;
    let ring = RingAnchor {
        left: centre_x - side / 2.0,
        top: centre_y - side / 2.0,
        side,
    };
    let dot = DotAnchor {
        x: centre_x + reach * width,
        y: centre_y + reach * height,
    };
    let mark_left = 2.0 * (ring.left + ring.side / 2.0) - dot.x;
    BotAnchors {
        ring,
        dot,
        mark_left,
    }
}

pub fn anchor_variables(anchors: BotAnchors) -> [(&'static str, String); 7] {
    let pct = |f: f64| format!("{:.2}%", f * 100.0);
    [
        ("--lc-bot-ring-left", pct(anchors.ring.left)),
        ("--lc-bot-ring-top", pct(anchors.ring.top)),
        ("--lc-bot-ring-side", pct(anchors.ring.side)),
        ("--lc-bot-dot-right", pct(1.0 - anchors.dot.x)),
        ("--lc-bot-dot-bottom", pct(1.0 - anchors.dot.y)),
        ("--lc-bot-dot-shift", "50%".to_owned()),
        ("--lc-bot-mark-left", pct(anchors.mark_left)),
    ]
}

// ---------------------------------------------------------------------------
// Visor, glyphs, front plane, and eye layout (`Bot.tsx`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisorBox {
    pub half_width: f64,
    pub half_height: f64,
    pub corner: f64,
    pub y: f64,
}

pub const EYES_VISOR: VisorBox = VisorBox {
    half_width: 27.0,
    half_height: 17.5,
    corner: 12.0,
    y: 1.5,
};

pub const MOUTH_VISOR: VisorBox = VisorBox {
    half_width: 27.0,
    half_height: 21.5,
    corner: 12.0,
    y: 5.5,
};

pub fn visor_outline(b: VisorBox) -> Vec<[f64; 2]> {
    let w = b.half_width;
    let h = b.half_height;
    let r = b.corner;
    let y = b.y;
    let mut pts = Vec::with_capacity(48);
    let edge = |pts: &mut Vec<[f64; 2]>, ax: f64, ay: f64, bx: f64, by: f64, steps: usize| {
        for i in 0..steps {
            let k = i as f64 / steps as f64;
            pts.push([ax + (bx - ax) * k, ay + (by - ay) * k]);
        }
    };
    let arc = |pts: &mut Vec<[f64; 2]>, cx: f64, cy: f64, from: f64| {
        for i in 0..6 {
            let t = from + (i as f64 / 6.0) * (PI / 2.0);
            pts.push([cx + r * t.cos(), cy + r * t.sin()]);
        }
    };
    edge(&mut pts, -w + r, y - h, w - r, y - h, 8);
    arc(&mut pts, w - r, y - h + r, -PI / 2.0);
    edge(&mut pts, w, y - h + r, w, y + h - r, 4);
    arc(&mut pts, w - r, y + h - r, 0.0);
    edge(&mut pts, w - r, y + h, -w + r, y + h, 8);
    arc(&mut pts, -w + r, y + h - r, PI / 2.0);
    edge(&mut pts, -w, y + h - r, -w, y - h + r, 4);
    arc(&mut pts, -w + r, y - h + r, PI);
    pts
}

pub fn fit_visor<F>(
    mut inside: F,
    face_x: f64,
    face_y: f64,
    face_scale: f64,
    base: VisorBox,
    margin: f64,
) -> VisorBox
where
    F: FnMut(f64, f64) -> bool,
{
    let scaled = |k: f64| VisorBox {
        half_width: base.half_width * k,
        half_height: base.half_height * k,
        corner: base.corner * k,
        y: base.y,
    };
    let mut k = 1.0_f64;
    while k > 0.5 {
        let bx = scaled(k);
        let room = VisorBox {
            half_width: bx.half_width + margin,
            half_height: bx.half_height + margin,
            corner: bx.corner + margin,
            y: bx.y,
        };
        if visor_outline(room)
            .into_iter()
            .all(|[x, y]| inside(face_x + x * face_scale, face_y + y * face_scale))
        {
            return bx;
        }
        k -= 0.025;
    }
    scaled(0.5)
}

/// Fitted screen visor for any outline, memoized per `(key, face)`.
pub fn visor_of(outline: &Outline, face: BotFace) -> VisorBox {
    let base = if face == BotFace::Mouth {
        MOUTH_VISOR
    } else {
        EYES_VISOR
    };
    if outline.screen {
        return base;
    }
    static CACHE: OnceLock<Mutex<HashMap<(String, BotFace), VisorBox>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let key = (outline.key.clone(), face);
    if let Ok(guard) = cache.lock() {
        if let Some(found) = guard.get(&key) {
            return *found;
        }
    }
    let subpaths = parse_svg_subpaths(&outline.body);
    let fitted = fit_visor(
        |x, y| point_in_subpaths(&subpaths, x, y),
        outline.face_x,
        outline.face_y,
        outline.face_scale,
        base,
        3.0,
    );
    if let Ok(mut guard) = cache.lock() {
        guard.insert(key, fitted);
    }
    fitted
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceAt {
    pub x: f64,
    pub y: f64,
    pub scale: f64,
}

pub const FRONT_DEPTH: f64 = 15.0 * 0.65;
pub const FRONT_RIM: f64 = 1.0 - (1.0 - 0.9) * 0.5;

pub fn front_plane(yaw: f64, pitch: f64, at: FaceAt) -> [f64; 6] {
    let held = |v: f64| -> f64 {
        if v.abs() < 0.22 {
            if v < 0.0 {
                -0.22
            } else {
                0.22
            }
        } else {
            v
        }
    };
    let cy = held(yaw.cos());
    let sy = yaw.sin();
    let cp = held(pitch.cos());
    let sp = pitch.sin();
    let fx = at.x - 50.0;
    let fy = at.y - 50.0;
    [
        cy * FRONT_RIM,
        sy * sp * FRONT_RIM,
        0.0,
        cp * FRONT_RIM,
        (cy * FRONT_RIM * fx + sy * FRONT_DEPTH - fx) / at.scale,
        (sy * sp * FRONT_RIM * fx + cp * FRONT_RIM * fy - cy * sp * FRONT_DEPTH - fy) / at.scale,
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub struct GlyphShape {
    pub lines: &'static [&'static [f64]],
    pub ring: Option<f64>,
    pub weight: f64,
    pub closed: bool,
}

static LINE_GT: &[f64] = &[-3.3, -5.4, 3.5, 0.0, -3.3, 5.4];
static LINE_LT: &[f64] = &[3.3, -5.4, -3.5, 0.0, 3.3, 5.4];
static LINE_UNDERSCORE: &[f64] = &[-4.6, 4.4, 4.6, 4.4];
static LINE_DASH: &[f64] = &[-4.8, 0.4, 4.8, 0.4];
static LINE_CARET: &[f64] = &[-5.2, 2.8, 0.0, -2.6, 5.2, 2.8];
static LINE_X_1: &[f64] = &[-3.9, -3.9, 3.9, 3.9];
static LINE_X_2: &[f64] = &[3.9, -3.9, -3.9, 3.9];
static LINE_BAR: &[f64] = &[0.0, -4.9, 0.0, 4.9];
static LINE_C: &[f64] = &[-4.6, -0.9, -2.6, 1.0, 0.0, 1.7, 2.6, 1.0, 4.6, -0.9];
static LINE_DOT: &[f64] = &[0.0, -0.01, 0.0, 0.01];
static LINE_BLOCK: &[f64] = &[0.0, -3.3, 0.0, 3.3];

static LINES_GT: &[&[f64]] = &[LINE_GT];
static LINES_LT: &[&[f64]] = &[LINE_LT];
static LINES_UNDERSCORE: &[&[f64]] = &[LINE_UNDERSCORE];
static LINES_DASH: &[&[f64]] = &[LINE_DASH];
static LINES_CARET: &[&[f64]] = &[LINE_CARET];
static LINES_X: &[&[f64]] = &[LINE_X_1, LINE_X_2];
static LINES_BAR: &[&[f64]] = &[LINE_BAR];
static LINES_O: &[&[f64]] = &[];
static LINES_C: &[&[f64]] = &[LINE_C];
static LINES_DOT: &[&[f64]] = &[LINE_DOT];
static LINES_BLOCK: &[&[f64]] = &[LINE_BLOCK];

pub fn glyph_shape(token: &str) -> Option<GlyphShape> {
    Some(match token {
        ">" => GlyphShape { lines: LINES_GT, ring: None, weight: 4.6, closed: false },
        "<" => GlyphShape { lines: LINES_LT, ring: None, weight: 4.6, closed: false },
        "_" => GlyphShape { lines: LINES_UNDERSCORE, ring: None, weight: 4.6, closed: false },
        "-" => GlyphShape { lines: LINES_DASH, ring: None, weight: 4.6, closed: false },
        "^" => GlyphShape { lines: LINES_CARET, ring: None, weight: 4.4, closed: false },
        "x" => GlyphShape { lines: LINES_X, ring: None, weight: 4.2, closed: false },
        "|" => GlyphShape { lines: LINES_BAR, ring: None, weight: 5.4, closed: false },
        "o" => GlyphShape { lines: LINES_O, ring: Some(3.9), weight: 3.8, closed: false },
        "c" => GlyphShape { lines: LINES_C, ring: None, weight: 3.8, closed: true },
        "•" => GlyphShape { lines: LINES_DOT, ring: None, weight: 8.4, closed: false },
        "▮" => GlyphShape { lines: LINES_BLOCK, ring: None, weight: 7.0, closed: false },
        _ => return None,
    })
}

pub fn blinking_dot(closing: f64) -> ([f64; 4], f64) {
    let c = closing.clamp(0.0, 1.0);
    let half = 4.8 * c;
    let line = [-half, 0.4 * c - 0.01 * (1.0 - c), half, 0.4 * c + 0.01 * (1.0 - c)];
    let weight = 8.4 + (4.6 - 8.4) * c;
    (line, weight)
}

pub const SCREEN_GLYPH_SCALE: f64 = 1.3;
pub const INKED_GLYPH_SCALE: f64 = 1.12;
const MOTION_REACH: f64 = 2.0;
const MOTION_STRETCH: f64 = 1.16;
pub const SCREEN_PAD: f64 = 1.5;

pub fn glyph_reach(shape: &GlyphShape) -> [f64; 2] {
    let mut x = 0.0_f64;
    let mut y = 0.0_f64;
    for line in shape.lines {
        let mut i = 0;
        while i + 1 < line.len() {
            x = x.max(line[i].abs());
            y = y.max(line[i + 1].abs());
            i += 2;
        }
    }
    if let Some(ring) = shape.ring {
        x = x.max(ring);
        y = y.max(ring);
    }
    let half = shape.weight / 2.0;
    [
        x * MOTION_STRETCH + MOTION_REACH + half,
        y * MOTION_STRETCH + MOTION_REACH + half,
    ]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenEyeLayout {
    pub scale: f64,
    pub centres: [[f64; 2]; 2],
}

pub fn screen_eye_layout(
    box_: VisorBox,
    pair: [&str; 2],
    look: [f64; 2],
    eye_y: f64,
) -> ScreenEyeLayout {
    let dot = GlyphShape {
        lines: LINES_DOT,
        ring: None,
        weight: 8.4,
        closed: false,
    };
    let r0 = glyph_reach(&glyph_shape(pair[0]).unwrap_or_else(|| dot.clone()));
    let r1 = glyph_reach(&glyph_shape(pair[1]).unwrap_or(dot));
    let reach_x = r0[0].max(r1[0]);
    let reach_y = r0[1].max(r1[1]);
    let k = (box_.half_width / EYES_VISOR.half_width)
        .min(box_.half_height / EYES_VISOR.half_height)
        .min(1.0)
        .max(0.5);
    let room_x = box_.half_width - SCREEN_PAD;
    let room_y = box_.half_height - SCREEN_PAD;
    let scale = (SCREEN_GLYPH_SCALE * k)
        .min(room_y / reach_y)
        .min((room_x - 0.5) / (2.0 * reach_x))
        .max(0.1);
    let spread = (12.5 * k)
        .min(room_x - reach_x * scale)
        .max(reach_x * scale + 0.5);
    let across = (room_x - spread - reach_x * scale).max(0.0);
    let down = (room_y - reach_y * scale).max(0.0);
    let x = (look[0] * 0.9).clamp(-across, across);
    let y = (eye_y + look[1] * 0.5).clamp(box_.y - down, box_.y + down);
    ScreenEyeLayout {
        scale,
        centres: [[x - spread, y], [x + spread, y]],
    }
}

/// The Claude mark's twelve rays `[degrees_clockwise_from_up, length_ratio, width_ratio]`.
pub const CLAUDE_MARK_RAYS: [[f64; 3]; 12] = [
    [333.0, 1.0, 1.15],
    [9.0, 0.89, 0.85],
    [40.0, 0.89, 1.0],
    [80.0, 0.87, 0.9],
    [102.0, 0.9, 0.95],
    [131.0, 0.9, 1.0],
    [149.0, 0.85, 0.9],
    [185.0, 0.89, 1.0],
    [214.0, 0.89, 1.05],
    [237.0, 0.86, 1.0],
    [268.0, 0.92, 0.95],
    [302.0, 0.94, 1.0],
];

#[cfg(test)]
mod tests {
    use super::*;

    const SPHERE: f64 = 30.0;
    const HALF_SPACING: f64 = 12.5;
    const IDLE_YAW: f64 = 36.0 * PI / 180.0;

    fn leading_eye_x(shape: &LocustBotShape) -> f64 {
        shape.face_x
            + shape.face_scale
                * SPHERE
                * ((HALF_SPACING / SPHERE).asin() + IDLE_YAW * shape.turn).sin()
    }

    fn right_edge_ellipse(path: &str) -> f64 {
        let rest = path.strip_prefix('M').unwrap_or("");
        let num = rest.split(' ').next().unwrap_or("");
        num.parse().unwrap()
    }

    fn count_subpaths(path: &str) -> usize {
        path.matches('M').count()
    }

    fn grow_y(y: f64) -> f64 {
        50.5 + (y - 50.5) * 1.26
    }

    fn parse_ml_points(subpath: &str) -> Vec<[f64; 2]> {
        let mut pts = Vec::new();
        let mut rest = subpath;
        while let Some(pos) = rest.find(|c| c == 'M' || c == 'L') {
            rest = &rest[pos + 1..];
            let mut split = rest.splitn(3, |c: char| c == ' ' || c == 'M' || c == 'L' || c == 'Z');
            if let (Some(xs), Some(ys)) = (split.next(), split.next()) {
                if let (Ok(x), Ok(y)) = (xs.parse::<f64>(), ys.parse::<f64>()) {
                    pts.push([x, y]);
                }
            }
        }
        pts
    }

    fn canvas_rgba(width: usize, height: usize, left: usize, top: usize, right: usize, bottom: usize) -> Vec<u8> {
        let mut data = vec![0u8; width * height * 4];
        for y in top..bottom {
            for x in left..right {
                data[(y * width + x) * 4 + 3] = 255;
            }
        }
        data
    }

    // -----------------------------------------------------------------------
    // a-bots-dot-and-ring-sit-on-the-bot.test.ts (5 cases)
    // -----------------------------------------------------------------------

    #[test]
    fn finds_the_painted_pixels_right_and_bottom_exclusive() {
        assert_eq!(
            painted_bounds(&canvas_rgba(40, 30, 5, 7, 21, 29), 40, 30),
            Some(PixelBounds {
                left: 5,
                top: 7,
                right: 21,
                bottom: 29,
            })
        );
    }

    #[test]
    fn ignores_an_antialiased_fringe_and_says_nothing_for_an_empty_canvas() {
        let mut data = canvas_rgba(10, 10, 2, 2, 8, 8);
        data[(0 * 10 + 0) * 4 + 3] = 20;
        assert_eq!(
            painted_bounds(&data, 10, 10),
            Some(PixelBounds {
                left: 2,
                top: 2,
                right: 8,
                bottom: 8,
            })
        );
        assert_eq!(painted_bounds(&vec![0u8; 400], 10, 10), None);
    }

    #[test]
    fn centres_the_ring_on_the_body_clear_of_it_by_ring_margin_on_its_longer_side() {
        let droid = BodyBox {
            left: 4.0 / 76.0,
            top: 31.0 / 76.0,
            right: 72.0 / 76.0,
            bottom: 95.0 / 76.0,
        };
        let anchors = anchors_of(droid);
        assert!(((anchors.ring.left + anchors.ring.side / 2.0) - (droid.left + droid.right) / 2.0).abs() < 1e-6);
        assert!(((anchors.ring.top + anchors.ring.side / 2.0) - (droid.top + droid.bottom) / 2.0).abs() < 1e-6);
        assert!((anchors.ring.side - (droid.right - droid.left + 2.0 * RING_MARGIN)).abs() < 1e-6);
    }

    #[test]
    fn puts_the_dot_on_the_lower_right_edge_not_off_the_corner_and_not_at_mid_height() {
        let droid = BodyBox {
            left: 4.0 / 76.0,
            top: 31.0 / 76.0,
            right: 72.0 / 76.0,
            bottom: 95.0 / 76.0,
        };
        let dot = anchors_of(droid).dot;
        let middle_y = (droid.top + droid.bottom) / 2.0;
        assert!(dot.x < droid.right);
        assert!(dot.y < droid.bottom);
        assert!(dot.x > (droid.left + droid.right) / 2.0);
        assert!(dot.y - middle_y > 0.3 * (droid.bottom - droid.top));
    }

    #[test]
    fn hands_them_to_the_css_as_percentages_of_the_box_the_dot_by_its_centre() {
        let vars = anchor_variables(anchors_of(BodyBox {
            left: 0.1,
            top: 0.2,
            right: 0.9,
            bottom: 1.0,
        }));
        let want_side = format!("{:.2}%", (0.8 + 2.0 * RING_MARGIN) * 100.0);
        assert_eq!(vars[2].1, want_side);
        assert_eq!(vars[5].1, "50%");
        let dot_right: f64 = vars[3].1.trim_end_matches('%').parse().unwrap();
        let want_right = (1.0 - (0.5 + (std::f64::consts::FRAC_1_SQRT_2 / 2.0) * 0.8)) * 100.0;
        assert!((dot_right - want_right).abs() < 0.05);
    }

    // -----------------------------------------------------------------------
    // locust-bots-keep-their-eyes-on-their-heads.test.ts (6 cases)
    // -----------------------------------------------------------------------

    #[test]
    fn swarm_draws_its_wings_behind_the_body_so_an_eye_can_never_be_drawn_on_one() {
        let swarm = locust_bot("swarm").unwrap();
        assert_eq!(count_subpaths(&swarm.body), 2);
        assert_eq!(count_subpaths(&swarm.parts), 6);
    }

    #[test]
    fn swarm_keeps_the_leading_eye_on_its_head_at_the_widest_idle_glance() {
        let swarm = locust_bot("swarm").unwrap();
        let z_idx = swarm.body.find('Z').unwrap();
        let head = &swarm.body[z_idx + 1..];
        assert!(leading_eye_x(swarm) < right_edge_ellipse(head));
        assert!(50.0 + 0.78 * SPHERE * ((HALF_SPACING / SPHERE).asin() + IDLE_YAW).sin() > 65.0);
    }

    #[test]
    fn critter_and_prompt_draw_legs_and_title_bar_dots_behind_the_body_so_an_eye_is_never_drawn_on_one() {
        let critter = locust_bot("critter").unwrap();
        let prompt = locust_bot("prompt").unwrap();
        assert_eq!(count_subpaths(&critter.body), 3);
        assert_eq!(count_subpaths(&critter.parts), 4);
        assert_eq!(count_subpaths(&prompt.body), 4);
        assert_eq!(prompt.parts, "");
    }

    #[test]
    fn critter_and_prompt_keep_the_leading_eye_inside_the_main_block_at_the_widest_idle_glance_clear_of_its_round_corner() {
        let critter = locust_bot("critter").unwrap();
        let prompt = locust_bot("prompt").unwrap();
        assert!(leading_eye_x(critter) < 85.0 - 6.0);
        assert!(leading_eye_x(prompt) < 50.0 + (83.5 - 50.0) * 1.26 - 9.0 * 1.26);
    }

    #[test]
    fn hopper_draws_its_legs_and_antennae_behind_the_body() {
        let hopper = locust_bot("hopper").unwrap();
        assert_eq!(count_subpaths(&hopper.body), 1);
        assert_eq!(count_subpaths(&hopper.parts), 6);
    }

    #[test]
    fn hopper_keeps_the_leading_eye_on_its_body_at_the_widest_idle_glance() {
        let hopper = locust_bot("hopper").unwrap();
        assert!(leading_eye_x(hopper) < right_edge_ellipse(&hopper.body));
    }

    // -----------------------------------------------------------------------
    // prompt-is-the-codex-mascot.test.ts (8 cases)
    // -----------------------------------------------------------------------

    #[test]
    fn prompt_has_a_cloud_for_a_head_as_one_outline_no_seams_inside_it_wide_its_bumps_along_its_top_and_sides() {
        let prompt = locust_bot("prompt").unwrap();
        let first = part_pieces(&prompt.body)[0];
        let points = parse_ml_points(first);
        assert!(points.len() > 100);
        let min_x = points.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
        let max_x = points.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
        let min_y = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        let max_y = points.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
        assert!((max_x - min_x) > (max_y - min_y));
        let top: Vec<f64> = points
            .iter()
            .filter(|p| p[1] > 0.0 && p[1] < grow_y(30.0))
            .map(|p| (p[0] - 50.0).hypot(p[1] - grow_y(38.0)))
            .collect();
        let mut turns = 0;
        for i in 2..top.len() {
            if (top[i] - top[i - 1]) * (top[i - 1] - top[i - 2]) < 0.0 {
                turns += 1;
            }
        }
        assert!(turns >= 3);
    }

    #[test]
    fn prompt_wears_its_screen_low_on_its_head_most_of_its_face_and_its_prompt_on_its_chest_below() {
        let prompt = locust_bot("prompt").unwrap();
        assert!(prompt.screen);
        assert!(prompt.face_y > 38.0);
        let chest = prompt.chest.unwrap();
        assert_eq!(chest.mark, ChestMarkKind::Prompt);
        assert!(chest.y > grow_y(55.5) + chest.size / 2.0);
        assert!(chest.y < grow_y(80.5) - chest.size / 2.0);
        assert_eq!(outline_of("prompt").chest, Some(chest));
    }

    #[test]
    fn prompt_has_its_mitts_and_legs_in_its_body_so_the_plush_furs_them_as_the_rest_of_it_no_parts() {
        let prompt = locust_bot("prompt").unwrap();
        assert_eq!(prompt.body.matches('A').count() / 2, 2);
        assert_eq!(prompt.parts, "");
    }

    #[test]
    fn prompt_wears_the_prompt_on_its_chest_only_the_two_mascots_wear_a_mark_there_each_its_own() {
        assert_eq!(locust_bot("spark").unwrap().chest.unwrap().mark, ChestMarkKind::Spark);
        for key in ["hopper", "swarm", "critter"] {
            assert_eq!(locust_bot(key).unwrap().chest, None);
        }
    }

    #[test]
    fn spark_is_one_outline_its_round_petals_standing_out_evenly_round_its_top_and_sides_a_notch_between_each() {
        let spark = locust_bot("spark").unwrap();
        let first = part_pieces(&spark.body)[0];
        let points = parse_ml_points(first);
        assert!(points.len() > 100);
        let reach: Vec<f64> = points
            .iter()
            .filter(|p| p[1] < 46.0)
            .map(|p| (p[0] - 50.0).hypot(p[1] - 46.0))
            .collect();
        let min_r = reach.iter().copied().fold(f64::INFINITY, f64::min);
        let max_r = reach.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        assert!(max_r - min_r > 5.0);
        let mut turns = 0;
        for i in 2..reach.len() {
            if (reach[i] - reach[i - 1]) * (reach[i - 1] - reach[i - 2]) < 0.0 {
                turns += 1;
            }
        }
        assert!(turns >= 8);
    }

    #[test]
    fn spark_wears_its_screen_in_its_middle_always_and_its_asterisk_under_it() {
        let spark = locust_bot("spark").unwrap();
        assert!(spark.screen);
        let chest = spark.chest.unwrap();
        assert!(chest.y > spark.face_y + 17.5 * spark.face_scale + chest.size / 2.0);
        assert_eq!(outline_of("spark").chest, Some(chest));
    }

    #[test]
    fn spark_has_its_legs_in_its_body_so_the_plush_furs_them_no_parts() {
        let spark = locust_bot("spark").unwrap();
        assert_eq!(spark.body.matches('A').count() / 2, 2);
        assert_eq!(spark.parts, "");
    }

    #[test]
    fn spark_is_picked_only_no_teammates_derived_face_becomes_it() {
        let idx = BOT_SHAPES.iter().position(|&s| s == "spark").unwrap();
        assert!(idx >= 20);
        assert!(screen_suits("spark"));
    }

    // -----------------------------------------------------------------------
    // Additional checks for all 23 shapes, visors, colors, and sizes
    // -----------------------------------------------------------------------

    #[test]
    fn all_23_shapes_parse_and_fit_visors() {
        for shape in BOT_SHAPES {
            let outline = outline_of(shape);
            let subpaths = parse_svg_subpaths(&outline.body);
            assert!(!subpaths.is_empty(), "{shape} body subpaths");
            assert!(
                point_in_subpaths(&subpaths, outline.face_x, outline.face_y),
                "{shape} face centre inside body"
            );
            let visor = visor_of(&outline, BotFace::Eyes);
            assert!(visor.half_width >= EYES_VISOR.half_width * 0.49, "{shape} visor width");
            assert!(visor.half_height >= EYES_VISOR.half_height * 0.49, "{shape} visor height");
        }
    }

    #[test]
    fn small_bot_draws_at_twice_its_size_rule_matches_bot_tsx() {
        assert_eq!(draw_size_for(32.0), 64.0);
        assert_eq!(draw_size_for(20.0), 40.0);
        assert_eq!(draw_size_for(33.0), 33.0);
        assert_eq!(draw_size_for(40.0), 40.0);
        assert_eq!(bot_size::THREAD_LIVE, 34.0);
        assert_eq!(bot_size::MEMORY_AUTHOR, 18.0);
    }
}
