//! A teammate's face: the bot its look comes to (shared/avatar.ts), drawn as the still Electron's TeammateBot made of
//! it (`scripts/bot-stills.mjs`, `look::bots`). The seed is the id, never the name, as in Electron: a rename never
//! changes a face, and two teammates who share a name never share one.

use super::bots;
use locust_core::native::FaceLook;
use rune::{El, ImageCache, row, text};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// avatar.ts seedAvatar: FNV-1a over the id (the first UTF-16 unit of each code point, as JavaScript's
/// `for (const character of id) character.charCodeAt(0)` reads it), an avalanche, then a byte window per part.
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
    [part(4, 6), part(12, 3), part(20, 4)]
}

/// avatar.ts botFor: the picked bot, or the one the three parts come to among the twenty derived shapes; the open mouth
/// (mouth 2) is the one that becomes a mouth.
pub fn bot_for(parts: [u8; 3], picked: Option<&(String, String)>) -> (String, &'static str) {
    if let Some((shape, face)) = picked {
        return (shape.clone(), if face == "mouth" { "mouth" } else { "eyes" });
    }
    let [headwear, accessory, mouth] = parts.map(usize::from);
    let index = (headwear * 3 * 4 + accessory * 4 + mouth) % bots::SHAPES.len();
    (bots::SHAPES[index].to_owned(), if mouth == 2 { "mouth" } else { "eyes" })
}

/// The bot (shape, face, hue) a teammate shows. Without a recorded look (a scene's made-up teammate), the parts are
/// seeded from the id and the hue picked from it too, so each still differs.
pub fn bot_of(id: &str, look: Option<&FaceLook>) -> (String, &'static str, String) {
    let parts = look.and_then(|l| l.parts).unwrap_or_else(|| seed_parts(id));
    let (shape, face) = bot_for(parts, look.and_then(|l| l.bot.as_ref()));
    let hue = look.map(|l| l.hue.clone()).unwrap_or_else(|| {
        let [h, a, m] = seed_parts(&format!("{id}#hue"));
        bots::HUES[(usize::from(h) * 12 + usize::from(a) * 4 + usize::from(m)) % bots::HUES.len()].to_owned()
    });
    (shape, face, hue)
}

/// `.lc-hometeam__tile`: 46 square, radius 10, the teammate's hue at 11% over the raised fill, edged at 20%, its bot
/// 34 high inside.
pub fn tile(cache: Option<&ImageCache>, id: &str, name: &str, look: Option<&FaceLook>, l: &super::controls::Look) -> El {
    let (_, _, hue) = bot_of(id, look);
    let tint = l.t.color(&format!("hue-{hue}"));
    row()
        .w(46.0)
        .h(46.0)
        .center()
        .radius(10.0)
        .fill(l.c("bg-raised").mix(tint, 0.11))
        .border(1.0, tint.alpha(0.2))
        .child(face(cache, id, name, look, 34.0, l))
}

/// A shape picked from outside the twenty derived ones (critter, prompt, spark) has no still yet: it shows the derived
/// bot instead, the one its parts come to.
fn drawable(id: &str, look: Option<&FaceLook>) -> (String, &'static str, String) {
    let (shape, face, hue) = bot_of(id, look);
    if bots::still(&shape, face, &hue).is_some() {
        return (shape, face, hue);
    }
    let parts = look.and_then(|l| l.parts).unwrap_or_else(|| seed_parts(id));
    let (shape, face) = bot_for(parts, None);
    (shape, face, hue)
}

