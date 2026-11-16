//! The look picker's rules, from `NewTeammateDialog.tsx`'s Look section and `avatar.ts`. Electron has no test file of
//! its own for these; each test is one behaviour of its code, with its values (lines cited). The dialog's two cases in
//! `every-teammate-is-a-bot.test.tsx` draw the window: `look_picker::tests` in the window crate.

use super::*;
use crate::bot_spec::{shuffled_avatar, BOT_SHAPES, HUES};

fn plain(shape: &str, face: BotFace, screen: Option<bool>) -> AvatarSpec {
    AvatarSpec { headwear: 1, accessory: 2, mouth: 3, bot: Some(BotSpec { shape: shape.into(), face, screen }), pet: None }
}

fn buddy() -> PetRef {
    PetRef { source: PetSource::Codex, id: "codex-buddy".into(), screen: None }
}

#[test]
fn every_shape_has_its_name_and_locusts_own_two_say_so() {
    let names: Vec<&str> = BOT_SHAPES.iter().map(|s| shape_name(s).unwrap()).collect();
    assert_eq!(names.len(), 23);
    assert_eq!(&names[..3], &["Clover", "Flower", "Triangle"]);
    assert_eq!(&names[18..], &["Hopper, a Locust", "Swarm, a Locust", "Critter", "Prompt", "Spark"]);
    assert_eq!(shape_name("robot"), None);
}

#[test]
fn the_colours_are_named_in_the_teams_order() {
    let hues: Vec<&str> = HUE_LABELS.iter().map(|(hue, _)| *hue).collect();
    assert_eq!(hues, HUES);
    let labels: Vec<&str> = HUE_LABELS.iter().map(|(_, label)| *label).collect();
    assert_eq!(labels, ["Lime", "Blue", "Violet", "Clay", "Teal", "Butter", "Rose", "Slate", "Pearl", "Indigo", "Coral"]);
}

#[test]
fn the_face_shown_is_a_screen_only_when_the_bot_wears_one() {
    // 425-427: alwaysScreen || (terminal && (look.screen ?? screenSuits(look.shape))).
    assert_eq!(face_choice(&plain("droid", BotFace::Eyes, None), true), FaceChoice::Screen, "a droid suits a screen");
    assert_eq!(face_choice(&plain("droid", BotFace::Mouth, None), false), FaceChoice::Mouth, "Terminal faces off");
    assert_eq!(face_choice(&plain("droid", BotFace::Eyes, Some(false)), true), FaceChoice::Eyes, "screen turned off");
    assert_eq!(face_choice(&plain("clover", BotFace::Eyes, None), true), FaceChoice::Eyes, "a clover does not suit one");
    assert_eq!(face_choice(&plain("clover", BotFace::Eyes, Some(true)), true), FaceChoice::Screen, "unless chosen");
    assert_eq!(face_choice(&plain("prompt", BotFace::Eyes, Some(false)), false), FaceChoice::Screen, "Prompt's is its screen");
    assert_eq!(face_choice(&plain("spark", BotFace::Mouth, None), false), FaceChoice::Screen);
}

#[test]
fn screen_waits_for_terminal_faces_and_the_mascots_keep_theirs() {
    // 636-649.
    let clover = plain("clover", BotFace::Eyes, None);
    assert_eq!(face_off(&clover, FaceChoice::Screen, false), Some(SCREEN_NEEDS_TERMINAL));
    assert_eq!(
        SCREEN_NEEDS_TERMINAL,
        "Turn Terminal faces on in Settings > Appearance to give a teammate a screen."
    );
    assert_eq!(face_off(&clover, FaceChoice::Screen, true), None);
    assert_eq!(face_off(&clover, FaceChoice::Eyes, false), None);
    let prompt = plain("prompt", BotFace::Eyes, None);
    assert_eq!(face_off(&prompt, FaceChoice::Eyes, true), Some("Prompt's face is its screen."));
    assert_eq!(face_off(&prompt, FaceChoice::Mouth, false), Some(MASCOT_FACE_IS_SCREEN));
    assert_eq!(face_off(&prompt, FaceChoice::Screen, false), None, "a mascot's screen is always on offer");
    assert_eq!(face_off(&plain("spark", BotFace::Eyes, None), FaceChoice::Eyes, true), Some(MASCOT_FACE_IS_SCREEN));
}

#[test]
fn a_face_choice_keeps_the_shape_screen_keeps_the_face_under_it_and_eyes_or_mouth_turn_the_screen_off() {
    // 653-659.
    let mouth = plain("ghost", BotFace::Mouth, None);
    assert_eq!(with_face(&mouth, FaceChoice::Screen).bot, Some(BotSpec { shape: "ghost".into(), face: BotFace::Mouth, screen: Some(true) }));
    assert_eq!(with_face(&mouth, FaceChoice::Eyes).bot, Some(BotSpec { shape: "ghost".into(), face: BotFace::Eyes, screen: Some(false) }));
    assert_eq!(with_face(&mouth, FaceChoice::Mouth).bot, Some(BotSpec { shape: "ghost".into(), face: BotFace::Mouth, screen: Some(false) }));
    let derived = crate::bot_spec::seed_avatar("tm_wren");
    let picked = with_face(&derived, FaceChoice::Mouth);
    assert_eq!(picked.bot.as_ref().map(|b| b.shape.clone()), Some(bot_for(&derived).shape), "a derived bot is pinned");
    assert_eq!((picked.headwear, picked.accessory, picked.mouth), (derived.headwear, derived.accessory, derived.mouth));
}

