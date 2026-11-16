//! The pet gallery's cases, one each, from Electron's `a-pet-sheet-is-checked-before-it-is-kept.test.ts`: a gallery
//! the tests serve (the catalog, its search pages, one catalog page and each pet's files).

use crate::pet_gallery::*;
use crate::pet_library::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static NEXT: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-gallery-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::SeqCst)));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn webp(width: u32, height: u32, alpha: bool) -> Vec<u8> {
    let mut b = vec![0u8; 64];
    b[0..4].copy_from_slice(b"RIFF");
    b[4..8].copy_from_slice(&56u32.to_le_bytes());
    b[8..12].copy_from_slice(b"WEBP");
    b[12..16].copy_from_slice(b"VP8L");
    b[16..20].copy_from_slice(&44u32.to_le_bytes());
    b[20] = 0x2f;
    b[21..25].copy_from_slice(&((width - 1) | ((height - 1) << 14) | (u32::from(alpha) << 28)).to_le_bytes());
    b
}

type Answer = Box<dyn Fn() -> Result<PetResponse, ()> + Send + Sync>;

#[derive(Clone, Default)]
struct Served {
    answers: Arc<Mutex<HashMap<String, Answer>>>,
    asked: Arc<Mutex<Vec<String>>>,
    offline: Arc<Mutex<bool>>,
}

impl PetFetch for Served {
    fn get(&self, url: &str) -> Result<PetResponse, ()> {
        self.asked.lock().unwrap().push(url.to_owned());
        if *self.offline.lock().unwrap() {
            return Err(());
        }
        match self.answers.lock().unwrap().get(url) {
            Some(answer) => answer(),
            None => Ok(plain(url, 404, b"not here".to_vec())),
        }
    }
}

fn plain(url: &str, status: u16, body: Vec<u8>) -> PetResponse {
    PetResponse { status, url: url.to_owned(), redirected: false, content_length: Some(body.len() as u64), body: Box::new(std::io::Cursor::new(body)) }
}

impl Served {
    fn serve(&self, url: &str, answer: impl Fn() -> Result<PetResponse, ()> + Send + Sync + 'static) {
        self.answers.lock().unwrap().insert(url.to_owned(), Box::new(answer));
    }
    fn serve_bytes(&self, url: &str, bytes: Vec<u8>) {
        let owned = url.to_owned();
        self.serve(url, move || Ok(plain(&owned, 200, bytes.clone())));
    }
    fn count(&self, ends: &str) -> usize {
        self.asked.lock().unwrap().iter().filter(|u| u.ends_with(ends)).count()
    }
}

struct Pet {
    id: &'static str,
    name: &'static str,
    sheet: Option<&'static str>,
    thumbnail: Option<&'static str>,
}

fn pet(id: &'static str, name: &'static str) -> Pet {
    Pet { id, name, sheet: None, thumbnail: None }
}

const PAGE: &str = "https://openpets.dev/pets/catalog.v3/page-000.json";
const SEARCH: &str = "https://openpets.dev/pets/catalog.v3/search.json";
const SEARCH_PAGE: &str = "https://openpets.dev/pets/catalog.v3/search-page-000.json";

fn sheet_at(id: &str) -> String {
    format!("https://openpets.dev/pets/{id}-openpets/spritesheet.webp")
}
fn thumb_at(id: &str) -> String {
    format!("https://openpets.dev/pets/{id}-openpets/thumb.webp")
}

fn gallery(pets: &[Pet]) -> Served {
    let served = Served::default();
    let json_body = |value: Value| value.to_string().into_bytes();
    served.serve_bytes(CATALOG_INDEX_URL, json_body(json!({"version": 3, "total": pets.len(), "pageSize": 100, "search": SEARCH, "pages": [PAGE]})));
    served.serve_bytes(SEARCH, json_body(json!({"version": 3, "total": pets.len(), "pageSize": 250, "pages": [SEARCH_PAGE]})));
    served.serve_bytes(
        SEARCH_PAGE,
        json_body(json!({"version": 3, "page": 0, "pets": pets.iter().map(|p| json!({"id": p.id, "displayName": p.name, "searchText": format!("{} a small friend", p.name), "catalogPage": 0, "featured": true, "original": false, "spriteVersionNumber": 2})).collect::<Vec<_>>()})),
    );
    served.serve_bytes(
        PAGE,
        json_body(json!({"version": 3, "page": 0, "pets": pets.iter().map(|p| json!({
            "id": p.id, "displayName": p.name, "description": format!("{}, drawn for the tests.", p.name),
            "thumbnail": p.thumbnail.map(str::to_owned).unwrap_or_else(|| thumb_at(p.id)),
            "spritesheet": p.sheet.map(str::to_owned).unwrap_or_else(|| sheet_at(p.id)),
            "spriteVersionNumber": 2})).collect::<Vec<_>>()})),
    );
    served
}

