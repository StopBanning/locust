//! Electron's `avatar.test.ts`, its five seeding cases: a teammate's look is a pure function of its id. (Its two
//! geometry cases are the PixelFace's CSS box-shadow drawing, retired in Electron and never ported.)
use super::*;
use crate::look_picker::avatar_from_json;
use serde_json::json;

fn key(a: &AvatarSpec) -> (u8, u8, u8) {
    (a.headwear, a.accessory, a.mouth)
}

// case: is a pure function of the id, so a rename never changes a face
#[test]
fn is_a_pure_function_of_the_id() {
    assert_eq!(seed_avatar("tm_abc123"), seed_avatar("tm_abc123"));
}

// case: gives two teammates with different ids different faces, whatever their names
#[test]
fn different_ids_give_different_faces() {
    let faces: std::collections::HashSet<(u8, u8, u8)> = (0..40).map(|i| key(&seed_avatar(&format!("tm_{i:024x}")))).collect();
    assert!(faces.len() > 20, "{}", faces.len());
}

// case: stays within every part table
#[test]
fn stays_within_every_part_table() {
    for i in 0..200 {
        let face = seed_avatar(&format!("tm_{i}"));
        assert!(face.headwear < HEADWEAR_COUNT && face.accessory < ACCESSORY_COUNT && face.mouth < MOUTH_COUNT);
        let read = avatar_from_json(&json!({"headwear": face.headwear, "accessory": face.accessory, "mouth": face.mouth})).unwrap();
        assert_eq!(key(&read), key(&face));
    }
}

// case: refuses an override outside the tables
#[test]
fn refuses_an_override_outside_the_tables() {
    for bad in [
        json!({"headwear": HEADWEAR_COUNT, "accessory": 0, "mouth": 0}),
        json!({"headwear": 1.5, "accessory": 0, "mouth": 0}),
        json!({"headwear": 0, "accessory": -1, "mouth": 0}),
        json!({"headwear": 0, "accessory": 0}),
        json!("lime"),
    ] {
        assert!(avatar_from_json(&bad).is_none(), "{bad}");
    }
}

// case: shuffles to a different look every time, cycling through all of them
#[test]
fn shuffles_to_a_different_look_every_time() {
    let mut face = seed_avatar("tm_shuffle");
    let mut seen = std::collections::HashSet::new();
    for _ in 0..12 {
        let next = shuffled_avatar(&face);
        assert_ne!(key(&next), key(&face));
        seen.insert(key(&next));
        face = next;
    }
    assert_eq!(seen.len(), 12);
}
