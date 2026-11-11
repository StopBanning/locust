//! Categorized Settings screen — Area 11 / P10.

use super::*;
use crate::settings_pages::{
    self, HOW_IT_WORKS, RUNS_IN_LOCUST, SEARCH_EMPTY, SEARCH_PLACEHOLDER, SETTINGS_PAGES,
    SETTINGS_TITLE, SettingsGroup, SettingsPageId,
};
use crate::workspace_settings::{
    self, DATA_FOLDER_HEADING, MEMORY_ASK, MEMORY_KEEP, MEMORY_OFF, MEMORY_WHEN,
    PROJECT_FOLDER_LEDE, THEME_HEADING, ThemeChoice,
};

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub page: Option<SettingsPageId>,
    pub search: TextEdit,
    pub settings: workspace_settings::WorkspaceSettings,
    pub theme: ThemeChoice,
    pub data_folder: String,
    pub error: String,
}

fn button(label: impl Into<String>, p: &Palette) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.0)
}

fn active_button(label: impl Into<String>, p: &Palette, on: bool) -> El {
    text(label.into())
        .font(type_::LABEL)
        .color(if on { p.text } else { p.secondary })
        .padding(7.0)
        .fill(if on { p.selected } else { p.field })
        .radius(6.0)
}

pub(super) fn open(s: &mut State) {
    s.settings_panel.open = true;
    s.settings_panel.page = SettingsPageId::from_str("appearance");
    reload(s);
}

fn reload(s: &mut State) {
    match &s.data {
        None => {
            s.settings_panel.error = "Native data folder could not be determined.".into();
        }
        Some(store) => {
            s.settings_panel.data_folder = store.root().display().to_string();
            match workspace_settings::load(store) {
                Ok(settings) => {
                    s.settings_panel.settings = settings;
                    s.settings_panel.error.clear();
                }
                Err(e) => s.settings_panel.error = e,
            }
            s.settings_panel.theme = workspace_settings::load_theme(store);
            s.appearance = s.settings_panel.theme.to_appearance();
            // Round-trip keeps State.appearance and the stored theme aligned.
            s.settings_panel.theme = ThemeChoice::from_appearance(s.appearance);
        }
    }
}

fn persist(s: &mut State) {
    let Some(store) = s.data.clone() else {
        s.settings_panel.error = "Native data folder could not be determined.".into();
        return;
    };
    match workspace_settings::save(&store, &s.settings_panel.settings) {
        Ok(()) => {
            s.settings_panel.error.clear();
            // memoryMode is read by people::brief and memory acceptance.
        }
        Err(e) => s.settings_panel.error = e,
    }
}

fn set_theme(s: &mut State, theme: ThemeChoice) {
    s.settings_panel.theme = theme;
    s.appearance = theme.to_appearance();
    if let Some(store) = &s.data
        && let Err(e) = workspace_settings::save_theme(store, theme)
    {
        s.settings_panel.error = e;
    }
}

