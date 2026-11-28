use super::{home_agents, Palette, State};
use locust_core::agents::{Auth, Found, Probe};

fn drawn(width: f32) -> rune::Runtime<State> {
    let mut state = State::default();
    state.settings_panel.agents = Some(
        ["codex", "claude", "opencode", "gemini"]
            .into_iter()
            .map(|id| Found {
                id: id.into(),
                display_name: id.into(),
                installed: true,
                version: None,
                auth: Auth::Authenticated,
                ready: true,
                status: Probe::Ready,
                unsupported_note: None,
                through_cli: false,
                checking: false,
            })
            .collect(),
    );
    let mut rt = rune::Runtime::new(
        state,
        |s, _| home_agents(s, &Palette::new(true)),
        (width, 200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn rect(rt: &rune::Runtime<State>, key: &str) -> [f32; 4] {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .unwrap_or_else(|| panic!("missing {key}"))
        .rect
}

// case: keeps Home's account marks in a row, the row wrapping before the marks stack
#[test]
fn narrow_home_wraps_the_header_before_stacking_account_marks() {
    let wide = drawn(600.0);
    let narrow = drawn(150.0);
    let center_y = |rt: &rune::Runtime<State>, key| {
        let [_, y, _, h] = rect(rt, key);
        y + h / 2.0
    };
    let offset = |rt| center_y(rt, "home-agents-mark-codex") - center_y(rt, "home-agents-title");
    assert!(offset(&wide).abs() < 1.0);
    assert!(offset(&narrow) > 10.0, "the account marks must move down as a unit");
    let bounds = |rt: &rune::Runtime<State>| rect(rt, "home-agents");
    for rt in [&wide, &narrow] {
        let [x, _, width, _] = bounds(rt);
        let rects: Vec<_> = ["codex", "claude", "opencode", "gemini"]
            .into_iter()
            .map(|id| rect(rt, &format!("home-agents-mark-{id}")))
            .collect();
        assert!(rects.iter().all(|r| (r[1] - rects[0][1]).abs() < 1.0), "{rects:?}");
        assert!(rects.windows(2).all(|pair| pair[1][0] >= pair[0][0] + pair[0][2]), "{rects:?}");
        assert!(rects.iter().all(|r| r[0] >= x && r[0] + r[2] <= x + width + 1.0), "{rects:?}");
    }
}
