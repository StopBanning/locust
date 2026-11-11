use super::*;
use crate::browser::{Command, Host, Profile, Shared};
use std::sync::{Arc, Mutex};
pub(super) struct Panel {
    pub host: Shared,
    pub address: TextEdit,
    pub menu: bool,
    pub initialized: bool,
    pub profile_refresh: Option<std::time::Instant>,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            host: Arc::new(Mutex::new(Host::default())),
            address: TextEdit::default(),
            menu: false,
            initialized: false,
            profile_refresh: None,
        }
    }
}
fn add(s: &mut State, profile: Profile) {
    let result = crate::invariants::lock(&s.browser.host).add(profile);
    match result {
        Ok(_) => s.browser.address.clear(),
        Err(e) => crate::invariants::lock(&s.browser.host).error = Some(e),
    }
    s.browser.menu = false;
}
pub(super) fn toggle(p: &Palette) -> El {
    text("Browser")
        .font(type_::LABEL)
        .color(p.secondary)
        .padding(7.)
        .on_press(|s: &mut State| {
            {
                let mut h = crate::invariants::lock(&s.browser.host);
                h.open = !h.open;
            }
            let need = {
                let h = crate::invariants::lock(&s.browser.host);
                h.open && h.tabs.is_empty()
            };
            if need {
                add(s, Profile(None));
            }
        })
}
fn submit(s: &mut State) {
    let id = crate::invariants::lock(&s.browser.host).active;
    if let Some(id) = id {
        let result =
            crate::invariants::lock(&s.browser.host).navigate(id, &s.browser.address.text, true);
        let mut h = crate::invariants::lock(&s.browser.host);
        h.error = result.err();
    }
}
fn command(s: &mut State, cmd: Command) {
    let mut h = crate::invariants::lock(&s.browser.host);
    let active = h.active;
    if let Some(t) = h.tabs.iter_mut().find(|t| Some(t.id) == active) {
        t.commands.push_back(cmd);
    }
}
pub(super) fn poll(s: &mut State, tx: &mut Tx) -> bool {
    #[cfg(target_os = "macos")]
    if !crate::browser_mac::is_attached() {
        return false;
    }
    if !s.browser.initialized {
        let path = std::path::Path::new(if cfg!(target_os = "macos") {
            "/Library/Application Support/Locust/policy.json"
        } else {
            "/etc/locust/policy.json"
        });
        let refusal = match std::fs::read(path) {
            Ok(body) => match serde_json::from_slice(&body) {
                Ok(policy) => crate::browser::policy_refusal(&policy, path),
                Err(e) => Some(format!(
                    "Organization policy could not be read: {e}. Nothing was started."
                )),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => Some(format!(
                "Organization policy could not be read ({}): {e}. Nothing was started.",
                path.display()
            )),
        };
        crate::invariants::lock(&s.browser.host).refusal = refusal;
        crate::browser_tools::install(s.browser.host.clone());
        s.browser.initialized = true;
        #[cfg(target_os = "macos")]
        {
            crate::invariants::lock(&s.browser.host).temporary =
                !crate::browser_mac::persistent_available();
        }
    }
    {
        let mut h = crate::invariants::lock(&s.browser.host);
        let previous = h.profiles.clone();
        h.namespace = s
            .data
            .as_ref()
            .map(|store| store.root().to_string_lossy().into_owned())
            .unwrap_or_default();
        h.occluded = s.pending_approval.is_some()
            || s.palette_open
            || s.keyboard_sheet.open
            || s.settings_panel.open
            || s.people_panel.open;
        if s.browser
            .profile_refresh
            .is_none_or(|t| t.elapsed() > Duration::from_secs(1))
        {
            let result = s
                .data
                .as_ref()
                .map(|store| {
                    store.seeded_json(
                        "teammates.json",
                        serde_json::json!({"schemaVersion":1,"teammates":[]}),
                    )
                })
                .transpose();
            match result {
                Ok(Some(record)) => {
                    h.profiles = record["teammates"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|t| t["ownBrowser"] == true)
                        .filter_map(|t| {
                            Some((
                                t["teammateId"].as_str()?.to_owned(),
                                t["name"].as_str()?.to_owned(),
                            ))
                        })
                        .collect()
                }
                Ok(None) => h.profiles.clear(),
                Err(e) => {
                    h.error = Some(e);
                    h.profiles.clear();
                }
            }
            s.browser.profile_refresh = Some(std::time::Instant::now());
        }
        // Runtime's cwd resolution includes workspace.json fallback.
        for (id, _) in previous {
            if !h.profiles.iter().any(|(p, _)| p == &id) {
                h.forget(Profile(Some(id)));
            }
        }
        h.workspace = selected_conversation(s).and_then(|(t, _)| {
            s.data
                .as_ref()
                .and_then(|store| native::folder_for(t, store.electron()).ok())
        });
    }
    tx.wake_after(Duration::from_millis(100));
    #[cfg(target_os = "macos")]
    match crate::browser_mac::sync(&s.browser.host) {
        Ok(changed) => changed,
        Err(e) => {
            let mut h = crate::invariants::lock(&s.browser.host);
            let changed = h.error.as_ref() != Some(&e);
            h.error = Some(e);
            changed
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}
pub(super) fn view(s: &State, p: &Palette) -> Option<El> {
    let h = crate::invariants::lock(&s.browser.host);
    if !h.open {
        return None;
    }
    let active = h.tabs.iter().find(|t| Some(t.id) == h.active);
    let profile = active
        .and_then(|t| t.profile.0.as_ref())
        .and_then(|id| h.profiles.iter().find(|(p, _)| p == id))
        .map(|(_, name)| format!("{name}'s sign-ins"))
        .unwrap_or_else(|| "Isolated session".into());
    let mut out = col()
        .w(440.)
        .shrink()
        .min_w(240.)
        .h_full()
        .fill(p.field)
        .padding(10.)
        .gap(8.)
        .child(
            row()
                .gap(8.)
                .child(text("Browser").color(p.text))
                .child(text(profile).font(type_::LABEL).color(p.muted))
                .child(
                    text("+")
                        .color(p.secondary)
                        .on_press(|s: &mut State| s.browser.menu = !s.browser.menu),
                )
                .child(
                    text("Hide browser")
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .on_press(|s: &mut State| {
                            crate::invariants::lock(&s.browser.host).open = false
                        }),
                ),
        );
    if h.temporary {
        out = out.child(
            text(crate::browser::TEMPORARY_SIGN_INS)
                .font(type_::LABEL)
                .color(p.amber)
                .w_full()
                .wrap(),
        );
    }
    if s.browser.menu {
        let mut menu = col().gap(6.).child(
            text("New tab")
                .color(p.secondary)
                .on_press(|s: &mut State| add(s, Profile(None))),
        );
        for (id, name) in &h.profiles {
            let id = id.clone();
            menu = menu.child(
                text(format!("New tab as {name}"))
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .on_press(move |s: &mut State| add(s, Profile(Some(id.clone())))),
            );
        }
        for teammate in &s.teammates {
            let id = teammate.id.clone();
            let on = h.profiles.iter().any(|(p, _)| p == &id);
            menu = menu.child(
                text(format!(
                    "{} · Own browser sign-ins: {}",
                    teammate.name,
                    if on { "on" } else { "off" }
                ))
                .font(type_::LABEL)
                .color(p.secondary)
                .on_press(move |s: &mut State| {
                    let result = (|| -> Result<(), String> {
                        let store = s.data.as_ref().ok_or("Native data is not ready.")?;
                        let mut record = store.seeded_json(
                            "teammates.json",
                            serde_json::json!({"schemaVersion":1,"teammates":[]}),
                        )?;
                        let entry = record["teammates"]
                            .as_array_mut()
                            .and_then(|a| a.iter_mut().find(|t| t["teammateId"] == id))
                            .ok_or("That teammate is no longer there.")?;
                        if on {
                            if let Some(obj) = entry.as_object_mut() {
                                obj.remove("ownBrowser");
                            }
                        } else {
                            entry["ownBrowser"] = serde_json::json!(true);
                        }
                        store.save_json("teammates.json", &record)
                    })();
                    crate::invariants::lock(&s.browser.host).error = result.err();
                    s.browser.profile_refresh = None;
                }),
            );
        }
        out = out.child(menu);
    }
    let mut tabs = row().gap(8.);
    for tab in &h.tabs {
        let id = tab.id;
        let address = tab.url.clone();
        tabs = tabs.child(
            row()
                .gap(4.)
                .child(
                    text(if tab.loading {
                        "Loading…".into()
                    } else {
                        tab.title.chars().take(25).collect::<String>()
                    })
                    .font(type_::LABEL)
                    .color(if Some(id) == h.active {
                        p.text
                    } else {
                        p.muted
                    })
                    .on_press(move |s: &mut State| {
                        crate::invariants::lock(&s.browser.host).active = Some(id);
                        s.browser.address.text = address.clone();
                    }),
                )
                .child(text("×").color(p.muted).on_press(move |s: &mut State| {
                    crate::invariants::lock(&s.browser.host).close(id)
                })),
        );
    }
    if let Some(profile) = active.and_then(|t| t.profile.0.clone()) {
        out = out.child(
            text("Forget sign-ins")
                .font(type_::LABEL)
                .color(p.secondary)
                .on_press(move |s: &mut State| {
                    crate::invariants::lock(&s.browser.host).forget(Profile(Some(profile.clone())))
                }),
        );
    }
    out = out
        .child(tabs)
        .child(
            row()
                .gap(8.)
                .child(
                    text("Back")
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .on_press(|s: &mut State| command(s, Command::Back)),
                )
                .child(
                    text("Forward")
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .on_press(|s: &mut State| command(s, Command::Forward)),
                )
                .child(
                    text(if active.is_some_and(|t| t.loading) {
                        "Stop"
                    } else {
                        "Reload"
                    })
                    .font(type_::LABEL)
                    .color(p.secondary)
                    .on_press(|s: &mut State| {
                        let loading = {
                            let h = crate::invariants::lock(&s.browser.host);
                            h.tabs.iter().any(|t| Some(t.id) == h.active && t.loading)
                        };
                        command(
                            s,
                            if loading {
                                Command::Stop
                            } else {
                                Command::Reload
                            },
                        );
                    }),
                ),
        )
        .child(
            row()
                .gap(6.)
                .child(
                    text_input(&s.browser.address, type_::BODY)
                        .key("browser-address")
                        .grow(1.)
                        .color(p.text)
                        .placeholder("Enter a web address or localhost URL", p.muted)
                        .padding(8.)
                        .fill(p.raised)
                        .on_edit(|s: &mut State| &mut s.browser.address)
                        .on_input_key(|s: &mut State, key: &KeyEvent, _: &mut Tx| {
                            if key.key == Key::Enter {
                                submit(s);
                                true
                            } else {
                                false
                            }
                        }),
                )
                .child(
                    text("Go")
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .on_press(submit),
                ),
        );
    if let Some(error) = h.error.as_ref().or(h.refusal.as_ref()) {
        out = out.child(
            text(error.clone())
                .color(p.red)
                .font(type_::LABEL)
                .w_full()
                .wrap(),
        );
    }
    if active.is_some_and(|t| t.url.starts_with("locust-page:")) {
        out = out.child(
            text("Local preview · web requests are blocked")
                .font(type_::LABEL)
                .color(p.muted),
        );
    }
    let mut viewport = col()
        .key("browser-viewport")
        .grow(1.)
        .w_full()
        .fill(p.bg)
        .on_layout(|s: &mut State, r| crate::invariants::lock(&s.browser.host).rect = r);
    if s.demo {
        viewport=viewport.padding(20.).gap(12.).child(text("Example Domain").color(p.text)).child(text("This region is reserved for the native web page. Headless scenes show panel chrome only.").font(type_::LABEL).color(p.muted).w_full().wrap());
    } else if active.is_none_or(|t| t.url.is_empty()) {
        viewport = viewport.padding(20.).child(
            text("Open a page to browse here.")
                .color(p.muted)
                .w_full()
                .wrap(),
        );
    }
    Some(out.child(viewport))
}
pub(super) fn scene(s: &mut State) {
    team_scene(s);
    s.browser.host = Arc::new(Mutex::new(Host::default()));
    let mut h = crate::invariants::lock(&s.browser.host);
    h.profiles = vec![("codec".into(), "Codec".into())];
    h.temporary = true;
    if let Ok(id) = h.add(Profile(Some("codec".into())))
        && let Some(t) = h.tabs.iter_mut().find(|t| t.id == id)
    {
        t.title = "Example Domain".into();
        t.url = "https://example.com/".into();
    }
    s.browser.address.text = "https://example.com/".into();
}
pub(super) fn light_scene(s: &mut State) {
    scene(s);
    s.appearance = Some(false);
}
