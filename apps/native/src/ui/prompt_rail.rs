//! A slim strip of your own messages, stuck to the thread's right edge.
//! Port of `apps/desktop/src/renderer/src/promptRail.ts` and
//! `apps/desktop/src/renderer/src/components/PromptRail.tsx`.

use super::*;
use std::time::{Duration, Instant};

/// How many of your own messages a conversation needs before the rail appears.
pub(super) const PROMPT_RAIL_MIN: usize = 4;

/// Clear points the strip needs to the right of the words, so it never sits on them.
pub(super) const PROMPT_RAIL_ROOM: f32 = 18.0;

/// Said on the hover, and the keys `prompt_rail_step` accepts.
pub(super) const PROMPT_RAIL_KEYS: &str = "Ctrl+Alt+Up and Ctrl+Alt+Down";

const HOVER_LIMIT: usize = 72;
const HIT_FLASH_MS: u64 = 2400;

fn strip_heading_prefix(s: &str) -> &str {
    let hashes = s.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) {
        let rest = &s[hashes..];
        if rest.starts_with(char::is_whitespace) {
            return rest.trim_start();
        }
    }
    s
}

fn strip_markdown_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after_open = &rest[open + 1..];
        if let Some(mid) = after_open.find("](") {
            let label = &after_open[..mid];
            let after_mid = &after_open[mid + 2..];
            if let Some(close) = after_mid.find(')') {
                out.push_str(label);
                rest = &after_mid[close + 1..];
                continue;
            }
        }
        out.push('[');
        rest = after_open;
    }
    out.push_str(rest);
    out
}

/// The first line of a message, as the hover should read it.
pub(super) fn prompt_rail_line(text: &str) -> String {
    let raw = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");
    let no_heading = strip_heading_prefix(raw);
    let no_links = strip_markdown_links(no_heading);
    let stripped: String = no_links
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '`' | '~'))
        .collect();
    let plain = stripped.split_whitespace().collect::<Vec<_>>().join(" ");
    if plain.chars().count() <= HOVER_LIMIT {
        return plain;
    }
    let prefix: String = plain.chars().take(HOVER_LIMIT - 1).collect();
    format!("{}…", prefix.trim_end())
}

pub(super) fn prompt_rail_scrollable(scroll_height: f32, client_height: f32) -> bool {
    scroll_height > client_height + 1.0
}

/// Space between the words and the scrollbar. The strip lives here, or not at all.
pub(super) fn prompt_rail_room(
    thread_right: f32,
    scrollbar: f32,
    padding_right: f32,
    column_right: f32,
) -> f32 {
    thread_right - scrollbar - padding_right - column_right
}

pub(super) fn prompt_rail_shows(
    messages: usize,
    scrollable: bool,
    compare: bool,
    room: f32,
) -> bool {
    messages >= PROMPT_RAIL_MIN && scrollable && !compare && room >= PROMPT_RAIL_ROOM
}

/// Where a mark sits on the strip: 0 at the top of the thread, 1 at the bottom.
pub(super) fn prompt_mark_place(message_top: f32, thread_height: f32) -> f32 {
    if thread_height <= 0.0 {
        return 0.0;
    }
    (message_top / thread_height).clamp(0.0, 1.0)
}

