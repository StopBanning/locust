//! Settings › Your own models (Electron `OwnModels.tsx`, 0.357): each model kept, with what it is and where, whether a
//! key is kept, and Uses tools or Chat only; and the form that adds one. The key field is masked, goes straight to the
//! operating system's protection, and is cleared once added; the window never sees it again. Test (asking the endpoint
//! what it serves) remains: this window has no network client yet.

use super::*;
use locust_core::own_models::PublicOwnModel;

#[derive(Default)]
pub(super) struct Card {
    pub models: Vec<PublicOwnModel>,
    pub name: TextEdit,
    pub model: TextEdit,
    pub address: TextEdit,
    pub key: TextEdit,
    pub chat_only: bool,
    /// What the last change said, and whether it went well.
    pub said: Option<(String, bool)>,
}

fn store(s: &State) -> Option<locust_core::own_models::OwnModels> {
    let data = s.data.as_ref()?;
    // The window's tests never reach the real keychain: without a stand-in of their own, nothing keeps a key.
    if cfg!(test) && !locust_core::own_models::has_stand_in() {
        return Some(locust_core::own_models::OwnModels::new(data.root(), std::sync::Arc::new(locust_core::own_models::NoBox)));
    }
    Some(locust_core::own_models::for_store(data))
}

pub(super) fn reload(s: &mut State) {
    let Some(models) = store(s) else { return };
    match models.list() {
        Ok(list) => s.own_models.models = list,
        Err(why) => s.own_models.said = Some((why, false)),
    }
}

fn add(s: &mut State) {
    let Some(models) = store(s) else { return };
    let card = &s.own_models;
    let key = card.key.text.clone();
    let answer = models.add(&card.name.text, &card.address.text, &card.model.text, Some(key.as_str()).filter(|k| !k.trim().is_empty()), card.chat_only);
    match answer {
        Ok(added) => {
            s.own_models.said = Some((format!("{} is in every teammate's model list, under Your models.", added.name), true));
            s.own_models.name = TextEdit::default();
            s.own_models.model = TextEdit::default();
            s.own_models.address = TextEdit::default();
            s.own_models.key = TextEdit::default();
            s.own_models.chat_only = false;
        }
        Err(why) => s.own_models.said = Some((why, false)),
    }
    reload(s);
}

fn host_of(base_url: &str) -> String {
    base_url.split("://").nth(1).unwrap_or(base_url).split('/').next().unwrap_or(base_url).to_owned()
}

fn field(l: &crate::look::controls::Look, edit: &TextEdit, key: &str, label: &str, placeholder: &str, pick: fn(&mut State) -> &mut TextEdit) -> El {
    col()
        .gap(4.0)
        .grow(1.0)
        .child(text(label.to_owned()).font(type_::LABEL).color(l.c("text-muted")))
        .child(
            text_input(edit, l.ty.body)
                .key(key.to_owned())
                .single_line()
                .label(label.to_owned())
                .placeholder(placeholder, l.c("text-muted"))
                .w_full()
                .h(32.0)
                .padding(6.0)
                .radius(6.0)
                .fill(l.c("bg-control"))
                .color(l.c("text-primary"))
                .on_edit(pick),
        )
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let card = &s.own_models;
    let mut out = col().w_full().gap(10.0).padding(10.0).key("own-models");
    for kept in &card.models {
        let (id, id2, name) = (kept.own_id.clone(), kept.own_id.clone(), kept.name.clone());
        let meta = format!("{} \u{b7} {} \u{b7} {}{}", kept.model, host_of(&kept.base_url), if kept.has_key { "key kept" } else { "no key" }, if kept.chat_only { " \u{b7} chat only" } else { "" });
        let chat_only = kept.chat_only;
        out = out.child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .key(format!("own-model-{}", kept.own_id))
                .child(col().grow(1.0).gap(2.0).child(text(kept.name.clone()).font(l.ty.control).color(l.c("text-primary"))).child(text(meta).font(type_::LABEL.mono()).color(l.c("text-secondary")).wrap_anywhere()))
                .child(crate::look::controls::switch(&l, !kept.chat_only, if kept.chat_only { "Chat only: its runs get no tools" } else { "Uses tools: it may read and change files" }, false).key(format!("own-model-tools-{}", kept.own_id)).on_press(move |s: &mut State| {
                    if let Some(models) = store(s)
                        && let Err(why) = models.set_chat_only(&id, !chat_only)
                    {
                        s.own_models.said = Some((why, false));
                    }
                    reload(s);
                }))
                .child(crate::look::controls::button(&l, "Remove", crate::look::controls::Tone::Neutral, false).key(format!("own-model-remove-{}", kept.own_id)).label(format!("Remove {name}")).on_press(move |s: &mut State| {
                    if let Some(models) = store(s)
                        && let Err(why) = models.remove(&id2)
                    {
                        s.own_models.said = Some((why, false));
                    }
                    reload(s);
                })),
        );
    }
    let filled = !card.name.text.trim().is_empty() && !card.model.text.trim().is_empty() && !card.address.text.trim().is_empty();
    out = out
        .child(row().w_full().gap(10.0).child(field(&l, &card.name, "own-model-name", "Name", "Acme Chat", |s| &mut s.own_models.name)).child(field(&l, &card.model, "own-model-model", "Model", "acme-70b", |s| &mut s.own_models.model)))
        .child(field(&l, &card.address, "own-model-address", "Address", "https://llm.example.com/v1", |s| &mut s.own_models.address))
        .child(
            col()
                .gap(4.0)
                .child(text("Key").font(type_::LABEL).color(l.c("text-muted")))
                .child(
                    text_input(&card.key, l.ty.body)
                        .key("own-model-key")
                        .single_line()
                        .secure()
                        .label("Key")
                        .placeholder("Leave empty if it asks for none", l.c("text-muted"))
                        .w_full()
                        .h(32.0)
                        .padding(6.0)
                        .radius(6.0)
                        .fill(l.c("bg-control"))
                        .color(l.c("text-primary"))
                        .on_edit(|s: &mut State| &mut s.own_models.key),
                ),
        )
        .child(
            row()
                .w_full()
                .gap(12.0)
                .center_y()
                .child(crate::look::controls::switch(&l, !card.chat_only, "Uses tools", false).key("own-model-new-tools").on_press(|s: &mut State| s.own_models.chat_only = !s.own_models.chat_only))
                .child(
                    col()
                        .grow(1.0)
                        .gap(2.0)
                        .child(text(if card.chat_only { "Chat only" } else { "Uses tools" }).font(l.ty.control).color(l.c("text-primary")))
                        .child(
                            text(if card.chat_only {
                                "Its teammates talk with it, and it reads and changes no files. For a model that cannot take tools."
                            } else {
                                "It may read and change files in the project, as other models do. Test turns this off if it cannot."
                            })
                            .font(type_::LABEL)
                            .color(l.c("text-secondary"))
                            .wrap(),
                        ),
                )
                .child(crate::look::controls::primary_button(&l, "Add model", !filled).key("own-model-add").on_press(|s: &mut State| {
                    if !s.own_models.name.text.trim().is_empty() && !s.own_models.model.text.trim().is_empty() && !s.own_models.address.text.trim().is_empty() {
                        add(s);
                    }
                })),
        );
    if let Some((words, good)) = &card.said {
        out = out.child(text(words.clone()).font(l.ty.body).color(if *good { l.c("text-secondary") } else { l.c("amber-text") }).wrap().key("own-model-said"));
    }
    out
}
