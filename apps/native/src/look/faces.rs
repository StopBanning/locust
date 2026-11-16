//! A teammate's face: the bot or pet its look comes to (`shared/avatar.ts`), drawn live
//! on Rune's vector `canvas(Shape)` using the geometry and motion rigs in `locust_core`.
//!
//! The seed is the id, never the name, as in Electron: a rename never changes a face, and
//! two teammates who share a name never share one.

use super::{
    bot_stage::PresenceDot,
    bots::{draw_bot_shape, draw_buddy_shape, static_bot_shape, static_buddy_shape, BotDrawSpec, BuddyDrawSpec},
};
use locust_core::bot_rig::{BotMotionLevel, FaceActivity, GlanceSide};
use locust_core::bot_spec::{fallback_hue_for, screen_suits, BotFace, PetSource, BOT_SHAPES, HUES};
use locust_core::buddy_moves::CODEX_BUDDY;
use locust_core::native::FaceLook;
use rune::{El, ImageCache, Tx, canvas, row};
use std::time::Duration;

#[cfg(test)]
thread_local! {
    /// The presence dot each face was last drawn with, by its slot, for the window's own tests.
    static DRAWN_PRESENCE: std::cell::RefCell<std::collections::HashMap<String, Option<PresenceDot>>> = Default::default();
}

#[cfg(test)]
thread_local! {
    /// The shape and hue each live face was last drawn as, by its slot, for the window's own tests.
    static DRAWN_LOOK: std::cell::RefCell<std::collections::HashMap<String, (String, String)>> = Default::default();
}

#[cfg(test)]
thread_local! {
    /// What each live face last drew, by its slot, for the window's own tests.
    static DRAWN_SHAPE: std::cell::RefCell<std::collections::HashMap<String, rune::Shape>> = Default::default();
}

#[cfg(test)]
thread_local! {
    /// The pet each live face last drew in its bot's place, by its slot, for the window's own tests.
    static DRAWN_PET: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
}

/// The pet the live face in `slot_key` last drew, if it drew one, on this thread.
#[cfg(test)]
pub fn drawn_pet(slot_key: &str) -> Option<String> {
    DRAWN_PET.with(|drawn| drawn.borrow().get(slot_key).cloned())
}

/// What the live face in `slot_key` last drew on this thread.
#[cfg(test)]
pub fn drawn_shape(slot_key: &str) -> Option<rune::Shape> {
    DRAWN_SHAPE.with(|drawn| drawn.borrow().get(slot_key).cloned())
}

/// The shape and hue the live face in `slot_key` was last drawn as on this thread.
#[cfg(test)]
pub fn drawn_look(slot_key: &str) -> Option<(String, String)> {
    DRAWN_LOOK.with(|drawn| drawn.borrow().get(slot_key).cloned())
}

/// The presence dot the face in `slot_key` was last drawn with on this thread; None when it was not drawn.
#[cfg(test)]
pub fn drawn_presence(slot_key: &str) -> Option<Option<PresenceDot>> {
    DRAWN_PRESENCE.with(|drawn| drawn.borrow().get(slot_key).copied())
}

#[derive(Debug, Clone, Copy)]
pub struct BotFaceOpts<'a> {
    pub activity: FaceActivity,
    pub glance: Option<GlanceSide>,
    pub presence: Option<PresenceDot>,
    pub runtime: Option<&'a str>,
    pub motion: BotMotionLevel,
    pub a11y_label: Option<&'a str>,
}

impl Default for BotFaceOpts<'_> {
    fn default() -> Self {
        Self {
            activity: FaceActivity::Idle,
            glance: None,
            presence: None,
            runtime: None,
            motion: BotMotionLevel::Full,
            a11y_label: None,
        }
    }
}

/// avatar.ts `seedAvatar`: FNV-1a over the id (the first UTF-16 unit of each code point).
pub fn seed_parts(id: &str) -> [u8; 3] {
    locust_core::bot_spec::seed_parts(id)
}

/// avatar.ts `botFor`: the picked bot, or the one the three parts come to among the twenty derived shapes.
pub fn bot_for(parts: [u8; 3], picked: Option<&(String, String)>) -> (String, &'static str) {
    let (shape, face) = locust_core::bot_spec::bot_for_parts(parts, picked);
    (shape, face.as_str())
}