fn library(served: &Served, now: Arc<Mutex<u64>>) -> (PathBuf, PetGallery) {
    let root = scratch();
    let lib = PetLibrary::new(root.join("pets"), root.join("bundled"), Some(root.join("codex")), Some(root.join("pets-removed.json")));
    let clock = now.clone();
    let g = PetGallery::new(lib, root.join("pets-cache"), Box::new(served.clone()), Box::new(move || *clock.lock().unwrap()));
    (root, g)
}

fn clock() -> Arc<Mutex<u64>> {
    Arc::new(Mutex::new(1_000_000))
}

fn installed(root: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root.join("pets")).map(|d| d.flatten().map(|e| e.file_name().into_string().unwrap()).collect()).unwrap_or_default();
    names.sort();
    names
}

#[test]
fn nothing_is_downloaded_until_a_pet_is_picked_and_then_only_that_one_pet() {
    let served = gallery(&[pet("luna-techbot", "Luna TechBot"), pet("robot", "Robot")]);
    served.serve_bytes(&sheet_at("luna-techbot"), webp(1536, 2288, true));
    served.serve_bytes(&thumb_at("robot"), webp(148, 160, true));
    let (root, g) = library(&served, clock());
    assert!(g.thumbnail("robot").unwrap().starts_with("data:image/webp;base64,"));
    assert_eq!(served.count("spritesheet.webp"), 0);
    let added = g.add("luna-techbot").unwrap();
    assert_eq!((added.id.as_str(), added.display_name.as_str(), added.description.as_str(), added.rows), ("luna-techbot", "Luna TechBot", "Luna TechBot, drawn for the tests.", 11));
    assert_eq!(served.asked.lock().unwrap().iter().filter(|u| u.ends_with("spritesheet.webp")).cloned().collect::<Vec<_>>(), [sheet_at("luna-techbot")]);
    assert_eq!(installed(&root), ["luna-techbot"]);
    let meta: Value = serde_json::from_str(&fs::read_to_string(root.join("pets/luna-techbot/pet.json")).unwrap()).unwrap();
    assert_eq!(meta["id"], "luna-techbot");
    assert_eq!(meta["displayName"], "Luna TechBot");
    assert_eq!(meta["spritesheetPath"], "spritesheet.webp");
    assert_eq!(meta["spriteVersionNumber"], 2);
    assert_eq!(meta["addedFrom"], "https://openpets.dev");
    assert_eq!(g.library.list().iter().map(|p| format!("{:?}/{}", p.source, p.id)).collect::<Vec<_>>(), ["Gallery/luna-techbot"]);
}

#[test]
fn a_sheet_that_is_not_the_shape_of_a_pet_is_refused_and_nothing_of_it_is_kept() {
    let served = gallery(&[pet("robot", "Robot"), pet("nori", "Nori"), pet("bitty", "Bitty")]);
    served.serve_bytes(&sheet_at("robot"), webp(1536, 2000, true));
    served.serve_bytes(&sheet_at("nori"), webp(1536, 2288, false));
    served.serve_bytes(&sheet_at("bitty"), b"<html>a page, not a pet</html> and more bytes to be long enough".to_vec());
    let (root, g) = library(&served, clock());
    assert!(g.add("robot").unwrap_err().contains("1536 x 2288"));
    assert!(g.add("nori").unwrap_err().contains("transparency"));
    assert!(g.add("bitty").unwrap_err().contains("not a WebP"));
    assert!(installed(&root).is_empty());
    assert_eq!(g.add("robot").unwrap_err(), "Robot was not added. Its sheet is 1536 x 2000; a version 2 pet's is 1536 x 2288.");
}

