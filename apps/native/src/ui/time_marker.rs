//! A hairline with the time on it, matching `apps/desktop/src/renderer/src/components/TimeMarker.tsx`.

use super::*;
use locust_core::time_marker::{ThreadMarker, clock_time};

pub(super) fn label_text(marker: &ThreadMarker) -> String {
    let clock = clock_time(&marker.at);
    let mut out = String::new();
    if let Some(day) = &marker.day {
        out.push_str(day);
        out.push_str(" · ");
    }
    out.push_str(&clock);
    out.push_str(" · ");
    out.push_str(&marker.elapsed);
    if let Some(note) = &marker.note {
        out.push_str(" · ");
        out.push_str(note);
    }
    out
}

pub(super) fn aria_label(marker: &ThreadMarker) -> String {
    let clock = clock_time(&marker.at);
    let prefix = marker
        .day
        .as_deref()
        .map(|d| format!("{d}, "))
        .unwrap_or_default();
    format!("{prefix}{clock}, {} minutes in", marker.minutes_in)
}

pub(super) fn view(marker: &ThreadMarker, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let line = || el().grow(1.0).h(1.0).fill(l.c("border-hairline"));
    row()
        .w_full()
        .center_y()
        .gap(16.0)
        .py(8.0)
        .role("separator")
        .label(aria_label(marker))
        .child(line())
        .child(
            text(label_text(marker))
                .font(l.ty.mono)
                .color(l.c("text-muted")),
        )
        .child(line())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_label_with_and_without_day_and_note() {
        let m = ThreadMarker {
            before_item: 2,
            at: "2026-09-22T09:30:00.000Z".into(),
            minutes_in: 90,
            note: Some("waited 5 min".into()),
            elapsed: "1h 30m in".into(),
            day: None,
        };
        let clock = clock_time(&m.at);
        assert_eq!(label_text(&m), format!("{clock} · 1h 30m in · waited 5 min"));
        assert_eq!(aria_label(&m), format!("{clock}, 90 minutes in"));

        let with_day = ThreadMarker {
            day: Some("Sep 23".into()),
            note: None,
            ..m
        };
        assert_eq!(label_text(&with_day), format!("Sep 23 · {clock} · 1h 30m in"));
        assert_eq!(aria_label(&with_day), format!("Sep 23, {clock}, 90 minutes in"));
    }
}