/// The bot (shape, face, hue) a teammate shows.
pub fn bot_of(id: &str, look: Option<&FaceLook>) -> (String, &'static str, String) {
    let parts = look.and_then(|l| l.parts).unwrap_or_else(|| seed_parts(id));
    let (shape, face) = bot_for(parts, look.and_then(|l| l.bot.as_ref()));
    let hue = look
        .map(|l| l.hue.clone())
        .filter(|h| HUES.contains(&h.as_str()))
        .unwrap_or_else(|| fallback_hue_for(id).to_owned());
    (shape, face, hue)
}

/// Whether a teammate's face wears a CRT terminal screen given their look and `terminal_faces`.
pub fn wears_screen(look: Option<&FaceLook>, shape: &str, terminal_faces: bool) -> bool {
    // A built-in screen is the shape itself, even with terminal faces off.
    if locust_core::bot_spec::outline_of(shape).screen {
        return true;
    }
    if !terminal_faces {
        return false;
    }
    if let Some(pet) = look.and_then(|l| l.pet.as_ref())
        && pet.id == CODEX_BUDDY.id
        && matches!(pet.source, PetSource::Codex | PetSource::Bundled)
    {
        return pet.screen.unwrap_or(true);
    }
    if let Some(explicit) = look.and_then(|l| l.bot_screen) {
        return explicit;
    }
    screen_suits(shape)
}

pub fn is_codex_buddy(look: Option<&FaceLook>) -> bool {
    look.and_then(|l| l.pet.as_ref())
        .is_some_and(|p| p.id == CODEX_BUDDY.id)
}

/// `.lc-hometeam__tile`: 46 square, radius 10, the teammate's hue at 11% over the raised fill, edged at 20%, its bot
/// 34 high inside.
pub fn tile(
    cache: Option<&ImageCache>,
    id: &str,
    name: &str,
    look: Option<&FaceLook>,
    terminal_faces: bool,
    l: &super::controls::Look,
) -> El {
    let _ = cache;
    let (_, _, hue) = bot_of(id, look);
    let tint = l.t.color(&format!("hue-{hue}"));
    row()
        .w(46.0)
        .h(46.0)
        .center()
        .radius(10.0)
        .fill(l.c("bg-raised").mix(tint, 0.11))
        .border(1.0, tint.alpha(0.2))
        .child(face(None, id, name, look, 34.0, terminal_faces, l))
}

/// Animated teammate tile for Home (`46` square, `34` inner rig-drawn bot).
pub fn bot_tile(
    s: &crate::State,
    slot_key: &str,
    id: &str,
    name: &str,
    opts: BotFaceOpts<'_>,
    l: &super::controls::Look,
) -> El {
    let look = s.looks.get(id);
    let (_, _, hue) = bot_of(id, look);
    let tint = l.t.color(&format!("hue-{hue}"));
    row()
        .w(46.0)
        .h(46.0)
        .center()
        .radius(10.0)
        .fill(l.c("bg-raised").mix(tint, 0.11))
        .border(1.0, tint.alpha(0.2))
        .child(bot_face(s, slot_key, id, name, 34.0, opts, l))
}

/// Static vector rig face at `size` points (draws all 23 shapes and Codex Buddy).
pub fn face(
    cache: Option<&ImageCache>,
    id: &str,
    _name: &str,
    look: Option<&FaceLook>,
    size: f32,
    terminal_faces: bool,
    l: &super::controls::Look,
) -> El {
    let _ = cache;
    let (shape, face_str, hue) = bot_of(id, look);
    let screen = wears_screen(look, &shape, terminal_faces);
    let rendered = if is_codex_buddy(look) {
        static_buddy_shape(&hue, size, screen, false, !l.k.dark)
    } else {
        let face_kind = BotFace::from_str(face_str).unwrap_or(BotFace::Eyes);
        let draw_shape = if BOT_SHAPES.contains(&shape.as_str()) {
            shape.as_str()
        } else {
            "circle"
        };
        static_bot_shape(draw_shape, face_kind, &hue, size, screen, false, !l.k.dark)
    };
    canvas(rendered)
        .key(format!("bot-static-{id}-{}", size.round() as i32))
        .size(size, size)
        .a11y_hidden()
}

/// Live animated vector rig face wired to `State.bot_stage`, workspace settings, and reduced motion.
pub fn bot_face(
    s: &crate::State,
    slot_key: &str,
    id: &str,
    name: &str,
    size: f32,
    opts: BotFaceOpts<'_>,
    l: &super::controls::Look,
) -> El {
    let look = s.looks.get(id);
    bot_face_with_look(s, slot_key, id, name, look, size, opts, l)
}

