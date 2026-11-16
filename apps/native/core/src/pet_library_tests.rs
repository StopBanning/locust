//! The pet library's own cases, one each, with the values of Electron's `a-pet-sheet-is-checked-before-it-is-kept.test.ts`
//! (and its disk and removal cases) and `shared/pets.ts`.

use crate::bot_spec::PetSource;
use crate::pet_library::*;
use crate::pets::*;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-pets-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// A WebP file's first bytes, for a sheet of this size: lossless, extended or lossy.
fn webp(kind: &str, width: u32, height: u32, alpha: bool, animated: bool, length: usize) -> Vec<u8> {
    let mut b = vec![0u8; length];
    b[0..4].copy_from_slice(b"RIFF");
    b[4..8].copy_from_slice(&((length - 8) as u32).to_le_bytes());
    b[8..12].copy_from_slice(b"WEBP");
    b[12..16].copy_from_slice(kind.as_bytes());
    b[16..20].copy_from_slice(&((length - 20) as u32).to_le_bytes());
    match kind {
        "VP8L" => {
            b[20] = 0x2f;
            let bits = (width - 1) | ((height - 1) << 14) | (u32::from(alpha) << 28);
            b[21..25].copy_from_slice(&bits.to_le_bytes());
        }
        "VP8X" => {
            b[20] = (if alpha { 0x10 } else { 0 }) | (if animated { 0x02 } else { 0 });
            b[24..27].copy_from_slice(&(width - 1).to_le_bytes()[..3]);
            b[27..30].copy_from_slice(&(height - 1).to_le_bytes()[..3]);
        }
        _ => {
            b[23..26].copy_from_slice(&[0x9d, 0x01, 0x2a]);
            b[26..28].copy_from_slice(&(width as u16).to_le_bytes());
            b[28..30].copy_from_slice(&(height as u16).to_le_bytes());
        }
    }
    b
}

fn lossless(width: u32, height: u32) -> Vec<u8> {
    webp("VP8L", width, height, true, false, 64)
}

fn size(width: u32, height: u32, alpha: bool, animated: bool) -> WebpSize {
    WebpSize { width, height, alpha, animated }
}

#[test]
fn a_webp_sheet_reads_lossless_extended_and_lossy() {
    assert_eq!(webp_size(&lossless(1536, 2288), None), Some(size(1536, 2288, true, false)));
    assert_eq!(webp_size(&webp("VP8X", 1536, 1872, true, true, 64), None), Some(size(1536, 1872, true, true)));
    assert_eq!(webp_size(&webp("VP8 ", 200, 100, true, false, 64), None), Some(size(200, 100, false, false)));
}

#[test]
fn a_webp_that_is_not_one_or_is_cut_short_is_refused() {
    let mut png = lossless(1536, 2288);
    png[0..4].copy_from_slice(b"\x89PNG");
    assert_eq!(webp_size(&png, None), None);
    // Its own length says 64 bytes; a download that stopped at 40 is not the image.
    assert_eq!(webp_size(&lossless(1536, 2288), Some(40)), None);
    let mut unsigned = lossless(1536, 2288);
    unsigned[20] = 0;
    assert_eq!(webp_size(&unsigned, None), None);
}

#[test]
fn a_version_2_sheet_is_exact_and_transparent() {
    assert_eq!(pet_rows_for(size(1536, 2288, true, false), 2), Ok(11));
    assert!(pet_rows_for(size(1536, 2000, true, false), 2).unwrap_err().contains("1536 x 2288"));
    assert!(pet_rows_for(size(1536, 2288, false, false), 2).unwrap_err().contains("transparency"));
    assert!(pet_rows_for(size(1536, 2288, true, true), 2).unwrap_err().contains("animation"));
}

