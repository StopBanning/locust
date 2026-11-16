//! The download half of the pet library (Electron `main/pet-library.ts`, `createPetLibrary`'s `download`, the catalog,
//! `thumbnail` and `add`): the openpets.dev gallery read only on a person's click, every answer held to OpenPets' rules.
//! A pet is downloaded only if it is one of the 21 picks; an address is used only if it passes `catalog_address`; an
//! answer that came from anywhere but where it was sent is refused; a size is counted as it arrives, so a lying length
//! cannot make Locust read more; and a pet is written beside, then moved into place, so it is never half there.
//! The network sits behind `PetFetch`, so the tests serve a gallery of their own.

use crate::bot_spec::is_pet_id;
use crate::pet_library::*;
use crate::pets::{pet_rows_for, webp_size};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// One answer: what a fetch got back, without having read its body yet.
pub struct PetResponse {
    pub status: u16,
    /// The address the answer came from.
    pub url: String,
    pub redirected: bool,
    pub content_length: Option<u64>,
    pub body: Box<dyn Read + Send>,
}

/// Fetches one address. Err: it could not be reached at all. A redirect must not be followed.
pub trait PetFetch: Send + Sync {
    fn get(&self, url: &str) -> Result<PetResponse, ()>;
}

/// The real thing: `ureq`, no redirects, `FETCH_TIMEOUT_MS`.
pub struct UreqFetch;

impl PetFetch for UreqFetch {
    fn get(&self, url: &str) -> Result<PetResponse, ()> {
        let agent = ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_millis(FETCH_TIMEOUT_MS)).build();
        let response = match agent.get(url).call() {
            Ok(response) => response,
            Err(ureq::Error::Status(_, response)) => response,
            Err(_) => return Err(()),
        };
        let status = response.status();
        Ok(PetResponse {
            status,
            url: response.get_url().to_owned(),
            redirected: (300..400).contains(&status),
            content_length: response.header("content-length").and_then(|v| v.parse().ok()),
            body: Box::new(response.into_reader()),
        })
    }
}

#[derive(Clone)]
struct GalleryIndex {
    fetched_at: u64,
    pages: Vec<String>,
    entries: Vec<GalleryEntry>,
}

pub struct PetGallery {
    pub library: PetLibrary,
    /// The gallery's catalog and small pictures: `<userData>/pets-cache`.
    cache_root: PathBuf,
    fetch: Box<dyn PetFetch>,
    now: Box<dyn Fn() -> u64 + Send + Sync>,
    index: Mutex<Option<GalleryIndex>>,
    pages: Mutex<HashMap<usize, (u64, Vec<CatalogEntry>)>>,
    /// Added one at a time: a second click on the same pet waits for the first.
    adding: Mutex<()>,
}

fn data_url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::from("data:image/webp;base64,");
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { ALPHABET[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { ALPHABET[n as usize & 63] as char } else { '=' });
    }
    out
}

fn unknown_catalog() -> String {
    "The gallery sent a catalog Locust does not know.".into()
}

impl PetGallery {
    pub fn new(library: PetLibrary, cache_root: PathBuf, fetch: Box<dyn PetFetch>, now: Box<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self {
            library,
            cache_root,
            fetch,
            now,
            index: Mutex::new(None),
            pages: Mutex::new(HashMap::new()),
            adding: Mutex::new(()),
        }
    }

    /// One address, read whole, refused past `max` bytes (counted as it arrives) and refused when the answer came from
    /// anywhere but where it was sent.
    fn download(&self, url: &str, max: u64) -> Result<Vec<u8>, String> {
        let response = self.fetch.get(url).map_err(|()| UNREACHABLE.to_owned())?;
        if response.redirected || (!response.url.is_empty() && response.url != url) {
            return Err("The gallery sent Locust somewhere else, so nothing was downloaded.".into());
        }
        if !(200..300).contains(&response.status) {
            return Err(format!("The gallery answered {}, so nothing was downloaded. Try again later.", response.status));
        }
        let too_big = || "That file is larger than a pet should be, so it was not downloaded.".to_owned();
        if response.content_length.is_some_and(|declared| declared > max) {
            return Err(too_big());
        }
        let mut bytes = Vec::new();
        response.body.take(max + 1).read_to_end(&mut bytes).map_err(|_| UNREACHABLE.to_owned())?;
        if bytes.len() as u64 > max {
            return Err(too_big());
        }
        Ok(bytes)
    }

    fn download_json(&self, url: &str) -> Result<Value, String> {
        let bytes = self.download(url, CATALOG_JSON_MAX)?;
        serde_json::from_slice(&bytes).map_err(|_| "The gallery sent something Locust could not read. Try again later.".to_owned())
    }

