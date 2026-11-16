//! The Pets row of the look picker: Electron's `PetPicks.tsx` (`PetPickTiles`, `PetCredit`, 0.564). The 21 pets Locust
//! offers as tiles under the bots, each its small picture; a pick not on this computer downloads from openpets.dev on its
//! click and goes on the teammate. A pet the teammate wears that is not offered (a bundled or Codex one) keeps a tile of
//! its own, shown as chosen. The library and the downloads are `locust_core::pet_gallery`'s, run off the window thread.
//!
//! Pets added here are kept in the Rust app's own data folder (`<native>/pets`), never Electron's: the Rust app reads
//! Electron's data and writes none of it until the switch.

use super::*;
use locust_core::bot_spec::{AvatarSpec, PetRef, PetSource};
use locust_core::look_picker as rules;
use locust_core::pet_gallery::{PetGallery, UreqFetch};
use locust_core::pet_library::{PetLibrary, PublicPet, PET_PICKS};
use std::collections::HashMap;
use std::sync::Arc;

/// Where the pets come from (`OPENPETS_LINK`) and where to report one (`OPENPETS_REPORT_LINK`).
pub(crate) const OPENPETS_LINK: &str = "https://openpets.dev";
pub(crate) const OPENPETS_REPORT_LINK: &str = "mailto:admin@openpets.dev?subject=A%20pet%20on%20openpets.dev"; // public-guard: allow (openpets.dev's own published report address, as Electron's outbound-links.ts)

/// A tile's small picture: asked for, drawn, or not to be had (the tile keeps its placeholder).
pub(crate) enum Picture {
    Asked,
    Drawn(rune::Image),
    None,
}

type Thumbs = Vec<(String, Option<(u32, u32, Vec<u8>)>)>;

#[derive(Default)]
pub(crate) struct PetPicks {
    pub(crate) pictures: HashMap<String, Picture>,
    /// The pets on this computer, once read: which picks need a download.
    pub(crate) installed: Option<Vec<PublicPet>>,
    /// The pick being added (one at a time, as Electron's `adding`).
    pub(crate) adding: Option<String>,
    /// What went wrong, in words.
    pub(crate) notice: Option<String>,
    /// The picks taken out of the picker (0.569, `pets-removed.json`).
    pub(crate) removed: Vec<String>,
    /// The ticked picks while choosing which to remove; None when not choosing (`usePetRemoval`).
    pub(crate) ticked: Option<Vec<String>>,
    pub(crate) removing: bool,
    pub(crate) show_removed: bool,
    /// What the last removal did: the words, and whether some could not be removed (amber).
    pub(crate) report: Option<(String, bool)>,
    gallery: Option<Arc<PetGallery>>,
    pub(crate) thumbs_job: rune::Latest<Thumbs>,
    pub(crate) list_job: rune::Latest<(Vec<PublicPet>, Vec<String>)>,
    pub(crate) add_job: rune::Latest<(String, Result<PublicPet, String>)>,
    pub(crate) remove_job: rune::Latest<Removal>,
}

/// What a removal did, pick by pick: removed, removed with its files kept for its wearers, or not removed.
#[derive(Default)]
pub(crate) struct Removal {
    pub(crate) removed: Vec<String>,
    pub(crate) kept: Vec<(String, Vec<String>)>,
    pub(crate) failed: Vec<String>,
}

