use super::*;

fn buddy(size: f32, look: f64, eyes: [&str; 2], seconds: f64) -> String {
    let snap = BuddySnapshot {
        pose: locust_core::buddy_rig::REST_POSE,
        look,
        eyes: eyes.map(str::to_owned),
        tone: Phosphor::Cyan,
        flash: None,
        flash_progress: 1.0,
        power_on: None,
        shut: 0.0,
        glass: None,
        screen: true,
        plush: false,
        s_seconds: seconds,
        hovered: false,
    };
    format!(
        "{:?}",
        draw_buddy_shape(&BuddyDrawSpec {
            hue: "pearl",
            size,
            light_theme: false,
            presence: None,
            ring: false,
            snap: &snap,
        })
    )
}

// Rune intentionally keeps Shape operations private. Read the same ordered fill and stroke
// recording other look tests inspect, not a second implementation of the painter.
fn ops(drawing: &str) -> Vec<(&str, &str)> {
    drawing
        .split(" { points: ")
        .skip(1)
        .map(|part| {
            let (points, fields) = part.split_once("]], ").expect("Rune shape points");
            (points, fields)
        })
        .collect()
}

fn pts(encoded: &str) -> impl Iterator<Item = [f32; 2]> + '_ {
    encoded.split("], [").map(|pair| {
        let (x, y) = pair.trim_start_matches('[').split_once(", ").unwrap();
        [x.parse().unwrap(), y.parse().unwrap()]
    })
}

fn colour(color: Color) -> String {
    format!("color: {color:?}")
}

fn glass(ops: &[(&str, &str)]) -> usize {
    let ground = colour(rgb8(12, 16, 22));
    ops.iter()
        .position(|(_, fields)| fields.starts_with(&ground))
        .expect("drawn Buddy glass")
}

fn lamps(ops: &[(&str, &str)]) -> Vec<usize> {
    let lit = colour(color_from_rgb_f64(
        phosphor_rgb(Phosphor::Cyan).map(f64::from),
        0.96,
    ));
    ops.iter()
        .enumerate()
        .filter_map(|(i, (_, fields))| fields.contains(&lit).then_some(i))
        .collect()
}

// case: draws his eyes as a bot screen’s, sized to the glass in the units a bot’s screen has
#[test]
fn draws_buddy_eyes_in_bot_screen_units() {
    let drawing = buddy(96.0, 0.0, ["|", "|"], 0.0);
    let ops = ops(&drawing);
    let panel: Vec<_> = pts(ops[glass(&ops)].0).collect();
    let (left, right) = panel
        .iter()
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(a, b), p| {
            (a.min(p[0]), b.max(p[0]))
        });
    let mid = (left + right) / 2.0;
    let bars = lamps(&ops);
    assert_eq!(bars.len(), 2);
    let l: Vec<_> = pts(ops[bars[0]].0).collect();
    let r: Vec<_> = pts(ops[bars[1]].0).collect();
    assert!(l.iter().all(|p| p[0] < mid) && r.iter().all(|p| p[0] > mid));
    assert!(((l[1][1] - l[0][1]).abs() - (r[1][1] - r[0][1]).abs()).abs() < 0.001);
    let face = screen_face_for(CODEX_BUDDY.id).unwrap();
    let w = f64::from(face.cells[BUDDY_RIG_PLATE_ROW][BUDDY_RIG_PLATE_COL][2]);
    assert!((glass_units(w) * w - 54.0).abs() < 1e-6);
    let height = (l[1][1] - l[0][1]).abs();
    let scale =
        f64::from(draw_size_for(96.0)) / pet_window(PET_FRAME_HEIGHT, BUDDY_RIG_MIDDLE).side;
    assert!((f64::from(height) - 9.8 * 54.0 / w * scale).abs() < 0.001);
}

// case: puts his ink round the glass first, then the glass inside it
#[test]
fn puts_buddy_ink_around_the_glass_first() {
    let drawing = buddy(96.0, 0.0, ["|", "|"], 0.0);
    let ops = ops(&drawing);
    let glass = glass(&ops);
    let (rim_path, rim) = ops[glass - 1];
    assert!(
        rim.starts_with("closed: true, width: ")
            && rim.contains(&colour(color_from_css(BUDDY_COLOR_INK, 1.0)))
    );
    // A centred stroke around the same polygon extends out by half its nonzero width.
    assert_eq!(rim_path, ops[glass].0);
    assert!(
        rim.split("width: ")
            .nth(1)
            .unwrap()
            .split(',')
            .next()
            .unwrap()
            .parse::<f32>()
            .unwrap()
            > 0.0
    );
}
