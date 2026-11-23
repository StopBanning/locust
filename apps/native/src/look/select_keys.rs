//! What one key does to a select or an open menu (`components/Select.tsx` `applySelectKey`, `stepSelect`,
//! `typeaheadIndex`, and `menuDigit.ts`).
//!
//! A SELECT MOVES, THEN CHOOSES: Up and Down, Home and End, and a typed letter move the highlight; Enter and Space set
//! the value; Escape closes and leaves it. 1-9 chooses the nth row of an open menu, counting from the top, unless a
//! search box has the keyboard or a modifier is held.

/// How long typed letters still count as one word (`TYPEAHEAD_MS`).
pub const TYPEAHEAD_SECONDS: f64 = 0.5;

/// The modifiers a digit is read with (`menuRowFromDigit`'s second argument).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
}

/// The row (0-based) a digit picks in an open menu: `1` is the top row, `9` the ninth. A search box keeps its digit,
/// and a chord belongs to the window.
pub fn menu_row_from_digit(key: &str, held: Held, typing: bool) -> Option<usize> {
    if typing || held.ctrl || held.meta || held.alt || held.shift {
        return None;
    }
    let mut chars = key.chars();
    match (chars.next(), chars.next()) {
        (Some(c @ '1'..='9'), None) => Some(c as usize - '1' as usize),
        _ => None,
    }
}

/// A key, named as the DOM names it: `"ArrowDown"`, `"Enter"`, `" "`, `"a"`.
#[derive(Clone, Debug, PartialEq)]
pub struct SelectKeyInput<'a> {
    pub open: bool,
    /// The highlighted row while open; -1 for none.
    pub active: i64,
    /// The chosen row; -1 for none.
    pub selected: i64,
    /// The rows that can be chosen, in order.
    pub enabled: &'a [i64],
    pub labels: &'a [&'a str],
    pub key: &'a str,
    pub alt: bool,
    pub ctrl: bool,
    pub meta: bool,
    /// Letters typed so far, before this key.
    pub query: &'a str,
    /// This key came while the letters before it still count.
    pub query_live: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectKeyResult {
    pub open: bool,
    pub active: i64,
    pub choose: bool,
    /// The key was used (the page's `preventDefault`).
    pub prevent: bool,
    pub query: String,
}

/// The next enabled row in `delta`'s direction, staying put at either end.
pub fn step_select(enabled: &[i64], from: i64, delta: i64) -> i64 {
    if enabled.is_empty() {
        return -1;
    }
    let Some(at) = enabled.iter().position(|&i| i == from) else {
        return if delta < 0 { enabled[enabled.len() - 1] } else { enabled[0] };
    };
    let next = at as i64 + delta;
    if next < 0 || next >= enabled.len() as i64 {
        return enabled[at];
    }
    enabled[next as usize]
}

/// The row a typed prefix jumps to. One letter starts after the highlight and wraps, so the same letter walks through
/// every match; a longer prefix stays on the highlight while it still matches, and otherwise takes the first match.
pub fn typeahead_index(labels: &[&str], enabled: &[i64], from: i64, query: &str) -> i64 {
    let wanted = query.to_lowercase();
    let matches: Vec<i64> = enabled
        .iter()
        .copied()
        .filter(|&i| labels.get(i as usize).is_some_and(|l| l.to_lowercase().starts_with(&wanted)))
        .collect();
    if matches.is_empty() {
        return from;
    }
    if wanted.chars().count() <= 1 {
        return matches.iter().copied().find(|&i| i > from).unwrap_or(matches[0]);
    }
    if matches.contains(&from) { from } else { matches[0] }
}