#[test]
fn a_version_1_sheet_reads_at_its_own_scale_eight_across_and_nine_down() {
    assert_eq!(pet_rows_for(size(1536, 1872, true, false), 1), Ok(9));
    assert_eq!(pet_rows_for(size(768, 936, true, false), 1), Ok(9));
    assert!(pet_rows_for(size(1536, 2000, true, false), 1).unwrap_err().contains("nine down"));
    assert!(pet_rows_for(size(1536, 1530, true, false), 1).unwrap_err().contains("shape"));
    assert!(pet_rows_for(size(384, 468, true, false), 1).unwrap_err().contains("too small"));
}

#[test]
fn each_state_plays_its_own_row_at_openpets_pace() {
    let row = |state| {
        let r = pet_row(state);
        (r.row, r.frames, r.ms, r.plays)
    };
    assert_eq!(row(PetState::Idle), (0, 6, 5500.0, None));
    assert_eq!(row(PetState::Waving), (3, 4, 700.0, Some(2)));
    assert_eq!(row(PetState::Jumping), (4, 5, 840.0, Some(2)));
    assert_eq!(row(PetState::Failed), (5, 8, 1220.0, Some(2)));
    assert_eq!(row(PetState::Waiting), (6, 6, 1010.0, None));
    assert_eq!(row(PetState::Running), (7, 6, 820.0, None));
    assert_eq!(row(PetState::Review), (8, 6, 1030.0, None));
    assert_eq!(PET_NEUTRAL, (0, 6));
}

#[test]
fn a_gazing_head_is_one_of_sixteen_cells_on_the_last_two_rows() {
    assert_eq!(pet_gaze_cell(0), Some((9, 0)));
    assert_eq!(pet_gaze_cell(7), Some((9, 7)));
    assert_eq!(pet_gaze_cell(8), Some((10, 0)));
    assert_eq!(pet_gaze_cell(15), Some((10, 7)));
    assert_eq!(pet_gaze_cell(16), None);
    assert_eq!(pet_gaze_cell(-1), None);
}

#[test]
fn the_pets_locust_offers_are_21_distinct_pets_each_with_an_id_the_library_accepts() {
    assert_eq!(PET_PICKS.len(), 21);
    let mut ids: Vec<&str> = PET_PICKS.iter().map(|p| p.0).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 21);
    for (id, name, rows) in PET_PICKS {
        assert!(crate::bot_spec::is_pet_id(id), "{id}");
        assert!(is_pet_pick(id));
        assert!(!name.trim().is_empty());
        assert!(rows == 9 || rows == 11);
    }
    assert!(!is_pet_pick("hoodie-cat"));
    assert_eq!(PET_PICKS[0], ("luna-techbot", "Luna TechBot", 11));
    assert_eq!(PET_PICKS[20], ("codex-buddy", "Codex Buddy", 9));
}

fn meta(extra: Value) -> Value {
    let mut base = json!({"id": "mochi", "displayName": "Mochi", "description": "One.", "spritesheetPath": "spritesheet.webp"});
    for (k, v) in extra.as_object().unwrap() {
        if v.is_null() { base.as_object_mut().unwrap().remove(k); } else { base[k] = v.clone(); }
    }
    base
}

