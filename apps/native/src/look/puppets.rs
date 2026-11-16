//! A teammate wearing one of the pets Colin kept, as a puppet on a bot's rig (Electron `components/PetPuppet.tsx`).
//! The rules and numbers are `locust_core::pet_puppets`'s; this is the drawing: the pet's resting drawing (cut once
//! from its sheet, off the window thread, in `pet_faces`), moved each frame by a bot's rig (`bot_stage`'s
//! `snapshot_puppet`) through its own style (`puppet_pose`), wearing a screen with a bot's eyes on its face
//! (`bots::draw_puppet_shape`), its ring and dot on what is drawn of it (`bots::puppet_marks_shape`).
//!
//! Electron draws all of it into one canvas under one transform. A Rune `Shape` draws no image, so the drawing is an
//! element of its own laid where the transform puts it: sized by the squash, placed by the pose, turned by the lean
//! about the box's point the transform turns about. The glass and eyes are points, mapped through the same matrix.

use super::bots::{draw_puppet_shape, puppet_marks_shape, PuppetDrawSpec};
use super::faces::BotFaceOpts;
use locust_core::bot_rig::{BotAvatarPose, FaceActivity, BOT_AVATAR_OVERSCAN, BOT_AVATAR_RISE};
use locust_core::buddy_moves::ScreenRectF;
use locust_core::pet_puppets::{motion_of, puppet_body, puppet_drawing, puppet_feet, puppet_pose, puppet_rect, puppet_transform, PetPuppet};
use rune::{canvas, el, El, Tx};
use std::time::Duration;

/// The resting drawing cut from its sheet: where in the sheet (x, y, w, h, whole pixels), and its pixels.
pub(crate) fn rest_crop(puppet: &PetPuppet) -> [u32; 4] {
    let (fw, fh) = (f64::from(puppet.frame_width), f64::from(puppet.frame_height));
    let (from, _) = puppet_drawing(puppet, fw, fh);
    // Whole pixels, never past the drawing's cell or below its neck: what `puppet_drawing` takes, rounded outward.
    let (row, column) = puppet.rest;
    let cell_x = column as f64 * fw;
    let cell_y = row as f64 * fh;
    let x0 = from.x.floor().max(cell_x);
    let y0 = from.y.floor().max(cell_y);
    let x1 = (from.x + from.w).ceil().min(cell_x + fw);
    let y1 = (from.y + from.h).ceil().min(cell_y + puppet.neck.map_or(fh, f64::from));
    [x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32]
}

/// The crop's pixels out of the whole sheet's (`width` across).
pub(crate) fn cut_rest(puppet: &PetPuppet, width: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let [x, y, w, h] = rest_crop(puppet);
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = ((row * width + x) * 4) as usize;
        out.extend_from_slice(&rgba[start..start + (w * 4) as usize]);
    }
    (w, h, out)
}

/// Where the crop sits in the box's units.
pub(crate) fn crop_in_box(puppet: &PetPuppet) -> ScreenRectF {
    let [x, y, w, h] = rest_crop(puppet);
    let (row, column) = puppet.rest;
    let cell_x = column as u32 * puppet.frame_width;
    let cell_y = row as u32 * puppet.frame_height;
    puppet_rect(puppet, f64::from(x - cell_x), f64::from(y - cell_y), f64::from(w), f64::from(h))
}

/// `puppet_transform` in the face's own points: its canvas is `BOT_AVATAR_OVERSCAN` times the face and raised
/// `BOT_AVATAR_RISE` (Electron's negative margins), so the box's centre lands on the face's.
pub fn face_matrix(pose: BotAvatarPose, size: f64, feet: Option<f64>) -> [f64; 6] {
    let mut m = puppet_transform(pose, size, 1.0, feet);
    let pull = (BOT_AVATAR_OVERSCAN - 1.0) / 2.0 * size;
    m[4] -= pull;
    m[5] -= pull + BOT_AVATAR_RISE * size;
    m
}

/// Where the drawing's element goes for a matrix and the pose it came from: its top-left point and size before it
/// turns, the point it turns about (a share of its own size), and how far it turns. The transform is a turn of a
/// squash (no skew), so the element sized by the squash and turned about the matrix's own origin is the same.
pub fn drawing_place(m: [f64; 6], pose: BotAvatarPose, to: ScreenRectF) -> ([f32; 4], [f32; 2], f32) {
    let kx = m[0].hypot(m[1]);
    let ky = m[2].hypot(m[3]);
    let rect = [(m[4] + to.x * kx) as f32, (m[5] + to.y * ky) as f32, (to.w * kx) as f32, (to.h * ky) as f32];
    let origin = [(-to.x / to.w) as f32, (-to.y / to.h) as f32];
    (rect, origin, pose.roll as f32)
}

/// The puppet's face at `size`: its drawing, its screen, its marks, alive on the rig.
#[allow(clippy::too_many_arguments)]
pub(crate) fn puppet_face(
    s: &crate::State,
    slot_key: &str,
    id: &str,
    puppet: &'static PetPuppet,
    image: &rune::Image,
    size: f32,
    opts: &BotFaceOpts<'_>,
    light_theme: bool,
) -> El {
    let key = format!("{slot_key}#puppet");
    let snap = s.bot_stage.snapshot_puppet(
        &key,
        id,
        opts.activity,
        opts.glance,
        opts.motion,
        s.settings_panel.settings.plush,
        s.reduced_motion.get(),
    );
    let pose = puppet_pose(snap.pose, motion_of(puppet));
    let m = face_matrix(pose, f64::from(size), puppet_feet(puppet));
    let ([x, y, w, h], [ox, oy], turn) = drawing_place(m, pose, crop_in_box(puppet));
    let screen = draw_puppet_shape(&PuppetDrawSpec { puppet, pose, matrix: m, size, snap: &snap });
    let marks = puppet_marks_shape(size, puppet_body(puppet), opts.activity == FaceActivity::Waiting, opts.presence, light_theme);
    let drawing = el().size(w, h).absolute(x, y).pixels(image.pixels()).origin(ox, oy).rotate(turn);
    let hover_key = key.clone();
    let pointer_key = key.clone();
    let face = el()
        .key(format!("puppet-{slot_key}"))
        .size(size, size)
        // Each frame is the rig's pose: nothing springs toward it.
        .instant_changes()
        .child(drawing)
        .child(canvas(screen).size(size, size).absolute(0.0, 0.0))
        .child(canvas(marks).size(size, size).absolute(0.0, 0.0))
        .on_hover_change(move |st: &mut crate::State, over: bool, tx: &mut Tx| {
            st.bot_stage.set_hover(&hover_key, over);
            if !st.reduced_motion.get() {
                tx.wake_after(Duration::from_millis(33));
            }
        })
        .on_pointer_move_local(move |st: &mut crate::State, at: rune::PointerEvent, tx: &mut Tx| {
            st.bot_stage.point_at(&pointer_key, f64::from(at.x), f64::from(at.y), f64::from(size));
            if !st.reduced_motion.get() {
                tx.wake_after(Duration::from_millis(33));
            }
        });
    match opts.a11y_label {
        Some(label) => face.label(label.to_owned()),
        None => face.a11y_hidden(),
    }
}

#[cfg(test)]
#[path = "puppets_tests.rs"]
mod tests;
