use super::*;
use locust_core::agent_card::Tone;
use rune::{Env, Runtime};

#[test]
fn usage_limit_meter_fill_reflects_calm_pressing_and_spent_thresholds() {
    let s = State::default();
    let mut rt = Runtime::new(
        s,
        |s, _| {
            let reading = "5-hour window 40% used · 7-day window 85% used · monthly window 100% used";
            plan_limit(s, &Palette::new(true), "claude", reading, chrono::Utc::now())
        },
        (600., 400.),
        Env::new(true, 1.),
    );
    for _ in 0..4 {
        rt.frame(0.05);
    }

    let l = crate::look::controls::Look::new(true);
    let fill_of = |name: &str| {
        let key = format!("usage-limit-claude-{name}-fill");
        rt.elements()
            .into_iter()
            .find(|el| !el.exiting && el.key.as_deref() == Some(key.as_str()))
            .unwrap_or_else(|| panic!("missing meter fill {key}"))
            .fill
    };

    assert_eq!(fill_of("5-hour window"), crate::agent_card_ui::bar_colour(&l, Tone::Calm));
    assert_eq!(fill_of("7-day window"), crate::agent_card_ui::bar_colour(&l, Tone::Pressing));
    assert_eq!(fill_of("monthly window"), crate::agent_card_ui::bar_colour(&l, Tone::Spent));
}
