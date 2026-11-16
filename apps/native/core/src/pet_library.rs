//! The pets a teammate can wear, kept by the host (Electron `main/pet-library.ts` and `shared/pet-picks.ts`, 0.563-0.569).
//! Three places a pet comes from: Locust's own (the bundled Hoodie Cat), the openpets.dev gallery (downloaded only on a
//! person's click, never shipped) and Codex's own folder (`~/.codex/pets`, read where it is and never written).
//! This file is the part that needs no network: the rules a pet.json and a catalog address are held to, the gallery's
//! two kinds of page, the 21 picks, and reading, listing, removing and "taken out of the picker" on disk. The window
//! never names a path or an address: a pet is a source and an id.

use crate::bot_spec::{is_pet_id, PetSource};
use crate::pets::{pet_rows_for, webp_size, WEBP_HEADER_BYTES};
use serde_json::Value;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const PET_JSON_MAX: u64 = 128 * 1024;
pub const PET_SHEET_MAX: u64 = 24 * 1024 * 1024;
pub const PET_THUMBNAIL_MAX: u64 = 1024 * 1024;
pub const CATALOG_JSON_MAX: u64 = 4 * 1024 * 1024;
/// OpenPets reads at most this many Codex pets (`maxCodexPets`).
pub const CODEX_PETS_MAX: usize = 100;
/// How long a read of the gallery's catalog is used before it is read again.
pub const CATALOG_FRESH_MS: u64 = 6 * 60 * 60 * 1000;
pub const THUMBNAILS_KEPT: usize = 400;
pub const FETCH_TIMEOUT_MS: u64 = 30_000;
pub const CATALOG_INDEX_URL: &str = "https://openpets.dev/pets/catalog.v3.json";

pub const NOT_ON_THIS_COMPUTER: &str = "That pet is not on this computer.";
pub const NOT_OFFERED: &str = "That pet is not one Locust offers.";
pub const NOT_LISTED: &str = "openpets.dev no longer lists that pet.";
pub const UNREACHABLE: &str = "openpets.dev could not be reached. Check your connection and try again.";

/// The 21 pets Locust offers, in the order Colin added them (2026-10-03): id, its maker's name, sheet rows.
pub const PET_PICKS: [(&str, &str, u8); 21] = [
    ("luna-techbot", "Luna TechBot", 11),
    ("pixel-terminal", "Pixel Terminal", 9),
    ("yeelight-scene-screen-commander", "智屏小司令", 9),
    ("reaper", "Reaper", 9),
    ("glitchcat", "Glitchcat", 9),
    ("dot", "Dot", 9),
    ("brew", "Brew", 9),
    ("rainbow-terminal-cat", "Rainbow Terminal Cat", 9),
    ("tmuxai", "TmuxAI Official", 9),
    ("meowbyte", "Meowbyte", 9),
    ("robot", "Robot", 9),
    ("astro-bot", "Astro Bot", 9),
    ("meowbot", "Meowbot", 9),
    ("bitty", "Bitty", 9),
    ("cloud-puff", "Cloud Puff", 9),
    ("nori", "Nori", 9),
    ("bankr", "Bankr", 9),
    ("cabin-face", "Cabin", 9),
    ("dumpster-fire", "Dumpster Fire", 9),
    ("macintosh", "Macintosh", 9),
    ("codex-buddy", "Codex Buddy", 9),
];

pub fn is_pet_pick(id: &str) -> bool {
    PET_PICKS.iter().any(|(pick, _, _)| *pick == id)
}

/// A pet on this computer, as the window is told of it (`PublicPet`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicPet {
    pub source: PetSource,
    pub id: String,
    pub display_name: String,
    pub description: String,
    /// 9 (a version 1 sheet) or 11 (version 2).
    pub rows: u8,
}

/// A pet.json, as Codex writes one and as Locust writes its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PetMeta {
    pub display_name: String,
    pub description: String,
    pub version: u8,
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn name_ok(name: &str) -> bool {
    !name.trim().is_empty() && utf16_len(name) <= 120
}

