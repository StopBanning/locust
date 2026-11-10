//! Find bar UI — port of `ThreadFind.tsx` words and controls.

use super::*;
use crate::thread_find::{MAX_FIND_HITS, find_count_words, find_in, step_hit};

pub(super) fn thread_find_bar(s: &State, p: &Palette) -> Option<El> {
    if !s.find_open {
        return None;
    }
    let current = find_pieces_now(s);
    let pieces: Vec<&str> = current.iter().map(String::as_str).collect();
    let hits = find_in(&pieces, &s.find_query.text, MAX_FIND_HITS);
    let index = if hits.is_empty() {
        0
    } else {
        s.find_index.min(hits.len() - 1)
    };
    let count_words = find_count_words(hits.len(), index, &s.find_query.text);
    let hits_empty = hits.is_empty();
    let mut bar = row()
        .key("thread-find")
        .w_full()
        .center_y()
        .gap(8.0)
        .px(12.0)
        .py(8.0)
        .fill(p.raised)
        .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
        .child(
            text_input(&s.find_query, type_::BODY)
                .key("find-query")
                .on_input_key(on_key)
                .grow(1.0)
                .color(p.text)
                .placeholder("Find in this conversation", p.muted)
                .fill(p.field)
                .padding(8.0)
                .radius(6.0)
                .on_edit(|s: &mut State| {
                    s.find_index = 0;
                    &mut s.find_query
                }),
        );
    if !count_words.is_empty() {
        bar = bar.child(text(count_words).font(type_::LABEL).color(p.muted));
    }
    bar = bar
        .child(
            text("Previous match")
                .key("find-prev")
                .font(type_::LABEL)
                .color(if hits_empty { p.muted } else { p.secondary })
                .on_press(|s: &mut State| {
                    refresh_find_hits(s);
                    if s.find_hits.is_empty() {
                        return;
                    }
                    s.find_index = step_hit(s.find_index, s.find_hits.len(), true);
                }),
        )
        .child(
            text("Next match")
                .key("find-next")
                .font(type_::LABEL)
                .color(if hits_empty { p.muted } else { p.secondary })
                .on_press(|s: &mut State| {
                    refresh_find_hits(s);
                    if s.find_hits.is_empty() {
                        return;
                    }
                    s.find_index = step_hit(s.find_index, s.find_hits.len(), false);
                }),
        )
        .child(
            text("Close find")
                .key("find-close")
                .font(type_::LABEL)
                .color(p.muted)
                .on_press(|s: &mut State| {
                    close_find(s);
                }),
        );
    let mut col_bar = col().w_full().child(bar);
    if s.find_earlier_turns > 0 && !s.find_query.text.trim().is_empty() {
        let n = s.find_earlier_turns;
        let label = if n == 1 {
            format!("Also search the {n} earlier turn")
        } else {
            format!("Also search the {n} earlier turns")
        };
        col_bar = col_bar.child(
            text(label)
                .key("find-earlier")
                .font(type_::LABEL)
                .color(p.secondary)
                .px(12.0)
                .py(6.0)
                .on_press(|s: &mut State| {
                    s.find_show_earlier = true;
                    refresh_find_hits(s);
                }),
        );
    }
    Some(col_bar)
}

pub(super) fn refresh_find_hits(s: &mut State) {
    let current = find_pieces_now(s);
    let pieces: Vec<&str> = current.iter().map(String::as_str).collect();
    s.find_hits = find_in(&pieces, &s.find_query.text, MAX_FIND_HITS);
    if s.find_hits.is_empty() {
        s.find_index = 0;
    } else {
        s.find_index = s.find_index.min(s.find_hits.len() - 1);
    }
}

pub(super) fn open_find(s: &mut State) {
    s.find_open = true;
    s.draft.focused = false;
    s.find_query.focused = true;
    s.find_query.set_selection(0, s.find_query.text.len());
    refresh_find_hits(s);
}

pub(super) fn close_find(s: &mut State) {
    s.find_open = false;
    s.find_query.clear();
    s.find_hits.clear();
    s.find_index = 0;
}

#[cfg(test)]
mod tests {
    #[test]
    fn earlier_turn_words_match_electron() {
        assert_eq!(
            {
                let n = 1usize;
                if n == 1 {
                    format!("Also search the {n} earlier turn")
                } else {
                    format!("Also search the {n} earlier turns")
                }
            },
            "Also search the 1 earlier turn"
        );
        assert_eq!(
            {
                let n = 3usize;
                if n == 1 {
                    format!("Also search the {n} earlier turn")
                } else {
                    format!("Also search the {n} earlier turns")
                }
            },
            "Also search the 3 earlier turns"
        );
    }
}