    fn stored_index(&self) -> Option<GalleryIndex> {
        let value: Value = serde_json::from_str(&fs::read_to_string(self.cache_root.join("gallery.json")).ok()?).ok()?;
        let fetched_at = value.get("fetchedAt")?.as_u64()?;
        let pages = value
            .get("pages")?
            .as_array()?
            .iter()
            .map(|page| catalog_address(page, AddressKind::Catalog))
            .collect::<Option<Vec<String>>>()?;
        let entries = parsed_gallery_entries(&json!({ "pets": value.get("entries")? }), pages.len());
        Some(GalleryIndex { fetched_at, pages, entries })
    }

    fn fresh_index(&self) -> Result<GalleryIndex, String> {
        let root = self.download_json(CATALOG_INDEX_URL)?;
        let (3, Some(listed)) = (root.get("version").and_then(Value::as_u64).unwrap_or(0), root.get("pages").and_then(Value::as_array)) else {
            return Err(unknown_catalog());
        };
        let pages: Vec<Option<String>> = listed.iter().take(200).map(|page| catalog_address(page, AddressKind::Catalog)).collect();
        let search = root.get("search").and_then(|s| catalog_address(s, AddressKind::Catalog));
        let (Some(search), false) = (search, pages.is_empty() || pages.iter().any(Option::is_none)) else {
            return Err(unknown_catalog());
        };
        let pages: Vec<String> = pages.into_iter().flatten().collect();
        let search_index = self.download_json(&search)?;
        let (3, Some(search_pages)) = (search_index.get("version").and_then(Value::as_u64).unwrap_or(0), search_index.get("pages").and_then(Value::as_array))
        else {
            return Err(unknown_catalog());
        };
        let search_pages: Vec<Option<String>> = search_pages.iter().take(50).map(|page| catalog_address(page, AddressKind::Catalog)).collect();
        if search_pages.iter().any(Option::is_none) {
            return Err(unknown_catalog());
        }
        let mut entries: Vec<GalleryEntry> = Vec::new();
        for page in search_pages.into_iter().flatten() {
            for entry in parsed_gallery_entries(&self.download_json(&page)?, pages.len()) {
                if !entries.iter().any(|seen| seen.id == entry.id) {
                    entries.push(entry);
                }
            }
        }
        let read = GalleryIndex { fetched_at: (self.now)(), pages, entries };
        let kept: Vec<Value> = read
            .entries
            .iter()
            .map(|e| {
                let mut v = json!({"id": e.id, "displayName": e.display_name, "searchText": e.search_text, "catalogPage": e.catalog_page, "featured": e.featured, "original": e.original});
                if e.rows == 11 {
                    v["spriteVersionNumber"] = json!(2);
                }
                v
            })
            .collect();
        // Not kept is not an error: it is read again next time.
        if fs::create_dir_all(&self.cache_root).is_ok() {
            let _ = fs::write(self.cache_root.join("gallery.json"), json!({"fetchedAt": read.fetched_at, "pages": read.pages, "entries": kept}).to_string());
        }
        Ok(read)
    }

    /// The catalog: fresh, or read again; when the gallery cannot be reached, the last one read, said to be stale.
    /// Returns the index and whether it is stale.
    pub fn gallery_index(&self) -> Result<(Vec<GalleryEntry>, bool), String> {
        let mut held = self.index.lock().unwrap_or_else(|p| p.into_inner());
        if held.is_none() {
            *held = self.stored_index();
        }
        if let Some(index) = held.as_ref()
            && (self.now)().saturating_sub(index.fetched_at) < CATALOG_FRESH_MS
        {
            return Ok((index.entries.clone(), false));
        }
        match self.fresh_index() {
            Ok(index) => {
                let entries = index.entries.clone();
                *held = Some(index);
                Ok((entries, false))
            }
            Err(error) => match held.as_ref() {
                Some(index) => Ok((index.entries.clone(), true)),
                None => Err(error),
            },
        }
    }

    /// The catalog page a pet is on, read once per `CATALOG_FRESH_MS`.
    fn catalog_entry(&self, id: &str) -> Result<CatalogEntry, String> {
        self.gallery_index()?;
        let (listed, address) = {
            let held = self.index.lock().unwrap_or_else(|p| p.into_inner());
            let index = held.as_ref().ok_or(NOT_LISTED)?;
            let listed = index.entries.iter().find(|e| e.id == id).ok_or(NOT_LISTED)?.clone();
            let address = index.pages.get(listed.catalog_page).ok_or(NOT_LISTED)?.clone();
            (listed, address)
        };
        let now = (self.now)();
        let cached = {
            let pages = self.pages.lock().unwrap_or_else(|p| p.into_inner());
            pages.get(&listed.catalog_page).filter(|(at, _)| now.saturating_sub(*at) < CATALOG_FRESH_MS).map(|(_, e)| e.clone())
        };
        let entries = match cached {
            Some(entries) => entries,
            None => {
                let entries = parsed_catalog_entries(&self.download_json(&address)?);
                self.pages.lock().unwrap_or_else(|p| p.into_inner()).insert(listed.catalog_page, (now, entries.clone()));
                entries
            }
        };
        entries.into_iter().find(|candidate| candidate.id == id).ok_or_else(|| NOT_LISTED.to_owned())
    }