/// The message whose top is nearest the top of the view. A tie keeps the earlier one.
pub(super) fn current_prompt_mark(tops: &[f32], view_top: f32) -> usize {
    let mut best = 0;
    let mut distance = f32::INFINITY;
    for (index, &top) in tops.iter().enumerate() {
        let next = (top - view_top).abs();
        if next < distance {
            best = index;
            distance = next;
        }
    }
    best
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct RailKey {
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
    pub shift: bool,
    pub key: Key,
}

impl From<&KeyEvent> for RailKey {
    fn from(event: &KeyEvent) -> Self {
        Self {
            ctrl: event.mods.ctrl,
            alt: event.mods.alt,
            meta: event.mods.cmd,
            shift: event.mods.shift,
            key: event.key.clone(),
        }
    }
}

/// Ctrl+Alt+Up / Ctrl+Alt+Down move among your own messages.
/// Alt+Up and Alt+Down stay the conversation step; they do not match here.
pub(super) fn prompt_rail_step(event: &RailKey) -> Option<i32> {
    if !event.ctrl || !event.alt || event.meta || event.shift {
        return None;
    }
    match event.key {
        Key::Up => Some(-1),
        Key::Down => Some(1),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct RailMotion {
    pub scroll_smooth: bool,
    pub highlight: bool,
}

/// Reduced motion jumps and does not flash the line. Otherwise the jump eases, and the line is marked.
pub(super) fn prompt_rail_motion(reduced: bool) -> RailMotion {
    if reduced {
        RailMotion {
            scroll_smooth: false,
            highlight: false,
        }
    } else {
        RailMotion {
            scroll_smooth: true,
            highlight: true,
        }
    }
}

#[derive(Default)]
pub(super) struct PromptRailState {
    pub viewport_rect: [f32; 4],
    pub column_rect: [f32; 4],
    pub tops: Vec<f32>,
    pub current: usize,
    pub hovered: Option<usize>,
    pub hit_key: Option<String>,
    pub hit_until: Option<Instant>,
    pub reduced_motion: bool,
}

impl PromptRailState {
    pub fn reset(&mut self) {
        self.tops.clear();
        self.current = 0;
        self.hovered = None;
        self.hit_key = None;
        self.hit_until = None;
    }

    pub fn set_top(&mut self, index: usize, top: f32) -> bool {
        if self.tops.len() <= index {
            self.tops.resize(index + 1, 0.0);
        }
        let prev = self.tops[index];
        self.tops[index] = top;
        (prev - top).abs() > 0.5
    }

    pub fn tick(&mut self, now: Instant) {
        if let Some(until) = self.hit_until
            && now >= until
        {
            self.hit_key = None;
            self.hit_until = None;
        }
    }
}

pub(super) fn shown_now(s: &State, user_count: usize) -> bool {
    let client_h = s.prompt_rail.viewport_rect[3];
    if client_h < 1.0 {
        return false;
    }
    let scroll_h = s.prompt_rail.column_rect[3];
    let thread_right = s.prompt_rail.viewport_rect[0] + s.prompt_rail.viewport_rect[2];
    let column_right = s.prompt_rail.column_rect[0] + s.prompt_rail.column_rect[2];
    let room = prompt_rail_room(thread_right, 0.0, 0.0, column_right);
    prompt_rail_shows(
        user_count,
        prompt_rail_scrollable(scroll_h, client_h),
        false,
        room,
    )
}

pub(super) fn jump(s: &mut State, index: usize, bubble_key: &str, tx: &mut Tx) {
    s.prompt_rail.hovered = None;
    s.prompt_rail.current = index;
    s.thread_away = false;
    let motion = prompt_rail_motion(s.prompt_rail.reduced_motion);
    if motion.highlight {
        s.prompt_rail.hit_key = Some(bubble_key.to_owned());
        s.prompt_rail.hit_until = Some(Instant::now() + Duration::from_millis(HIT_FLASH_MS));
    } else {
        s.prompt_rail.hit_key = None;
        s.prompt_rail.hit_until = None;
    }
    tx.scroll_key_to(bubble_key.to_owned(), 0.0);
}

fn menu_is_open(s: &State) -> bool {
    s.org_menu_id.is_some()
        || s.model_menu.is_some()
        || s.mode_menu
        || s.reference_menu
        || s.plus_menu
        || s.palette_open
}

pub(super) fn handle_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(step) = prompt_rail_step(&RailKey::from(event)) else {
        return false;
    };
    if menu_is_open(s) {
        return false;
    }
    let Some((_, conversation)) = selected_conversation(s) else {
        return false;
    };
    let conv_key = s
        .ledger
        .as_ref()
        .and_then(|p| p.conversation.conversation_id.as_deref())
        .unwrap_or("preview")
        .to_owned();
    let user_keys: Vec<String> = if let Some(pane) = &s.ledger {
        pane.items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                matches!(item, ledger::Item::User(_))
                    .then(|| format!("{conv_key}/ledger-item-{i}"))
            })
            .collect()
    } else {
        conversation
            .turns
            .iter()
            .enumerate()
            .filter_map(|(i, (who, _))| {
                (who == "You").then(|| format!("{conv_key}/ledger-item-{i}"))
            })
            .collect()
    };
    if !shown_now(s, user_keys.len()) {
        return false;
    }
    let count = user_keys.len();
    let next = (s.prompt_rail.current as i32 + step).clamp(0, count as i32 - 1) as usize;
    jump(s, next, &user_keys[next], tx);
    true
}

