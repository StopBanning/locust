//! settings-read-like-claudes.test.ts (0.393), drawn: pages under group labels with an icon each; no card around a
//! setting, rows on hairlines; a setting that is one control drawn as one line; "How it works" in sentence case.
use super::*;
use crate::settings_pages::{SETTINGS_PAGES, SettingsPageId};
use rune::{ElementInfo, Env, Runtime};

fn view(s: &State, _: &mut Cx<State>) -> El {
    super::view(s, &Palette::new(true))
}

fn window(id: SettingsPageId) -> Runtime<State> {
    let mut s = State::default();
    s.settings_panel.open = true;
    s.settings_panel.page = Some(id);
    let mut rt = Runtime::new(s, view, (1200.0, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn shown(rt: &Runtime<State>) -> Vec<ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn keyed<'a>(all: &'a [ElementInfo], key: &str) -> &'a ElementInfo {
    all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}"))
}

/// The words drawn in the Settings list (the column left of the pane), top to bottom.
fn nav_words(all: &[ElementInfo]) -> Vec<&ElementInfo> {
    let pane = keyed(all, "settings-nav-app").rect;
    let mut words: Vec<_> = all
        .iter()
        .filter(|e| e.role == "text" && e.text.is_some() && e.rect[0] < pane[0] + pane[2] && e.rect[1] >= pane[1] - 40.0)
        .collect();
    words.sort_by(|a, b| a.rect[1].total_cmp(&b.rect[1]));
    words
}

#[test]
fn files_every_page_under_a_group_with_an_icon_the_set_can_draw() {
    let ids: Vec<_> = SETTINGS_PAGES.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["app", "chat", "appearance", "privacy", "teammates", "memory", "relay", "runtimes", "models", "connectors", "cloud", "workspace", "whatsnew"]
    );
    // settingsPages.ts's icon for each page, by its name in Icon.tsx.
    let icons: Vec<_> = SETTINGS_PAGES.iter().map(|p| p.icon.name).collect();
    assert_eq!(
        icons,
        ["settings", "message", "palette", "shield", "users", "brain", "message", "terminal", "chip", "plug", "cloud", "folder", "spark"]
    );
    for page in SETTINGS_PAGES {
        assert!(
            crate::look::icons::ALL.iter().any(|i| std::ptr::eq(*i, page.icon)),
            "{}: {} is not in the set",
            page.id.as_str(),
            page.icon.name
        );
    }
}

#[test]
fn keeps_each_group_together_in_order_so_every_label_is_said_once() {
    let groups: Vec<_> = SETTINGS_PAGES.iter().map(|p| p.group.label()).collect();
    let runs: Vec<_> = groups.iter().enumerate().filter(|(i, g)| *i == 0 || groups[i - 1] != **g).map(|(_, g)| *g).collect();
    assert_eq!(runs, ["Locust", "Team", "Agents", "This folder", "About"]);
    // Drawn: each group label once in the list.
    let all = shown(&window(SettingsPageId::App));
    let said: Vec<_> = nav_words(&all).into_iter().filter_map(|e| e.text.clone()).collect();
    for group in runs {
        assert_eq!(said.iter().filter(|w| *w == group).count(), 1, "{group} in {said:?}");
    }
}

#[test]
fn draws_the_group_label_above_its_first_page_and_the_icon_beside_each_name() {
    let all = shown(&window(SettingsPageId::App));
    let words = nav_words(&all);
    let top_of = |said: &str| words.iter().find(|e| e.text.as_deref() == Some(said)).map(|e| e.rect[1]);
    for (index, page) in SETTINGS_PAGES.iter().enumerate() {
        let entry = keyed(&all, &format!("settings-nav-{}", page.id.as_str()));
        let icon = keyed(&all, &format!("settings-navicon-{}", page.id.as_str()));
        let name = all
            .iter()
            .find(|e| e.text.as_deref() == Some(page.label) && e.parent.is_some_and(|p| all.get(p).is_some_and(|p| p.key == entry.key)))
            .or_else(|| all.iter().find(|e| e.text.as_deref() == Some(page.label) && e.rect[1] >= entry.rect[1] && e.rect[1] < entry.rect[1] + entry.rect[3]))
            .unwrap_or_else(|| panic!("no name for {}", page.label));
        assert_eq!((icon.rect[2], icon.rect[3]), (16.0, 16.0), "{}: the icon is 16 pt", page.label);
        assert!(icon.rect[0] + icon.rect[2] <= name.rect[0], "{}: the icon is before the name", page.label);
        let (icon_mid, name_mid) = (icon.rect[1] + icon.rect[3] / 2.0, name.rect[1] + name.rect[3] / 2.0);
        assert!((icon_mid - name_mid).abs() < 4.0, "{}: beside it, on its line", page.label);
        assert!(icon.rect[1] >= entry.rect[1] && icon.rect[1] + icon.rect[3] <= entry.rect[1] + entry.rect[3]);
        if index == 0 || SETTINGS_PAGES[index - 1].group != page.group {
            let label = top_of(page.group.label()).unwrap_or_else(|| panic!("no {} label", page.group.label()));
            assert!(label < entry.rect[1], "{} is above {}", page.group.label(), page.label);
            if index > 0 {
                let before = keyed(&all, &format!("settings-nav-{}", SETTINGS_PAGES[index - 1].id.as_str()));
                assert!(label > before.rect[1], "{} comes after {}", page.group.label(), SETTINGS_PAGES[index - 1].label);
            }
        }
    }
}

