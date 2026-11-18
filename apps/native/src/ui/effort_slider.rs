//! Claude Code's effort control, at the foot of the model picker: Electron's `components/EffortSlider.tsx` as
//! `Composer.tsx` mounts it in the RoutePicker's effort slot (`.lc-effortpanel`).
//!
//! Top to bottom: the word Effort and the level's name, a ? whose hover says what the level costs; Faster and Smarter
//! at the ends; a rounded 20px track with a dot at every stop and the part up to the level filled; one white 16 x 20
//! thumb with 4px corners. A held thumb follows the pointer along the track and the level is picked as the nearest
//! stop changes; letting go settles it on that stop. The track is also the keyboard's control (a slider: arrows step a
//! level, Home and End go to the ends). A model with fast variants gets the Fast variant switch under it.
//!
//! Not drawn: the library's gooey squash and stretch (Rune has no goo filter); the thumb is the plain rounded
//! rectangle Claude Code draws at rest.

use super::*;
use locust_core::effort_levels::{
    EFFORT_TRACK_WIDTH, THUMB_RADIUS, THUMB_WIDTH, TRACK_HEIGHT, effort_description, effort_footer, effort_name,
    effort_scale, held_at, join_effort, nearest_stop, split_effort, stop_position,
};
use std::rc::Rc;

/// What picking a level does: the caller turns the level string into its own pick.
pub(crate) type PickLevel = Rc<dyn Fn(&mut State, String)>;

