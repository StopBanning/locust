//! The Teammate look picker, in the teammate form (`people_panel`), which is both the new-teammate dialog and the
//! editor, as Electron's `NewTeammateDialog.tsx` is. It holds the preview that moves (509-516), the colour swatches
//! (541-559), the Look head (Face, Shuffle look; 632-711) and the shape grid (712-727). The rules (what is chosen,
//! offered, refused and saved) are `locust_core::look_picker`'s. See `notes/LOOK-NOTES.md`.

use super::*;
use crate::look::bot_stage::PresenceDot;
use crate::look::faces::{self, BotFaceOpts};
use locust_core::bot_rig::{BotMotionLevel, FaceActivity};
use locust_core::bot_spec::{bot_for, seed_avatar, AvatarSpec, BOT_SHAPES};
use locust_core::look_picker::{self as rules, FaceChoice};
use locust_core::native::FaceLook;
use serde_json::{json, Value};

/// The look being edited, from the form's entry: its own, or the one the store shows for a record without one.
pub(super) fn avatar_of(entry: &Value) -> AvatarSpec {
    rules::avatar_from_json(&entry["avatar"])
        .unwrap_or_else(|| seed_avatar(entry["teammateId"].as_str().unwrap_or("")))
}

/// The look a form opens with: a fresh draft's for a new teammate, else the record's (NewTeammateDialog.tsx 310-313).
pub(super) fn opening_avatar(entry: &Value, creating: bool) -> Value {
    rules::avatar_json(&if creating { rules::draft_avatar() } else { avatar_of(entry) })
}

/// What the faces draw for this hue and look.
pub(super) fn face_look(hue: &str, avatar: &AvatarSpec) -> FaceLook {
    FaceLook {
        hue: hue.to_owned(),
        parts: Some([avatar.headwear, avatar.accessory, avatar.mouth]),
        bot: avatar.bot.as_ref().map(|b| (b.shape.clone(), b.face.as_str().to_owned())),
        bot_screen: avatar.bot.as_ref().and_then(|b| b.screen),
        pet: avatar.pet.clone(),
    }
}

fn hue_of(s: &State) -> String {
    s.people_panel.entry["hue"].as_str().unwrap_or("lime").to_owned()
}

fn set_avatar(s: &mut State, next: AvatarSpec) {
    s.people_panel.entry["avatar"] = rules::avatar_json(&next);
}

/// A tile of a radio group: its label said, its state checked, and the lime edge while chosen (`.lc-look`, `.lc-hue`).
fn radio(l: &crate::look::controls::Look, el: El, key: String, label: &str, chosen: bool) -> El {
    let el = el
        .key(key)
        .role("radio")
        .label(label.to_owned())
        .checked(chosen)
        .focusable(true)
        .focus_ring(2.0, 0.0, l.c("focus-ring"));
    if chosen { el.fill(l.c("lime-tint")) } else { el.hover(l.c("hover-surface")) }
}

/// The preview: the teammate's own bot at 64, working, with the working dot, at full motion, so the person sees what a
/// live teammate does.
pub(super) fn preview(s: &State, l: &crate::look::controls::Look) -> El {
    let avatar = avatar_of(&s.people_panel.entry);
    let look = face_look(&hue_of(s), &avatar);
    let id = s.people_panel.entry["teammateId"].as_str().unwrap_or("").to_owned();
    let name = s.people_panel.fields.first().map(|f| f.text.trim().to_owned()).unwrap_or_default();
    faces::bot_face_with_look(
        s,
        "team-look-preview",
        &id,
        &name,
        Some(&look),
        64.0,
        BotFaceOpts {
            activity: FaceActivity::Working,
            presence: Some(PresenceDot::Working),
            motion: BotMotionLevel::Full,
            ..Default::default()
        },
        l,
    )
}

/// "Avatar colour": the eleven, each the teammate's bot at 24 in that colour, or a plain chip while a pet is worn.
pub(super) fn swatches(s: &State, l: &crate::look::controls::Look) -> El {
    let avatar = avatar_of(&s.people_panel.entry);
    let current = hue_of(s);
    let terminal = s.settings_panel.settings.terminal_faces;
    let mut group = row().key("team-hues").role("radiogroup").label("Avatar colour").gap(6.0).flex_wrap();
    for (hue, label) in rules::HUE_LABELS {
        let inner = if avatar.pet.is_none() {
            faces::face(None, &format!("look-hue-{hue}"), label, Some(&face_look(hue, &avatar)), 24.0, terminal, l)
        } else {
            row().w(24.0).h(24.0).radius(12.0).fill(l.c(&format!("hue-{hue}"))).border(1.0, l.c("border-button"))
        };
        let tile = row()
            .padding(3.0)
            .radius(l.t.px("radius-button"))
            .border(1.0, if current == hue { l.c("lime-border-strong") } else { rune::Color::TRANSPARENT })
            .child(inner);
        group = group.child(
            radio(l, tile, format!("team-hue-{hue}"), label, current == hue)
                .on_press(move |s: &mut State| s.people_panel.entry["hue"] = json!(hue)),
        );
    }
    group
}