/// OpenPets' `validateCodexPetMetadata`, a little kinder (`parsedPetMeta`): the id must be its folder's name; a name of
/// 1-120 characters; a description of up to 500 that may be empty; the sheet named as Codex names it; version 2 said
/// only as the number 2. Err is the refusal, in words.
pub fn parsed_pet_meta(value: &Value, folder: &str) -> Result<PetMeta, String> {
    let Some(record) = value.as_object() else {
        return Err("Its pet.json is not an object.".into());
    };
    if record.get("id").and_then(Value::as_str) != Some(folder) {
        return Err("Its pet.json names another pet than its folder.".into());
    }
    let display_name = record.get("displayName").and_then(Value::as_str).map(str::trim).unwrap_or("");
    if display_name.is_empty() || utf16_len(display_name) > 120 {
        return Err("Its pet.json has no name, or one too long.".into());
    }
    let description = match record.get("description") {
        None => "",
        Some(Value::String(text)) if utf16_len(text) <= 500 => text.as_str(),
        Some(_) => return Err("Its pet.json has a description too long.".into()),
    };
    if record.get("spritesheetPath").and_then(Value::as_str) != Some("spritesheet.webp") {
        return Err("Its pet.json does not name spritesheet.webp.".into());
    }
    let Some(version) = sprite_version(record.get("spriteVersionNumber")) else {
        return Err("Its pet.json names a sheet version Locust does not know.".into());
    };
    Ok(PetMeta {
        display_name: display_name.to_owned(),
        description: description.trim().to_owned(),
        version,
    })
}

/// Absent is version 1; exactly the number 2 is version 2; anything else is not a version.
fn sprite_version(value: Option<&Value>) -> Option<u8> {
    match value {
        None => Some(1),
        Some(Value::Number(number)) if number.as_f64() == Some(2.0) => Some(2),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressKind {
    File,
    Catalog,
}

/// A catalog address by OpenPets' rule (`catalogAddress`): https on openpets.dev, under `/pets/` (`/pets/catalog.v3/` for
/// the catalog's own pages), no port, no credentials. Returned as the URL the fetch compares its answer with.
pub fn catalog_address(raw: &Value, kind: AddressKind) -> Option<String> {
    let raw = raw.as_str()?;
    if utf16_len(raw) > 2048 {
        return None;
    }
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.port().is_some() {
        return None;
    }
    if url.host_str() != Some("openpets.dev") {
        return None;
    }
    let prefix = if kind == AddressKind::Catalog { "/pets/catalog.v3/" } else { "/pets/" };
    if !url.path().starts_with(prefix) || url.path().contains("..") {
        return None;
    }
    Some(url.to_string())
}

/// A pet in the gallery's search index: enough to list, find and filter it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GalleryEntry {
    pub id: String,
    pub display_name: String,
    pub search_text: String,
    pub catalog_page: usize,
    pub featured: bool,
    pub original: bool,
    pub rows: u8,
}

/// A pet on one of the catalog's pages: where its sheet and small picture are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    pub id: String,
    pub display_name: String,
    pub description: String,
    pub thumbnail: String,
    pub spritesheet: String,
    pub version: u8,
}

fn pet_list(value: &Value) -> &[Value] {
    value.get("pets").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

fn clip(text: &str, units: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        used += ch.len_utf16();
        if used > units {
            break;
        }
        out.push(ch);
    }
    out
}

fn id_of(pet: &Value) -> Option<&str> {
    pet.get("id").and_then(Value::as_str).filter(|id| is_pet_id(id))
}