/// `listed`: "a", "a and b", "a, b and c".
pub(crate) fn listed(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// The words a removal leaves (`confirm` in usePetRemoval), and whether they are amber: some could not be removed.
pub(crate) fn removal_report(done: &Removal) -> (String, bool) {
    let name = |id: &String| pick_name(id).to_owned();
    let mut said: Vec<String> = Vec::new();
    let removed = done.removed.len();
    if removed > 0 {
        said.push(format!("Removed {removed} {} from the picker; Show removed brings any back.", if removed == 1 { "pet" } else { "pets" }));
    }
    for (id, wearers) in &done.kept {
        said.push(format!("{} stays on {} until {} another look.", name(id), listed(wearers), if wearers.len() == 1 { "they get" } else { "each gets" }));
    }
    if !done.failed.is_empty() {
        let failed: Vec<String> = done.failed.iter().map(name).collect();
        said.push(format!("{} could not be removed and {} still in the picker.", listed(&failed), if failed.len() == 1 { "is" } else { "are" }));
    }
    (said.join(" "), !done.failed.is_empty())
}

/// The library and gallery for this data folder: added pets and the gallery cache in the Rust app's own folder, Codex's
/// pets read where they are.
pub(crate) fn library_for(store: &locust_core::store::Store) -> PetLibrary {
    let root = store.root();
    let codex = std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".codex").join("pets"));
    PetLibrary::new(root.join("pets"), root.join("pets-bundled"), codex, None)
}

fn gallery_for(store: &locust_core::store::Store) -> PetGallery {
    let root = store.root();
    let library = library_for(store);
    let now = || std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    PetGallery::new(library, root.join("pets-cache"), Box::new(UreqFetch), Box::new(now))
}

/// A WebP picture as RGBA, for `ImageCache::rgba`. None when it does not decode.
pub(crate) fn decode_webp(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let picture = image::load_from_memory_with_format(bytes, image::ImageFormat::WebP).ok()?;
    let rgba = picture.to_rgba8();
    Some((rgba.width(), rgba.height(), rgba.into_raw()))
}

/// Starts reading the list and the pictures the first time the row is drawn with a data folder. Nothing is downloaded
/// but the small pictures of the picks (Electron reads them as the row comes into view).
pub(crate) fn wake(s: &mut State) {
    if s.pet_picks.gallery.is_some() {
        return;
    }
    let Some(store) = s.data.as_ref() else { return };
    let gallery = Arc::new(gallery_for(store));
    s.pet_picks.gallery = Some(gallery.clone());
    for (id, _, _) in PET_PICKS {
        s.pet_picks.pictures.insert(id.to_owned(), Picture::Asked);
    }
    let list = gallery.clone();
    s.pet_picks.list_job.request((), move |()| (list.library.list(), list.library.removed_picks()));
    s.pet_picks.thumbs_job.request((), move |()| {
        PET_PICKS
            .iter()
            .map(|(id, _, _)| (id.to_string(), gallery.thumbnail_bytes(id).ok().and_then(|bytes| decode_webp(&bytes))))
            .collect()
    });
}

/// Takes what landed. True when the row should be drawn again.
pub(crate) fn poll(s: &mut State) -> bool {
    if s.people_panel.form_open {
        wake(s);
    }
    let mut changed = false;
    if let Some((list, removed)) = s.pet_picks.list_job.take() {
        s.pet_picks.installed = Some(list);
        s.pet_picks.removed = removed;
        changed = true;
    }
    if let Some(done) = s.pet_picks.remove_job.take() {
        s.pet_picks.removing = false;
        s.pet_picks.ticked = None;
        s.pet_picks.report = Some(removal_report(&done));
        if let Some(gallery) = s.pet_picks.gallery.clone() {
            s.pet_picks.list_job.request((), move |()| (gallery.library.list(), gallery.library.removed_picks()));
        }
        changed = true;
    }
    if let Some(thumbs) = s.pet_picks.thumbs_job.take() {
        for (id, picture) in thumbs {
            let drawn = match (picture, s.image_cache.as_ref()) {
                (Some((w, h, rgba)), Some(cache)) => Picture::Drawn(cache.rgba(w, h, rgba)),
                _ => Picture::None,
            };
            s.pet_picks.pictures.insert(id, drawn);
        }
        changed = true;
    }
    if let Some((id, answer)) = s.pet_picks.add_job.take() {
        s.pet_picks.adding = None;
        match answer {
            Ok(pet) => {
                wear(s, &pet.id, pet.source);
                if let Some(gallery) = s.pet_picks.gallery.clone() {
                    s.pet_picks.list_job.request((), move |()| (gallery.library.list(), gallery.library.removed_picks()));
                }
            }
            Err(message) => {
                let name = pick_name(&id);
                s.pet_picks.notice = Some(if message.is_empty() {
                    format!("{name} could not be added. Nothing of it was kept on this computer.")
                } else {
                    message
                });
            }
        }
        changed = true;
    }
    changed
}

