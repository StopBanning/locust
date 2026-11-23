//! A RIGHT-CLICK MENU STAYS IN THE WINDOW, and reads the way Claude's does (ContextMenu.tsx).
//!
//! Colin, 2026-09-22, with his menu beside the one the Claude app draws: "right click folding under window, also can
//! we clean it up a little bit like the one you use?" So: placed by `route_rows::place_menu` (flipped to the other side
//! of the pointer along whichever axis it would cross, held off the edge when neither side has room); no title bar,
//! what was clicked is the menu's accessible name; short rows with a real one-letter key drawn at the end; a hairline
//! between kinds of action; Delete in red and asking in place ("Delete for good?") before it acts; lists behind `›`;
//! a row that cannot be used now is greyed with its reason said out loud, never an arrow into a list that would
//! refuse. It closes on Escape and on a press anywhere else.

use super::*;
use locust_core::route_rows::place_menu;

/// The menu's width: Electron's `min-width: 208px` plus its 4px padding each side, rounded as its test measures.
pub(super) const MENU_WIDTH: f32 = 220.0;
const SUB_WIDTH: f32 = 188.0;
const PAD: f32 = 4.0;
const ROW_H: f32 = 28.0;
const WHY_H: f32 = 17.0;
const SEP_H: f32 = 9.0;
/// How close a menu may come to the window's edge (ContextMenu.tsx `EDGE`).
const EDGE: f32 = 8.0;

/// One row (ContextMenuItem).
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Item {
    pub(super) label: String,
    /// A real key while the menu is open, drawn upper case at the row's end.
    pub(super) shortcut: Option<char>,
    /// A list behind the row; choosing the row means choosing from it.
    pub(super) submenu: Option<Vec<Item>>,
    pub(super) checked: bool,
    /// Shown instead of `label` once pressed once: the second press acts.
    pub(super) confirm_label: Option<String>,
    pub(super) danger: bool,
    pub(super) divider_above: bool,
    pub(super) disabled_reason: Option<String>,
    /// What choosing it does, handed to `run`.
    pub(super) action: String,
}

impl Item {
    pub(super) fn new(label: &str, action: &str) -> Item {
        Item { label: label.into(), action: action.into(), ..Item::default() }
    }
    pub(super) fn key(mut self, c: char) -> Item {
        self.shortcut = Some(c);
        self
    }
    pub(super) fn divided(mut self) -> Item {
        self.divider_above = true;
        self
    }
    pub(super) fn danger(mut self, confirm: &str) -> Item {
        self.danger = true;
        self.confirm_label = Some(confirm.into());
        self
    }
    pub(super) fn checked(mut self, on: bool) -> Item {
        self.checked = on;
        self
    }
    pub(super) fn list(mut self, entries: Vec<Item>) -> Item {
        self.submenu = Some(entries);
        self
    }
    pub(super) fn refused(mut self, why: Option<&str>) -> Item {
        self.disabled_reason = why.map(str::to_owned);
        self
    }
}

/// An open menu (ContextMenuState, with the armed item and the open list).
#[derive(Clone, Debug, Default)]
pub(super) struct MenuState {
    pub(super) x: f32,
    pub(super) y: f32,
    /// What was right-clicked: the menu's accessible name, not drawn.
    pub(super) title: String,
    /// The conversation it acts on.
    pub(super) target: String,
    pub(super) items: Vec<Item>,
    pub(super) armed: Option<String>,
    pub(super) open_sub: Option<String>,
}

fn slug(label: &str) -> String {
    label
        .chars()
        .filter_map(|c| if c.is_ascii_alphanumeric() { Some(c.to_ascii_lowercase()) } else if c == ' ' { Some('-') } else { None })
        .collect()
}

fn row_height(item: &Item) -> f32 {
    if item.disabled_reason.is_some() { ROW_H + WHY_H } else { ROW_H }
}