/// The search index's entries that read; any other is left out, not the whole index (`parsedGalleryEntries`).
pub fn parsed_gallery_entries(value: &Value, catalog_pages: usize) -> Vec<GalleryEntry> {
    let mut entries = Vec::new();
    for pet in pet_list(value) {
        let Some(id) = id_of(pet) else { continue };
        let Some(name) = pet.get("displayName").and_then(Value::as_str).filter(|name| name_ok(name)) else { continue };
        let Some(page) = pet.get("catalogPage").and_then(Value::as_u64).filter(|page| (*page as usize) < catalog_pages) else {
            continue;
        };
        let Some(version) = sprite_version(pet.get("spriteVersionNumber")) else { continue };
        entries.push(GalleryEntry {
            id: id.to_owned(),
            display_name: name.trim().to_owned(),
            search_text: pet.get("searchText").and_then(Value::as_str).map(|t| clip(t, 400)).unwrap_or_default(),
            catalog_page: page as usize,
            featured: pet.get("featured") == Some(&Value::Bool(true)),
            original: pet.get("original") == Some(&Value::Bool(true)),
            rows: if version == 2 { 11 } else { 9 },
        });
    }
    entries
}

/// A catalog page's entries that read, their addresses held to OpenPets' rule (`parsedCatalogEntries`).
pub fn parsed_catalog_entries(value: &Value) -> Vec<CatalogEntry> {
    let mut entries = Vec::new();
    for pet in pet_list(value) {
        let Some(id) = id_of(pet) else { continue };
        let Some(thumbnail) = pet.get("thumbnail").and_then(|t| catalog_address(t, AddressKind::File)) else {
            continue;
        };
        let Some(spritesheet) = pet.get("spritesheet").and_then(|t| catalog_address(t, AddressKind::File)) else {
            continue;
        };
        let Some(name) = pet.get("displayName").and_then(Value::as_str).filter(|name| name_ok(name)) else { continue };
        let Some(version) = sprite_version(pet.get("spriteVersionNumber")) else { continue };
        entries.push(CatalogEntry {
            id: id.to_owned(),
            display_name: name.trim().to_owned(),
            description: pet.get("description").and_then(Value::as_str).map(|d| clip(d, 500).trim().to_owned()).unwrap_or_default(),
            thumbnail,
            spritesheet,
            version,
        });
    }
    entries
}

/// Where the pets are kept on this computer.
pub struct PetLibrary {
    /// Pets added from the gallery: `<userData>/pets`.
    pub installed_root: PathBuf,
    /// Locust's own, beside the app.
    pub bundled_root: PathBuf,
    /// `~/.codex/pets`, read only.
    pub codex_root: Option<PathBuf>,
    /// The picks taken out of the picker: `<userData>/pets-removed.json`.
    pub removed_file: PathBuf,
    removed_lock: Mutex<()>,
}

struct ReadPet {
    pet: PublicPet,
    version: u8,
}

impl PetLibrary {
    pub fn new(installed_root: PathBuf, bundled_root: PathBuf, codex_root: Option<PathBuf>, removed_file: Option<PathBuf>) -> Self {
        let removed_file = removed_file.unwrap_or_else(|| {
            let mut name = installed_root.clone().into_os_string();
            name.push("-removed.json");
            PathBuf::from(name)
        });
        Self { installed_root, bundled_root, codex_root, removed_file, removed_lock: Mutex::new(()) }
    }

    fn root_of(&self, source: PetSource) -> Option<&Path> {
        match source {
            PetSource::Bundled => Some(&self.bundled_root),
            PetSource::Gallery => Some(&self.installed_root),
            PetSource::Codex => self.codex_root.as_deref(),
        }
    }

    /// A root that is itself a link is not read at all (OpenPets refuses a linked Codex folder).
    fn usable_root(root: &Path) -> bool {
        fs::symlink_metadata(root).map(|info| info.is_dir()).unwrap_or(false)
    }

