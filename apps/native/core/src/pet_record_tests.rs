//! Electron's `a-pet-sheet-is-checked-before-it-is-kept.test.ts`, "a pet on a teammate's record": the last two cases of
//! that file. The rest are carried in `pet_library_tests.rs` and `pet_gallery_tests.rs`.

use crate::bot_spec::{seed_avatar, shuffled_avatar, AvatarSpec, BotFace, BotSpec, PetRef, PetSource};
use crate::look_picker::{avatar_from_json, avatar_json};
use crate::people;
use crate::store::Store;
use serde_json::json;

struct TempRoot(std::path::PathBuf);
impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch() -> (TempRoot, std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("locust-pet-record-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let native = dir.join("native");
    let electron = dir.join("electron");
    std::fs::create_dir_all(&native).unwrap();
    std::fs::create_dir_all(&electron).unwrap();
    (TempRoot(dir), native, electron)
}

#[test]
fn a_pet_on_a_teammates_record_is_kept_through_a_save_and_a_reopen_beside_the_bot_it_falls_back_to() {
    let (_root, native, electron) = scratch();
    let look = AvatarSpec {
        bot: Some(BotSpec { shape: "droid".into(), face: BotFace::Eyes, screen: None }),
        pet: Some(PetRef { source: PetSource::Gallery, id: "yuyu-chibi".into(), screen: None }),
        ..seed_avatar("draft")
    };
    let store = Store::new(native.clone(), electron.clone()).unwrap();
    let id = people::save_teammate_id(
        &store,
        json!({"teammateId": "", "name": "Yuyu", "hue": "rose", "role": "Docs & QA", "avatar": avatar_json(&look)}),
    )
    .unwrap();
    let reopened = Store::new(native, electron).unwrap();
    let team = people::team(&reopened).unwrap();
    let row = team["teammates"].as_array().unwrap().iter().find(|r| r["teammateId"] == json!(id)).unwrap();
    assert_eq!(avatar_from_json(&row["avatar"]), Some(look.clone()));
    assert_eq!(row["avatar"], avatar_json(&look));
}

#[test]
fn a_pet_that_names_no_real_source_or_an_id_that_is_a_path_is_refused() {
    let base = avatar_json(&seed_avatar("draft"));
    let with_pet = |pet: serde_json::Value| {
        let mut a = base.clone();
        a["pet"] = pet;
        a
    };
    assert!(avatar_from_json(&with_pet(json!({"source": "bundled", "id": "hoodie-cat"}))).is_some());
    assert!(avatar_from_json(&with_pet(json!({"source": "web", "id": "hoodie-cat"}))).is_none());
    assert!(avatar_from_json(&with_pet(json!({"source": "codex", "id": "../../secrets"}))).is_none());
    assert!(avatar_from_json(&with_pet(json!({"source": "gallery", "id": "builtin"}))).is_none());
    // cleanAvatar: a field that is not the pet's own (a path) is dropped.
    let cleaned = avatar_from_json(&with_pet(json!({"source": "codex", "id": "mochi", "path": "C:\\x"}))).unwrap();
    assert_eq!(avatar_json(&cleaned)["pet"], json!({"source": "codex", "id": "mochi"}));
    // A shuffle rolls through the bots: it takes the pet off.
    assert_eq!(shuffled_avatar(&cleaned).pet, None);
}