/// The Look section: its head (the label, the Face choice, Shuffle look) and the 23 shapes.
pub(super) fn section(s: &State, l: &crate::look::controls::Look) -> El {
    let avatar = avatar_of(&s.people_panel.entry);
    let hue = hue_of(s);
    let terminal = s.settings_panel.settings.terminal_faces;
    let mut head = row()
        .key("team-look-head")
        .w_full()
        .gap(8.0)
        .center_y()
        .child(text("Look").font(type_::LABEL).color(l.c("text-secondary")).grow(1.0));
    match &avatar.pet {
        None => {
            let chosen = rules::face_choice(&avatar, terminal);
            let mut faces_group = row()
                .key("team-look-face")
                .role("radiogroup")
                .label("Face")
                .gap(2.0)
                .padding(2.0)
                .radius(9.0)
                .fill(l.c("segment-track"))
                .border(1.0, l.c("border-hairline"));
            for choice in FaceChoice::ALL {
                let off = rules::face_off(&avatar, choice, terminal);
                faces_group = faces_group.child(segment(l, format!("team-look-face-{}", choice.label().to_lowercase()), choice.label(), chosen == choice, off, move |s: &mut State| {
                    let next = rules::with_face(&avatar_of(&s.people_panel.entry), choice);
                    set_avatar(s, next);
                }));
            }
            head = head.child(faces_group);
        }
        // A worn pet keeps its own face. Codex Buddy, the pet this app draws, has a screen of its own to choose, and so
        // has a kept pet whose sheet is the one measured, a puppet with one (NewTeammateDialog.tsx 669).
        Some(pet)
            if faces::is_codex_buddy(Some(&face_look(&hue, &avatar))) || crate::pet_faces::is_measured_puppet(s, pet) =>
        {
            let mut faces_group = row()
                .key("team-look-face")
                .role("radiogroup")
                .label("Face")
                .gap(2.0)
                .padding(2.0)
                .radius(9.0)
                .fill(l.c("segment-track"))
                .border(1.0, l.c("border-hairline"));
            for (screen, label, title) in [(false, "As drawn", "As its maker drew it."), (true, "Screen", "A screen whose eyes say what it is doing.")] {
                let off = (screen && !terminal).then_some(rules::SCREEN_NEEDS_TERMINAL);
                let chosen = rules::pet_screen_chosen(pet, screen, terminal);
                let key = format!("team-look-pet-{}", if screen { "screen" } else { "drawn" });
                faces_group = faces_group.child(
                    segment(l, key, label, chosen, off, move |s: &mut State| {
                        let next = rules::with_pet_screen(&avatar_of(&s.people_panel.entry), screen);
                        set_avatar(s, next);
                    })
                    .help(off.unwrap_or(title)),
                );
            }
            head = head.child(faces_group);
        }
        Some(_) => {}
    }
    head = head.child(
        crate::look::controls::ghost_button(l, "Shuffle look").key("team-look-shuffle").on_press(|s: &mut State| {
            let next = locust_core::bot_spec::shuffled_avatar(&avatar_of(&s.people_panel.entry));
            set_avatar(s, next);
        }),
    );
    let shown = bot_for(&avatar).shape;
    let mut grid = row().key("team-look-grid").role("radiogroup").label("Look").w_full().gap(6.0).flex_wrap();
    for shape in BOT_SHAPES {
        let name = rules::shape_name(shape).unwrap_or(shape);
        let chosen = avatar.pet.is_none() && shown == shape;
        let look = face_look(&hue, &rules::with_shape(&avatar, shape));
        let tile = row()
            .w(44.0)
            .h(44.0)
            .center()
            .radius(l.t.px("radius-control"))
            .border(1.0, if chosen { l.c("lime-border") } else { l.c("border-card") })
            .child(faces::face(None, &format!("look-shape-{shape}"), name, Some(&look), 30.0, terminal, l));
        grid = grid.child(
            radio(l, tile, format!("team-look-shape-{shape}"), name, chosen).help(name).on_press(move |s: &mut State| {
                let next = rules::with_shape(&avatar_of(&s.people_panel.entry), shape);
                set_avatar(s, next);
            }),
        );
    }
    col()
        .key("team-look")
        .w_full()
        .gap(8.0)
        .pt(4.0)
        .child(head)
        .child(grid)
        .child(crate::pet_picks::row_of_pets(s, &avatar, l))
}

/// One choice of a segmented Face group (`.lc-lookface button`): 24 high, the chosen one raised, an unusable one dimmed
/// with its reason as its title.
fn segment(
    l: &crate::look::controls::Look,
    key: String,
    label: &str,
    chosen: bool,
    off: Option<&'static str>,
    press: impl Fn(&mut State) + 'static,
) -> El {
    let ink = if chosen { l.c("text-primary") } else { l.c("text-muted") };
    let mut el = row()
        .h(24.0)
        .px(10.0)
        .center()
        .radius(7.0)
        .child(text(label.to_owned()).font(rune::TextStyle::new(12.0, 500.0)).color(ink))
        .key(key)
        .role("radio")
        .label(label.to_owned())
        .checked(chosen);
    if chosen {
        el = el.fill(l.c("segment-thumb"));
    }
    match off {
        Some(why) => el.opacity(0.45).disabled(true).help(why),
        None => el.focusable(true).focus_ring(2.0, 0.0, l.c("focus-ring")).hover(l.c("hover-surface")).on_press(press),
    }
}

#[cfg(test)]
#[path = "look_picker_tests.rs"]
pub(crate) mod tests;

#[cfg(test)]
mod buddy_picker_tests;