/// The menu's size before it has been measured: rows, hairlines and padding.
pub(super) fn estimated_size(items: &[Item]) -> (f32, f32) {
    let mut h = PAD * 2.0 + 2.0;
    for (i, item) in items.iter().enumerate() {
        if item.divider_above && i > 0 {
            h += SEP_H;
        }
        h += row_height(item);
    }
    (MENU_WIDTH, h)
}

/// Where the row of `index` starts below the menu's top.
fn row_top(items: &[Item], index: usize) -> f32 {
    let mut y = PAD + 1.0;
    for (i, item) in items.iter().enumerate() {
        if item.divider_above && i > 0 {
            y += SEP_H;
        }
        if i == index {
            break;
        }
        y += row_height(item);
    }
    y
}

/// The right-click menu for one sidebar conversation (App.tsx `openMissionMenu`), with the actions this window has.
pub(super) fn conversation_menu(s: &State, mission_id: &str, title: &str, at: (f32, f32)) -> MenuState {
    let missions = sidebar::recent_as_missions(s);
    let row = missions.iter().find(|m| m.mission_id == mission_id);
    let keys = row.map(conversation_list::conversation_keys).unwrap_or_else(|| vec![mission_id.to_owned()]);
    let pinned = s.org.pins.iter().any(|k| keys.contains(k));
    let settled = row.is_some_and(|m| conversation_list::held_for(m, &s.org.settled).is_some());
    let live = row.is_some_and(|m| m.phase == "running");
    let group_of = keys.iter().find_map(|k| s.org.members.get(k)).map(|m| m.group_id.clone());
    let mut items = vec![
        Item::new("Open", "open").key('o'),
        Item::new("Open beside", "beside").key('b'),
        Item::new("Rename", "rename").key('r'),
        Item::new(if pinned { "Unpin" } else { "Pin to top" }, "pin").key('p'),
    ];
    if settled {
        items.push(Item::new("Bring back", "settle").key('k'));
    } else {
        items.push(Item::new("Settle", "settle").key('t'));
        items.push(Item::new("Snooze", "").list(
            settled::snooze_choices(chrono::Local::now())
                .into_iter()
                .map(|choice| Item::new(&choice.label, &format!("snooze:{}", choice.until)))
                .collect(),
        ));
    }
    let mut groups: Vec<Item> = s
        .org
        .groups
        .iter()
        .map(|g| Item::new(&g.name, &format!("group:{}", g.group_id)).checked(group_of.as_deref() == Some(g.group_id.as_str())))
        .collect();
    groups.push(Item::new("Ungrouped", "group:").checked(group_of.is_none()));
    groups.push(Item::new("New project…", "new-project"));
    items.push(Item::new("Move to project", "").divided().list(groups));
    items.push(Item::new("Save the record…", "save-record").key('e').divided());
    items.push(Item::new("Copy its record id", "copy-id").key('c'));
    items.push(
        Item::new("Delete", "delete")
            .key('d')
            .divided()
            .danger("Delete for good?")
            .refused(live.then_some("This is still running. Stop it first.")),
    );
    MenuState { x: at.0, y: at.1, title: title.to_owned(), target: mission_id.to_owned(), items, armed: None, open_sub: None }
}

/// Does what `item` says (ContextMenu.tsx `choose`): a refused row or a list does nothing; a destructive row arms
/// on its first press and acts on its second.
pub(super) fn choose(s: &mut State, item: &Item, tx: &mut Tx) {
    let Some(menu) = s.row_menu.as_mut() else { return };
    if item.disabled_reason.is_some() {
        return;
    }
    if item.submenu.is_some() {
        // Opened by the press as by hover; a press does not close what hovering just opened under it.
        menu.open_sub = Some(item.label.clone());
        return;
    }
    if item.confirm_label.is_some() && menu.armed.as_deref() != Some(item.label.as_str()) {
        menu.armed = Some(item.label.clone());
        return;
    }
    let target = menu.target.clone();
    s.row_menu = None;
    run(s, &target, &item.action, tx);
}

