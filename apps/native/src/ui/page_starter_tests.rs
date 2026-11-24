//! Electron's `a-build-starter-makes-one-page.test.ts` through Home: the starter rows `home_view` draws for a team with a
//! coder, each with its glyph, and the press that puts a starter's prompt in the composer (`apply_starter`). Expected
//! words are Electron's, written out.
use super::*;
use regex::Regex;

// case: asks for one named .html file with everything inside it
#[test]
fn the_page_starter_asks_for_one_named_html_file() {
    let mut s = State::default();
    home_scene(&mut s);
    let mut rt = rune::Runtime::new(s, crate::view, (1200.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    // The rows Home drew whose glyph is the page.
    let drawn: Vec<rune::ElementInfo> = rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("starter-"))).collect();
    let pages: Vec<String> = drawn
        .iter()
        .filter_map(|e| {
            let label = rt.elements().into_iter().find(|x| x.path.starts_with(&format!("{}/", e.path)) && x.role == "button")?.label?;
            std::ptr::eq(starter_icon(&label), &crate::look::icons::FILE).then_some(label)
        })
        .collect();
    assert_eq!(pages, ["Build a landing page"]);
    let starter = starters::coder_starters().into_iter().find(|st| st.label == "Build a landing page").expect("the page starter");
    assert!(starter.compare_changes);
    assert!(drawn.iter().any(|e| e.key.as_deref() == Some(&format!("starter-{}", starter.id))));
    // Pressed, the composer holds its words, for the person to finish.
    let row = drawn.iter().find(|e| e.key.as_deref() == Some(&format!("starter-{}", starter.id))).expect("drawn").clone();
    let [x, y, w, h] = row.rect;
    rt.pointer_down(x + w / 4.0, y + h / 2.0);
    rt.pointer_up(x + w / 4.0, y + h / 2.0);
    rt.frame(0.05);
    let mut prompt = String::new();
    rt.update(|s, _| prompt = s.draft.text.clone());
    assert!(Regex::new(r"^Make [a-z]+\.html: ").unwrap().is_match(&prompt), "{prompt}");
    assert!(prompt.contains("One file"), "{prompt}");
    // For anything the person is making, not one example business: they finish the words.
    assert!(prompt.ends_with(": "), "{prompt:?}");
    assert!(!Regex::new(r"(?i)coffee").unwrap().is_match(&format!("{} {prompt}", starter.line)));
}
