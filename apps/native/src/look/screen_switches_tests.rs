//! Drawn screen cases from a-screen-switches-on-and-its-dots-blink.test.ts.
use super::*;
use locust_core::bot_rig::{FLASH_S, flash_fade, power_on_light};
use rune::{Cx, Env, Runtime, canvas};
use std::cell::RefCell;

#[derive(Clone)]
struct Screen {
    seconds: f64,
    boot: Option<f64>,
    flash: Option<(Phosphor, f64)>,
}
thread_local! { static DRAWN: RefCell<String> = const { RefCell::new(String::new()) }; }
fn view(s: &Screen, _: &mut Cx<Screen>) -> rune::El {
    let boot = s
        .boot
        .filter(|at| s.seconds < at + locust_core::bot_rig::POWER_ON_S)
        .map(|at| power_on_light((s.seconds - at) / locust_core::bot_rig::POWER_ON_S));
    let shape = draw_screen_light(
        Shape::new(),
        &[[-30.0, -20.0], [30.0, -20.0], [30.0, 20.0], [-30.0, 20.0]],
        Phosphor::Cyan,
        s.flash,
        s.flash
            .map(|(_, at)| flash_fade(s.seconds - at))
            .unwrap_or(1.0),
        boot,
        &["o".into(), "o".into()],
        [-11.5, 0.0],
        [11.5, 0.0],
        1.0,
        1.0,
        1.0,
        &|p| [p[0] as f32, p[1] as f32],
        1.0,
        s.seconds,
        0.0,
    );
    DRAWN.with(|d| *d.borrow_mut() = format!("{shape:?}"));
    canvas(shape).w(100.0).h(100.0)
}
fn drawn(seconds: f64, boot: Option<f64>, flash: Option<(Phosphor, f64)>) -> String {
    let mut rt = Runtime::new(
        Screen {
            seconds,
            boot,
            flash,
        },
        view,
        (200.0, 200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.016);
    assert!(!rt.elements().is_empty());
    DRAWN.with(|d| d.borrow().clone())
}
fn eye_colour(rgb: [f64; 3]) -> String {
    format!("{:?}", color_from_rgb_f64(rgb, 0.96))
}

// case: is dark while it switches on: the light first, the eyes only once it has settled
#[test]
fn is_dark_while_it_switches_on_the_light_first_the_eyes_only_once_it_has_settled() {
    let line = drawn(10.1, Some(10.0), None);
    assert!(
        line.contains("Stroke") && !line.contains("Fill") && !line.contains("closed: true"),
        "{line}"
    );
    let flash = drawn(10.45, Some(10.0), None);
    assert!(
        flash.contains("Fill") && !flash.contains("closed: true"),
        "{flash}"
    );
    let eyes = drawn(10.81, Some(10.0), None);
    assert!(
        eyes.contains("closed: true") && !eyes.contains("Fill") && !eyes.contains("closed: false"),
        "{eyes}"
    );
    assert_eq!(drawn(9.8, Some(10.0), None), format!("{:?}", Shape::new()));
}

// case: is simply on once it has switched on, when it never had to, and on a still bot (reduced motion)
#[test]
fn is_simply_on_once_it_has_switched_on_when_it_never_had_to_and_on_a_still_bot() {
    for (s, b) in [(11.0, Some(10.0)), (10.0, None), (0.0, Some(0.0))] {
        let drawn = drawn(s, b, None);
        assert!(drawn.contains("closed: true"), "{drawn}");
        assert!(drawn.contains(&eye_colour(phosphor_rgb(Phosphor::Cyan).map(f64::from))));
    }
}

// case: flashes green as it finishes and settles back to its own light
#[test]
fn flashes_green_as_it_finishes_and_settles_back_to_its_own_light() {
    let green = phosphor_rgb(Phosphor::Green).map(f64::from);
    let cyan = phosphor_rgb(Phosphor::Cyan).map(f64::from);
    for t in [0.1, 0.25] {
        assert!(drawn(5.0 + t, None, Some((Phosphor::Green, 5.0))).contains(&eye_colour(green)));
    }
    let mid = drawn(5.7, None, Some((Phosphor::Green, 5.0)));
    assert!(!mid.contains(&eye_colour(green)) && !mid.contains(&eye_colour(cyan)));
    for t in [FLASH_S + 0.1, 3.0] {
        assert_eq!(
            drawn(5.0 + t, None, Some((Phosphor::Green, 5.0))),
            drawn(5.0 + t, None, None)
        );
    }
}

// case: is its own light on a still bot (reduced motion), and the flash fades smoothly to nothing
#[test]
fn is_its_own_light_on_a_still_bot_and_the_flash_fades_smoothly_to_nothing() {
    assert_eq!(
        drawn(0.0, None, Some((Phosphor::Green, 0.0))),
        drawn(0.0, None, None)
    );
    let mut last = -1.0;
    for i in 0..=140 {
        let a = flash_fade(i as f64 / 100.0);
        assert!(a >= last);
        last = a;
    }
    assert_eq!(flash_fade(0.0), 0.0);
    assert_eq!(flash_fade(FLASH_S), 1.0);
    let cyan = Phosphor::Cyan.lit_css();
    assert_eq!(
        locust_core::bot_spec::mix_phosphor_css(Phosphor::Green.lit_css(), cyan, 1.0),
        locust_core::bot_spec::mix_phosphor_css(cyan, cyan, 1.0)
    );
}

#[test]
fn a_drawn_dot_blinks_into_the_dash() {
    let dot = |open| {
        format!(
            "{:?}",
            paint_glyph_at(
                Shape::new(),
                "•",
                0.0,
                0.0,
                1.0,
                open,
                &|p| [p[0] as f32, p[1] as f32],
                1.0,
                rgba8(255, 255, 255, 255),
                rgba8(255, 255, 255, 80)
            )
        )
    };
    assert!(dot(0.0).contains("[-4.8, 0.4]"));
    assert!(dot(0.0).contains("[4.8, 0.4]"));
    assert!(!dot(1.0).contains("[-4.8, 0.4]"));
}
