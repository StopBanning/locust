//! Electron `renderer/src/the-window-in-a-browser-and-its-cover.test.tsx`:
//! in a browser the window has no window controls of its own; on this computer, while a browser
//! has the window, the window is a cover that says so and takes it back.

use super::*;
use locust_core::desk_host::{
    DeskAsk, DeskHelloInfo, DeskHost, DeskHostDeps, DeskSender, DeskTell, DeskWindow,
};
use rune::{Cx, Env, Runtime};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn cover_runtime(state: State) -> Runtime<State> {
    fn render(s: &State, _: &mut Cx<State>) -> El {
        view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(state, render, (800.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn bar_runtime(in_browser: bool, platform: &'static str) -> Runtime<(bool, &'static str)> {
    fn render(s: &(bool, &'static str), _: &mut Cx<(bool, &'static str)>) -> El {
        let p = Palette::new(true);
        let mut root = col().w_full();
        if let Some(controls) = window_controls_bar(s.0, s.1, &p) {
            root = root.child(controls);
        }
        root
    }
    let mut rt = Runtime::new(
        (in_browser, platform),
        render,
        (800.0, 120.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

#[test]
// case: draws its own window controls on this computer, and none in a browser
fn draws_its_own_window_controls_on_this_computer_and_none_in_a_browser() {
    let local = bar_runtime(false, "win32");
    let local_els = local.elements();
    assert!(
        local_els
            .iter()
            .any(|el| el.label.as_deref() == Some("Minimize")),
        "expected Minimize control on this computer"
    );
    assert!(
        local_els
            .iter()
            .any(|el| el.key.as_deref() == Some("window-controls")),
        "expected window-controls container on this computer"
    );

    let remote = bar_runtime(true, "win32");
    let remote_els = remote.elements();
    assert!(
        !remote_els
            .iter()
            .any(|el| el.key.as_deref() == Some("window-controls")),
        "expected no window controls in a browser"
    );
    assert!(
        !remote_els
            .iter()
            .any(|el| el.label.as_deref() == Some("Minimize")),
        "expected no Minimize button in a browser"
    );

    // On macOS, the OS draws its own traffic lights.
    let mac = bar_runtime(false, "darwin");
    assert!(
        !mac.elements()
            .iter()
            .any(|el| el.key.as_deref() == Some("window-controls")),
        "expected no custom window controls on darwin"
    );
}

#[test]
// case: covers this computer’s window with what has happened and the way back
fn covers_this_computers_window_with_what_has_happened_and_the_way_back() {
    let mut state = State::default();
    apply_cover(&mut state, true, None);
    let rt = cover_runtime(state);
    let words: String = rt
        .elements()
        .into_iter()
        .filter_map(|el| el.text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        words.contains("Locust is open in a browser"),
        "missing cover title: {words}"
    );
    assert!(
        words.contains("Your teammates keep working on this computer"),
        "missing cover body: {words}"
    );
    assert!(
        words.contains("Use Locust here"),
        "missing take-back button: {words}"
    );

    // Caller path through DeskHost: Take raises the cover, pressing "Use Locust here" calls take_back.
    let told: Arc<Mutex<Vec<(String, Vec<DeskTell>)>>> = Arc::new(Mutex::new(Vec::new()));
    let covers: Arc<Mutex<Vec<bool>>> = Arc::new(Mutex::new(Vec::new()));
    let told_c = told.clone();
    let covers_c = covers.clone();
    let host = Arc::new(DeskHost::new(DeskHostDeps {
        handlers: Arc::new(Mutex::new(HashMap::new())),
        window: Arc::new(|| {
            Some(DeskWindow {
                destroyed: false,
                web_contents: DeskSender {
                    id: 1,
                    main_frame_id: 2,
                    process_id: 3,
                    destroyed: false,
                },
            })
        }),
        tell: Arc::new(move |device, items| {
            told_c
                .lock()
                .unwrap()
                .push((device.to_owned(), items.to_vec()));
            true
        }),
        cover: Arc::new(move |on| covers_c.lock().unwrap().push(on)),
        hello: Arc::new(|| {
            Ok(DeskHelloInfo {
                version: "0.830.0".into(),
                computer: "DESK-PC".into(),
                theme: "dark".into(),
            })
        }),
        waiting: Arc::new(Vec::new),
        changed: None,
    }));

    host.handle("DESK-PC", &[DeskAsk::Take { id: "t1".into() }]);
    assert_eq!(*covers.lock().unwrap(), vec![true]);
    assert_eq!(host.active().as_deref(), Some("DESK-PC"));

    let mut covered_state = State::default();
    let host_for_back = Arc::clone(&host);
    covered_state.elsewhere_cover.on_take_back = Some(Arc::new(move || {
        host_for_back.take_back();
    }));
    apply_cover(&mut covered_state, true, host.active());
    let mut rt = cover_runtime(covered_state);
    let named_words: String = rt
        .elements()
        .into_iter()
        .filter_map(|el| el.text)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        named_words.contains("DESK-PC has it now. Your teammates keep working on this computer"),
        "expected device name in cover body: {named_words}"
    );

    let (x, y) = rt
        .find("elsewhere-take-back")
        .expect("elsewhere-take-back button on screen");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);

    assert!(!rt.state.elsewhere_cover.covered);
    assert_eq!(rt.state.elsewhere_cover.device, None);
    assert_eq!(host.active(), None);
    assert_eq!(*covers.lock().unwrap(), vec![true, false]);
    host.flush();
    let sent = told.lock().unwrap();
    assert!(
        sent.iter()
            .any(|(dev, items)| dev == "DESK-PC"
                && items.contains(&DeskTell::Taken {
                    by: "computer".into()
                })),
        "expected Taken(computer) sent to browser: {sent:?}"
    );
}
