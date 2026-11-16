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
    gallery: Option<Arc<PetGallery>>,
    pub(crate) thumbs_job: rune::Latest<Thumbs>,
    pub(crate) list_job: rune::Latest<Vec<PublicPet>>,
    pub(crate) add_job: rune::Latest<(String, Result<PublicPet, String>)>,
}

/// The library and gallery for this data folder: added pets and the gallery cache in the Rust app's own folder, Codex's
/// pets read where they are.
fn gallery_for(store: &locust_core::store::Store) -> PetGallery {
    let root = store.root();
    let codex = std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".codex").join("pets"));
    let library = PetLibrary::new(root.join("pets"), root.join("pets-bundled"), codex, None);
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
    s.pet_picks.list_job.request((), move |()| list.library.list());
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
    if let Some(list) = s.pet_picks.list_job.take() {
        s.pet_picks.installed = Some(list);
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
                    s.pet_picks.list_job.request((), move |()| gallery.library.list());
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
    let here = s
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

/// The Pets heading, the tiles and the credit line, under the bots.
pub(crate) fn row_of_pets(s: &State, avatar: &AvatarSpec, l: &crate::look::controls::Look) -> El {
    let worn = avatar.pet.as_ref();
    let mut grid = row().key("team-pets").role("radiogroup").label("Pets").w_full().gap(6.0).flex_wrap();
    // A pet the teammate wears that is not offered keeps its tile, shown as chosen.
    if let Some(pet) = worn.filter(|p| !(p.source == PetSource::Gallery && PET_PICKS.iter().any(|(id, _, _)| *id == p.id))) {
        grid = grid.child(tile(l, picture(s, &pet.id, l), format!("team-pet-{}", pet.id), &pet.id, true, false));
    }
    for (id, name, _) in PET_PICKS {
        let chosen = worn.is_some_and(|p| p.source == PetSource::Gallery && p.id == id);
        let here = s
            .pet_picks
            .installed
            .as_ref()
            .is_some_and(|pets| pets.iter().any(|p| p.source == PetSource::Gallery && p.id == id));
        let busy = s.pet_picks.adding.as_deref() == Some(id);
        let said = tile_help(name, here);
        let mut t = tile(l, picture(s, id, l), format!("team-pet-{id}"), name, chosen, busy).help(said.clone());
        if said != name {
            t = t.description(said);
        }
        if s.pet_picks.adding.is_none() {
            t = t.on_press(move |s: &mut State| pick(s, id));
        }
        grid = grid.child(t);
    }
    let credit = row()
        .key("team-pets-credit")
        .gap(4.0)
        .flex_wrap()
        .child(text("Pets from".to_owned()).font(type_::LABEL).color(l.c("text-muted")))
        .child(
            crate::look::controls::ghost_button(l, "openpets.dev")
                .key("team-pets-openpets")
                .on_press(|_: &mut State| open_link(OPENPETS_LINK)),
        )
        .child(
            text(", made by its community; each downloads the first time it is picked. Rights stay with each pet\u{2019}s maker.".to_owned())
                .font(type_::LABEL)
                .color(l.c("text-muted"))
                .wrap(),
        )
        .child(
            crate::look::controls::ghost_button(l, "Report a pet")
                .key("team-pets-report")
                .on_press(|_: &mut State| open_link(OPENPETS_REPORT_LINK)),
        );
    let mut out = col()
        .key("team-pets-section")
        .w_full()
        .gap(8.0)
        .pt(4.0)
        .child(text("Pets".to_owned()).font(type_::LABEL).color(l.c("text-secondary")))
        .child(grid)
        .child(credit);
    if let Some(notice) = &s.pet_picks.notice {
        out = out.child(text(notice.clone()).key("team-pets-notice").font(type_::LABEL).color(l.c("amber")).wrap());
    }
    out
}

#[cfg(test)]
#[path = "pet_picks_tests.rs"]
pub(crate) mod tests;