pub(super) fn view(s: &State, items: &[ledger::Item], p: &Palette) -> Option<El> {
    let conv_key = s
        .ledger
        .as_ref()
        .and_then(|p| p.conversation.conversation_id.as_deref())
        .unwrap_or("preview");
    let marks: Vec<(String, String)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, item)| match item {
            ledger::Item::User(body) => {
                Some((format!("{conv_key}/ledger-item-{i}"), prompt_rail_line(body)))
            }
            _ => None,
        })
        .collect();

    if !shown_now(s, marks.len()) {
        return None;
    }

    let l = crate::look::controls::Look::new(p.dark);
    let client_h = s.prompt_rail.viewport_rect[3];
    let scroll_h = s.prompt_rail.column_rect[3];
    let track_h = (client_h - 56.0).max(14.0);
    let current = s.prompt_rail.current.min(marks.len().saturating_sub(1));
    let platform = if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    };
    let keys_hint = crate::keyboard_shortcuts::keys_on(PROMPT_RAIL_KEYS, platform, true);

    let mut track = el().key("prompt-rail-track").w(14.0).h(track_h);
    for (index, (bubble_key, line)) in marks.iter().enumerate() {
        let top = s.prompt_rail.tops.get(index).copied().unwrap_or(0.0);
        let place = prompt_mark_place(top, scroll_h);
        let y = (place * track_h - 7.0).clamp(0.0, (track_h - 14.0).max(0.0));
        let active = index == current || s.prompt_rail.hovered == Some(index);
        let dot_size = if active { 8.0 } else { 6.0 };
        let dot_color = l.c(if active { "text-primary" } else { "text-muted" });
        let key_owned = bubble_key.clone();
        let mut mark = el()
            .key(format!("prompt-rail-mark-{index}"))
            .pin(Some(0.0), Some(y), None, None)
            .size(14.0, 14.0)
            .radius(7.0)
            .center()
            .role("button")
            .label(line.clone())
            .child(el().size(dot_size, dot_size).radius(dot_size / 2.0).fill(dot_color))
            .on_hover_change(move |s: &mut State, over, _| {
                if over {
                    s.prompt_rail.hovered = Some(index);
                } else if s.prompt_rail.hovered == Some(index) {
                    s.prompt_rail.hovered = None;
                }
            })
            .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                jump(s, index, &key_owned, tx);
            });
        if s.prompt_rail.hovered == Some(index) {
            mark = mark.child(
                col()
                    .key("prompt-rail-tip")
                    .anchored_fit(
                        format!("prompt-rail-mark-{index}"),
                        rune::Side::Left,
                        Align::Center,
                        8.0,
                    )
                    .max_w(240.0)
                    .py(6.0)
                    .px(10.0)
                    .gap(2.0)
                    .radius(l.t.px("radius-card"))
                    .fill(l.c("bg-raised"))
                    .border(1.0, l.c("border-card"))
                    .child(
                        text(line.clone())
                            .font(l.ty.body)
                            .color(l.c("text-primary"))
                            .truncate(),
                    )
                    .child(
                        text(keys_hint.clone())
                            .font(l.ty.body)
                            .color(l.c("text-muted")),
                    ),
            );
        }
        track = track.child(mark);
    }

    Some(
        el().key("prompt-rail")
            .pin(None, Some(28.0), Some(2.0), Some(28.0))
            .w(14.0)
            .label("Your messages")
            .child(track),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(ctrl: bool, alt: bool, meta: bool, shift: bool, k: Key) -> RailKey {
        RailKey {
            ctrl,
            alt,
            meta,
            shift,
            key: k,
        }
    }

    #[test]
    fn gives_rail_to_four_messages_when_scrollable_and_room_suffices() {
        assert!(prompt_rail_shows(PROMPT_RAIL_MIN, true, false, PROMPT_RAIL_ROOM));
        assert!(!prompt_rail_shows(PROMPT_RAIL_MIN - 1, true, false, PROMPT_RAIL_ROOM));
        assert!(!prompt_rail_shows(PROMPT_RAIL_MIN, false, false, PROMPT_RAIL_ROOM));
        assert!(!prompt_rail_shows(PROMPT_RAIL_MIN, true, true, PROMPT_RAIL_ROOM));
        assert!(!prompt_rail_shows(PROMPT_RAIL_MIN, true, false, PROMPT_RAIL_ROOM - 1.0));
        assert!(prompt_rail_scrollable(800.0, 400.0));
        assert!(!prompt_rail_scrollable(400.0, 400.0));
        assert!(!prompt_rail_scrollable(401.0, 400.0));
        assert_eq!(prompt_rail_room(1000.0, 12.0, 6.0, 900.0), 82.0);
    }

    #[test]
    fn keeps_marks_in_order_and_current_is_nearest_top() {
        let tops = [0.0, 400.0, 900.0, 1500.0];
        let places: Vec<f32> = tops.iter().map(|&t| prompt_mark_place(t, 2000.0)).collect();
        assert_eq!(places, vec![0.0, 0.2, 0.45, 0.75]);
        assert_eq!(current_prompt_mark(&tops, 0.0), 0);
        assert_eq!(current_prompt_mark(&tops, 420.0), 1);
        assert_eq!(current_prompt_mark(&tops, 2000.0), 3);
        // Equidistant: the earlier message stays current.
        assert_eq!(current_prompt_mark(&[0.0, 100.0], 50.0), 0);
    }

    #[test]
    fn jumps_with_ctrl_alt_up_and_down_only() {
        assert_eq!(prompt_rail_step(&key(true, true, false, false, Key::Down)), Some(1));
        assert_eq!(prompt_rail_step(&key(true, true, false, false, Key::Up)), Some(-1));
        // Alt+Up / Alt+Down move between conversations, and only when Ctrl is not held.
        assert_eq!(prompt_rail_step(&key(false, true, false, false, Key::Up)), None);
        assert_eq!(prompt_rail_step(&key(false, true, false, false, Key::Down)), None);
        // Talk is Ctrl+Alt+Space. Find is Ctrl+F. Pointing is Ctrl+Shift+C. Terminal is Ctrl+`.
        assert_eq!(prompt_rail_step(&key(true, true, false, false, Key::Text(" ".into()))), None);
        assert_eq!(prompt_rail_step(&key(true, false, false, true, Key::Text("c".into()))), None);
        assert_eq!(prompt_rail_step(&key(true, false, false, false, Key::Text("f".into()))), None);
        assert_eq!(prompt_rail_step(&key(true, false, false, false, Key::Text("`".into()))), None);
        assert_eq!(
            prompt_rail_motion(true),
            RailMotion {
                scroll_smooth: false,
                highlight: false
            }
        );
        assert_eq!(
            prompt_rail_motion(false),
            RailMotion {
                scroll_smooth: true,
                highlight: true
            }
        );
        assert_eq!(PROMPT_RAIL_KEYS, "Ctrl+Alt+Up and Ctrl+Alt+Down");
        // Mac says Option for Alt while keeping Control where Ctrl is listened for alone.
        assert_eq!(
            crate::keyboard_shortcuts::keys_on(PROMPT_RAIL_KEYS, "darwin", true),
            "Ctrl+Option+Up and Ctrl+Option+Down"
        );
    }

    #[test]
    fn takes_hover_line_from_message() {
        assert_eq!(
            prompt_rail_line("Where does the prompt rail sit?\nThe rest is underneath."),
            "Where does the prompt rail sit?"
        );
        assert_eq!(
            prompt_rail_line("**How far apart** are the marks?"),
            "How far apart are the marks?"
        );
        assert_eq!(
            prompt_rail_line("  \nWhat does a hover say?"),
            "What does a hover say?"
        );
        assert_eq!(
            prompt_rail_line("[the notes](notes.md) are the point"),
            "the notes are the point"
        );
        let long = "Where does a click land when the first line keeps going well past what a hover should show";
        let shortened = prompt_rail_line(long);
        assert!(shortened.ends_with('…'));
        assert!(shortened.chars().count() <= 72);
    }
}
