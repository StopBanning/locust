use super::*;
use locust_core::home_cover::DOZE_AFTER_MS;
use rune::{Env, Runtime};

fn home(ready: bool, tube: &str, reduced: bool) -> Runtime<State> {
    let mut state = State::default();
    state.demo = true;
    state.home.home_open = true;
    state.settings_panel.agents = ready.then(Vec::new);
    state.settings_panel.settings.tube = tube.into();
    state.reduced_motion.set(reduced);
    let mut rt = Runtime::new(
        state,
        |s, _| home_view(s, &Palette::new(true)),
        (1215.0, 900.0),
        Env::new(true, 1.0),
    );
    rt.poll = Some(crate::poll);
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..12 {
        rt.frame(0.05);
    }
}

fn click_glass(rt: &mut Runtime<State>) {
    let (x, y) = rt.find("home-cover-glass").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn awake(rt: &Runtime<State>) -> bool {
    rt.find("home-cover-bot-sable-watching").is_some() && rt.find("home-cover-zzz").is_none()
}

#[test]
fn main_poll_dozes_the_cover_sleeper_after_the_pointer_leaves() {
    let mut rt = home(true, "off", false);
    let ([x, y, w, h], _) = rt.presented("home-cover-face-sable").unwrap();
    assert!(rt.find("home-cover-zzz").is_some());
    rt.pointer_move(x + w * 3.0, y + h * 0.5);
    settle(&mut rt);
    assert!(!awake(&rt));
    // Close to the sleeper but outside its drawing.
    rt.pointer_move(x + w * 1.5, y + h * 0.5);
    settle(&mut rt);
    assert!(awake(&rt));
    rt.pointer_move(16.0, 16.0);
    settle(&mut rt);
    assert!(
        awake(&rt),
        "the sleeper stays awake after the pointer leaves"
    );
    assert!(rt.state.home.cover_sleeper.away_since.is_some());

    std::thread::sleep(Duration::from_millis(DOZE_AFTER_MS + 50));
    rt.frame(4.1);
    settle(&mut rt);
    assert!(
        rt.find("home-cover-zzz").is_some(),
        "the sleeper dozes without another pointer event"
    );
}

#[test]
fn main_poll_powers_on_the_cover_when_ready_and_clicking_the_glass_replays_it() {
    let mut rt = home(false, "subtle", false);
    assert_eq!(rt.state.home.cover_lit_count, 0);
    assert!(rt.find("home-cover-power-0").is_none());

    rt.update(|s, _| s.settings_panel.agents = Some(Vec::new()));
    settle(&mut rt);
    assert_eq!(rt.state.home.cover_lit_count, 1);
    assert!(rt.find("home-cover-power-0").is_some());
    click_glass(&mut rt);
    assert_eq!(
        rt.state.home.cover_replays, 0,
        "the first lighting can finish"
    );

    rt.update(|s, _| s.home.cover_lit_at = Some(cover_now() - COVER_POWER_SECONDS - 1.0));
    settle(&mut rt);
    assert!(rt.find("home-cover-power-0").is_none());
    click_glass(&mut rt);
    assert_eq!(rt.state.home.cover_lit_count, 2);
    assert!(rt.find("home-cover-power-1").is_some());

    for (tube, reduced) in [("off", false), ("full", true)] {
        let mut rt = home(true, tube, reduced);
        click_glass(&mut rt);
        assert_eq!(rt.state.home.cover_lit_count, 0);
        assert!(rt.find("home-cover-power-0").is_none());
    }
}
