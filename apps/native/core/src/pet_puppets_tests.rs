//! the-kept-pets-are-puppets.test.tsx: the cases that are pure (the kept list, the cut, the screens, the rig's
//! transform, the sheet check, each pet's motion), one test each, with Electron's values. `toBeCloseTo` is
//! |difference| < 0.005 (its default two digits).

use super::*;
use crate::bot_rig::rest_pose;
use crate::bot_rig::BotAvatarState;
use crate::buddy_moves::{screen_fits, CODEX_BUDDY};
use crate::pets::PET_COLUMNS;

const KEPT: [&str; 8] = ["cabin-face", "astro-bot", "meowbot", "macintosh", "bitty", "tmuxai", "rainbow-terminal-cat", "nori"];
const CUT: [&str; 3] = ["astro-bot", "meowbot", "tmuxai"];
const STANDING: [&str; 4] = ["macintosh", "bitty", "rainbow-terminal-cat", "nori"];

fn close(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() < 0.005, "{what}: {a} is not close to {b}");
}

fn puppet(id: &str) -> &'static PetPuppet {
    puppet_for(id).unwrap_or_else(|| panic!("no puppet {id}"))
}

/// The resting drawing's painted left, top, right and bottom, as measured.
fn rest_edges(p: &PetPuppet) -> [f64; 4] {
    p.edges[p.rest.0][p.rest.1].map(f64::from)
}

/// The rig's pose at rest (TS `pose()`), changed by `f`.
fn pose(f: impl FnOnce(&mut BotAvatarPose)) -> BotAvatarPose {
    let mut p = rest_pose(BotAvatarState::Default);
    f(&mut p);
    p
}

