//! Electron's `a-teammate-can-be-a-pet.test.tsx`, "the look picker": the four cases, drawn in the real teammate form and
//! read through its accessibility tree. No data folder, so nothing is read or downloaded.

use super::*;
use crate::look_picker::tests::{checked_in, radios_in};
use serde_json::{json, Value};

fn form(avatar: Option<Value>) -> State {
    let mut s = State::default();
    s.settings_panel.settings.terminal_faces = true;
    let team = match &avatar {
        Some(avatar) => json!([{"teammateId":"tm_pet", "name":"Mochi", "hue":"lime", "role":"Docs & QA", "createdAt":"2026-10-03T00:00:00.000Z", "avatar": avatar}]),
        None => json!([]),
    };
    s.people_panel.team = json!({"teammates": team});
    crate::people_panel::edit(&mut s, avatar.as_ref().map(|_| "tm_pet"));
    s.people_panel.form_open = true;
    s
}

fn render(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| crate::people_panel::edit_form(s, &Palette::new(true)), (1100.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn wearing_cat() -> Value {
    json!({"headwear": 1, "accessory": 2, "mouth": 3, "bot": {"shape": "droid", "face": "eyes"}, "pet": {"source": "bundled", "id": "hoodie-cat"}})
}

fn text_of(rt: &rune::Runtime<State>) -> String {
    rt.accessibility_json().to_string()
}

#[test]
fn offers_exactly_the_pets_locust_offers_under_the_bots_and_no_gallery() {
    let rt = render(form(None));
    let tiles = radios_in(&rt, "team-pets");
    let names: Vec<&str> = tiles.iter().filter_map(|t| t["name"].as_str()).collect();
    assert_eq!(names, PET_PICKS.iter().map(|(_, name, _)| *name).collect::<Vec<_>>());
    let all = text_of(&rt);
    assert!(!all.contains("Browse the gallery") && !all.contains("Search pets"));
    assert!(!all.contains("team-pet-hoodie-cat"), "no bundled Hoodie Cat tile");
    // Not on this computer yet: each says it downloads when picked.
    for tile in &tiles {
        assert!(tile.to_string().contains("downloads from openpets.dev when picked"), "{tile}");
    }
    assert!(all.contains("Rights stay with each pet\u{2019}s maker."));
}

#[test]
fn keeps_a_pet_the_teammate_already_wears_that_is_not_offered_shown_as_chosen() {
    let rt = render(form(Some(wearing_cat())));
    assert_eq!(checked_in(&rt, "team-pets"), vec!["hoodie-cat".to_owned()]);
    assert_eq!(radios_in(&rt, "team-pets").len(), PET_PICKS.len() + 1);
}

#[test]
fn under_a_pet_steps_the_face_choice_aside_and_shows_plain_colours() {
    let rt = render(form(Some(wearing_cat())));
    assert!(!text_of(&rt).contains("\"team-look-face\""), "no Face choice under a pet");
    // No bot tile is the chosen one while a pet is worn.
    assert!(checked_in(&rt, "team-look-grid").is_empty());
}

#[test]
fn keeps_the_face_choice_for_a_bot() {
    let rt = render(form(Some(json!({"headwear": 1, "accessory": 2, "mouth": 3, "bot": {"shape": "droid", "face": "eyes"}}))));
    assert!(text_of(&rt).contains("\"team-look-face\""));
    assert_eq!(checked_in(&rt, "team-look-grid").len(), 1);
}

#[test]
fn a_tile_says_its_name_once_the_pet_is_here_and_that_it_downloads_when_it_is_not() {
    assert_eq!(tile_help("Reaper", true), "Reaper");
    assert_eq!(tile_help("Reaper", false), "Reaper: downloads from openpets.dev when picked");
}

#[test]
fn a_webp_decodes_to_its_pixels_and_anything_else_to_none() {
    // A 1x1 lossless WebP of one opaque red pixel, made by the same library.
    let mut red = Vec::new();
    image::codecs::webp::WebPEncoder::new_lossless(&mut red).encode(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8).unwrap();
    let (w, h, rgba) = decode_webp(&red).expect("decodes");
    assert_eq!((w, h, rgba.len()), (1, 1, 4));
    assert_eq!(rgba, vec![255, 0, 0, 255]);
    assert!(decode_webp(b"not a picture").is_none());
}

#[test]
fn a_pick_already_on_this_computer_goes_on_the_teammate_at_once_beside_its_bot() {
    let mut s = form(Some(json!({"headwear": 1, "accessory": 2, "mouth": 3, "bot": {"shape": "droid", "face": "eyes"}})));
    s.pet_picks.installed = Some(vec![PublicPet {
        source: PetSource::Gallery,
        id: "reaper".into(),
        display_name: "Reaper".into(),
        description: String::new(),
        rows: 11,
    }]);
    s.pet_picks.notice = Some("an old notice".into());
    pick(&mut s, "reaper");
    assert_eq!(s.people_panel.entry["avatar"]["pet"], json!({"source": "gallery", "id": "reaper"}));
    assert_eq!(s.people_panel.entry["avatar"]["bot"], json!({"shape": "droid", "face": "eyes"}), "the bot it falls back to stays");
    assert_eq!(s.pet_picks.notice, None);
    assert_eq!(s.pet_picks.adding, None, "nothing to download");
    let rt = render(s);
    assert_eq!(checked_in(&rt, "team-pets"), vec!["Reaper".to_owned()]);
}