fn run(s: &mut State, target: &str, action: &str, tx: &mut Tx) {
    if crate::header_menu::run(s, action) {
        return;
    }
    if crate::rail::run_needs_you_action(s, action, tx) {
        return;
    }
    match action {
        "" => {}
        "open" => peer_thread::open_mission_by_id(s, target.to_owned(), tx),
        "copy-id" => tx.copy(target.to_owned()),
        other => sidebar::apply_org_action(s, target, other),
    }
}

/// A letter chooses the top-level row that shows it; Left closes an open list first.
fn on_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    let Some(menu) = s.row_menu.as_ref() else { return false };
    if matches!(event.key, Key::Left) && menu.open_sub.is_some() {
        if let Some(m) = s.row_menu.as_mut() {
            m.open_sub = None;
        }
        return true;
    }
    let Key::Text(typed) = &event.key else { return false };
    if event.mods.ctrl || event.mods.alt || event.mods.cmd {
        return false;
    }
    let mut chars = typed.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else { return false };
    let Some(item) = menu.items.iter().find(|i| i.shortcut.is_some_and(|k| k.eq_ignore_ascii_case(&c))).cloned() else {
        return false;
    };
    choose(s, &item, tx);
    true
}

fn close(s: &mut State) {
    s.row_menu = None;
}

fn sub_panel(l: &crate::look::controls::Look, menu: &MenuState, parent: &Item, at: (f32, f32), win: (f32, f32)) -> El {
    let entries = parent.submenu.clone().unwrap_or_default();
    let h = PAD * 2.0 + 2.0 + entries.len() as f32 * ROW_H;
    // To the right of its row, to the left when the right would cross the window's edge; lifted by however much it
    // would hang below the bottom.
    let left = if at.0 + SUB_WIDTH > win.0 - EDGE { at.0 - MENU_WIDTH - SUB_WIDTH + 2.0 } else { at.0 };
    let below = at.1 + h - (win.1 - EDGE);
    let top = if below > 0.0 { at.1 - below.min((at.1 - EDGE).max(0.0)) } else { at.1 };
    let mut panel = surface(l, col().key(format!("row-menu-sub-{}", slug(&parent.label))).role("menu").label(parent.label.clone()))
        .absolute(left, top)
        .w(SUB_WIDTH);
    let _ = menu;
    for entry in entries {
        let pick = entry.clone();
        let mut r = item_row(l, &format!("row-menu-sub-item-{}", slug(&entry.label)), &entry.label, false)
            .role("menuitemradio")
            .checked(entry.checked)
            .on_press_tx(move |s: &mut State, tx| {
                let Some(menu) = s.row_menu.take() else { return };
                run(s, &menu.target, &pick.action, tx);
            });
        if entry.checked {
            r = r.child(text("✓").key(format!("row-menu-tick-{}", slug(&entry.label))).font(l.ty.label).color(l.c("text-muted")).a11y_hidden());
        }
        panel = panel.child(r);
    }
    panel
}

fn surface(l: &crate::look::controls::Look, e: El) -> El {
    let mut e = e.padding(PAD).radius(10.0).fill(l.c("bg-raised")).border(1.0, l.c("border-strong"));
    for shadow in l.t.shadows("shadow-dropdown") {
        e = e.box_shadow(shadow);
    }
    e
}

fn item_row(l: &crate::look::controls::Look, key: &str, label: &str, danger: bool) -> El {
    let ink = if danger { l.c("red-text") } else { l.c("text-primary") };
    row()
        .key(key.to_owned())
        .role("menuitem")
        .label(label.to_owned())
        .w_full()
        .min_h(ROW_H)
        .px(10.0)
        .gap(16.0)
        .radius(6.0)
        .center_y()
        .hover(if danger { l.c("red-tint") } else { l.c("bg-selected") })
        .child(text(label.to_owned()).key(format!("{key}-label")).font(l.ty.control).color(ink).grow(1.0).shrink().truncate())
}