    fn prune_thumbnails(&self, root: &std::path::Path) {
        let Ok(dir) = fs::read_dir(root) else { return };
        let mut aged: Vec<(std::time::SystemTime, PathBuf)> =
            dir.flatten().filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect();
        if aged.len() <= THUMBNAILS_KEPT {
            return;
        }
        aged.sort();
        let extra = aged.len() - THUMBNAILS_KEPT;
        for (_, path) in aged.into_iter().take(extra) {
            let _ = fs::remove_file(path);
        }
    }

    /// A pick's small picture as a data URL, kept so the next ask is not a download. Only the pets Locust offers.
    pub fn thumbnail(&self, id: &str) -> Result<String, String> {
        if !is_pet_id(id) || !is_pet_pick(id) {
            return Err(NOT_OFFERED.into());
        }
        let root = self.cache_root.join("thumbnails");
        let kept = root.join(format!("{id}.webp"));
        if let Ok(bytes) = fs::read(&kept) {
            if let Ok(file) = fs::File::options().write(true).open(&kept) {
                let _ = file.set_modified(std::time::SystemTime::now());
            }
            return Ok(data_url(&bytes));
        }
        let entry = self.catalog_entry(id)?;
        let bytes = self.download(&entry.thumbnail, PET_THUMBNAIL_MAX)?;
        if webp_size(&bytes, None).is_none() {
            return Err("The gallery sent a picture Locust could not read.".into());
        }
        if fs::create_dir_all(&root).is_ok() && fs::write(&kept, &bytes).is_ok() {
            self.prune_thumbnails(&root);
        }
        Ok(data_url(&bytes))
    }

    /// Adds one of the picks to this computer: downloaded, checked, written beside and moved into place. A pet added
    /// before is not downloaded again. Picked again, it is back in the picker.
    pub fn add(&self, id: &str) -> Result<PublicPet, String> {
        if !is_pet_id(id) || !is_pet_pick(id) {
            return Err(NOT_OFFERED.into());
        }
        let _one_at_a_time = self.adding.lock().unwrap_or_else(|p| p.into_inner());
        let pet = match self.library.added(id) {
            Some(pet) => pet,
            None => self.download_pet(id)?,
        };
        let _ = self.library.bring_back(id);
        Ok(pet)
    }

    fn download_pet(&self, id: &str) -> Result<PublicPet, String> {
        let entry = self.catalog_entry(id)?;
        let bytes = self.download(&entry.spritesheet, PET_SHEET_MAX)?;
        let size = webp_size(&bytes, Some(bytes.len() as u64))
            .ok_or_else(|| format!("{}’s sheet is not a WebP image, so it was not added.", entry.display_name))?;
        let rows = pet_rows_for(size, entry.version).map_err(|why| format!("{} was not added. {}", entry.display_name, why))?;
        let mut meta = json!({
            "id": id,
            "displayName": entry.display_name,
            "description": entry.description,
            "spritesheetPath": "spritesheet.webp",
        });
        if entry.version == 2 {
            meta["spriteVersionNumber"] = json!(2);
        }
        // Where it came from, for the person and for whoever reads this folder next.
        meta["addedFrom"] = json!("https://openpets.dev");
        meta["addedAt"] = json!(chrono::DateTime::from_timestamp_millis((self.now)() as i64).map(|t| t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()).unwrap_or_default());
        let saved = || format!("{} could not be saved on this computer.", entry.display_name);
        let root = &self.library.installed_root;
        fs::create_dir_all(root).map_err(|_| saved())?;
        // Written beside, then moved into place: a pet is never half there.
        let incoming = root.join(format!(".incoming-{id}-{:08x}", std::process::id() ^ ((self.now)() as u32)));
        let written = fs::create_dir(&incoming)
            .and_then(|()| fs::write(incoming.join("spritesheet.webp"), &bytes))
            .and_then(|()| fs::write(incoming.join("pet.json"), format!("{}\n", serde_json::to_string_pretty(&meta).unwrap_or_default())))
            .and_then(|()| fs::rename(&incoming, root.join(id)));
        if written.is_err() {
            let _ = fs::remove_dir_all(&incoming);
            // Added meanwhile (another window, another click): that one stands.
            return self.library.added(id).ok_or_else(saved);
        }
        Ok(PublicPet { source: crate::bot_spec::PetSource::Gallery, id: id.to_owned(), display_name: entry.display_name, description: entry.description, rows })
    }
}