#[test]
fn a_file_larger_than_a_pet_is_refused_whether_it_says_so_or_not() {
    let served = gallery(&[pet("dot", "Dot"), pet("brew", "Brew")]);
    let dot = sheet_at("dot");
    served.serve(&sheet_at("dot"), move || {
        let mut r = plain(&dot, 200, webp(1536, 2288, true));
        r.content_length = Some(PET_SHEET_MAX + 1);
        Ok(r)
    });
    let brew = sheet_at("brew");
    // No length given, and more than the cap sent: counted as it arrives.
    served.serve(&sheet_at("brew"), move || {
        Ok(PetResponse { status: 200, url: brew.clone(), redirected: false, content_length: None, body: Box::new(std::io::repeat(0).take(PET_SHEET_MAX + 5)) })
    });
    let (_, g) = library(&served, clock());
    assert_eq!(g.add("dot").unwrap_err(), "That file is larger than a pet should be, so it was not downloaded.");
    assert_eq!(g.add("brew").unwrap_err(), "That file is larger than a pet should be, so it was not downloaded.");
}

#[test]
fn an_answer_that_came_from_somewhere_else_is_refused() {
    let served = gallery(&[pet("reaper", "Reaper"), pet("glitchcat", "Glitchcat")]);
    served.serve(&sheet_at("reaper"), || {
        let mut r = plain("https://elsewhere.example/sheet.webp", 200, webp(1536, 2288, true));
        r.redirected = true;
        Ok(r)
    });
    served.serve(&sheet_at("glitchcat"), || Ok(plain("https://elsewhere.example/other.webp", 200, webp(1536, 2288, true))));
    let (root, g) = library(&served, clock());
    assert_eq!(g.add("reaper").unwrap_err(), "The gallery sent Locust somewhere else, so nothing was downloaded.");
    assert_eq!(g.add("glitchcat").unwrap_err(), "The gallery sent Locust somewhere else, so nothing was downloaded.");
    assert!(installed(&root).is_empty());
}

#[test]
fn an_address_off_openpets_dev_is_never_fetched_nor_a_pet_locust_does_not_offer() {
    let mut glitch = pet("glitchcat", "Glitchcat");
    glitch.sheet = Some("https://evil.example/pets/glitchcat/spritesheet.webp");
    let mut meow = pet("meowbot", "Meowbot");
    meow.sheet = Some("https://openpets.dev/elsewhere/spritesheet.webp");
    let served = gallery(&[glitch, meow, pet("yuyu-chibi", "Yuyu")]);
    let (_, g) = library(&served, clock());
    assert_eq!(g.add("glitchcat").unwrap_err(), "openpets.dev no longer lists that pet.");
    assert_eq!(g.add("meowbot").unwrap_err(), "openpets.dev no longer lists that pet.");
    // In the catalog, but not one of the picks: refused before anything is asked.
    let before = served.asked.lock().unwrap().len();
    let long = "a".repeat(70);
    for id in ["yuyu-chibi", "../robot", "builtin", "Robot", "", long.as_str()] {
        assert_eq!(g.add(id).unwrap_err(), "That pet is not one Locust offers.", "{id}");
        assert_eq!(g.thumbnail(id).unwrap_err(), "That pet is not one Locust offers.", "{id}");
    }
    assert_eq!(served.asked.lock().unwrap().len(), before);
    assert!(served.asked.lock().unwrap().iter().all(|u| u.starts_with("https://openpets.dev/pets/")));
    assert_eq!(served.count("spritesheet.webp"), 0);
}

#[test]
fn it_says_so_in_words_when_openpets_dev_cannot_be_reached_at_all() {
    let served = gallery(&[]);
    *served.offline.lock().unwrap() = true;
    let (_, g) = library(&served, clock());
    let words = "openpets.dev could not be reached. Check your connection and try again.";
    assert_eq!(g.thumbnail("robot").unwrap_err(), words);
    assert_eq!(g.add("robot").unwrap_err(), words);
}

