use super::*;
use rune::{Env, Runtime};

#[test]
fn rail_flyout_status_line_uses_the_teammates_live_tone_colour() {
    let mut s = State::default();
    s.demo = true;
    s.layout_pref = "compact".into();
    s.teammates = vec![Teammate {
        id: "tm_wren".into(),
        name: "Wren".into(),
        role: "Code & Migrations".into(),
        runtime: Some("claude".into()),
        model: None,
        account: None,
        busy: true,
        runs_in: "ask".into(),
        folder: None,
    }];
    s.rail_open = Some("tm_wren".into());
    s.rail_pinned = true;

    let mut rt = Runtime::new(s, |s, _| flyout(s, &Palette::new(true), 0.0).unwrap(), (400., 400.), Env::new(true, 1.));
    for _ in 0..4 {
        rt.frame(0.05);
    }

    let l = crate::look::controls::Look::new(true);
    let status = rt
        .elements()
        .into_iter()
        .find(|el| !el.exiting && el.text.as_deref() == Some("Code & Migrations · working"))
        .expect("working status line in the rail flyout");
    assert_eq!(
        status.text_color,
        Some(crate::teammate_faces::tone_color(&l, locust_core::face_status::Tone::Live)),
        "the flyout status line uses the live tone colour rather than plain secondary text"
    );
    assert_ne!(status.text_color, Some(l.c("text-secondary")));
}