/// The one-line settings Rust draws, on the page each lives on: the line's key, and the key or words of its control.
const LINES: &[(SettingsPageId, &str, &str, &str)] = &[
    (SettingsPageId::Teammates, "Auto mode", "settings-line-auto-mode", "settings-auto-mode"),
    (SettingsPageId::Appearance, "Reply text size", "settings-line-reply-size", "settings-reply-size-largest"),
    (SettingsPageId::Appearance, "Code font", "settings-line-code-font", "settings-code-font-paper"),
    (SettingsPageId::Appearance, "Terminal faces", "settings-line-terminal-faces", "settings-terminal-faces"),
    (SettingsPageId::Appearance, "What a face says", "settings-face-legend-row", "settings-face-legend"),
    (SettingsPageId::Appearance, "Sidebar", "settings-line-sidebar", "Rail"),
];

#[test]
fn puts_no_card_around_a_setting_rows_sit_on_the_pane_with_a_hairline_between() {
    let hairline = crate::look::controls::Look::new(true).c("border-hairline");
    for (page, name, key, _) in LINES {
        let all = shown(&window(*page));
        let line = keyed(&all, key);
        let row = keyed(&all, &format!("{key}-row"));
        let rule = keyed(&all, &format!("{key}-hairline"));
        for (what, e) in [("line", line), ("row", row)] {
            assert_eq!(e.fill.a, 0.0, "{name}: no card, the {what} sits on the pane");
            assert_eq!(e.radius, 0.0, "{name}: no rounded box around the {what}");
        }
        assert_eq!(rule.fill, hairline, "{name}: the hairline");
        assert_eq!(rule.rect[3], 1.0, "{name}: one point");
        assert_eq!(rule.rect[2], line.rect[2], "{name}: across the line");
        assert!(rule.rect[1] <= row.rect[1], "{name}: the hairline is above its row");
    }
    // And no page boxes a setting in: nothing in the pane wider than a field is a filled, rounded card (Startup,
    // Report a problem and each chat app were). Fields, 32 pt tall, are controls, not cards.
    for page in SETTINGS_PAGES {
        let all = shown(&window(page.id));
        let pane = keyed(&all, "settings-nav-app").rect;
        let cards: Vec<_> = all
            .iter()
            .filter(|e| e.rect[0] >= pane[0] + pane[2] && e.rect[2] > 400.0 && e.rect[3] > 40.0 && e.radius > 0.0 && e.fill.a > 0.0)
            .map(|e| (e.key.clone(), e.rect))
            .collect();
        assert!(cards.is_empty(), "{}: a card around a setting {cards:?}", page.id.as_str());
    }
}

// Plans, Your skills, Boot screen and Voice typing are not drawn in Rust ("runs in Locust for now"); the six that are
// drawn are held here.
#[test]
fn draws_a_setting_that_is_one_control_as_one_line_name_and_description_left_the_control_right() {
    for (page, name, key, control) in LINES {
        let all = shown(&window(*page));
        let row = keyed(&all, &format!("{key}-row"));
        let heading = all
            .iter()
            .find(|e| e.text.as_deref() == Some(*name) && e.role == "heading")
            .unwrap_or_else(|| panic!("no heading {name}"));
        let control = all
            .iter()
            .find(|e| e.key.as_deref() == Some(*control) || (e.key.is_none() && e.text.as_deref() == Some(*control)))
            .unwrap_or_else(|| panic!("no control for {name}"));
        let inside = |e: &ElementInfo| e.rect[1] >= row.rect[1] && e.rect[1] + e.rect[3] <= row.rect[1] + row.rect[3] + 0.5;
        assert!(inside(heading) && inside(control), "{name}: name and control on the one line");
        assert!(heading.rect[0] < row.rect[0] + row.rect[2] / 2.0, "{name}: the name is on the left");
        assert!(control.rect[0] > heading.rect[0] + heading.rect[2], "{name}: the control is right of the words");
        assert!(control.rect[0] > row.rect[0] + row.rect[2] / 2.0, "{name}: the control is on the right");
        let control_mid = control.rect[1] + control.rect[3] / 2.0;
        let row_mid = row.rect[1] + row.rect[3] / 2.0;
        assert!((control_mid - row_mid).abs() < 6.0, "{name}: the control is centred on the line ({control_mid} vs {row_mid})");
    }
}