#[test]
fn a_pet_json_is_held_to_openpets_rules_with_a_refusal_in_words() {
    let ok = parsed_pet_meta(&meta(json!({})), "mochi").unwrap();
    assert_eq!((ok.display_name.as_str(), ok.description.as_str(), ok.version), ("Mochi", "One.", 1));
    assert_eq!(parsed_pet_meta(&meta(json!({"spriteVersionNumber": 2})), "mochi").unwrap().version, 2);
    assert_eq!(parsed_pet_meta(&json!([1]), "mochi"), Err("Its pet.json is not an object.".into()));
    assert_eq!(parsed_pet_meta(&meta(json!({})), "other"), Err("Its pet.json names another pet than its folder.".into()));
    let long = "x".repeat(121);
    assert_eq!(parsed_pet_meta(&meta(json!({"displayName": "  "})), "mochi"), Err("Its pet.json has no name, or one too long.".into()));
    assert_eq!(parsed_pet_meta(&meta(json!({"displayName": long})), "mochi"), Err("Its pet.json has no name, or one too long.".into()));
    assert_eq!(parsed_pet_meta(&meta(json!({"description": "x".repeat(501)})), "mochi"), Err("Its pet.json has a description too long.".into()));
    assert_eq!(parsed_pet_meta(&meta(json!({"description": null})), "mochi").unwrap().description, "");
    assert_eq!(parsed_pet_meta(&meta(json!({"spritesheetPath": "x.png"})), "mochi"), Err("Its pet.json does not name spritesheet.webp.".into()));
    assert_eq!(parsed_pet_meta(&meta(json!({"spriteVersionNumber": 3})), "mochi"), Err("Its pet.json names a sheet version Locust does not know.".into()));
    assert_eq!(parsed_pet_meta(&meta(json!({"spriteVersionNumber": "2"})), "mochi"), Err("Its pet.json names a sheet version Locust does not know.".into()));
}

#[test]
fn a_catalog_address_is_https_on_openpets_dev_under_pets_with_no_port_or_credentials() {
    let file = |s: &str| catalog_address(&json!(s), AddressKind::File);
    let page = |s: &str| catalog_address(&json!(s), AddressKind::Catalog);
    assert_eq!(file("https://openpets.dev/pets/robot-openpets/spritesheet.webp").as_deref(), Some("https://openpets.dev/pets/robot-openpets/spritesheet.webp"));
    assert_eq!(page("https://openpets.dev/pets/catalog.v3/page-000.json").as_deref(), Some("https://openpets.dev/pets/catalog.v3/page-000.json"));
    assert_eq!(page("https://openpets.dev/pets/robot/x.webp"), None);
    for bad in [
        "http://openpets.dev/pets/a.webp",
        "https://evil.example/pets/a.webp",
        "https://openpets.dev/elsewhere/a.webp",
        "https://openpets.dev:8443/pets/a.webp",
        "https://user:pw@openpets.dev/pets/a.webp", // public-guard: allow
        "https://openpets.dev/pets/../secret",
        "https://zip.openpets.dev/pets/a.zip",
        "not a url",
    ] {
        assert_eq!(file(bad), None, "{bad}");
    }
    assert_eq!(catalog_address(&json!(5), AddressKind::File), None);
    assert_eq!(file(&format!("https://openpets.dev/pets/{}", "a".repeat(2050))), None);
}

#[test]
fn the_search_index_leaves_out_an_entry_that_does_not_read_and_keeps_the_rest() {
    let index = json!({"pets": [
        {"id": "robot", "displayName": " Robot ", "searchText": "a robot", "catalogPage": 0, "featured": true, "spriteVersionNumber": 2},
        {"id": "Bad Id", "displayName": "Bad", "catalogPage": 0},
        {"id": "nori", "displayName": "", "catalogPage": 0},
        {"id": "dot", "displayName": "Dot", "catalogPage": 5},
        {"id": "brew", "displayName": "Brew", "catalogPage": 0, "spriteVersionNumber": 3},
        {"id": "bitty", "displayName": "Bitty", "catalogPage": 0, "original": true}
    ]});
    let entries = parsed_gallery_entries(&index, 1);
    assert_eq!(entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["robot", "bitty"]);
    assert_eq!((entries[0].display_name.as_str(), entries[0].rows, entries[0].featured, entries[0].original), ("Robot", 11, true, false));
    assert_eq!((entries[1].rows, entries[1].featured, entries[1].original), (9, false, true));
    assert!(parsed_gallery_entries(&json!({"pets": 5}), 1).is_empty());
    let clipped = parsed_gallery_entries(&json!({"pets": [{"id": "robot", "displayName": "R", "catalogPage": 0, "searchText": "y".repeat(500)}]}), 1);
    assert_eq!(clipped[0].search_text.len(), 400);
}

