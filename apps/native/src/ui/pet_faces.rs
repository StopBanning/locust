//! A teammate wearing a pet: the pet drawn where the bot was (Electron's `TeammateBot` with a pet, `PetSprite.tsx`), its
//! row played for what the teammate is doing (`locust_core::pet_motion`), with a bot's marks kept: the waiting ring and
//! the presence dot. A pet that cannot be drawn shows its bot, never an empty box (`a-teammate-can-be-a-pet.test.tsx`).
//! Codex Buddy keeps its own drawing (`buddy_moves`), so it never comes here.
//!
//! Each worn pet's sheet is read and cut into its cells once, off the window thread; the cells become images.

use super::*;
use locust_core::bot_rig::FaceActivity;
use locust_core::bot_spec::{PetRef, PetSource};
use locust_core::pet_motion::{pet_cell_at, pet_next_change_in, pet_state_for};
use locust_core::pets::PetState;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};

/// A sheet cut into cells: (row, column) to its pixels, and how many rows it has.
type Cells = (Vec<((u32, u32), u32, u32, Vec<u8>)>, u8);

pub(crate) enum Sheet {
    Asked,
    Ready { cells: HashMap<(u32, u32), rune::Image>, rows: u8 },
    Missing,
}

pub(crate) struct PetFaces {
    pub(crate) sheets: RefCell<HashMap<String, Sheet>>,
    /// Pets a face wanted while no sheet was asked for: read on the next poll.
    wanted: RefCell<Vec<PetRef>>,
    /// When each face's current state began, so a row that plays then settles starts from its first cell.
    clocks: RefCell<HashMap<String, (PetState, f64)>>,
    /// The soonest a drawn pet changes cell, in seconds: the poll wakes the window for it.
    next_change: Cell<Option<f64>>,
    tx: Sender<(String, Option<Cells>)>,
    rx: Receiver<(String, Option<Cells>)>,
    pub(crate) waker: RefCell<Option<rune::Waker>>,
}

impl Default for PetFaces {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            sheets: RefCell::default(),
            wanted: RefCell::default(),
            clocks: RefCell::default(),
            next_change: Cell::new(None),
            tx,
            rx,
            waker: RefCell::new(None),
        }
    }
}

fn key_of(pet: &PetRef) -> String {
    let source = match pet.source {
        PetSource::Gallery => "gallery",
        PetSource::Codex => "codex",
        PetSource::Bundled => "bundled",
    };
    format!("{source}/{}", pet.id)
}

fn now() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

/// A sheet's pixels cut into cells, eight across and `rows` down.
pub(crate) fn cut_sheet(width: u32, height: u32, rgba: &[u8], rows: u8) -> Vec<((u32, u32), u32, u32, Vec<u8>)> {
    let (cell_w, cell_h) = (width / 8, height / u32::from(rows));
    let mut out = Vec::new();
    for row in 0..u32::from(rows) {
        for col in 0..8 {
            let mut cell = Vec::with_capacity((cell_w * cell_h * 4) as usize);
            for y in 0..cell_h {
                let start = (((row * cell_h + y) * width + col * cell_w) * 4) as usize;
                cell.extend_from_slice(&rgba[start..start + (cell_w * 4) as usize]);
            }
            out.push(((row, col), cell_w, cell_h, cell));
        }
    }
    out
}

/// The pet's face at `size`, or None while its sheet is not ready (or cannot be drawn): the caller draws the bot.
#[allow(clippy::too_many_arguments)]
pub(crate) fn pet_face(
    s: &State,
    slot_key: &str,
    pet: &PetRef,
    activity: FaceActivity,
    size: f32,
    presence: Option<crate::look::bot_stage::PresenceDot>,
    light_theme: bool,
) -> Option<El> {
    let faces = &s.pet_faces;
    let key = key_of(pet);
    let sheets = faces.sheets.borrow();
    let Some(sheet) = sheets.get(&key) else {
        drop(sheets);
        let mut wanted = faces.wanted.borrow_mut();
        if !wanted.iter().any(|w| key_of(w) == key) {
            wanted.push(pet.clone());
        }
        return None;
    };
    let Sheet::Ready { cells, rows } = sheet else { return None };
    let state = pet_state_for(activity);
    let still = s.reduced_motion.get();
    let at = now();
    let started = {
        let mut clocks = faces.clocks.borrow_mut();
        let entry = clocks.entry(slot_key.to_owned()).or_insert((state, at));
        if entry.0 != state {
            *entry = (state, at);
        }
        entry.1
    };
    let elapsed = (at - started) * 1000.0;
    let cell = pet_cell_at(state, elapsed, *rows, still);
    if let Some(ms) = pet_next_change_in(state, elapsed, still) {
        let seconds = ms / 1000.0;
        faces.next_change.set(Some(faces.next_change.get().map_or(seconds, |n| n.min(seconds))));
    }
    let image = cells.get(&(cell.row, cell.column)).or_else(|| cells.get(&(0, 0)))?;
    let marks = crate::look::bots::pet_marks_shape(size, activity == FaceActivity::Waiting, presence, light_theme);
    Some(
        el()
            .key(format!("pet-{slot_key}"))
            .size(size, size)
            .child(el().size(size, size).pixels_fit(image.pixels(), rune::SurfaceFit::Fit))
            .child(rune::canvas(marks).size(size, size).absolute(0.0, 0.0))
            .a11y_hidden(),
    )
}

/// Reads the sheets faces asked for, takes the cut ones in, and wakes the window when a drawn pet changes cell.
pub(crate) fn poll(s: &mut State, tx: &mut Tx) -> bool {
    let mut changed = false;
    let wanted: Vec<PetRef> = s.pet_faces.wanted.borrow_mut().drain(..).collect();
    if !wanted.is_empty()
        && let Some(store) = s.data.as_ref()
    {
        for pet in wanted {
            let key = key_of(&pet);
            s.pet_faces.sheets.borrow_mut().insert(key.clone(), Sheet::Asked);
            let library = crate::pet_picks::library_for(store);
            let send = s.pet_faces.tx.clone();
            let waker = s.pet_faces.waker.borrow().clone();
            std::thread::spawn(move || {
                let cut = library.sheet(pet.source, &pet.id).ok().and_then(|(bytes, rows)| {
                    let (w, h, rgba) = crate::pet_picks::decode_webp(&bytes)?;
                    Some((cut_sheet(w, h, &rgba, rows), rows))
                });
                let _ = send.send((key, cut));
                if let Some(wake) = waker {
                    wake();
                }
            });
        }
    }
    while let Ok((key, cut)) = s.pet_faces.rx.try_recv() {
        let sheet = match (cut, s.image_cache.as_ref()) {
            (Some((cells, rows)), Some(cache)) => Sheet::Ready {
                cells: cells.into_iter().map(|(at, w, h, rgba)| (at, cache.rgba(w, h, rgba))).collect(),
                rows,
            },
            _ => Sheet::Missing,
        };
        s.pet_faces.sheets.borrow_mut().insert(key, sheet);
        changed = true;
    }
    if let Some(seconds) = s.pet_faces.next_change.take() {
        tx.wake_after(Duration::from_secs_f64(seconds.max(0.016)));
    }
    changed
}

#[cfg(test)]
#[path = "pet_faces_tests.rs"]
pub(crate) mod tests;