fn deferred(heading: &str, p: &Palette) -> El {
    col()
        .w_full()
        .gap(4.0)
        .padding(10.0)
        .child(text(heading.to_owned()).font(type_::TITLE).color(p.text))
        .child(
            text(RUNS_IN_LOCUST.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap(),
        )
}

fn page_body(s: &State, p: &Palette, page: SettingsPageId) -> El {
    let query = s.settings_panel.search.text.trim().to_owned();
    let meta = settings_pages::page_by_id(page);
    let headings: Vec<&str> = if query.is_empty() {
        meta.headings.to_vec()
    } else {
        settings_pages::matched_headings(meta, &query)
    };
    let mut body = col().w_full().gap(8.0).padding(12.0);
    body = body.child(text(meta.label.to_owned()).font(type_::TITLE).color(p.text));

    match page {
        SettingsPageId::Appearance => {
            // Theme is drawn on Appearance (Electron ThemeSetting), even though
            // it is not in the search-index headings list.
            if query.is_empty()
                || settings_pages::matches("theme", &query)
                || settings_pages::matches("dark", &query)
                || settings_pages::matches("light", &query)
            {
                let theme = s.settings_panel.theme;
                body = body.child(
                    col()
                        .w_full()
                        .gap(6.0)
                        .padding(10.0)
                        .child(
                            text(THEME_HEADING.to_owned())
                                .font(type_::TITLE)
                                .color(p.text),
                        )
                        .child(
                            text(workspace_settings::theme_lede(
                                theme,
                                Some(p.bg == Palette::new(true).bg),
                            ))
                            .font(type_::LABEL)
                            .color(p.secondary)
                            .wrap(),
                        )
                        .child(
                            row()
                                .gap(6.0)
                                .child(
                                    active_button(
                                        ThemeChoice::Dark.label(),
                                        p,
                                        theme == ThemeChoice::Dark,
                                    )
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::Dark)),
                                )
                                .child(
                                    active_button(
                                        ThemeChoice::Light.label(),
                                        p,
                                        theme == ThemeChoice::Light,
                                    )
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::Light)),
                                )
                                .child(
                                    active_button(
                                        ThemeChoice::System.label(),
                                        p,
                                        theme == ThemeChoice::System,
                                    )
                                    .on_press(|s: &mut State| set_theme(s, ThemeChoice::System)),
                                ),
                        ),
                );
            }
            for heading in headings {
                body = body.child(deferred(heading, p));
            }
        }
        SettingsPageId::Teammates => {
            for heading in headings {
                match heading {
                    "Saved approvals" => {
                        body = body
                            .child(
                                text(
                                    crate::approval_rule_store::SAVED_APPROVALS_HEADING.to_owned(),
                                )
                                .font(type_::TITLE)
                                .color(p.text)
                                .padding(10.0),
                            )
                            .child(
                                text(crate::approval_rule_store::SAVED_APPROVALS_LEDE.to_owned())
                                    .font(type_::LABEL)
                                    .color(p.secondary)
                                    .wrap()
                                    .padding(10.0),
                            )
                            .child(button("Open saved approvals", p).on_press(|s: &mut State| {
                                s.settings_panel.open = false;
                                crate::people_panel::open(s);
                                s.people_panel.tab = 2;
                                s.people_panel.memory_tab = false;
                            }));
                    }
                    _ => body = body.child(deferred(heading, p)),
                }
            }
        }
        SettingsPageId::Memory => {
            for heading in headings {
                if heading == "What your team remembers" {
                    let mode = s.settings_panel.settings.memory_mode.as_str();
                    body = body.child(
                        col()
                            .w_full()
                            .gap(6.0)
                            .padding(10.0)
                            .child(text(heading.to_owned()).font(type_::TITLE).color(p.text))
                            .child(
                                text(MEMORY_WHEN.to_owned())
                                    .font(type_::LABEL)
                                    .color(p.secondary),
                            )
                            .child(
                                row()
                                    .gap(6.0)
                                    .child(active_button(MEMORY_KEEP, p, mode == "auto").on_press(
                                        |s: &mut State| {
                                            s.settings_panel.settings.memory_mode = "auto".into();
                                            persist(s);
                                        },
                                    ))
                                    .child(active_button(MEMORY_ASK, p, mode == "ask").on_press(
                                        |s: &mut State| {
                                            s.settings_panel.settings.memory_mode = "ask".into();
                                            persist(s);
                                        },
                                    ))
                                    .child(active_button(MEMORY_OFF, p, mode == "off").on_press(
                                        |s: &mut State| {
                                            s.settings_panel.settings.memory_mode = "off".into();
                                            persist(s);
                                        },
                                    )),
                            ),
                    );
                } else {
                    body = body.child(deferred(heading, p));
                }
            }
        }
        SettingsPageId::Workspace => {
            body = body
                .child(
                    text(PROJECT_FOLDER_LEDE.to_owned())
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .wrap()
                        .padding(10.0),
                )
                .child(
                    col()
                        .w_full()
                        .gap(4.0)
                        .padding(10.0)
                        .child(text(DATA_FOLDER_HEADING.to_owned()).font(type_::TITLE).color(p.text))
                        .child(
                            text(if s.settings_panel.data_folder.is_empty() {
                                "No folder chosen".into()
                            } else {
                                s.settings_panel.data_folder.clone()
                            })
                            .font(type_::LABEL)
                            .color(p.text)
                            .wrap(),
                        )
                        .child(
                            text(if s.settings_panel.data_folder.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "id {}",
                                    workspace_settings::workspace_id_for(std::path::Path::new(
                                        &s.settings_panel.data_folder
                                    ))
                                )
                            })
                            .font(type_::LABEL)
                            .color(p.muted)
                            .wrap(),
                        )
                        .child(
                            text("Native keeps its own settings copy here, seeded once from Electron. Electron's folder is never written.")
                                .font(type_::LABEL)
                                .color(p.muted)
                                .wrap(),
                        ),
                );
            for heading in headings {
                body = body.child(deferred(heading, p));
            }
        }

        _ => {
            for heading in headings {
                body = body.child(deferred(heading, p));
            }
        }
    }

    body = body.child(
        text(HOW_IT_WORKS.to_owned())
            .font(type_::LABEL)
            .color(p.muted)
            .padding(10.0),
    );
    body
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let query = s.settings_panel.search.text.trim().to_owned();
    let mut nav = col().w(200.0).gap(2.0).padding(8.0);
    let mut last_group: Option<SettingsGroup> = None;
    for page in SETTINGS_PAGES {
        if !query.is_empty() && !settings_pages::page_matches(page, &query) {
            continue;
        }
        if last_group != Some(page.group) {
            last_group = Some(page.group);
            nav = nav.child(
                text(page.group.label().to_owned())
                    .font(type_::LABEL)
                    .color(p.muted)
                    .px(8.0)
                    .py(6.0),
            );
        }
        let id = page.id;
        let selected = s.settings_panel.page == Some(id);
        nav = nav.child(active_button(page.label, p, selected).w_full().on_press(
            move |s: &mut State| {
                s.settings_panel.page = Some(id);
            },
        ));
    }
    if last_group.is_none() {
        nav = nav.child(
            text(SEARCH_EMPTY.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap()
                .padding(8.0),
        );
    }

    let page = s
        .settings_panel
        .page
        .filter(|id| {
            query.is_empty()
                || settings_pages::page_matches(settings_pages::page_by_id(*id), &query)
        })
        .or_else(|| {
            SETTINGS_PAGES
                .iter()
                .find(|page| query.is_empty() || settings_pages::page_matches(page, &query))
                .map(|p| p.id)
        });

    let mut content = col().grow(1.0).min_h(400.0).scroll(ScrollSpec {
        stick_bottom: false,
    });
    if let Some(page) = page {
        // as_str keeps the page id vocabulary tied to SETTINGS_PAGES.
        let _page_id = page.as_str();
        content = content.child(page_body(s, p, page));
    } else {
        content = content.child(
            text(SEARCH_EMPTY.to_owned())
                .font(type_::LABEL)
                .color(p.muted)
                .wrap()
                .padding(16.0),
        );
    }

    let mut shell = col()
        .w_full()
        .h_full()
        .fill(p.bg)
        .child(
            row()
                .w_full()
                .padding(12.0)
                .gap(8.0)
                .center_y()
                .child(
                    text(SETTINGS_TITLE.to_owned())
                        .font(type_::TITLE)
                        .color(p.text)
                        .grow(1.0),
                )
                .child(button("Close", p).on_press(|s: &mut State| s.settings_panel.open = false)),
        )
        .child(
            col().w_full().padding(12.0).child(
                text_input(&s.settings_panel.search, type_::BODY)
                    .key("settings-search")
                    .w_full()
                    .padding(10.0)
                    .radius(6.0)
                    .fill(p.field)
                    .color(p.text)
                    .placeholder(SEARCH_PLACEHOLDER, p.muted)
                    .on_edit(|s: &mut State| &mut s.settings_panel.search),
            ),
        );
    if !s.settings_panel.error.is_empty() {
        shell = shell.child(
            text(s.settings_panel.error.clone())
                .font(type_::LABEL)
                .color(p.red)
                .wrap()
                .padding(12.0),
        );
    }
    shell.child(
        row()
            .w_full()
            .grow(1.0)
            .child(nav.fill(p.sidebar).h_full())
            .child(content.fill(p.bg).h_full()),
    )
}