#[test]
fn a_catalog_page_keeps_only_entries_whose_addresses_pass() {
    let page = json!({"pets": [
        {"id": "robot", "displayName": "Robot", "description": "  Small. ", "thumbnail": "https://openpets.dev/pets/robot/thumb.webp", "spritesheet": "https://openpets.dev/pets/robot/spritesheet.webp", "spriteVersionNumber": 2},
        {"id": "glitchcat", "displayName": "G", "thumbnail": "https://openpets.dev/pets/g/t.webp", "spritesheet": "https://evil.example/pets/g/s.webp"},
        {"id": "meowbot", "displayName": "M", "thumbnail": "https://openpets.dev/elsewhere/t.webp", "spritesheet": "https://openpets.dev/pets/m/s.webp"}
    ]});
    let entries = parsed_catalog_entries(&page);
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].id.as_str(), entries[0].description.as_str(), entries[0].version), ("robot", "Small.", 2));
    assert_eq!(entries[0].spritesheet, "https://openpets.dev/pets/robot/spritesheet.webp");
}

fn pet_folder(root: &Path, id: &str, extra: Value, sheet: &[u8]) {
    fs::create_dir_all(root.join(id)).unwrap();
    let mut m = json!({"id": id, "displayName": "A Pet", "description": "One.", "spritesheetPath": "spritesheet.webp", "spriteVersionNumber": 2});
    for (k, v) in extra.as_object().unwrap() {
        if v.is_null() { m.as_object_mut().unwrap().remove(k); } else { m[k] = v.clone(); }
    }
    fs::write(root.join(id).join("pet.json"), m.to_string()).unwrap();
    fs::write(root.join(id).join("spritesheet.webp"), sheet).unwrap();
}

fn library(root: &Path) -> PetLibrary {
    PetLibrary::new(root.join("pets"), root.join("bundled"), Some(root.join("codex")), Some(root.join("pets-removed.json")))
}

fn names(pets: &[PublicPet]) -> Vec<String> {
    pets.iter().map(|p| format!("{:?}/{}/{}", p.source, p.id, p.rows)).collect()
}

#[test]
fn the_pets_on_this_computer_are_locusts_own_the_gallerys_and_codexs_each_read_where_it_is() {
    let root = scratch();
    pet_folder(&root.join("bundled"), "hoodie-cat", json!({"displayName": "Hoodie Cat"}), &lossless(1536, 2288));
    pet_folder(&root.join("pets"), "yuyu-chibi", json!({"displayName": "Yuyu"}), &lossless(1536, 2288));
    pet_folder(&root.join("codex"), "mochi", json!({"displayName": "Mochi", "spriteVersionNumber": null}), &lossless(1536, 1872));
    let lib = library(&root);
    assert_eq!(names(&lib.list()), ["Bundled/hoodie-cat/11", "Gallery/yuyu-chibi/11", "Codex/mochi/9"]);
    let (bytes, rows) = lib.sheet(PetSource::Codex, "mochi").unwrap();
    assert_eq!(rows, 9);
    assert_eq!(webp_size(&bytes, None).unwrap().height, 1872);
}

#[test]
fn a_pet_whose_json_names_another_or_whose_sheet_is_the_wrong_shape_is_left_out() {
    let root = scratch();
    pet_folder(&root.join("codex"), "impostor", json!({"id": "someone-else"}), &lossless(1536, 2288));
    pet_folder(&root.join("codex"), "squashed", json!({}), &lossless(1536, 2000));
    pet_folder(&root.join("codex"), "fine", json!({}), &lossless(1536, 2288));
    let lib = library(&root);
    assert_eq!(lib.list().iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["fine"]);
    assert!(lib.sheet(PetSource::Codex, "impostor").unwrap_err().contains("another pet"));
}