/// Live animated vector rig face using an explicit `FaceLook` (used by the Teammate look picker preview).
#[allow(clippy::too_many_arguments)]
pub fn bot_face_with_look(
    s: &crate::State,
    slot_key: &str,
    id: &str,
    name: &str,
    look: Option<&FaceLook>,
    size: f32,
    opts: BotFaceOpts<'_>,
    l: &super::controls::Look,
) -> El {
    let (shape_name, face_str, hue) = bot_of(id, look);
    #[cfg(test)]
    DRAWN_PRESENCE.with(|drawn| drawn.borrow_mut().insert(slot_key.to_owned(), opts.presence));
    #[cfg(test)]
    DRAWN_LOOK.with(|drawn| drawn.borrow_mut().insert(slot_key.to_owned(), (shape_name.clone(), hue.clone())));
    let terminal_faces = s.settings_panel.settings.terminal_faces;
    let plush = s.settings_panel.settings.plush;
    let reduced = s.reduced_motion.get();
    let screen = wears_screen(look, &shape_name, terminal_faces);
    // A worn pet is drawn where the bot was, with a bot's marks; until its sheet is ready, or when it cannot be drawn,
    // the bot stands in (a-teammate-can-be-a-pet.test.tsx).
    if !is_codex_buddy(look)
        && let Some(pet) = look.and_then(|l| l.pet.as_ref())
        && let Some(drawn) = crate::pet_faces::pet_face(s, slot_key, pet, opts.activity, size, opts.presence, !l.k.dark)
    {
        #[cfg(test)]
        DRAWN_PET.with(|d| d.borrow_mut().insert(slot_key.to_owned(), pet.id.clone()));
        return drawn;
    }
    #[cfg(test)]
    DRAWN_PET.with(|d| {
        d.borrow_mut().remove(slot_key);
    });
    let shape = if is_codex_buddy(look) {
        let snap = s.bot_stage.snapshot_buddy(
            slot_key,
            id,
            opts.activity,
            opts.glance,
            opts.motion,
            screen,
            plush,
            reduced,
        );
        draw_buddy_shape(&BuddyDrawSpec {
            hue: &hue,
            size,
            light_theme: !l.k.dark,
            presence: opts.presence,
            ring: opts.activity == FaceActivity::Waiting,
            runtime: opts.runtime,
            snap: &snap,
        })
    } else {
        let face_kind = BotFace::from_str(face_str).unwrap_or(BotFace::Eyes);
        let snap = s.bot_stage.snapshot_bot(
            slot_key,
            id,
            opts.activity,
            opts.glance,
            opts.motion,
            screen,
            plush,
            reduced,
        );
        draw_bot_shape(&BotDrawSpec {
            shape: &shape_name,
            face: face_kind,
            hue: &hue,
            size,
            light_theme: !l.k.dark,
            presence: opts.presence,
            ring: opts.activity == FaceActivity::Waiting,
            runtime: opts.runtime,
            snap: &snap,
        })
    };

    #[cfg(test)]
    DRAWN_SHAPE.with(|drawn| drawn.borrow_mut().insert(slot_key.to_owned(), shape.clone()));
    let hover_slot = slot_key.to_owned();
    let pointer_slot = slot_key.to_owned();
    let mut el = canvas(shape)
        .key(format!("bot-{slot_key}"))
        .size(size, size)
        .on_hover_change(move |st: &mut crate::State, over: bool, tx: &mut Tx| {
            st.bot_stage.set_hover(&hover_slot, over);
            if !st.reduced_motion.get() {
                tx.wake_after(Duration::from_millis(33));
            }
        })
        // Bot.tsx `watchPointer`: where the pointer is, so a face that has noticed it looks toward it (`pointerPull`).
        .on_pointer_move_local(move |st: &mut crate::State, at: rune::PointerEvent, tx: &mut Tx| {
            st.bot_stage.point_at(&pointer_slot, f64::from(at.x), f64::from(at.y), f64::from(size));
            if !st.reduced_motion.get() {
                tx.wake_after(Duration::from_millis(33));
            }
        });
    el = match opts.a11y_label {
        Some(lbl) => el.label(lbl.to_owned()),
        None => el.a11y_hidden(),
    };
    el
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::bot_spec::draw_size_for;
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
            let want = [
                f["headwear"].as_u64().unwrap() as u8,
                f["accessory"].as_u64().unwrap() as u8,
                f["mouth"].as_u64().unwrap() as u8,
            ];
            assert_eq!(parts, want, "parts for {id:?}");
            let (shape, face) = bot_for(parts, None);
            assert_eq!(
                (shape.as_str(), face),
                (f["shape"].as_str().unwrap(), f["face"].as_str().unwrap()),
                "bot for {id:?}"
            );
        }
    }

    #[test]
    fn a_picked_bot_and_a_recorded_look_win_over_the_seed() {
        let look = FaceLook {
            hue: "teal".into(),
            parts: Some([0, 0, 0]),
            bot: Some(("ghost".into(), "mouth".into())),
            bot_screen: None,
            pet: None,
        };
        assert_eq!(
            bot_of("anyone", Some(&look)),
            ("ghost".into(), "mouth", "teal".into())
        );
        let unpicked = FaceLook {
            hue: "rose".into(),
            parts: Some([0, 0, 2]),
            bot: None,
            bot_screen: None,
            pet: None,
        };
        assert_eq!(
            bot_of("anyone", Some(&unpicked)),
            (super::super::bots::SHAPES[2].to_owned(), "mouth", "rose".into())
        );
    }

    #[test]
    fn every_bot_is_drawn_as_vector_shape_in_every_hue() {
        for shape in BOT_SHAPES {
            for face_kind in [BotFace::Eyes, BotFace::Mouth] {
                for hue in HUES {
                    let s = static_bot_shape(shape, face_kind, hue, 32.0, true, false, false);
                    assert!(!s.is_empty(), "{shape} {face_kind:?} {hue}");
                }
            }
        }
        for hue in HUES {
            let buddy_drawn = static_buddy_shape(hue, 34.0, false, false, false);
            assert!(!buddy_drawn.is_empty(), "buddy drawn {hue}");
            let buddy_screen = static_buddy_shape(hue, 34.0, true, true, true);
            assert!(!buddy_screen.is_empty(), "buddy screen {hue}");
        }
    }

    #[test]
    fn draws_at_twice_the_box_size_up_to_32_so_small_faces_stay_crisp() {
        assert_eq!(draw_size_for(18.0), 36.0);
        assert_eq!(draw_size_for(32.0), 64.0);
        let small = format!("{:?}", static_bot_shape("spark", BotFace::Eyes, "lime", 18.0, false, false, false));
        assert!(small.contains("view_box: Some([36.0, 36.0])"), "{small}");
        let thirty_two = format!("{:?}", static_bot_shape("spark", BotFace::Eyes, "lime", 32.0, false, false, false));
        assert!(thirty_two.contains("view_box: Some([64.0, 64.0])"), "{thirty_two}");
    }

    #[test]
    fn draws_at_box_size_above_32() {
        assert_eq!(draw_size_for(33.0), 33.0);
        assert_eq!(draw_size_for(40.0), 40.0);
        assert_eq!(draw_size_for(84.0), 84.0);
        let big = format!("{:?}", static_bot_shape("circle", BotFace::Mouth, "blue", 40.0, true, false, false));
        assert!(big.contains("view_box: Some([40.0, 40.0])"), "{big}");
    }

    #[test]
    fn codex_buddy_screen_respects_terminal_faces_and_as_drawn_override() {
        let mut look = FaceLook {
            hue: "blue".into(),
            parts: Some([0, 0, 0]),
            bot: None,
            bot_screen: None,
            pet: Some(locust_core::bot_spec::PetRef {
                source: PetSource::Codex,
                id: CODEX_BUDDY.id.into(),
                screen: None,
            }),
        };
        assert!(wears_screen(Some(&look), "circle", true));
        assert!(!wears_screen(Some(&look), "circle", false));
        look.pet.as_mut().unwrap().screen = Some(false);
        assert!(!wears_screen(Some(&look), "circle", true));
        look.pet.as_mut().unwrap().screen = Some(true);
        assert!(wears_screen(Some(&look), "circle", true));
    }

    #[test]
    fn turning_terminal_faces_off_draws_classic_eyes_and_leaves_codex_buddy_as_drawn() {
        let classic = FaceLook {
            hue: "lime".into(),
            parts: Some([0, 0, 0]),
            bot: Some(("circle".into(), "eyes".into())),
            bot_screen: Some(true),
            pet: None,
        };
        assert!(!wears_screen(Some(&classic), "circle", false));
        assert!(wears_screen(Some(&classic), "circle", true));
        let buddy = FaceLook {
            hue: "blue".into(),
            parts: Some([0, 0, 0]),
            bot: None,
            bot_screen: None,
            pet: Some(locust_core::bot_spec::PetRef {
                source: PetSource::Bundled,
                id: CODEX_BUDDY.id.into(),
                screen: Some(true),
            }),
        };
        assert!(!wears_screen(Some(&buddy), "circle", false));
    }
}

