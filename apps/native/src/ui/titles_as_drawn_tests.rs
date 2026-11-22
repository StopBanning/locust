//! Electron's `ui-face-is-bundled.test.ts`, its title half ("a title is set the way the wordmark is"), drawn headless.
//!
//! Electron's test reads each selector's FIRST block in `shell.css` (`indexOf`). Two of those blocks were overridden
//! later in the same file and are not what Electron draws: 0.862's round 3 (Colin, 3 Nov) sets `.lc-screen__title` in
//! bold sentence case ("Settings", not S E T T I N G S, so the LOCUST lockup stands alone), and the Settings pane sets a
//! box's heading as a Mac's bold 13, one step over its rows. These tests hold the Rust window to what Electron draws
//! after the cascade, not to the blocks its check happens to read first.
use super::*;
use rune::{ElementInfo, Env, Runtime, TextStyle};

fn shown(rt: &Runtime<State>) -> Vec<ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn words(all: &[ElementInfo]) -> Vec<String> {
    all.iter().filter(|e| e.role == "text").filter_map(|e| e.text.clone()).collect()
}

fn drawn(state: State, view: fn(&State, &mut Cx<State>) -> El) -> Vec<ElementInfo> {
    let mut rt = Runtime::new(state, view, (1200.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    shown(&rt)
}

fn board(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(crate::home_panel::board_view(s, &Palette::new(true)))
}

fn memory(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(memory_screen(s, &Palette::new(true)))
}

fn same_style(a: TextStyle, b: TextStyle) -> bool {
    (a.size, a.weight, a.tracking, a.family) == (b.size, b.weight, b.tracking, b.family)
}

#[test]
fn a_screens_title_is_bold_and_keeps_its_own_case() {
    // case: a screen's title takes the title tokens, in capitals
    // Drawn as Electron draws it since 0.862 (`.lc-screen__title`, the later block): bold, in sentence case.
    let board_words = words(&drawn(State::default(), board));
    assert!(board_words.iter().any(|w| w == "Board"), "{board_words:?}");
    assert!(!board_words.iter().any(|w| w == "BOARD"), "{board_words:?}");
    let memory_words = words(&drawn(State::default(), memory));
    assert!(memory_words.iter().any(|w| w == "Memory"), "{memory_words:?}");
    assert!(!memory_words.iter().any(|w| w == "MEMORY"), "{memory_words:?}");
    let mut s = State::default();
    s.settings_panel.open = true;
    let settings_words = words(&drawn(s, crate::view));
    assert!(settings_words.iter().any(|w| w == crate::settings_pages::SETTINGS_TITLE), "{settings_words:?}");
    assert!(!settings_words.iter().any(|w| w == "SETTINGS"), "{settings_words:?}");
    // The title tokens Electron's test also pins, read from the same tokens.css Rust compiles in: 700 for the title
    // weight, and the wordmark-style title role Rust keeps (`LocustType::title`) takes it.
    assert_eq!(crate::look::tokens(true).raw("title-weight"), "700");
    assert_eq!(crate::look::locust_type().title.weight, 700.0);
}

#[test]
fn a_section_inside_a_screen_is_a_sentence_one_step_above_its_text() {
    // case: a section inside a screen is a sentence, one step above its text (Colin, 0.356)
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::Appearance);
    let all = drawn(s, crate::view);
    let headings: Vec<&ElementInfo> = all.iter().filter(|e| e.role == "heading" || (e.role == "text" && e.text.as_deref() == Some("Code font"))).collect();
    let code_font = headings.iter().find(|e| e.text.as_deref() == Some("Code font")).unwrap_or_else(|| panic!("no Code font heading: {:?}", words(&all)));
    assert!(!all.iter().any(|e| e.text.as_deref() == Some("CODE FONT")), "a section heading is a sentence, not capitals");
    // One step above its text: the heading's line is taller than the line of words under it.
    let under = all
        .iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("The face for code, paths and commands")))
        .expect("the code font's lede");
    assert!(under.rect[1] > code_font.rect[1], "the lede sits under its heading");
    // The box heading's style is the Mac's bold 13 Electron's Settings pane sets (`.lc-settings__section >
    // .lc-settings__heading`), above the 12px label its lede is set in.
    assert_eq!((rune::tok::type_::TITLE.size, rune::tok::type_::TITLE.weight), (13.0, 600.0));
    assert!(rune::tok::type_::TITLE.size > rune::tok::type_::LABEL.size && rune::tok::type_::TITLE.family != rune::Family::Mono);
    // tokens.css still names the section size Electron's test pins.
    assert_eq!(crate::look::tokens(true).raw("text-section"), "16px");
}

#[test]
fn a_section_label_below_a_title_stays_a_label_not_a_second_title() {
    // case: a section label below a title stays a label, not a second title
    // `.lc-settings__heading--section` restates every property the heading sets: mono, the label size, tracked,
    // capitals, muted. Rust's label is `controls::label`.
    let l = crate::look::controls::Look::new(true);
    let label = |_: &State, _: &mut Cx<State>| -> El { col().child(crate::look::controls::label(&l_dark(), "Rooms")) };
    let all = drawn(State::default(), label);
    let drawn_label = all.iter().find(|e| e.role == "text").expect("the label");
    assert_eq!(drawn_label.text.as_deref(), Some("ROOMS"));
    assert_eq!(drawn_label.text_color, Some(l.c("text-muted")));
    let (label_ty, title_ty) = (l.ty.label, l.ty.title);
    assert!(label_ty.family == rune::Family::Mono && title_ty.family != rune::Family::Mono, "a label is in the mono face");
    assert!(label_ty.size < title_ty.size && label_ty.weight < title_ty.weight);
    assert!(label_ty.tracking > 0.0, "a label is tracked");
    assert!(!same_style(label_ty, rune::tok::type_::TITLE), "a label is not set as a section heading");
}

fn l_dark() -> crate::look::controls::Look {
    crate::look::controls::Look::new(true)
}

#[test]
fn a_name_inside_a_title_keeps_its_own_spelling() {
    // case: a name inside a title keeps its own spelling
    let mut s = State::default();
    s.home.folder_path = Some("/Users/someone/work/McShop-iOS".into());
    let all = drawn(s, memory);
    let said = words(&all);
    assert!(said.iter().any(|w| w == "This folder · McShop-iOS"), "{said:?}");
    assert!(!said.iter().any(|w| w.contains("MCSHOP")), "{said:?}");
}