#[test]
fn a_gallery_that_answers_with_an_error_says_which_and_one_that_sends_rubbish_says_that() {
    let served = gallery(&[pet("robot", "Robot")]);
    served.serve(&sheet_at("robot"), || Ok(plain(&sheet_at("robot"), 503, b"down".to_vec())));
    let (_, g) = library(&served, clock());
    assert_eq!(g.add("robot").unwrap_err(), "The gallery answered 503, so nothing was downloaded. Try again later.");
    let rubbish = gallery(&[]);
    rubbish.serve_bytes(CATALOG_INDEX_URL, b"{ not json".to_vec());
    let (_, g) = library(&rubbish, clock());
    assert_eq!(g.add("robot").unwrap_err(), "The gallery sent something Locust could not read. Try again later.");
    let strange = gallery(&[]);
    strange.serve_bytes(CATALOG_INDEX_URL, json!({"version": 2, "pages": []}).to_string().into_bytes());
    let (_, g) = library(&strange, clock());
    assert_eq!(g.add("robot").unwrap_err(), "The gallery sent a catalog Locust does not know.");
}

#[test]
fn a_pet_added_before_is_not_downloaded_again_and_picking_a_removed_pick_brings_it_back() {
    let served = gallery(&[pet("robot", "Robot")]);
    served.serve_bytes(&sheet_at("robot"), webp(1536, 2288, true));
    let (root, g) = library(&served, clock());
    g.add("robot").unwrap();
    g.library.remove("robot", true).unwrap();
    assert_eq!(g.library.removed_picks(), ["robot"]);
    let asked = served.asked.lock().unwrap().len();
    g.add("robot").unwrap();
    assert_eq!(served.asked.lock().unwrap().len(), asked);
    assert!(g.library.removed_picks().is_empty());
    assert_eq!(installed(&root), ["robot"]);
}

#[test]
fn the_catalog_is_used_for_six_hours_then_read_again_and_the_last_read_stands_in_when_the_gallery_is_gone() {
    let served = gallery(&[pet("robot", "Robot")]);
    served.serve_bytes(&thumb_at("robot"), webp(148, 160, true));
    let now = clock();
    let (_, g) = library(&served, now.clone());
    g.thumbnail("robot").unwrap();
    g.gallery_index().unwrap();
    assert_eq!(served.count("catalog.v3.json"), 1);
    *now.lock().unwrap() += CATALOG_FRESH_MS - 1;
    g.gallery_index().unwrap();
    assert_eq!(served.count("catalog.v3.json"), 1);
    *now.lock().unwrap() += 1;
    let (entries, stale) = g.gallery_index().unwrap();
    assert_eq!((served.count("catalog.v3.json"), stale, entries.len()), (2, false, 1));
    *now.lock().unwrap() += CATALOG_FRESH_MS;
    *served.offline.lock().unwrap() = true;
    let (entries, stale) = g.gallery_index().unwrap();
    assert_eq!((stale, entries.len()), (true, 1));
}

#[test]
fn the_catalog_read_is_kept_on_disk_so_a_new_start_does_not_ask_again() {
    let served = gallery(&[pet("robot", "Robot")]);
    let now = clock();
    let (root, g) = library(&served, now.clone());
    g.gallery_index().unwrap();
    let again = PetGallery::new(
        PetLibrary::new(root.join("pets"), root.join("bundled"), None, None),
        root.join("pets-cache"),
        Box::new(served.clone()),
        Box::new({ let n = now.clone(); move || *n.lock().unwrap() }),
    );
    let (entries, stale) = again.gallery_index().unwrap();
    assert_eq!((served.count("catalog.v3.json"), stale, entries.len(), entries[0].rows), (1, false, 1, 11));
}

