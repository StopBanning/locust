use super::*;

// case: gives the density shapes the 64 asset
#[test]
fn density_shapes_keep_their_64_drawings() {
    // thinking-orbs 0.3.1, MODE_FRAMES at its still time. Each asset has its own point count,
    // with an edge mesh only on the connection drawing.
    for (state, shape, speed, dots, lines, first_x) in [
        ("composing", Shape::Ribbon, 2.34, 566, 0, 37.322389),
        ("listening", Shape::Wave, 4.388, 134, 0, 29.764214),
        ("solving", Shape::Rubik, 1.82, 138, 0, 32.999536),
        ("searching", Shape::Globe, 2.015, 204, 0, 33.490622),
        ("connecting", Shape::Web, 3.315, 48, 81, 23.996950),
        ("weaving", Shape::Braid, 1.625, 153, 0, 34.742259),
    ] {
        let preset = preset(state, Surface::Live).unwrap();
        assert_eq!(
            (preset.shape, preset.size, preset.speed),
            (shape, 64, speed),
            "{state}"
        );
        let frame = preset.frame(0.6);
        assert_eq!(
            (frame.dots.len(), frame.lines.len()),
            (dots, lines),
            "{state}"
        );
        assert!((frame.dots[0].center[0] - first_x).abs() < 1e-6, "{state}");
    }
}

// case: leaves the outline shapes on their own 20px design
#[test]
fn outlines_use_the_fattened_20_drawings() {
    let ring = preset("breathing", Surface::Live).unwrap();
    let morph = preset("shaping", Surface::Live).unwrap();
    assert_eq!((ring.shape, ring.size, ring.speed), (Shape::Ring, 20, 3.78));
    assert_eq!(
        (morph.shape, morph.size, morph.speed),
        (Shape::Morph, 20, 2.08)
    );
    let ring = ring.frame(0.6);
    assert_eq!(ring.dots.len(), 120);
    assert!(
        ring.dots
            .iter()
            .all(|d| (d.center[0] - 10.0).hypot(d.center[1] - 10.0) > 5.0)
    );
    let square = morph.frame(5.0);
    assert_eq!(square.dots.len(), 18);
    for dot in square.dots {
        assert!((dot.radius - 0.83119365).abs() < 1e-7);
        let [x, y, _] = dot.center;
        assert!(((x - 10.0).abs().max((y - 10.0).abs()) - 5.91).abs() < 0.1);
    }
}

// case: paints the 64 design down with a filter that looks at every pixel
#[test]
fn downsamples_coverage_from_every_one_of_the_sixteen_samples() {
    assert_eq!(PAINT_DOWN_FACTOR, 4);
    for sample in 0..16 {
        let mut raster = Raster {
            side: 4,
            pixels: vec![Sample::default(); 16],
        };
        raster.pixels[sample].over(0.8, 1.0);
        assert_eq!(raster.downsample(1, PAINT_DOWN_FACTOR), [204, 204, 204, 16]);
    }
    // One fully covered sample and one translucent sample of a different shade.
    let mut raster = Raster {
        side: 4,
        pixels: vec![Sample::default(); 16],
    };
    raster.pixels[1].over(1.0, 1.0);
    raster.pixels[15].over(0.2, 0.5);
    assert_eq!(raster.downsample(1, PAINT_DOWN_FACTOR), [187, 187, 187, 24]);
}

fn drawn(state: &'static str, surface: Surface) -> rune::ElementInfo {
    let mut rt = rune::Runtime::new(
        (crate::State::default(), state, surface),
        |(s, state, surface), _| {
            rune::col().child(view("orb".into(), state, *surface, s, true).unwrap())
        },
        (100.0, 100.0),
        rune::Env::new(true, 2.0),
    );
    rt.frame(0.0);
    rt.elements()
        .into_iter()
        .find(|e| e.trace_name == Some("thinking-orb"))
        .unwrap()
}

// case: never scales a raster to get a bigger orb
#[test]
fn the_outline_raster_stays_at_twenty_in_the_larger_box() {
    for state in ["breathing", "shaping"] {
        let image = drawn(state, Surface::Live);
        assert_eq!(&image.rect[2..], &[20.0, 20.0], "{state}");
        assert_eq!(image.scale, 1.0, "{state}");
        assert_eq!(&image.rect[..2], &[3.0, 3.0], "{state}");
    }
    let image = drawn("composing", Surface::Live);
    assert_eq!(&image.rect[2..], &[26.0, 26.0]);
    assert_eq!(image.scale, 1.0);
}

// case: gives the live line a box big enough for the asset
#[test]
fn the_live_surface_reserves_twenty_six_points() {
    // The live line will use this surface once messages.rs can call it.
    assert_eq!(Surface::Live.width(), 26);
    let image = drawn("solving", Surface::Live);
    assert_eq!(&image.rect[2..], &[26.0, 26.0]);
}

#[test]
fn motion_follows_the_frame_beat_and_reduced_motion_uses_the_still_pose() {
    let preset = preset("listening", Surface::PlanStep).unwrap();
    assert_eq!(
        (preset.shape, preset.size, preset.speed),
        (Shape::Wave, 20, 3.998)
    );
    assert_eq!(preset.frame(0.6).dots.len(), 42);
    assert_eq!(sample_time(&preset, None), 0.6);
    assert_eq!(sample_time(&preset, Some(30)), 3.998);
    let first = paint(&preset.frame(sample_time(&preset, Some(0))), 40, 2, true);
    let later = paint(&preset.frame(sample_time(&preset, Some(15))), 40, 2, true);
    assert_ne!(first, later);
    assert!(first.chunks_exact(4).any(|px| px[3] > 0));
}