/// Opens one of the two links above in the person's browser or mail app; never from a test.
fn open_link(url: &'static str) {
    #[cfg(not(test))]
    {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open").arg(url).spawn();
        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd").args(["/c", "start", "", url]).spawn();
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    #[cfg(test)]
    let _ = url;
}

fn pick_name(id: &str) -> &str {
    PET_PICKS.iter().find(|(pick, _, _)| *pick == id).map_or(id, |(_, name, _)| name)
}

/// Puts the pet on the teammate being edited, beside the bot it falls back to (`onWear`).
fn wear(s: &mut State, id: &str, source: PetSource) {
    let current = crate::look_picker::avatar_of(&s.people_panel.entry);
    let next = AvatarSpec { pet: Some(PetRef { source, id: id.to_owned(), screen: None }), ..current };
    s.people_panel.entry["avatar"] = rules::avatar_json(&next);
}

/// A pick, picked: worn at once when it is on this computer, else added first (`wear` in PetPickTiles).
fn pick(s: &mut State, id: &'static str) {
    s.pet_picks.report = None;
    let gone = s.pet_picks.removed.iter().any(|r| r == id);
    let here = !gone && s
        .pet_picks
        .installed
        .as_ref()
        .is_some_and(|pets| pets.iter().any(|p| p.source == PetSource::Gallery && p.id == id));
    if here {
        s.pet_picks.notice = None;
        wear(s, id, PetSource::Gallery);
        return;
    }
    let Some(gallery) = s.pet_picks.gallery.clone() else { return };
    if s.pet_picks.adding.is_some() {
        return;
    }
    s.pet_picks.adding = Some(id.to_owned());
    s.pet_picks.notice = None;
    s.pet_picks.add_job.request((), move |()| (id.to_owned(), gallery.add(id)));
}

/// Remove some...: the tiles turn into ticks.
pub(crate) fn start_removing(s: &mut State) {
    s.pet_picks.notice = None;
    s.pet_picks.report = None;
    s.pet_picks.show_removed = false;
    s.pet_picks.ticked = Some(Vec::new());
}

pub(crate) fn toggle_tick(s: &mut State, id: &str) {
    if let Some(ticked) = s.pet_picks.ticked.as_mut() {
        if let Some(at) = ticked.iter().position(|t| t == id) {
            ticked.remove(at);
        } else {
            ticked.push(id.to_owned());
        }
    }
}

/// The teammates wearing this pick, by name: its files stay so their face does.
pub(crate) fn wearers(s: &State, id: &str) -> Vec<String> {
    s.people_panel.team["teammates"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["avatar"]["pet"]["source"] == "gallery" && t["avatar"]["pet"]["id"] == id)
        .filter_map(|t| t["name"].as_str().map(str::to_owned))
        .collect()
}

/// Remove N: each ticked pick out of the picker, one at a time, and off this computer unless a teammate wears it.
pub(crate) fn confirm_removing(s: &mut State) {
    let Some(ticked) = s.pet_picks.ticked.clone() else { return };
    if ticked.is_empty() || s.pet_picks.removing {
        return;
    }
    let Some(gallery) = s.pet_picks.gallery.clone() else { return };
    let plan: Vec<(String, Vec<String>)> = ticked.iter().map(|id| (id.clone(), wearers(s, id))).collect();
    s.pet_picks.removing = true;
    s.pet_picks.remove_job.request((), move |()| {
        let mut done = Removal::default();
        for (id, wearing) in plan {
            match gallery.library.remove(&id, !wearing.is_empty()) {
                Ok(()) if wearing.is_empty() => done.removed.push(id),
                Ok(()) => {
                    done.removed.push(id.clone());
                    done.kept.push((id, wearing));
                }
                Err(_) => done.failed.push(id),
            }
        }
        done
    });
}

/// The words a tile says when hovered (`title` in PetPickTiles).
pub(crate) fn tile_help(name: &str, here: bool) -> String {
    if here { name.to_owned() } else { format!("{name}: downloads from openpets.dev when picked") }
}

fn picture(s: &State, id: &str, l: &crate::look::controls::Look) -> El {
    match s.pet_picks.pictures.get(id) {
        Some(Picture::Drawn(image)) => el().w(36.0).h(36.0).pixels_fit(image.pixels(), rune::SurfaceFit::Fit),
        _ => row().w(36.0).h(36.0).radius(8.0).fill(l.c("bg-raised")),
    }
}

fn tile(l: &crate::look::controls::Look, inner: El, key: String, label: &str, chosen: bool, busy: bool) -> El {
    let tile = row()
        .w(44.0)
        .h(44.0)
        .center()
        .radius(l.t.px("radius-control"))
        .border(1.0, if chosen { l.c("lime-border") } else { l.c("border-card") })
        .child(inner)
        .key(key)
        .role("radio")
        .label(label.to_owned())
        .checked(chosen)
        .focusable(true)
        .focus_ring(2.0, 0.0, l.c("focus-ring"));
    let tile = if chosen { tile.fill(l.c("lime-tint")) } else { tile.hover(l.c("hover-surface")) };
    if busy { tile.opacity(0.6) } else { tile }
}

/// "Remove some..." or, while choosing, "Cancel" and "Remove N" (`PetRemovalControls`), beside the Pets heading.
fn removal_controls(s: &State, l: &crate::look::controls::Look) -> El {
    let ghost = crate::look::controls::ghost_button;
    let Some(ticked) = &s.pet_picks.ticked else {
        return ghost(l, "Remove some\u{2026}").key("team-pets-remove-some").on_press(start_removing);
    };
    let count = ticked.len();
    let label = if s.pet_picks.removing {
        "Removing\u{2026}".to_owned()
    } else if count == 0 {
        "Tick the pets to remove".to_owned()
    } else {
        format!("Remove {count}")
    };
    let mut cancel = ghost(l, "Cancel").key("team-pets-remove-cancel");
    if !s.pet_picks.removing {
        cancel = cancel.on_press(|s: &mut State| s.pet_picks.ticked = None);
    }
    let mut remove = ghost(l, &label).key("team-pets-remove-confirm");
    remove = if count == 0 || s.pet_picks.removing { remove.opacity(0.45).disabled(true) } else { remove.on_press(confirm_removing) };
    row().gap(4.0).child(cancel).child(remove)
}

/// The Pets heading, the tiles and the credit line, under the bots.
pub(crate) fn row_of_pets(s: &State, avatar: &AvatarSpec, l: &crate::look::controls::Look) -> El {
    let worn = avatar.pet.as_ref();
    let picks = &s.pet_picks;
    let mut grid = row().key("team-pets").role("radiogroup").label("Pets").w_full().gap(6.0).flex_wrap();
    // A pet the teammate wears that is not offered keeps its tile, shown as chosen.
    if picks.ticked.is_none()
        && let Some(pet) = worn.filter(|p| !(p.source == PetSource::Gallery && PET_PICKS.iter().any(|(id, _, _)| *id == p.id)))
    {
        grid = grid.child(tile(l, picture(s, &pet.id, l), format!("team-pet-{}", pet.id), &pet.id, true, false));
    }
    for (id, name, _) in PET_PICKS {
        let gone = picks.removed.iter().any(|r| r == id);
        // Removed picks are left out, unless the person asked to see them (and is not choosing more to remove).
        if gone && !(picks.show_removed && picks.ticked.is_none()) {
            continue;
        }
        if let Some(ticked) = &picks.ticked {
            let on = ticked.iter().any(|t| t == id);
            let mut t = row()
                .w(44.0)
                .h(44.0)
                .center()
                .radius(l.t.px("radius-control"))
                .border(1.0, if on { l.c("amber") } else { l.c("border-card") })
                .child(picture(s, id, l))
                .key(format!("team-pet-{id}"))
                .role("checkbox")
                .label(format!("Remove {name}"))
                .checked(on)
                .focusable(true)
                .focus_ring(2.0, 0.0, l.c("focus-ring"))
                .help(if on { format!("{name} will be removed") } else { format!("Tick {name} to remove it") });
            if !picks.removing {
                t = t.on_press(move |s: &mut State| toggle_tick(s, id));
            }
            grid = grid.child(t);
            continue;
        }
        let chosen = worn.is_some_and(|p| p.source == PetSource::Gallery && p.id == id);
        let here = picks.installed.as_ref().is_some_and(|pets| pets.iter().any(|p| p.source == PetSource::Gallery && p.id == id));
        let busy = picks.adding.as_deref() == Some(id);
        let said = if gone { format!("{name}: removed; pick it to bring it back") } else { tile_help(name, here) };
        let mut t = tile(l, picture(s, id, l), format!("team-pet-{id}"), name, chosen, busy).help(said.clone());
        if said != name {
            t = t.description(said);
        }
        if gone {
            t = t.opacity(0.55);
        }
        if picks.adding.is_none() {
            t = t.on_press(move |s: &mut State| pick(s, id));
        }
        grid = grid.child(t);
    }
    let muted = |words: String| text(words).font(type_::LABEL).color(l.c("text-muted")).wrap();
    let credit = row()
        .key("team-pets-credit")
        .gap(4.0)
        .flex_wrap()
        .child(muted("Pets from".to_owned()))
        .child(crate::look::controls::ghost_button(l, "openpets.dev").key("team-pets-openpets").on_press(|_: &mut State| open_link(OPENPETS_LINK)))
        .child(muted(", made by its community; each downloads the first time it is picked. Rights stay with each pet\u{2019}s maker.".to_owned()))
        .child(crate::look::controls::ghost_button(l, "Report a pet").key("team-pets-report").on_press(|_: &mut State| open_link(OPENPETS_REPORT_LINK)));
    let head = row()
        .key("team-pets-head")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text("Pets".to_owned()).font(type_::LABEL).color(l.c("text-secondary")).grow(1.0))
        .child(removal_controls(s, l));
    let mut out = col().key("team-pets-section").w_full().gap(8.0).pt(4.0).child(head).child(grid);
    if let Some(notice) = &picks.notice {
        out = out.child(text(notice.clone()).key("team-pets-notice").font(type_::LABEL).color(l.c("amber")).wrap());
    }
    if let Some((words, warn)) = &picks.report {
        out = out.child(text(words.clone()).key("team-pets-report-line").font(type_::LABEL).color(if *warn { l.c("amber") } else { l.c("text-muted") }).wrap());
    }
    if picks.ticked.is_none() && !picks.removed.is_empty() {
        out = out.child(
            row()
                .key("team-pets-removed")
                .gap(4.0)
                .center_y()
                .child(muted(format!("{} removed. ", picks.removed.len())))
                .child(
                    crate::look::controls::ghost_button(l, if picks.show_removed { "Hide removed" } else { "Show removed" })
                        .key("team-pets-show-removed")
                        .on_press(|s: &mut State| s.pet_picks.show_removed = !s.pet_picks.show_removed),
                ),
        );
    }
    out.child(credit)
}

#[cfg(test)]
#[path = "pet_picks_tests.rs"]
pub(crate) mod tests;