/// What one key does to an open or closed select. Choosing is separate from moving.
pub fn apply_select_key(input: &SelectKeyInput) -> SelectKeyResult {
    let held = if input.query_live { input.query.to_owned() } else { String::new() };
    let stay = SelectKeyResult { open: input.open, active: input.active, choose: false, prevent: false, query: held.clone() };
    if input.enabled.is_empty() || input.ctrl || input.meta {
        return stay;
    }
    let selected =
        if input.selected >= 0 && input.enabled.contains(&input.selected) { input.selected } else { input.enabled[0] };
    let moved = |open: bool, active: i64, choose: bool| SelectKeyResult { open, active, choose, prevent: true, query: String::new() };
    if input.open {
        let digit = menu_row_from_digit(input.key, Held { ctrl: input.ctrl, meta: input.meta, alt: input.alt, shift: false }, false);
        if let Some(nth) = digit {
            return match input.enabled.get(nth) {
                Some(&chosen) => moved(false, chosen, true),
                None => SelectKeyResult { prevent: true, ..stay },
            };
        }
    }

    if input.key.chars().count() == 1 && input.key != " " && !input.alt {
        let letter = input.key.to_lowercase();
        let query = format!("{held}{letter}");
        let from = if input.open { input.active } else { selected_or(input) };
        let prefixed = typeahead_index(input.labels, input.enabled, from, &query);
        let repeated = query.chars().all(|c| c.to_string() == letter);
        let active = if prefixed == from && query.chars().count() > 1 && repeated {
            typeahead_index(input.labels, input.enabled, from, &letter)
        } else {
            prefixed
        };
        let landed = if active < 0 { None } else { input.labels.get(active as usize).map(|l| l.to_lowercase()) };
        let Some(landed) = landed.filter(|l| l.starts_with(&query) || (repeated && l.starts_with(&letter))) else {
            return SelectKeyResult { prevent: true, ..stay };
        };
        let query = if landed.starts_with(&query) { query } else { letter };
        return SelectKeyResult { open: true, active, choose: false, prevent: true, query };
    }

    match input.key {
        "ArrowDown" | "ArrowUp" => {
            if input.alt && input.key == "ArrowUp" {
                return if input.open { moved(false, selected, false) } else { stay };
            }
            let from = if input.open { input.active } else { selected_or(input) };
            moved(true, step_select(input.enabled, from, if input.key == "ArrowDown" { 1 } else { -1 }), false)
        }
        "Home" | "End" => {
            let active = if input.key == "Home" { input.enabled[0] } else { input.enabled[input.enabled.len() - 1] };
            moved(true, active, false)
        }
        "Enter" | " " => {
            if !input.open {
                return moved(true, selected, false);
            }
            let active = if input.enabled.contains(&input.active) { input.active } else { selected };
            moved(false, active, true)
        }
        "Escape" if input.open => moved(false, selected, false),
        "Tab" if input.open => SelectKeyResult { open: false, active: selected, choose: false, prevent: false, query: String::new() },
        _ => stay,
    }
}

/// Electron reads `input.selected` raw where it starts a move from a closed select.
fn selected_or(input: &SelectKeyInput) -> i64 {
    input.selected
}