    /// A file inside `root`, read whole: a real file (not a link), no larger than `max` before it is read or after, and,
    /// its links resolved, still inside `root`. Ok(None) when it is not there; Err in words when it is there and fails.
    fn read_inside(root: &Path, path: &Path, max: u64) -> Result<Option<Vec<u8>>, String> {
        let Ok(info) = fs::symlink_metadata(path) else { return Ok(None) };
        if !info.is_file() {
            return Err("A pet file is not a plain file, so it was not read.".into());
        }
        if info.len() > max {
            return Err("A pet file is larger than a pet should be, so it was not read.".into());
        }
        let real = Self::contained(root, path)?;
        let bytes = fs::read(real).map_err(|_| "A pet file could not be read.".to_owned())?;
        if bytes.len() as u64 > max {
            return Err("A pet file is larger than a pet should be, so it was not read.".into());
        }
        Ok(Some(bytes))
    }

    fn contained(root: &Path, path: &Path) -> Result<PathBuf, String> {
        let outside = || "A pet file is outside its folder, so it was not read.".to_owned();
        let real = fs::canonicalize(path).map_err(|_| outside())?;
        let real_root = fs::canonicalize(root).map_err(|_| outside())?;
        if real != real_root && real.starts_with(&real_root) {
            Ok(real)
        } else {
            Err(outside())
        }
    }

    /// The first bytes of a sheet, for its size, by the same rules as a whole read.
    fn read_head(root: &Path, path: &Path) -> Result<Vec<u8>, String> {
        let real = Self::contained(root, path)?;
        let mut file = fs::File::open(real).map_err(|_| "A pet file could not be read.".to_owned())?;
        let mut head = vec![0u8; WEBP_HEADER_BYTES];
        let mut got = 0;
        while got < head.len() {
            match file.read(&mut head[got..]) {
                Ok(0) => break,
                Ok(n) => got += n,
                Err(_) => break,
            }
        }
        head.truncate(got);
        Ok(head)
    }

    /// One pet's folder: its pet.json and its sheet's size, checked. Err says why not.
    fn read_pet(&self, source: PetSource, id: &str) -> Result<ReadPet, String> {
        let not_here = || NOT_ON_THIS_COMPUTER.to_owned();
        let root = self.root_of(source).ok_or_else(not_here)?;
        if !is_pet_id(id) || !Self::usable_root(root) {
            return Err(not_here());
        }
        let folder = root.join(id);
        let info = fs::symlink_metadata(&folder).map_err(|_| not_here())?;
        if !info.is_dir() {
            return Err(not_here());
        }
        let meta_bytes = Self::read_inside(root, &folder.join("pet.json"), PET_JSON_MAX)?.ok_or("That pet has no pet.json.")?;
        let parsed: Value = serde_json::from_slice(&meta_bytes).map_err(|_| "That pet’s pet.json does not read.".to_owned())?;
        let meta = parsed_pet_meta(&parsed, id)?;
        let sheet_path = folder.join("spritesheet.webp");
        let sheet_info = fs::symlink_metadata(&sheet_path).map_err(|_| "That pet has no sheet.".to_owned())?;
        if !sheet_info.is_file() || sheet_info.len() > PET_SHEET_MAX {
            return Err("That pet’s sheet is not a plain file of a pet’s size.".into());
        }
        let head = Self::read_head(root, &sheet_path)?;
        let size = webp_size(&head, Some(sheet_info.len())).ok_or("That pet’s sheet is not a WebP image.")?;
        let rows = pet_rows_for(size, meta.version)?;
        Ok(ReadPet {
            pet: PublicPet { source, id: id.to_owned(), display_name: meta.display_name, description: meta.description, rows },
            version: meta.version,
        })
    }

    fn read_pets(&self, source: PetSource, cap: usize) -> Vec<PublicPet> {
        let Some(root) = self.root_of(source) else { return Vec::new() };
        if !Self::usable_root(root) {
            return Vec::new();
        }
        let Ok(dir) = fs::read_dir(root) else { return Vec::new() };
        let mut names: Vec<String> = dir
            .flatten()
            .filter(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| is_pet_id(name))
            .collect();
        names.sort();
        names.truncate(cap);
        names.iter().filter_map(|id| self.read_pet(source, id).ok().map(|read| read.pet)).collect()
    }

