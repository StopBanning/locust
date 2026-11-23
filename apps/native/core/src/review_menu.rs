//! ASK FOR A REVIEW, in a conversation's More actions (Electron `reviewMenu.ts`, 0.799). One row that opens the
//! reviewers beside it, and the pair on two different agents below a hairline. Colin, 2026-10-19: "ask for a review
//! should prob be folded into a dropdown" -- five teammates and a pair were six rows burying the rest of the menu. A lone
//! reviewer with no pair stays one row that says who.

/// Anyone a menu row can name.
pub trait Named {
    fn name(&self) -> &str;
}

/// One row of a menu (`ContextMenuItem`, as far as this row needs it). `on_select` is what choosing it asks for; a row
/// with a `submenu` opens it beside itself instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuRow<A> {
    pub label: String,
    pub on_select: Option<A>,
    pub submenu: Option<Vec<MenuRow<A>>>,
    pub divider_above: bool,
}

impl<A> MenuRow<A> {
    fn choose(label: String, on_select: A) -> Self {
        Self { label, on_select: Some(on_select), submenu: None, divider_above: false }
    }
}

/// `reviewMenuItem`: the one row for asking a review, or none with nobody to ask. `ask` and `ask_two` say what choosing
/// a reviewer, or the pair, asks for.
pub fn review_menu_item<T: Named, P: Named, A>(
    reviewers: &[T],
    pair: Option<(&P, &P)>,
    agent_of: impl Fn(&P) -> String,
    ask: impl Fn(&T) -> A,
    ask_two: impl Fn(&P, &P) -> A,
) -> Option<MenuRow<A>> {
    if let ([lone], None) = (reviewers, pair) {
        return Some(MenuRow::choose(format!("Ask {} for a review", lone.name()), ask(lone)));
    }
    let mut asks: Vec<MenuRow<A>> = reviewers.iter().map(|reviewer| MenuRow::choose(reviewer.name().to_owned(), ask(reviewer))).collect();
    if let Some((a, b)) = pair {
        let label = format!("{} ({}) and {} ({})", a.name(), agent_of(a), b.name(), agent_of(b));
        let divider_above = !asks.is_empty();
        asks.push(MenuRow { divider_above, ..MenuRow::choose(label, ask_two(a, b)) });
    }
    (!asks.is_empty()).then(|| MenuRow { label: "Ask for a review".into(), on_select: None, submenu: Some(asks), divider_above: false })
}

#[cfg(test)]
#[path = "review_menu_tests.rs"]
mod tests;