#[test]
fn a_thumbnail_is_kept_and_not_asked_for_twice_and_a_picture_that_is_not_a_webp_is_refused() {
    let served = gallery(&[pet("robot", "Robot"), pet("nori", "Nori")]);
    served.serve_bytes(&thumb_at("robot"), webp(148, 160, true));
    served.serve_bytes(&thumb_at("nori"), b"not a picture at all, but long enough to look at".to_vec());
    let (root, g) = library(&served, clock());
    let first = g.thumbnail("robot").unwrap();
    assert_eq!(g.thumbnail("robot").unwrap(), first);
    assert_eq!(served.count("robot-openpets/thumb.webp"), 1);
    assert!(root.join("pets-cache/thumbnails/robot.webp").exists());
    assert_eq!(g.thumbnail("nori").unwrap_err(), "The gallery sent a picture Locust could not read.");
    assert!(!root.join("pets-cache/thumbnails/nori.webp").exists());
}

#[test]
fn a_thumbnail_larger_than_a_megabyte_is_refused() {
    let served = gallery(&[pet("robot", "Robot")]);
    let url = thumb_at("robot");
    let at = url.clone();
    served.serve(&at, move || Ok(PetResponse { status: 200, url: url.clone(), redirected: false, content_length: None, body: Box::new(std::io::repeat(0).take(PET_THUMBNAIL_MAX + 9)) }));
    let (_, g) = library(&served, clock());
    assert_eq!(g.thumbnail("robot").unwrap_err(), "That file is larger than a pet should be, so it was not downloaded.");
}

#[test]
fn only_four_hundred_thumbnails_are_kept_the_oldest_going_first() {
    let served = gallery(&[pet("robot", "Robot")]);
    served.serve_bytes(&thumb_at("robot"), webp(148, 160, true));
    let (root, g) = library(&served, clock());
    let dir = root.join("pets-cache/thumbnails");
    fs::create_dir_all(&dir).unwrap();
    for n in 0..THUMBNAILS_KEPT {
        let path = dir.join(format!("old-{n:03}.webp"));
        fs::write(&path, b"x").unwrap();
        let file = fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000 + n as u64)).unwrap();
    }
    g.thumbnail("robot").unwrap();
    let left: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().into_string().unwrap()).collect();
    assert_eq!(left.len(), THUMBNAILS_KEPT);
    assert!(left.contains(&"robot.webp".to_owned()));
    assert!(!left.contains(&"old-000.webp".to_owned()));
    assert!(left.contains(&"old-398.webp".to_owned()));
}

#[test]
fn a_pet_taken_off_after_adding_is_gone_from_the_list_and_the_disk() {
    let served = gallery(&[pet("luna-techbot", "Luna TechBot")]);
    served.serve_bytes(&sheet_at("luna-techbot"), webp(1536, 2288, true));
    let (root, g) = library(&served, clock());
    g.add("luna-techbot").unwrap();
    g.library.remove("luna-techbot", false).unwrap();
    assert!(installed(&root).is_empty());
    assert!(g.library.list().is_empty());
}

#[test]
fn a_pet_added_meanwhile_by_another_window_stands_and_the_add_says_so_rather_than_failing() {
    let served = gallery(&[pet("robot", "Robot")]);
    let (root, g) = library(&served, clock());
    let folder = root.join("pets/robot");
    let url = sheet_at("robot");
    let at = url.clone();
    // While this download runs, another window finishes adding the same pet.
    served.serve(&at, move || {
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("pet.json"), json!({"id": "robot", "displayName": "Robot", "description": "Theirs.", "spritesheetPath": "spritesheet.webp", "spriteVersionNumber": 2}).to_string()).unwrap();
        fs::write(folder.join("spritesheet.webp"), webp(1536, 2288, true)).unwrap();
        Ok(plain(&url, 200, webp(1536, 2288, true)))
    });
    let added = g.add("robot").unwrap();
    assert_eq!(added.description, "Theirs.");
    assert_eq!(installed(&root), ["robot"]);
}

#[test]
fn a_thumbnail_is_handed_over_as_standard_base64_with_its_padding() {
    use crate::pet_gallery::data_url;
    let body = |bytes: &[u8]| data_url(bytes).trim_start_matches("data:image/webp;base64,").to_owned();
    assert_eq!(body(b"Man"), "TWFu");
    assert_eq!(body(b"Ma"), "TWE=");
    assert_eq!(body(b"M"), "TQ==");
    assert_eq!(body(b""), "");
    assert_eq!(body(&[0xfb, 0xff, 0xfe]), "+//+");
}