    /// Locust's own, then the gallery's added ones, then Codex's.
    pub fn list(&self) -> Vec<PublicPet> {
        let mut pets = self.read_pets(PetSource::Bundled, 10);
        pets.extend(self.read_pets(PetSource::Gallery, 1000));
        pets.extend(self.read_pets(PetSource::Codex, CODEX_PETS_MAX));
        pets
    }

    /// A pet's sheet, checked, to draw: its bytes and rows.
    pub fn sheet(&self, source: PetSource, id: &str) -> Result<(Vec<u8>, u8), String> {
        let read = self.read_pet(source, id)?;
        let root = self.root_of(source).ok_or_else(|| NOT_ON_THIS_COMPUTER.to_owned())?;
        let bytes = Self::read_inside(root, &root.join(id).join("spritesheet.webp"), PET_SHEET_MAX)?.ok_or("That pet has no sheet.")?;
        // The whole file, checked again: it is what is drawn.
        let still_reads = webp_size(&bytes, Some(bytes.len() as u64)).is_some_and(|size| pet_rows_for(size, read.version).is_ok());
        if !still_reads {
            return Err("That pet’s sheet changed and no longer reads.".into());
        }
        Ok((bytes, read.pet.rows))
    }

    /// The picks taken out of the picker, oldest first. A file that does not read is no removals, never an error.
    pub fn removed_picks(&self) -> Vec<String> {
        let Ok(text) = fs::read_to_string(&self.removed_file) else { return Vec::new() };
        let Ok(value) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
        let mut ids: Vec<String> = Vec::new();
        for id in value.get("ids").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
            if let Some(id) = id.as_str()
                && is_pet_id(id)
                && is_pet_pick(id)
                && !ids.iter().any(|kept| kept == id)
            {
                ids.push(id.to_owned());
            }
        }
        ids
    }

    /// One write at a time, each from the last one's result: two quick removals both stay.
    fn change_removed(&self, change: impl FnOnce(Vec<String>) -> Vec<String>) -> Result<(), String> {
        let _one_at_a_time = self.removed_lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let ids = change(self.removed_picks());
        let mut incoming = self.removed_file.clone().into_os_string();
        incoming.push(format!(".{}.tmp", std::process::id()));
        let body = serde_json::to_string_pretty(&serde_json::json!({ "schemaVersion": 1, "ids": ids })).map_err(|e| e.to_string())?;
        fs::write(&incoming, format!("{body}\n")).map_err(|e| e.to_string())?;
        fs::rename(&incoming, &self.removed_file).map_err(|e| e.to_string())
    }

    /// Picked again: back in the picker.
    pub fn bring_back(&self, id: &str) -> Result<(), String> {
        if self.removed_picks().iter().any(|removed| removed == id) {
            self.change_removed(|ids| ids.into_iter().filter(|kept| kept != id).collect())?;
        }
        Ok(())
    }

    /// Takes a pick out of the picker, and its download with it unless `keep_files` (a pet a teammate wears keeps its
    /// files, so their face stays).
    pub fn remove(&self, id: &str, keep_files: bool) -> Result<(), String> {
        if !is_pet_id(id) {
            return Err(NOT_ON_THIS_COMPUTER.into());
        }
        if is_pet_pick(id) {
            self.change_removed(|mut ids| {
                if !ids.iter().any(|kept| kept == id) {
                    ids.push(id.to_owned());
                }
                ids
            })?;
        }
        if keep_files {
            return Ok(());
        }
        let folder = self.installed_root.join(id);
        let Ok(info) = fs::symlink_metadata(&folder) else { return Ok(()) };
        if !info.is_dir() {
            return Err(NOT_ON_THIS_COMPUTER.into());
        }
        fs::remove_dir_all(&folder).map_err(|e| e.to_string())
    }
}