pub(super) fn scene(s: &mut State) {
    team_scene(s);
    s.appearance = Some(true);
    s.settings_panel = Panel {
        open: true,
        page: Some(SettingsPageId::Appearance),
        theme: ThemeChoice::Dark,
        data_folder: "/Users/locust/Library/Application Support/Locust-native".into(),
        settings: workspace_settings::WorkspaceSettings::default(),
        ..Panel::default()
    };
}

pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
    s.settings_panel.theme = ThemeChoice::Light;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_settings_action_keeps_electron_fingerprint() {
        use std::{
            fs,
            path::{Path, PathBuf},
            time::SystemTime,
        };
        fn fingerprint(at: &Path) -> Vec<(PathBuf, u64, SystemTime, Vec<u8>)> {
            let mut entries: Vec<_> = fs::read_dir(at).unwrap().map(Result::unwrap).collect();
            entries.sort_by_key(|e| e.path());
            let mut out = Vec::new();
            for e in entries {
                let m = e.metadata().unwrap();
                out.push((
                    e.path(),
                    m.len(),
                    m.modified().unwrap(),
                    if m.is_file() {
                        fs::read(e.path()).unwrap()
                    } else {
                        Vec::new()
                    },
                ));
                if m.is_dir() {
                    out.extend(fingerprint(&e.path()));
                }
            }
            out
        }
        let root = std::env::temp_dir().join(format!("settings-review-{}", std::process::id()));
        fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
        let electron = root.join("electron");
        fs::write(
            electron.join("teammates.json"),
            r#"{"schemaVersion":1,"teammates":[],"settings":{"memoryMode":"auto"}}"#,
        )
        .unwrap();
        fs::write(electron.join("appearance.json"), r#"{"theme":"light"}"#).unwrap();
        fs::write(electron.join("mission-ledger/sentinel"), "untouched").unwrap();
        let before = fingerprint(&electron);
        let mut s = State {
            data: Some(store::Store::new(root.join("native"), electron.clone()).unwrap()),
            ..State::default()
        };
        open(&mut s);
        assert_eq!(s.appearance, Some(false));
        for theme in [ThemeChoice::Dark, ThemeChoice::Light, ThemeChoice::System] {
            set_theme(&mut s, theme);
            assert_eq!(
                workspace_settings::load_theme(s.data.as_ref().unwrap()),
                theme
            );
        }
        for mode in ["ask", "off", "auto"] {
            s.settings_panel.settings.memory_mode = mode.into();
            persist(&mut s);
            assert_eq!(
                workspace_settings::load(s.data.as_ref().unwrap())
                    .unwrap()
                    .memory_mode,
                mode
            );
        }
        for page in SETTINGS_PAGES {
            s.settings_panel.page = Some(page.id);
            let _ = view(&s, &Palette::new(true));
        }
        crate::people_panel::open(&mut s);
        let _ = crate::usage_ledger::usage_turns(s.data.as_ref().unwrap()).unwrap();
        assert!(s.settings_panel.error.is_empty());
        assert_eq!(fingerprint(&electron), before);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn scene_opens_appearance() {
        let mut s = State::default();
        scene(&mut s);
        assert!(s.settings_panel.open);
        assert_eq!(s.settings_panel.page, Some(SettingsPageId::Appearance));
    }
}