#[test]
fn a_shape_keeps_the_face_chosen_so_far_and_takes_off_the_pet() {
    // withShape and withoutPet (212-216, 429-432, 722).
    let screen = plain("ghost", BotFace::Mouth, Some(true));
    assert_eq!(with_shape(&screen, "hopper").bot, Some(BotSpec { shape: "hopper".into(), face: BotFace::Mouth, screen: Some(true) }));
    let unset = plain("ghost", BotFace::Eyes, None);
    assert_eq!(with_shape(&unset, "cat").bot.unwrap().screen, None, "no screen choice made, none given");
    let worn = AvatarSpec { pet: Some(buddy()), ..screen };
    let next = with_shape(&worn, "star");
    assert_eq!(next.pet, None);
    assert_eq!(next.bot.unwrap().shape, "star");
}

#[test]
fn shuffle_rolls_the_parts_on_and_drops_the_picked_bot_and_the_pet() {
    let worn = AvatarSpec { pet: Some(buddy()), ..plain("hopper", BotFace::Mouth, Some(true)) };
    let next = shuffled_avatar(&worn);
    assert_eq!((next.headwear, next.accessory, next.mouth), (2, 0, 0));
    assert_eq!((next.bot, next.pet), (None, None));
}

#[test]
fn a_worn_pets_face_is_as_drawn_or_its_screen_while_terminal_faces_is_on() {
    // 669-670 and 680-696.
    let pet = buddy();
    assert!(pet_screen_chosen(&pet, true, true));
    assert!(!pet_screen_chosen(&pet, false, true));
    assert!(pet_screen_chosen(&pet, false, false), "Terminal faces off: as drawn");
    assert!(!pet_screen_chosen(&pet, true, false));
    let drawn = PetRef { screen: Some(false), ..buddy() };
    assert!(pet_screen_chosen(&drawn, false, true));
    let worn = AvatarSpec { pet: Some(buddy()), ..plain("ghost", BotFace::Eyes, None) };
    assert_eq!(with_pet_screen(&worn, false).pet.unwrap().screen, Some(false));
    assert_eq!(with_pet_screen(&plain("ghost", BotFace::Eyes, None), true).pet, None, "no pet, nothing to change");
}

#[test]
fn two_dialogs_opened_in_a_row_start_from_different_draft_ids() {
    assert_eq!(draft_seed_id(1_763_200_000_000, 0.25), "draft_1763200000000_0.25");
    let ids: std::collections::HashSet<String> = (0..20)
        .map(|_| {
            let a = draft_avatar();
            format!("{:?}", (a.headwear, a.accessory, a.mouth))
        })
        .collect();
    assert!(ids.len() > 1, "twenty drafts all seeded the same look");
}

#[test]
fn an_avatar_is_read_only_when_it_is_one_as_is_avatar_spec_says() {
    use serde_json::json;
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"bot":{"shape":"ghost","face":"eyes"}})).is_some());
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"bot":{"shape":"robot","face":"eyes"}})).is_none());
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"bot":{"shape":"ghost","face":"nose"}})).is_none());
    assert!(avatar_from_json(&json!({"headwear":6,"accessory":0,"mouth":0})).is_none(), "out of range");
    assert!(avatar_from_json(&json!({"headwear":1.5,"accessory":0,"mouth":0})).is_none(), "not whole");
    assert!(avatar_from_json(&json!({"headwear":1.0,"accessory":0,"mouth":0})).is_some(), "1.0 is 1 to JavaScript");
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"bot":null})).is_none(), "null is not absent");
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"bot":{"shape":"ghost","face":"eyes","screen":"yes"}})).is_none());
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"pet":{"source":"codex","id":"codex-buddy"}})).is_some());
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"pet":{"source":"codex","id":"builtin"}})).is_none());
    assert!(avatar_from_json(&json!({"headwear":1,"accessory":0,"mouth":0,"pet":{"source":"web","id":"cat"}})).is_none());
    assert!(avatar_from_json(&json!({"accessory":0,"mouth":0})).is_none());
}

#[test]
fn nothing_but_the_known_fields_reaches_disk() {
    use serde_json::json;
    // every-teammate-is-a-bot's validation case, as the record is written.
    let extra = json!({"headwear":1,"accessory":0,"mouth":0,"bot":{"shape":"ghost","face":"eyes","size":900},"script":"x"});
    assert_eq!(avatar_json(&avatar_from_json(&extra).unwrap()), json!({"headwear":1,"accessory":0,"mouth":0,"bot":{"shape":"ghost","face":"eyes"}}));
    let full = json!({"headwear":0,"accessory":2,"mouth":1,"bot":{"shape":"cat","face":"mouth","screen":false},"pet":{"source":"bundled","id":"codex-buddy","screen":true,"x":1}});
    assert_eq!(avatar_json(&avatar_from_json(&full).unwrap()), json!({"headwear":0,"accessory":2,"mouth":1,"bot":{"shape":"cat","face":"mouth","screen":false},"pet":{"source":"bundled","id":"codex-buddy","screen":true}}));
}