#[test]
fn a_sheet_cut_short_or_not_a_webp_is_left_out() {
    let root = scratch();
    pet_folder(&root.join("codex"), "cut", json!({}), &lossless(1536, 2288)[..40]);
    pet_folder(&root.join("codex"), "page", json!({}), b"<html>a page, not a pet</html> and more bytes after it");
    assert!(library(&root).list().is_empty());
    assert!(library(&root).sheet(PetSource::Codex, "page").unwrap_err().contains("not a WebP"));
}

#[cfg(unix)]
#[test]
fn nothing_is_read_through_a_link_not_a_linked_pet_folder_nor_a_linked_codex_folder() {
    let root = scratch();
    let outside = scratch();
    pet_folder(&outside, "stray", json!({}), &lossless(1536, 2288));
    fs::create_dir_all(root.join("pets")).unwrap();
    std::os::unix::fs::symlink(outside.join("stray"), root.join("pets").join("stray")).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("codex")).unwrap();
    let lib = library(&root);
    assert!(lib.list().is_empty());
    assert_eq!(lib.sheet(PetSource::Gallery, "stray"), Err("That pet is not on this computer.".into()));
    assert_eq!(lib.sheet(PetSource::Codex, "stray"), Err("That pet is not on this computer.".into()));
}

#[cfg(unix)]
#[test]
fn a_pet_file_that_is_a_link_out_of_its_folder_is_not_read() {
    let root = scratch();
    let outside = scratch();
    pet_folder(&root.join("codex"), "sneaky", json!({}), &lossless(1536, 2288));
    fs::write(outside.join("elsewhere.json"), "{}").unwrap();
    fs::remove_file(root.join("codex/sneaky/pet.json")).unwrap();
    std::os::unix::fs::symlink(outside.join("elsewhere.json"), root.join("codex/sneaky/pet.json")).unwrap();
    assert_eq!(library(&root).sheet(PetSource::Codex, "sneaky"), Err("A pet file is not a plain file, so it was not read.".into()));
}

#[test]
fn at_most_one_hundred_codex_pets_are_read() {
    let root = scratch();
    for n in 0..103 {
        pet_folder(&root.join("codex"), &format!("p{n:03}"), json!({}), &lossless(1536, 2288));
    }
    let listed = library(&root).list();
    assert_eq!(listed.len(), CODEX_PETS_MAX);
    assert_eq!(listed[0].id, "p000");
}

#[test]
fn taking_a_pet_off_the_computer_deletes_its_folder_and_refuses_a_path() {
    let root = scratch();
    pet_folder(&root.join("pets"), "luna-techbot", json!({}), &lossless(1536, 2288));
    let lib = library(&root);
    lib.remove("luna-techbot", false).unwrap();
    assert!(!root.join("pets/luna-techbot").exists());
    assert_eq!(lib.remove("../pets", false), Err("That pet is not on this computer.".into()));
    assert!(root.join("pets").exists());
}