/// Decoded stills, one per bot, so a face is decoded once however often it is drawn.
fn decoded(cache: &ImageCache, key: &(String, &'static str, String)) -> Option<rune::Image> {
    static MEMO: OnceLock<Mutex<HashMap<(String, &'static str, String), rune::Image>>> = OnceLock::new();
    let memo = MEMO.get_or_init(Default::default);
    if let Some(image) = memo.lock().ok()?.get(key) {
        return Some(image.clone());
    }
    let bytes = bots::still(&key.0, key.1, &key.2)?;
    let image = cache.load(Arc::<[u8]>::from(bytes));
    // Headless snapshots and the first live frame need the decode done before paint.
    let _ = image.wait();
    memo.lock().ok()?.insert(key.clone(), image.clone());
    Some(image)
}

/// A teammate's face at `size` points: its bot's still, or its initial on a square when no image cache is set up yet.
pub fn face(cache: Option<&ImageCache>, id: &str, name: &str, look: Option<&FaceLook>, size: f32, l: &super::controls::Look) -> El {
    let key = drawable(id, look);
    if let Some(image) = cache.and_then(|c| decoded(c, &key)) {
        // The still's bot fills 96 of its 104 pixels; scale the cell so the bot itself is `size`.
        let cell = size * bots::CELL as f32 / 96.0;
        let inset = (cell - size) / 2.0;
        return row()
            .w(size)
            .h(size)
            .center()
            .child(row().w(cell).h(cell).m_all(-inset).pixels(image.pixels()))
            .a11y_hidden();
    }
    let initial = name.trim_start().chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    row()
        .w(size)
        .h(size)
        .center()
        .radius((size / 4.0).round())
        .fill(l.c("bg-control"))
        .child(text(initial).font(rune::TextStyle::new((size * 0.42).round(), 600.0)).color(l.c("text-secondary")))
        .a11y_hidden()
}

trait MarginAll {
    fn m_all(self, m: f32) -> Self;
}
impl MarginAll for El {
    fn m_all(self, m: f32) -> Self {
        self.mx(m).my(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// Electron's avatar.ts, run over 100 ids by scripts/face-goldens.mjs: the same parts, shape and face for each.
    #[test]
    fn faces_match_electron_for_a_hundred_ids() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("parity/faces.json");
        let goldens: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let faces = goldens["faces"].as_array().unwrap();
        assert_eq!(faces.len(), 100);
        for f in faces {
            let id = f["id"].as_str().unwrap();
            let parts = seed_parts(id);
            let want = [f["headwear"].as_u64().unwrap() as u8, f["accessory"].as_u64().unwrap() as u8, f["mouth"].as_u64().unwrap() as u8];
            assert_eq!(parts, want, "parts for {id:?}");
            let (shape, face) = bot_for(parts, None);
            assert_eq!((shape.as_str(), face), (f["shape"].as_str().unwrap(), f["face"].as_str().unwrap()), "bot for {id:?}");
        }
    }

    #[test]
    fn a_picked_bot_and_a_recorded_look_win_over_the_seed() {
        let look = FaceLook { hue: "teal".into(), parts: Some([0, 0, 0]), bot: Some(("ghost".into(), "mouth".into())) };
        assert_eq!(bot_of("anyone", Some(&look)), ("ghost".into(), "mouth", "teal".into()));
        let unpicked = FaceLook { hue: "rose".into(), parts: Some([0, 0, 2]), bot: None };
        assert_eq!(bot_of("anyone", Some(&unpicked)), (bots::SHAPES[2].to_owned(), "mouth", "rose".into()));
    }

    #[test]
    fn every_derived_bot_has_a_still_in_every_hue() {
        for shape in bots::SHAPES {
            for face in ["eyes", "mouth"] {
                for hue in bots::HUES {
                    let png = bots::still(shape, face, hue).unwrap_or_else(|| panic!("{shape} {face} {hue}"));
                    assert_eq!(&png[1..4], b"PNG");
                }
            }
        }
        // A picked shape with no still falls back to its derived bot rather than drawing nothing.
        let look = FaceLook { hue: "lime".into(), parts: Some([0, 0, 0]), bot: Some(("spark".into(), "eyes".into())) };
        assert!(bots::still(&drawable("x", Some(&look)).0, "eyes", "lime").is_some());
    }
}