#[cfg(test)]
mod renderer_screen_tests {
    use super::*;
    use locust_core::bot_spec::{AvatarSpec, BotSpec, SCREEN_SHAPES, clean_avatar};
    fn look(shape: &str, screen: Option<bool>) -> FaceLook {
        FaceLook { hue: "blue".into(), parts: Some([0, 0, 0]),
            bot: Some((shape.into(), "eyes".into())), bot_screen: screen, pet: None }
    }
    #[test]
    fn suits_the_made_boxy_shapes_and_not_the_grown_bumpy_ones() {
        for shape in ["droid", "mech", "prompt", "critter", "square", "pill", "hexagon", "pebble", "circle", "ghost", "cat"] { assert!(screen_suits(shape), "{shape}"); }
        for shape in ["star", "flower", "clover", "cloud", "drop", "blob", "puddle", "triangle", "alien", "hopper", "swarm"] { assert!(!screen_suits(shape), "{shape}"); }
    }
    #[test]
    fn names_only_shapes_that_exist() {
        for shape in SCREEN_SHAPES { assert!(BOT_SHAPES.contains(&shape), "{shape}"); }
    }
    #[test]
    fn keeps_the_choice_and_refuses_anything_but_true_or_false() {
        // Option<bool> enforces the type internally; also test the untyped disk boundary.
        for screen in [Some(false), None] {
            let bot = BotSpec { shape: "droid".into(), face: BotFace::Eyes, screen };
            let avatar = AvatarSpec { headwear: 0, accessory: 0, mouth: 0, bot: Some(bot.clone()), pet: None };
            assert_eq!(clean_avatar(&avatar).bot, Some(bot));
        }
        for (screen, valid) in [(Some(serde_json::json!(true)), true), (None, true), (Some(serde_json::json!("yes")), false)] {
            let mut raw = serde_json::json!({"schemaVersion":1,"teammates":[{
                "teammateId":"tm_wren","name":"Wren","role":"Custom","hue":"blue",
                "createdAt":"2026-11-01T00:00:00Z",
                "avatar":{"headwear":0,"accessory":0,"mouth":0,
                    "bot":{"shape":"star","face":"eyes","screen":screen}}
            }]});
            if screen.is_none() { raw["teammates"][0]["avatar"]["bot"].as_object_mut().unwrap().remove("screen"); }
            let parsed = locust_core::native::parse_looks(&raw.to_string());
            assert_eq!(parsed["tm_wren"].bot.is_some(), valid);
        }
    }
    #[test]
    fn is_a_screen_where_the_shape_suits_one_and_its_own_eyes_where_it_does_not_until_chosen() {
        assert!(wears_screen(None, "droid", true));
        assert!(!wears_screen(None, "star", true));
    }
    #[test]
    fn follows_the_teammates_choice_either_way() {
        for (shape, screen) in [("star", true), ("droid", false), ("flower", true)] {
            let picked = look(shape, Some(screen));
            let (actual_shape, _, _) = bot_of("tm_wren", Some(&picked));
            assert_eq!(wears_screen(Some(&picked), &actual_shape, true), screen, "{shape}");
            assert!(!static_bot_shape(&actual_shape, BotFace::Eyes, "blue", 40.0, screen, false, false).is_empty());
        }
    }
    #[test]
    fn is_never_a_screen_with_terminal_faces_off_but_prompts_whose_face_it_is() {
        assert!(!wears_screen(Some(&look("droid", Some(true))), "droid", false));
        assert!(wears_screen(Some(&look("prompt", Some(false))), "prompt", false));
    }
}

#[cfg(test)]
#[path = "teammate_bot_markup_tests.rs"]
mod teammate_bot_markup_tests;