/// A Rune key as the DOM names it, for `apply_select_key`; `None` for keys a select never reads.
pub fn dom_key(key: &rune::Key) -> Option<String> {
    use rune::Key;
    Some(match key {
        Key::Up => "ArrowUp".into(),
        Key::Down => "ArrowDown".into(),
        Key::Home => "Home".into(),
        Key::End => "End".into(),
        Key::Enter => "Enter".into(),
        Key::Escape => "Escape".into(),
        Key::Tab => "Tab".into(),
        Key::Text(t) => t.clone(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    //! Electron's own tables (`a-select-moves-and-chooses-by-the-keyboard.test.tsx`, `menuDigit.test.ts`), case for case.
    use super::*;

    const LABELS: [&str; 4] = ["Ask", "Brief", "Detailed", "Folder"];
    const ENABLED: [i64; 4] = [0, 1, 2, 3];

    struct Over<'a> {
        open: bool,
        active: i64,
        enabled: &'a [i64],
        ctrl: bool,
        query: &'a str,
        query_live: bool,
    }

    fn base() -> Over<'static> {
        Over { open: false, active: -1, enabled: &ENABLED, ctrl: false, query: "", query_live: false }
    }

    fn press(key: &str, over: Over) -> SelectKeyResult {
        apply_select_key(&SelectKeyInput {
            open: over.open,
            active: over.active,
            selected: 1,
            enabled: over.enabled,
            labels: &LABELS,
            key,
            alt: false,
            ctrl: over.ctrl,
            meta: false,
            query: over.query,
            query_live: over.query_live,
        })
    }

    fn open_at(active: i64) -> Over<'static> {
        Over { open: true, active, ..base() }
    }

    fn r(open: bool, active: i64, choose: bool) -> (bool, i64, bool) {
        (open, active, choose)
    }

    fn oac(x: &SelectKeyResult) -> (bool, i64, bool) {
        (x.open, x.active, x.choose)
    }

    #[test]
    fn opens_on_the_chosen_row_and_arrows_move_without_choosing() {
        let opened = press("Enter", base());
        assert_eq!((oac(&opened), opened.prevent), (r(true, 1, false), true));
        assert_eq!(oac(&press("ArrowDown", open_at(1))), r(true, 2, false));
        assert_eq!(press("ArrowDown", open_at(3)).active, 3);
        assert_eq!(press("ArrowUp", open_at(0)).active, 0);
        let home = press("Home", open_at(2));
        assert_eq!((home.active, home.choose), (0, false));
        assert_eq!(oac(&press("1", open_at(3))), r(false, 0, true));
        let four = press("4", open_at(0));
        assert_eq!((four.active, four.choose), (3, true));
        assert!(!press("9", open_at(0)).choose);
        let end = press("End", open_at(0));
        assert_eq!((end.active, end.choose), (3, false));
    }

    #[test]
    fn chooses_the_highlight_with_enter_or_space_and_escape_leaves_the_value() {
        assert_eq!(oac(&press("Enter", open_at(2))), r(false, 2, true));
        let space = press(" ", open_at(0));
        assert_eq!((space.choose, space.active), (true, 0));
        assert_eq!(oac(&press("Escape", open_at(3))), r(false, 1, false));
        let tab = press("Tab", open_at(3));
        assert_eq!((tab.open, tab.choose, tab.prevent), (false, false, false));
        assert_eq!(oac(&press("ArrowDown", base())), r(true, 2, false));
    }

    #[test]
    fn skips_a_disabled_row_and_a_letter_jumps_then_the_same_letter_walks() {
        let skip = [0, 2, 3];
        assert_eq!(press("ArrowDown", Over { enabled: &skip, ..open_at(0) }).active, 2);
        let d = press("d", open_at(0));
        assert_eq!((oac(&d), d.query.as_str()), (r(true, 2, false), "d"));
        assert_eq!(press("d", Over { query: "d", query_live: true, ..open_at(2) }).active, 2);
        let de = press("e", Over { query: "d", query_live: true, ..open_at(2) });
        assert_eq!((de.active, de.query.as_str()), (2, "de"));
        let z = press("z", open_at(1));
        assert_eq!((z.open, z.active, z.query.as_str()), (true, 1, ""));
        let chord = press("b", Over { ctrl: true, ..base() });
        assert_eq!((chord.open, chord.prevent), (false, false));
        // The walk itself, where two rows share a letter: from the first B the same letter goes on to the next, and
        // from the last it wraps to the first.
        let labels = ["Ask", "Brief", "Bold", "Folder"];
        let walk = |active: i64, query: &str| {
            apply_select_key(&SelectKeyInput {
                open: true,
                active,
                selected: 1,
                enabled: &ENABLED,
                labels: &labels,
                key: "b",
                alt: false,
                ctrl: false,
                meta: false,
                query,
                query_live: !query.is_empty(),
            })
        };
        assert_eq!(walk(0, "").active, 1);
        assert_eq!(walk(1, "b").active, 2);
        assert_eq!(walk(2, "b").active, 1);
    }

    const BARE: Held = Held { ctrl: false, meta: false, alt: false, shift: false };

    #[test]
    fn counts_from_the_top_1_through_9() {
        assert_eq!(menu_row_from_digit("1", BARE, false), Some(0));
        assert_eq!(menu_row_from_digit("9", BARE, false), Some(8));
        assert_eq!(menu_row_from_digit("0", BARE, false), None);
    }

    #[test]
    fn leaves_a_digit_alone_in_a_search_box_and_leaves_chords_alone() {
        assert_eq!(menu_row_from_digit("1", BARE, true), None);
        assert_eq!(menu_row_from_digit("1", Held { ctrl: true, ..BARE }, false), None);
        assert_eq!(menu_row_from_digit("1", Held { shift: true, ..BARE }, false), None);
        assert_eq!(menu_row_from_digit("a", BARE, false), None);
    }
}