#[test]
fn several_taken_out_of_the_picker_at_once_all_stay_a_worn_ones_files_stay_and_picking_one_brings_it_back() {
    let root = scratch();
    pet_folder(&root.join("pets"), "luna-techbot", json!({}), &lossless(1536, 2288));
    pet_folder(&root.join("pets"), "robot", json!({}), &lossless(1536, 2288));
    let lib = std::sync::Arc::new(library(&root));
    assert!(lib.removed_picks().is_empty());
    let handles: Vec<_> = [("luna-techbot", false), ("robot", true), ("reaper", false)]
        .into_iter()
        .map(|(id, keep)| {
            let lib = lib.clone();
            std::thread::spawn(move || lib.remove(id, keep).unwrap())
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    let mut removed = lib.removed_picks();
    removed.sort();
    assert_eq!(removed, ["luna-techbot", "reaper", "robot"]);
    // A worn pet's files stay, so its teammate's face does.
    let mut left: Vec<String> = fs::read_dir(root.join("pets")).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
    left.sort();
    assert_eq!(left, ["robot"]);
    // Picked again: back in the picker.
    lib.bring_back("robot").unwrap();
    let mut removed = lib.removed_picks();
    removed.sort();
    assert_eq!(removed, ["luna-techbot", "reaper"]);
}

#[test]
fn a_removed_list_that_does_not_read_is_nothing_removed_and_one_that_does_keeps_only_picks() {
    let root = scratch();
    let lib = library(&root);
    fs::write(root.join("pets-removed.json"), "{ not json").unwrap();
    assert!(lib.removed_picks().is_empty());
    fs::write(root.join("pets-removed.json"), json!({"schemaVersion": 1, "ids": ["robot", "../pets", "yuyu-chibi", 5, "robot"]}).to_string()).unwrap();
    assert_eq!(lib.removed_picks(), ["robot"]);
}

#[test]
fn a_pet_that_is_not_a_pick_is_deleted_without_being_listed_as_removed() {
    let root = scratch();
    pet_folder(&root.join("pets"), "yuyu-chibi", json!({}), &lossless(1536, 2288));
    let lib = library(&root);
    lib.remove("yuyu-chibi", false).unwrap();
    assert!(lib.removed_picks().is_empty());
    assert!(!root.join("pets/yuyu-chibi").exists());
}

#[test]
fn a_version_1_sheets_frames_must_be_within_two_percent_of_a_pets_shape() {
    // 1536 x 1827 is frames 192 x 203: 2.5% off the 192:208 shape.
    assert!(pet_rows_for(size(1536, 1827, true, false), 1).unwrap_err().contains("shape"));
    // 1536 x 1863 is frames 192 x 207: within 2%.
    assert_eq!(pet_rows_for(size(1536, 1863, true, false), 1), Ok(9));
}

#[test]
fn a_file_one_byte_longer_than_its_riff_length_is_the_padding_byte_and_two_is_not() {
    let sheet = lossless(1536, 2288);
    assert!(webp_size(&sheet, Some(64)).is_some());
    assert!(webp_size(&sheet, Some(65)).is_some());
    assert!(webp_size(&sheet, Some(66)).is_none());
}

/// Beyond Electron: the Rust app keeps its own pets folder until the switch, and copies in the pets added in Electron's.
#[test]
fn the_pets_added_in_the_other_app_are_copied_in_checked_and_the_other_folder_is_only_read() {
    let root = scratch();
    let electron = root.join("electron-pets");
    pet_folder(&electron, "nori", json!({"displayName": "Nori"}), &lossless(1536, 2288));
    pet_folder(&electron, "squashed", json!({}), &lossless(1536, 2000));
    pet_folder(&electron, "yuyu-chibi", json!({"displayName": "Electron's Yuyu"}), &lossless(1536, 2288));
    pet_folder(&root.join("pets"), "yuyu-chibi", json!({"displayName": "Yuyu"}), &lossless(1536, 2288));
    let before: Vec<_> = fs::read_dir(&electron).unwrap().flatten().map(|e| e.file_name()).collect();
    let lib = library(&root);
    assert_eq!(lib.import_from(&electron), 1, "nori only: squashed is not the shape of a pet, yuyu is here already");
    assert_eq!(names(&lib.list()).iter().filter(|n| n.starts_with("Gallery")).cloned().collect::<Vec<_>>(), ["Gallery/nori/11", "Gallery/yuyu-chibi/11"]);
    assert_eq!(lib.list().iter().find(|p| p.id == "yuyu-chibi").unwrap().display_name, "Yuyu", "the one here stands");
    assert!(lib.sheet(PetSource::Gallery, "nori").is_ok());
    let after: Vec<_> = fs::read_dir(&electron).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(before, after, "nothing in the other folder changed");
    assert_eq!(lib.import_from(&electron), 0, "a second time copies nothing");
    assert_eq!(lib.import_from(&root.join("not-there")), 0);
}