/// The panel for a model that lists `supported` levels, on `effort` (or its default), for `runtime`.
pub(crate) fn effort_panel(
    s: &State,
    l: &crate::look::controls::Look,
    supported: &[String],
    effort: Option<&str>,
    runtime: &str,
    pick: PickLevel,
) -> El {
    let scale = effort_scale(supported);
    let bases = scale.bases;
    let count = bases.len();
    let shown_effort = effort.map(str::to_owned).or_else(|| bases.first().cloned()).unwrap_or_default();
    let (base, fast) = split_effort(&shown_effort);
    let (base, fast) = (base.to_owned(), fast);
    let index = bases.iter().position(|b| *b == base).unwrap_or(0);
    let held = s.effort_held;
    let x = held.unwrap_or_else(|| stop_position(index, count));
    let shown = match held {
        Some(at) => nearest_stop(at, count),
        None => index,
    };
    let level = bases.get(shown).or(bases.first()).cloned().unwrap_or_default();
    let meaning = effort_description(&level);
    let name = effort_name(&level);

    // `.lc-effortpanel__head`: Effort, the level's name, and Claude Code's ? carrying what the level costs.
    let what_label = match meaning {
        Some(m) => format!("{name}: {m}"),
        None => "How hard the model thinks before it answers".to_owned(),
    };
    let what = row()
        .key("effort-what")
        .size(16.0, 16.0)
        .center()
        .radius(8.0)
        .border(1.0, l.c("border-button"))
        .child(text("?").font(l.ty.mono).color(l.c("text-muted")))
        .role("img")
        .label(match meaning {
            Some(m) => format!("{name}: {m}"),
            None => format!("{name} effort"),
        });
    let what = crate::hover::tip(s, l, "effort-what", &what_label, what);
    let head = row()
        .key("effort-head")
        .w_full()
        .gap(6.0)
        .center_y()
        .child(text("Effort").key("effort-label").font(l.ty.control).color(l.c("text-muted")))
        .child(
            text(format!("{name}{}", if fast { " · Fast" } else { "" }))
                .key("effort-now")
                .font(l.ty.control)
                .color(l.c("text-primary")),
        )
        .child(row().grow(1.0))
        .child(what);
    let ends = row()
        .key("effort-ends")
        .w(EFFORT_TRACK_WIDTH)
        .justify_between()
        .child(text("Faster").font(l.ty.body).color(l.c("text-muted")))
        .child(text("Smarter").font(l.ty.body).color(l.c("text-muted")));

    // The track, the fill up to the thumb, a dot at every stop, the thumb.
    let mut scale_el = el()
        .key("effort-scale")
        .w(EFFORT_TRACK_WIDTH)
        .h(TRACK_HEIGHT)
        .child(el().key("effort-track").absolute(0.0, 0.0).w(EFFORT_TRACK_WIDTH).h(TRACK_HEIGHT).radius(TRACK_HEIGHT / 2.0).fill(l.c("effort-track")))
        .child(el().key("effort-fill").absolute(0.0, 0.0).w(x).h(TRACK_HEIGHT).radius(TRACK_HEIGHT / 2.0).fill(l.c("effort-fill")));
    for stop in 0..count {
        let passed = stop <= shown;
        scale_el = scale_el.child(
            el().key(if passed { format!("effort-notch-passed-{stop}") } else { format!("effort-notch-{stop}") })
                .absolute(stop_position(stop, count) - 1.0, TRACK_HEIGHT / 2.0 - 1.0)
                .size(2.0, 2.0)
                .radius(1.0)
                .fill(l.c(if passed { "effort-dot-passed" } else { "effort-dot" }))
                .a11y_hidden(),
        );
    }
    let mut thumb = el()
        .key("effort-thumb")
        .absolute(x - THUMB_WIDTH / 2.0, 0.0)
        .size(THUMB_WIDTH, TRACK_HEIGHT)
        .radius(THUMB_RADIUS)
        .fill(l.c("effort-thumb"))
        .a11y_hidden();
    for shadow in l.t.shadows("effort-thumb-shadow").into_iter().chain(l.t.shadows("effort-thumb-lift")) {
        thumb = thumb.box_shadow(shadow);
    }
    scale_el = scale_el.child(thumb);

    // The control for the keyboard and the screen reader, and the pointer's track.
    scale_el = scale_el
        .role("slider")
        .label("Reasoning effort")
        .range_value(index as f64, 0.0, count.saturating_sub(1) as f64)
        .range_step(1.0)
        .a11y_value(effort_name(bases.get(index).map(String::as_str).unwrap_or(&level)));
    if count < 2 {
        scale_el = scale_el.disabled(true);
    } else {
        let (drag_bases, drag_levels, drag_pick) = (bases.clone(), supported.to_vec(), pick.clone());
        scale_el = scale_el
            .on_drag(move |s: &mut State, d: &mut rune::Drag, _: &mut Tx| {
                if d.phase == rune::Phase::Up {
                    s.effort_held = None;
                    return;
                }
                // The drag is in the track's own points, so the track's left is 0.
                let place = held_at(d.x, 0.0);
                s.effort_held = Some(place);
                let stop = nearest_stop(place, drag_bases.len());
                if stop != index
                    && let Some(level) = join_effort(&drag_bases[stop], fast, &drag_levels)
                {
                    drag_pick(s, level);
                }
            })
            .on_step({
                let (bases, levels, pick) = (bases.clone(), supported.to_vec(), pick.clone());
                move |s: &mut State, delta, _: &mut Tx| {
                    let next = (index as i64 + delta as i64).clamp(0, bases.len() as i64 - 1) as usize;
                    if next != index
                        && let Some(level) = join_effort(&bases[next], fast, &levels)
                    {
                        pick(s, level);
                    }
                }
            })
            .on_step_limit({
                let (bases, levels, pick) = (bases.clone(), supported.to_vec(), pick.clone());
                move |s: &mut State, max, _: &mut Tx| {
                    let next = if max { bases.len() - 1 } else { 0 };
                    if let Some(level) = join_effort(&bases[next], fast, &levels) {
                        pick(s, level);
                    }
                }
            });
    }

    let mut panel = col()
        .key("effort-panel")
        .w_full()
        .gap(8.0)
        .role("group")
        .label("Reasoning effort")
        .child(head)
        .child(ends)
        .child(scale_el);
    if scale.has_fast {
        let (levels, pick, fast_base) = (supported.to_vec(), pick.clone(), base.clone());
        let switch = row()
            .key("effort-fast")
            .w(EFFORT_TRACK_WIDTH)
            .justify_between()
            .center_y()
            .gap(8.0)
            .py(6.0)
            .px(8.0)
            .radius(l.t.px("radius-control"))
            .border(1.0, l.c("border-control"))
            .child(text("Fast variant").font(l.ty.body).color(l.c("text-secondary")))
            .child(crate::look::controls::switch(l, fast, "Fast variant", false).a11y_hidden())
            .role("switch")
            .label("Fast variant")
            .checked(fast)
            .on_press(move |s: &mut State| {
                if let Some(level) = join_effort(&fast_base, !fast, &levels) {
                    pick(s, level);
                }
            });
        panel = panel.child(switch);
    }
    if let Some(foot) = effort_footer(runtime) {
        panel = panel.child(text(foot).key("effort-footer").font(l.ty.body).color(l.c("text-muted")));
    }
    panel
}

#[cfg(test)]
#[path = "effort_slider_tests.rs"]
mod tests;