/// The open menu, drawn over the whole window: a press anywhere else closes it, Escape closes it, and it sits where
/// `place_menu` puts a menu of `measured` size (or its estimate, before it has been laid out) in a window `win` big.
pub(super) fn view(s: &State, p: &Palette, win: (f32, f32), measured: Option<(f32, f32)>) -> Option<El> {
    let menu = s.row_menu.as_ref()?;
    let l = crate::look::controls::Look::new(p.dark);
    let size = measured.unwrap_or_else(|| estimated_size(&menu.items));
    let (left, top) = place_menu((menu.x, menu.y), size, win);
    let mut list = surface(&l, col().key("row-menu").role("menu").label(menu.title.clone()))
        .absolute(left, top)
        .w(MENU_WIDTH)
        .on_key(on_key);
    let mut sub = None;
    for (i, item) in menu.items.iter().enumerate() {
        if item.divider_above && i > 0 {
            list = list.child(
                el().key(format!("row-menu-sep-{i}")).role("separator").h(1.0).mx(6.0).my(4.0).fill(l.c("border-hairline")),
            );
        }
        let key = format!("row-menu-item-{}", slug(&item.label));
        let chosen = item.clone();
        if item.submenu.is_some() && item.disabled_reason.is_none() {
            // A row that opens a list rather than doing something: on hover as on press, as a submenu does everywhere.
            let open = menu.open_sub.as_deref() == Some(item.label.as_str());
            let hovered = item.label.clone();
            let mut r = item_row(&l, &key, &item.label, false)
                .expanded(open)
                .child(text("›").key(format!("{key}-more")).font(l.ty.label).color(l.c("text-muted")).a11y_hidden())
                .on_hover_change(move |s: &mut State, over, _| {
                    if over && let Some(m) = s.row_menu.as_mut() {
                        m.open_sub = Some(hovered.clone());
                    }
                })
                .on_press_tx(move |s: &mut State, tx| choose(s, &chosen, tx));
            if open {
                r = r.fill(l.c("bg-selected"));
                sub = Some(sub_panel(&l, menu, item, (left + MENU_WIDTH - 1.0, top + row_top(&menu.items, i) - PAD - 1.0), win));
            }
            list = list.child(r);
            continue;
        }
        let armed = menu.armed.as_deref() == Some(item.label.as_str());
        let shown = if armed { item.confirm_label.clone().unwrap_or_else(|| item.label.clone()) } else { item.label.clone() };
        let refused = item.disabled_reason.is_some();
        let mut r = item_row(&l, &key, &shown, item.danger && !refused);
        if let Some(c) = item.shortcut.filter(|_| !refused) {
            r = r.shortcut_hint(c.to_ascii_uppercase().to_string()).child(
                text(c.to_ascii_uppercase().to_string()).key(format!("{key}-key")).font(l.ty.mono).color(l.c("text-muted")).a11y_hidden(),
            );
        }
        if let Some(why) = &item.disabled_reason {
            // Said out loud, not left in a tooltip: a greyed row whose reason only shows on hover reads as broken.
            r = col()
                .key(key.clone())
                .role("menuitem")
                .label(item.label.clone())
                .description(why.clone())
                .disabled(true)
                .w_full()
                .min_h(ROW_H)
                .px(10.0)
                .py(6.0)
                .gap(1.0)
                .child(text(item.label.clone()).key(format!("{key}-label")).font(l.ty.control).color(l.c("text-muted")).truncate())
                .child(text(why.clone()).key(format!("{key}-why")).font(l.ty.body).color(l.c("text-muted")).wrap());
        } else {
            r = r.focusable(true).on_press_tx(move |s: &mut State, tx| choose(s, &chosen, tx));
        }
        list = list.child(r);
    }
    Some(
        el().key("row-menu-layer")
            .role("none")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .child(
                el().key("row-menu-scrim")
                    .role("none")
                    .absolute(0.0, 0.0)
                    .w_full()
                    .h_full()
                    .on_press(close)
                    .on_secondary_press(|s: &mut State, _, _| close(s)),
            )
            .child(list)
            .children(sub)
            .modal(false, close),
    )
}

#[cfg(test)]
#[path = "context_menu_tests.rs"]
mod tests;
