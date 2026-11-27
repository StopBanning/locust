//! The Routines screen as Electron draws it (`AutomationsScreen.tsx`, `RoutineTemplates.tsx`, `Screens.tsx`
//! `ScreenHeader`, 0.882), driven headless in the real window. The owner's live hour (26 Nov, row 38) found template rows
//! with no description, "Use template" on every row, "Close" twice (the page's and the Starter templates box's), and the
//! screen's actions as plain words at the left.
use super::*;
use rune::{ElementInfo, Env, Runtime};

fn window(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, crate::view, (1200.0, 2400.0), Env::new(true, 1.0));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(0.05);
    }
}

fn shown(rt: &Runtime<State>) -> Vec<ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    shown(rt).into_iter().find(|e| e.key.as_deref() == Some(key)).map(|e| e.rect).unwrap_or_else(|| panic!("no {key} on screen"))
}

fn words_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    shown(rt).into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text)
}

/// Every word drawn inside the element `key`.
fn text_in(rt: &Runtime<State>, key: &str) -> String {
    let [x, y, w, h] = rect(rt, key);
    shown(rt)
        .into_iter()
        .filter_map(|e| {
            let (cx, cy) = (e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0);
            (cx >= x && cx <= x + w && cy >= y && cy <= y + h).then_some(e.text?)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn empty_shelf_state() -> State {
    let mut s = State::default();
    crate::scenes::team_scene(&mut s);
    s.routines.open = true;
    s.routines.file = json!({"schemaVersion": 1, "routines": []});
    s
}

fn one_routine_state() -> State {
    let mut s = State::default();
    scene(&mut s);
    s
}

#[test]
fn the_header_has_its_actions_at_the_far_end_and_no_close() {
    // Screens.tsx:155-162 and AutomationsScreen.tsx:152-179: title, meta, then `.lc-screen__actions` (`margin-left:
    // auto`, shell.css:22355) holding Import routine and New routine as `lc-ghostbutton`s. There is no Close.
    let rt = window(one_routine_state());
    let header = rect(&rt, "routines-header");
    assert_eq!(header[3], 60.0, "{header:?}");
    let (heading, meta) = (rect(&rt, "routines-heading"), rect(&rt, "routines-meta"));
    assert!(meta[0] > heading[0] + heading[2] && meta[0] - (heading[0] + heading[2]) <= 24.0, "the count is beside the title");
    let (import, new) = (rect(&rt, "routine-import-file"), rect(&rt, "routine-new"));
    assert!(import[0] < new[0], "Import routine, then New routine");
    let right = header[0] + header[2];
    assert!(right - (new[0] + new[2]) <= 30.0, "the actions end at the header's far side: {new:?} in {header:?}");
    assert!(import[0] > header[0] + header[2] / 2.0, "not at the left: {import:?}");
    assert!(import[1] >= header[1] && import[1] + import[3] <= header[1] + header[3], "in the header, not under it");
    // Boxed ghost buttons, not bare words: 32 high (`.lc-ghostbutton`).
    assert_eq!(new[3], 32.0, "{new:?}");
    let shelf = text_in(&rt, "routines-shelf");
    assert!(!shelf.contains("Close"), "no Close on the screen: {shelf}");
}

#[test]
fn an_empty_shelf_lists_the_templates_under_its_sentence_each_with_what_it_does() {
    // AutomationsScreen.tsx:274-277: the head, then `RoutineTemplateList` right under it.
    let rt = window(empty_shelf_state());
    assert_eq!(words_of(&rt, "routines-templates-head").as_deref(), Some("Start from a template, or press New routine to write your own."));
    let (head, list) = (rect(&rt, "routines-templates-head"), rect(&rt, "routine-templates"));
    assert!(list[1] >= head[1] + head[3], "the list is under its sentence");
    // RoutineTemplates.tsx:48-50: each row is its name, `templateMeta`, and its summary.
    let shipped = locust_core::routine_templates::list_shipped_templates();
    assert!(shipped.len() > 3);
    for info in &shipped {
        assert_eq!(words_of(&rt, &format!("routine-template-what-{}", info.id)).as_deref(), Some(info.summary.as_str()), "{}", info.id);
        assert!(!info.summary.is_empty(), "{}", info.id);
        assert_eq!(
            words_of(&rt, &format!("routine-template-meta-{}", info.id)),
            Some(locust_core::routine_templates::template_meta(info)),
            "{}",
            info.id
        );
        let row = text_in(&rt, &format!("routine-use-template-{}", info.id));
        assert!(row.contains(&info.name), "{row}");
    }
    // The row is the button: no "Use template", and no second box with its own Close.
    let words = text_in(&rt, "routines-shelf");
    assert!(!words.contains("Use template"), "{words}");
    assert!(!words.contains("Starter templates"), "{words}");
    assert!(!words.contains("Close"), "{words}");
}

#[test]
fn a_template_row_says_its_facts_at_the_far_end_of_its_name_and_its_summary_under_both() {
    // `.lc-templates__row` (shell.css:3128-3178): name and meta on one line in two columns, the summary across both.
    let rt = window(empty_shelf_state());
    let id = &locust_core::routine_templates::list_shipped_templates()[0].id;
    let (row, meta, what) = (
        rect(&rt, &format!("routine-use-template-{id}")),
        rect(&rt, &format!("routine-template-meta-{id}")),
        rect(&rt, &format!("routine-template-what-{id}")),
    );
    assert!(meta[0] > row[0] + row[2] / 2.0, "the facts sit at the row's far side: {meta:?} in {row:?}");
    assert!(what[1] >= meta[1] + meta[3] - 1.0, "the summary is under the name line");
}

#[test]
fn pressing_a_template_row_opens_its_preview() {
    // RoutineTemplates.tsx:43: the row's own onClick is `onUse(template.id)`; a plain template opens the preview.
    let mut rt = window(empty_shelf_state());
    let plain = locust_core::routine_templates::list_shipped_templates().into_iter().find(|t| !t.chain).expect("a plain template");
    press(&mut rt, &format!("routine-use-template-{}", plain.id));
    assert!(rt.state.routines.import_preview.is_some(), "the preview opened for {}", plain.id);
}

#[test]
fn with_routines_the_add_row_names_the_templates_and_opens_the_picker() {
    // AutomationsScreen.tsx:470-478: "Start from a template", `templatesLine(templates)` under it.
    let mut rt = window(one_routine_state());
    let shipped = locust_core::routine_templates::list_shipped_templates();
    assert_eq!(words_of(&rt, "routine-templates-line"), Some(locust_core::routine_templates::templates_line(&shipped)));
    assert!(rt.find("routine-templates").is_none(), "the list is a row away once there are routines");
    press(&mut rt, "routine-templates-open");
    assert!(rt.state.routines.templates_open);
    // RoutineTemplates.tsx:66-75: the title, one × named Close, the note, the list.
    assert_eq!(words_of(&rt, "routine-templates-title").as_deref(), Some("Start from a template"));
    assert_eq!(words_of(&rt, "routine-templates-note").as_deref(), Some("Each opens as a preview. Nothing is added until you give it to a teammate."));
    let closes = shown(&rt).into_iter().filter(|e| e.label.as_deref() == Some("Close") || e.text.as_deref() == Some("Close")).count();
    assert_eq!(closes, 1, "one way to close");
    assert_eq!(words_of(&rt, "routine-templates-close").as_deref(), Some("×"));
    assert!(rt.find(&format!("routine-use-template-{}", shipped[0].id)).is_some());
    press(&mut rt, "routine-templates-close");
    assert!(!rt.state.routines.templates_open, "× closes it");
}

#[test]
fn a_routines_actions_are_boxed_at_the_rows_far_end() {
    // AutomationsScreen.tsx:347-417: Run and the row's other actions sit after the name and schedule, at the right,
    // as `lc-ghostbutton`s.
    let rt = window(one_routine_state());
    let (card, run, edit) = (rect(&rt, "routine-rt_review"), rect(&rt, "routine-run-rt_review"), rect(&rt, "routine-actions-rt_review"));
    assert!(card[0] + card[2] - (run[0] + run[2]) <= 30.0, "Run ends at the row's right: {run:?} in {card:?}");
    assert_eq!(run[3], 32.0, "Run is a boxed button: {run:?}");
    let first = shown(&rt)
        .into_iter()
        .filter(|e| e.role == "button" && e.rect[1] >= edit[1] && e.rect[1] + e.rect[3] <= edit[1] + edit[3] + 0.5)
        .map(|e| e.rect[0])
        .fold(f32::MAX, f32::min);
    assert!(first > card[0] + card[2] / 2.0, "no action at the left: first at {first}, card {card:?}");
}