#[test]
fn says_how_it_works_in_sentence_case() {
    let mut rt = window(SettingsPageId::Teammates);
    let all = shown(&rt);
    let summary = keyed(&all, "how-it-works-auto-mode");
    assert_eq!(summary.label.as_deref(), Some("How it works"));
    assert_eq!(summary.expanded, Some(false), "closed until pressed");
    assert!(all.iter().any(|e| e.text.as_deref() == Some("How it works")), "said as a sentence says it");
    assert!(!all.iter().any(|e| e.text.as_deref().is_some_and(|t| t.eq_ignore_ascii_case("how it works") && t != "How it works")));
    assert!(!all.iter().any(|e| e.key.as_deref() == Some("how-it-works-auto-mode-text")));
    assert!(keyed(&all, "how-it-works-auto-mode").rect[1] > keyed(&all, "settings-line-auto-mode").rect[1], "under Auto mode");

    let press = |rt: &mut Runtime<State>| {
        let (x, y) = rt.find("how-it-works-auto-mode").expect("the summary");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
        rt.frame(1.0);
    };
    press(&mut rt);
    let all = shown(&rt);
    assert_eq!(keyed(&all, "how-it-works-auto-mode").expanded, Some(true));
    let said: String = all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    assert!(said.contains("Every other mode confines a run to the folder you chose for it"), "{said}");
    assert!(said.contains("switching it off here stops the next run, including one a teammate or a routine was about to start"));
    press(&mut rt);
    assert!(!shown(&rt).iter().any(|e| e.key.as_deref() == Some("how-it-works-auto-mode-text")), "a second press folds it");
}

/// Where Electron says "How it works" (`<More>`) under a section Rust draws, and the page it is on.
const FOLDED: &[(&str, SettingsPageId)] = &[
    ("auto-mode", SettingsPageId::Teammates),
    ("report-a-problem", SettingsPageId::App),
    ("between-teammates", SettingsPageId::Relay),
    ("own-models", SettingsPageId::Models),
    ("github", SettingsPageId::Connectors),
    ("memory", SettingsPageId::Memory),
    ("chat-apps", SettingsPageId::App),
    ("privacy", SettingsPageId::Privacy),
    ("updates", SettingsPageId::App),
];

// Not an Electron case of its own: the fold the sentence-case case holds on Auto mode, held wherever it is used.
#[test]
fn each_section_electron_folds_says_how_it_works_under_it_and_opens_on_its_own_words() {
    let keys: Vec<_> = crate::how_it_works::FOLDS.iter().map(|(k, _, _)| *k).collect();
    assert_eq!(keys, FOLDED.iter().map(|(k, _)| *k).collect::<Vec<_>>());
    for (key, page) in FOLDED {
        let (_, heading, words) = crate::how_it_works::FOLDS.iter().find(|(k, _, _)| k == key).unwrap();
        assert!(crate::settings_pages::page_by_id(*page).headings.contains(heading), "{key}: under {heading}");
        assert!(!words.is_empty());
        let mut s = State::default();
        s.settings_panel.open = true;
        s.settings_panel.page = Some(*page);
        let mut rt = Runtime::new(s, view, (1200.0, 9000.0), Env::new(true, 1.0));
        rt.frame(0.0);
        let summary = format!("how-it-works-{key}");
        let all = shown(&rt);
        assert_eq!(keyed(&all, &summary).expanded, Some(false), "{key}: closed until pressed");
        let (x, y) = rt.find(&summary).unwrap_or_else(|| panic!("{key}: no fold on screen"));
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
        rt.frame(1.0);
        let all = shown(&rt);
        let said: Vec<_> = all.iter().filter_map(|e| e.text.clone()).collect();
        for paragraph in *words {
            assert!(said.iter().any(|t| t == paragraph), "{key}: {paragraph}");
        }
        // Only this one opened.
        let open = all.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("how-it-works-")) && e.expanded == Some(true)).count();
        assert_eq!(open, 1, "{key}");
    }
}

