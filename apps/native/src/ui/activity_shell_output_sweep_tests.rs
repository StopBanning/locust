//! Command output through the live activity drawer, not a separate test formatter.
use super::*;

fn drawing(output: String) -> Vec<String> {
    let item = ledger::Item::Tool {
        name: "shell".into(),
        status: ledger::Status::Done,
        summary: "npm test".into(),
        detail: Box::new(ledger::ToolDetail {
            id: "test-command".into(), arguments: "npm test".into(), output,
            finished: true, ..Default::default()
        }),
    };
    let mut s = State::default();
    s.activity_open.insert(activity_drawer_key("sweep", &item), true);
    let mut rt = rune::Runtime::new(
        (s, item),
        |(s, item), _| activity_item(item, &Palette::new(true), s, "sweep"),
        (900.0, 800.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

fn lines(first: usize, last: usize) -> String {
    (first..=last).map(|n| format!("line {n}")).collect::<Vec<_>>().join("\n")
}

// case: keeps a short output whole, and asks for no elision control
#[test]
fn a_short_command_output_is_drawn_without_a_more_control() {
    let full = lines(1, 3);
    let words = drawing(full.clone());
    assert!(words.contains(&full), "{words:?}");
    assert!(words.iter().all(|word| !word.contains("more lines")), "{words:?}");
}

// case: keeps BOTH ends of a long one, as two halves the caller can put a control between
#[test]
fn the_drawer_shows_both_ends_of_a_long_command_output() {
    let words = drawing(lines(1, 1200));
    let head = words.iter().position(|w| w == &lines(1, 8)).expect("output head");
    let tail = words.iter().position(|w| w == &lines(1193, 1200)).expect("output tail");
    let more = words.iter().position(|w| w == "1184 more lines").expect("output disclosure");
    assert!(head < more && more < tail, "{words:?}");
}

// case: bounds by LINES rather than pixels, so nothing needs an inner scroller
#[test]
fn the_drawer_elides_at_seventeen_lines_not_sixteen() {
    let sixteen = lines(1, 16);
    let words = drawing(sixteen.clone());
    assert!(words.contains(&sixteen));
    assert!(words.iter().all(|w| !w.contains("more lines")));
    let words = drawing(lines(1, 17));
    assert!(words.iter().any(|w| w == "1 more lines"), "{words:?}");
}

// case: does not end on the blank line almost every command leaves behind
#[test]
fn the_drawer_discards_the_final_newline_before_bounding_the_tail() {
    let words = drawing(format!("{}\n", lines(1, 400)));
    assert!(words.contains(&lines(393, 400)), "{words:?}");
}

// case: says how much it left out rather than trailing off
#[test]
fn the_drawer_names_exactly_how_many_lines_it_omitted() {
    let words = drawing(lines(1, 500));
    assert!(words.iter().any(|w| w == "484 more lines"), "{words:?}");
}

// case: previews its words closed, and not open, where the words are underneath
#[test]
fn the_drawn_lone_thought_previews_its_words_only_while_closed() {
    let thought = "I will check the Roth IRA and related news. The account is read-only.";
    let item = ledger::Item::Thought {
        id: "thought-preview".into(), text: thought.into(), duration_ms: Some(3_000),
    };
    let key = "sweep/group/thought-preview";
    let button_key = format!("fold/{key}");
    let mut rt = rune::Runtime::new(
        (State::default(), item),
        |(s, item), _| activity_group(std::slice::from_ref(item), &Palette::new(true), s, "sweep"),
        (900.0, 800.0), rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let heading = |els: &[rune::ElementInfo]| {
        let button = els.iter().find(|el| !el.exiting && el.key.as_deref() == Some(button_key.as_str())).unwrap();
        (button.label.clone().unwrap(), button.expanded)
    };
    let (closed, expanded) = heading(&rt.elements());
    assert_eq!(expanded, Some(false));
    assert!(closed.starts_with("Thought for 3s: I will check the Roth IRA and related news."), "{closed}");
    assert!(closed.len() < 100, "{closed}");
    rt.update(|(s, _), _| { s.activity_open.insert(key.into(), true); });
    rt.frame(0.0);
    let els = rt.elements();
    assert_eq!(heading(&els), ("Thought for 3s".into(), Some(true)));
    assert!(els.iter().any(|el| !el.exiting && el.text.as_deref().is_some_and(|text| text.contains(thought))), "{els:?}");
}