/// Where a point of the box (its units) lands on the canvas, in device pixels.
fn place(m: [f64; 6], x: f64, y: f64) -> (f64, f64) {
    (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
}

pub(crate) struct Move {
    pub row: usize,
    pub column: usize,
    pub edge: usize,
    pub by: i64,
}

/// A sheet painted where the measured one is, drawing by drawing: each cell's four edges, one of them moved.
pub(crate) fn sheet_like(p: &PetPuppet, moved: Option<Move>) -> (Vec<u8>, usize, usize) {
    sheet_of(p.frame_width as usize, p.frame_height as usize, p.edges, moved)
}

pub(crate) fn sheet_of(fw: usize, fh: usize, edges: &[&[[u32; 4]]], moved: Option<Move>) -> (Vec<u8>, usize, usize) {
    let width = fw * PET_COLUMNS as usize;
    let height = fh * 9;
    let mut data = vec![0u8; width * height * 4];
    for (r, row) in edges.iter().enumerate() {
        for (c, cell) in row.iter().enumerate() {
            let mut b = cell.map(i64::from);
            if let Some(m) = &moved
                && m.row == r
                && m.column == c
            {
                b[m.edge] += m.by;
            }
            for y in (r as i64 * fh as i64 + b[1])..(r as i64 * fh as i64 + b[3]) {
                for x in (c as i64 * fw as i64 + b[0])..(c as i64 * fw as i64 + b[2]) {
                    data[(y as usize * width + x as usize) * 4 + 3] = 255;
                }
            }
        }
    }
    (data, width, height)
}

// ---- the pets Colin kept ----

#[test]
fn are_the_eight_he_named_each_once_codex_buddy_keeps_his_own_drawings() {
    let mut ids: Vec<&str> = PET_PUPPETS.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    let mut kept = KEPT.to_vec();
    kept.sort_unstable();
    assert_eq!(ids, kept);
    assert!(puppet_for("codex-buddy").is_none());
    assert!(puppet_for("hoodie-cat").is_none());
}

// ---- nothing of a head is cut ----

#[test]
fn keeps_all_of_each_drawing_above_its_neck_from_its_very_top_no_ear_antenna_or_headphone_is_lost() {
    for p in &PET_PUPPETS {
        let [left, top, right, bottom] = rest_edges(p);
        let paint = p.paint.map(f64::from);
        assert_eq!(paint[1], top, "{}", p.id);
        assert!(paint[0] >= left, "{}", p.id);
        assert!(paint[2] <= right, "{}", p.id);
        // Kept whole, all of it.
        if p.neck.is_none() {
            assert_eq!(paint, [left, top, right, bottom], "{}", p.id);
        }
    }
}

#[test]
fn cuts_only_a_body_that_walks_and_a_stand_straight_across_its_neck_well_below_its_screen() {
    let cut: Vec<&str> = PET_PUPPETS.iter().filter(|p| p.neck.is_some()).map(|p| p.id).collect();
    assert_eq!(cut, CUT);
    for id in CUT {
        let p = puppet(id);
        let neck = f64::from(p.neck.unwrap_or(0));
        let [_, _, _, bottom] = rest_edges(p);
        // Something is cut: its body is below its neck.
        assert!(neck < bottom - 30.0, "{id}");
        assert!(f64::from(p.paint[3]) <= neck, "{id}");
        let [_, glass_y, _, glass_h] = p.glass.map(f64::from);
        assert!(neck - (glass_y + glass_h) > 10.0, "{id}");
    }
}

#[test]
fn shows_in_its_box_all_of_what_is_kept_with_a_little_air_centred_on_it() {
    for p in &PET_PUPPETS {
        let [left, top, side] = p.window;
        let [pl, pt, pr, pb] = p.paint.map(f64::from);
        assert!(left <= pl, "{}", p.id);
        assert!(top <= pt, "{}", p.id);
        assert!(left + side >= pr, "{}", p.id);
        assert!(top + side >= pb, "{}", p.id);
        assert!((left + side / 2.0 - (pl + pr) / 2.0).abs() < 2.0, "{}", p.id);
        assert!((top + side / 2.0 - (pt + pb) / 2.0).abs() < 2.0, "{}", p.id);
        assert!(side / (pr - pl).max(pb - pt) < 1.1, "{}", p.id);
    }
}

// ---- their screens ----

#[test]
fn each_screen_sits_on_its_face_inside_what_is_drawn_of_it_above_its_neck() {
    for p in &PET_PUPPETS {
        let [x, y, w, h] = p.glass;
        assert!(x >= p.paint[0], "{}", p.id);
        assert!(x + w <= p.paint[2], "{}", p.id);
        assert!(y >= p.paint[1], "{}", p.id);
        assert!(y + h <= p.paint[3], "{}", p.id);
    }
}

#[test]
fn each_has_room_for_a_bots_two_eyes_in_its_box() {
    for p in &PET_PUPPETS {
        let glass = puppet_glass(p);
        assert!(glass.w >= 28.0, "{}: {}", p.id, glass.w);
        assert!(glass.h >= 14.0, "{}: {}", p.id, glass.h);
    }
}

#[test]
fn cut_from_its_body_a_heads_screen_is_a_quarter_as_big_again_as_its_pets_fitted_whole() {
    for id in CUT {
        let p = puppet(id);
        let w = f64::from(p.glass[2]);
        assert!(puppet_glass(p).w / ((w * 100.0) / f64::from(p.frame_height)) > 1.2, "{id}");
    }
}

// ---- drawn on a bot's rig ----

#[test]
fn draws_its_resting_drawing_as_much_as_its_window_shows_and_nothing_below_its_neck() {
    for p in &PET_PUPPETS {
        let (fw, fh) = (f64::from(p.frame_width), f64::from(p.frame_height));
        let (from, _) = puppet_drawing(p, fw, fh);
        let cell_x = p.rest.1 as f64 * fw;
        let cell_y = p.rest.0 as f64 * fh;
        let paint = p.paint.map(f64::from);
        assert!(from.x >= cell_x, "{}", p.id);
        assert!(from.x + from.w <= cell_x + fw, "{}", p.id);
        assert!(from.y >= cell_y, "{}", p.id);
        assert!(from.y + from.h <= cell_y + p.neck.map_or(fh, f64::from), "{}", p.id);
        // All of what is kept.
        assert!(from.x <= cell_x + paint[0], "{}", p.id);
        assert!(from.x + from.w >= cell_x + paint[2], "{}", p.id);
        assert!(from.y <= cell_y + paint[1], "{}", p.id);
        assert!(from.y + from.h >= cell_y + paint[3], "{}", p.id);
    }
}

#[test]
fn lays_its_window_on_the_bots_body_box_a_hundred_units_across() {
    for p in &PET_PUPPETS {
        let [left, top, side] = p.window;
        let r = puppet_rect(p, left, top, side, side);
        close(r.x, -50.0, p.id);
        close(r.y, -50.0, p.id);
        close(r.w, 100.0, p.id);
        close(r.h, 100.0, p.id);
    }
}

#[test]
fn stands_where_a_bot_does_its_box_centred_on_the_canvas_and_raised_as_a_bots() {
    let (size, dpr) = (44.0, 2.0);
    let side = size * BOT_AVATAR_OVERSCAN * dpr;
    let rest = puppet_transform(pose(|_| {}), size, dpr, None);
    assert_eq!(place(rest, 0.0, 0.0), (side / 2.0, side / 2.0 + BOT_AVATAR_RISE * size * dpr));
    let (left, top) = place(rest, -50.0, -50.0);
    let (right, bottom) = place(rest, 50.0, 50.0);
    close(right - left, size * dpr, "width");
    close(bottom - top, size * dpr, "height");
}

#[test]
fn lands_on_its_foot_squashed_half_as_far_as_a_screen_faced_bot() {
    let size = 44.0;
    let rest = puppet_transform(pose(|_| {}), size, 1.0, None);
    let landing = pose(|p| {
        p.sx = 1.2;
        p.sy = 0.8;
    });
    let squashed = puppet_transform(puppet_pose(landing, HEAD_MOTION), size, 1.0, None);
    // Its foot stays where it was; the height it loses comes off its top.
    close(place(squashed, 0.0, 50.0).1, place(rest, 0.0, 50.0).1, "foot");
    let height = place(squashed, 0.0, 50.0).1 - place(squashed, 0.0, -50.0).1;
    let width = place(squashed, 50.0, 0.0).0 - place(squashed, -50.0, 0.0).0;
    close(height / size, 1.0 - 0.2 * PUPPET_SQUASH, "height");
    close(width / size, 1.0 + 0.2 * PUPPET_SQUASH, "width");
    let most = 0.5;
    assert!(PUPPET_SQUASH <= most);
}

#[test]
fn leans_with_the_rig_and_moves_a_little_the_way_it_turns_and_nods() {
    let size = 44.0;
    let rest = puppet_transform(pose(|_| {}), size, 1.0, None);
    let leaning = puppet_transform(pose(|p| p.roll = 0.2), size, 1.0, None);
    close(leaning[1], 0.2f64.sin() * (size / 100.0), "lean");
    let turned = place(puppet_transform(pose(|p| p.yaw = 0.5), size, 1.0, None), 0.0, 0.0).0 - place(rest, 0.0, 0.0).0;
    assert!(turned > 0.0, "{turned}");
    assert!(turned < 0.05 * size, "{turned}");
    let lifted = place(puppet_transform(pose(|p| p.pitch = 0.12), size, 1.0, None), 0.0, 0.0).1 - place(rest, 0.0, 0.0).1;
    assert!(lifted < 0.0, "{lifted}");
}

#[test]
fn puts_its_ring_and_dot_on_what_is_drawn_of_it_not_on_a_body_it_no_longer_has() {
    for p in &PET_PUPPETS {
        let body = puppet_body(p);
        assert!(body.left < body.right, "{}", p.id);
        assert!(body.top < body.bottom, "{}", p.id);
        for edge in [body.left, body.top, body.right, body.bottom] {
            assert!((0.0..=1.0).contains(&edge), "{}: {edge}", p.id);
        }
    }
    // Astro Bot's ends at its helmet's rim; its feet were below the bottom of its box.
    let astro = puppet("astro-bot");
    let [_, top, side] = astro.window;
    close(puppet_body(astro).bottom, (136.0 - top) / side, "astro");
    assert!(puppet_body(astro).bottom < 0.92);
}

// ---- its sheet is checked before it becomes a puppet ----

#[test]
fn takes_the_sheet_measured_by_each_drawings_four_edges() {
    for p in &PET_PUPPETS {
        let (data, w, h) = sheet_like(p, None);
        assert!(print_fits(p, &data, w, h), "{}", p.id);
    }
}

#[test]
fn takes_it_re_compressed_an_edge_a_pixel_or_two_off() {
    let p = puppet("bitty");
    let (data, w, h) = sheet_like(p, Some(Move { row: 4, column: 1, edge: 1, by: i64::from(EDGE_SLACK) }));
    assert!(print_fits(p, &data, w, h));
}

#[test]
fn refuses_one_redrawn_taller_or_shorter_though_its_sides_are_where_they_were() {
    // Some makers fill every drawing edge to edge: a drawing's sides alone do not tell one sheet from another.
    let p = puppet("nori");
    for edge in [1, 3] {
        let (data, w, h) = sheet_like(p, Some(Move { row: 6, column: 2, edge, by: i64::from(EDGE_SLACK) + 2 }));
        assert!(!print_fits(p, &data, w, h), "edge {edge}");
    }
}

// ---- its sheet, as the window reads it (`measured_puppet`, which `pet_faces` calls on each sheet it reads) ----

#[test]
fn makes_it_a_puppet_when_it_is_the_sheet_measured_and_not_when_it_is_another() {
    let bitty = puppet("bitty");
    let (data, w, h) = sheet_like(bitty, None);
    assert!(measured_puppet("bitty", &data, w, h).is_some_and(|p| std::ptr::eq(p, bitty)));
    let (moved, w2, h2) = sheet_like(bitty, Some(Move { row: 2, column: 3, edge: 0, by: i64::from(EDGE_SLACK) + 3 }));
    assert!(measured_puppet("bitty", &moved, w2, h2).is_none());
    // Another pet drawn the same is still not Bitty.
    assert!(measured_puppet("hoodie-cat", &data, w, h).is_none());
}

#[test]
fn leaves_codex_buddy_his_screen_and_makes_no_puppet_of_him() {
    // Codex Buddy's sheet as his table has it: his two edges, his maker's rows 5 to 203.
    let rows: Vec<Vec<[u32; 4]>> =
        CODEX_BUDDY.edges.iter().map(|row| row.iter().map(|[l, r]| [*l, 5, *r, 203]).collect()).collect();
    let refs: Vec<&[[u32; 4]]> = rows.iter().map(Vec::as_slice).collect();
    let (data, w, h) = sheet_of(192, 208, &refs, None);
    assert!(screen_fits(&CODEX_BUDDY, &data, w, h));
    assert!(measured_puppet("codex-buddy", &data, w, h).is_none());
}

// ---- a teammate wearing one: the rule under it (its cases are src/look/puppets_tests.rs) ----

#[test]
fn wears_a_puppet_only_measured_with_terminal_faces_on_and_not_asked_to_be_as_drawn() {
    let bitty = |screen| PetRef { source: crate::bot_spec::PetSource::Gallery, id: "bitty".into(), screen };
    assert!(wears_puppet(&bitty(None), true, true));
    assert!(!wears_puppet(&bitty(None), true, false), "Terminal faces off");
    assert!(!wears_puppet(&bitty(Some(false)), true, true), "as drawn");
    assert!(wears_puppet(&bitty(Some(true)), true, true), "screen");
    assert!(!wears_puppet(&bitty(None), false, true), "a sheet not measured");
}

// ---- each pet moves as what it is ----

fn feet_of(p: &PetPuppet) -> f64 {
    puppet_rect(p, 0.0, f64::from(p.paint[3]), 0.0, 0.0).y
}

#[test]
fn gives_every_kept_pet_a_style_heads_floating_and_the_rest_standing_on_the_ground() {
    let mut styled: Vec<&str> = PUPPET_MOTION.iter().map(|(id, _)| *id).collect();
    styled.sort_unstable();
    let mut kept = KEPT.to_vec();
    kept.sort_unstable();
    assert_eq!(styled, kept);
    for p in &PET_PUPPETS {
        assert_eq!(motion_of(p).grounded, STANDING.contains(&p.id), "{}", p.id);
        assert_eq!(puppet_feet(p).is_some(), STANDING.contains(&p.id), "{}", p.id);
    }
    for id in ["astro-bot", "meowbot"] {
        assert_eq!(motion_of(puppet(id)), HEAD_MOTION, "{id}");
    }
}

#[test]
fn never_takes_more_of_the_rig_than_a_bot_would_no_style_rises_squashes_or_leans_further() {
    for (id, m) in PUPPET_MOTION {
        assert!(m.lift <= 1.0, "{id}");
        assert!(m.squash <= 1.0, "{id}");
        assert!(m.lean <= 1.0, "{id}");
    }
}

#[test]
fn keeps_one_standing_on_the_ground_never_below_it_and_the_cat_never_off_it() {
    for id in STANDING {
        let m = motion_of(puppet(id));
        close(puppet_pose(pose(|p| p.y = 3.0), m).y, 0.0, id);
        close(puppet_pose(pose(|p| p.y = -28.0), m).y, -28.0 * m.lift, id);
    }
    close(puppet_pose(pose(|p| p.y = -28.0), motion_of(puppet("rainbow-terminal-cat"))).y, 0.0, "cat");
    // Hops stay small for the machines on their feet, and smaller for the sushi.
    for id in ["macintosh", "bitty", "nori"] {
        assert!(motion_of(puppet(id)).lift <= 0.5, "{id}");
    }
}

#[test]
fn rocks_one_standing_on_its_feet_and_squashes_it_about_them_they_stay_put() {
    for id in ["macintosh", "bitty", "nori"] {
        let p = puppet(id);
        let feet = feet_of(p);
        let m = motion_of(p);
        let at = |rig: BotAvatarPose| place(puppet_transform(puppet_pose(rig, m), 44.0, 1.0, Some(feet)), 0.0, feet);
        let rest = at(pose(|_| {}));
        let rocked = at(pose(|p| p.roll = 0.2));
        let landed = at(pose(|p| {
            p.sx = 1.25;
            p.sy = 0.75;
        }));
        close(rocked.0, rest.0, id);
        close(rocked.1, rest.1, id);
        close(landed.1, rest.1, id);
    }
}

#[test]
fn keeps_a_hard_thing_hard_the_phone_never_squashes_the_machines_hardly_do_the_sushi_does() {
    let landing = pose(|p| {
        p.sx = 1.25;
        p.sy = 0.75;
    });
    let cabin = puppet_pose(landing, motion_of(puppet("cabin-face")));
    assert_eq!([cabin.sx, cabin.sy], [1.0, 1.0]);
    for id in ["macintosh", "bitty"] {
        assert!(puppet_pose(landing, motion_of(puppet(id))).sy > 0.97, "{id}");
    }
    assert!(puppet_pose(landing, motion_of(puppet("nori"))).sy < 0.8);
}

#[test]
fn gives_a_soft_body_a_wobble_and_a_lying_one_a_breath_and_a_head_neither() {
    let breathing = pose(|p| p.breath = 1.0);
    let nori = puppet_pose(breathing, motion_of(puppet("nori")));
    assert!(nori.sx > 1.0);
    assert!(nori.sy < 1.0);
    assert!(puppet_pose(breathing, motion_of(puppet("rainbow-terminal-cat"))).sy > 1.0);
    let head = puppet_pose(breathing, HEAD_MOTION);
    assert_eq!([head.sx, head.sy], [1.0, 1.0]);
    assert_eq!(PUPPET_SQUASH, HEAD_MOTION.squash);
}

// ---- every screen switches on again as a bot's does; Codex Buddy, on his own rig ----
// The same file's three Codex Buddy cases, through `buddy_moves`'s public conductor.

mod buddy {
    use crate::bot_rig::crt_rule;
    use crate::bot_spec::Phosphor;
    use crate::buddy_moves::{resting_move, workout_move, Move, PetScreenAsk, PetScreenConductor, CODEX_BUDDY};

    fn ask(eyes: Option<[&str; 2]>, key: &str, r#move: Move, rests: bool) -> PetScreenAsk {
        PetScreenAsk {
            eyes: eyes.map(|e| e.map(str::to_owned)),
            phosphor: Phosphor::Cyan,
            flash: None,
            key: key.to_owned(),
            r#move,
            rests,
            crt: None,
        }
    }

    #[test]
    fn codex_buddys_for_the_changes_worth_looking_up_for_and_only_when_asked() {
        let run = |crt: bool| -> (Option<f64>, Option<f64>) {
            let rule = |from: Option<&str>, to: &str, count: u32| crt_rule(0.37, from, Some(to), count);
            let rule: Option<&dyn Fn(Option<&str>, &str, u32) -> Option<crate::bot_rig::CrtKind>> = if crt { Some(&rule) } else { None };
            let idle = ask(None, "idle", resting_move(), false);
            let done = ask(Some(["^", "^"]), "done", resting_move(), false);
            let mut conductor = PetScreenConductor::new(&CODEX_BUDDY, 0.37, &idle);
            let at_idle = conductor.frame(5.0, &idle, None, rule).booting;
            conductor.frame(5.05, &done, None, rule);
            (at_idle, conductor.frame(5.3, &done, None, rule).booting)
        };
        assert!(run(true).0.is_none(), "idle does not switch on");
        assert!(run(true).1.is_some(), "a finish does");
        assert!(run(false).1.is_none(), "only when asked");
    }

    #[test]
    fn moves_a_little_every_frame_while_he_works_never_held_and_never_snapped() {
        let working = ask(Some([">", "\u{25ae}"]), "working", workout_move(), false);
        let mut conductor = PetScreenConductor::new(&CODEX_BUDDY, 0.37, &working);
        let mut last = conductor.frame(10.0, &working, None, None).rig;
        let (mut held, mut biggest) = (0, 0.0f64);
        let mut at = 10.0 + 1.0 / 30.0;
        while at < 16.0 {
            let now = conductor.frame(at, &working, None, None).rig;
            let moved = [(now.pose.left, last.pose.left), (now.pose.right, last.pose.right)]
                .iter()
                .flat_map(|(n, l)| [(n.abduct - l.abduct).abs(), (n.flex - l.flex).abs(), (n.bend - l.bend).abs()])
                .fold(0.0f64, f64::max);
            if moved < 1e-6 {
                held += 1;
            }
            biggest = biggest.max(moved);
            last = now;
            at += 1.0 / 30.0;
        }
        // Between sets he stands a breath; otherwise every frame is a new pose, none of them a jump.
        assert!(held < 60, "{held}");
        assert!(biggest < 0.25, "{biggest}");
    }

    #[test]
    fn rests_only_once_his_last_pose_has_settled_so_his_clock_still_stops() {
        let moving = ask(None, "idle", resting_move(), false);
        let resting = ask(None, "idle", resting_move(), true);
        let mut conductor = PetScreenConductor::new(&CODEX_BUDDY, 0.37, &moving);
        conductor.frame(3.0, &moving, None, None);
        let mut settled_at = None;
        let mut t = 3.0;
        while t < 8.0 && settled_at.is_none() {
            if conductor.frame(t, &resting, None, None).settled {
                settled_at = Some(t);
            }
            t += 1.0 / 30.0;
        }
        assert!(settled_at.is_some());
    }
}
