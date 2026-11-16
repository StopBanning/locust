//! Settings › Appearance › "What a face says" (`FaceLegend.tsx`, 2026-10-05): one teammate going through every face a
//! teammate has, each named under it in the app's own words, each with the dot a teammate's has for it. The window
//! draws it (`settings_panel.rs`); the steps, their times, names and dots, and when the next one is due, are here.

use crate::bot_rig::{face_label, FaceActivity};
use crate::face_status::Presence;

/// One step of the legend: the face, and how long it is shown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegendStep {
    pub activity: FaceActivity,
    pub seconds: f64,
}

/// `FACE_LEGEND`, in its order. A subagent at work wears the work face, so delegating is shown as working.
pub const FACE_LEGEND: [LegendStep; 8] = [
    LegendStep { activity: FaceActivity::Idle, seconds: 2.6 },
    LegendStep { activity: FaceActivity::Thinking, seconds: 3.4 },
    LegendStep { activity: FaceActivity::Working, seconds: 3.0 },
    LegendStep { activity: FaceActivity::Responding, seconds: 2.8 },
    LegendStep { activity: FaceActivity::Done, seconds: 2.4 },
    LegendStep { activity: FaceActivity::Waiting, seconds: 3.0 },
    LegendStep { activity: FaceActivity::Receiving, seconds: 2.6 },
    LegendStep { activity: FaceActivity::Blocked, seconds: 2.8 },
];

/// The teammate it shows: a droid with eyes, in teal (`LEGEND_LOOK`, `hue="teal"`), at 40.
pub const LEGEND_SHAPE: &str = "droid";
pub const LEGEND_HUE: &str = "teal";
pub const LEGEND_SIZE: f32 = 40.0;

/// `legendPresence`: its dot for each, as a teammate's is (status.ts `facePresenceFor`).
pub fn legend_presence(activity: FaceActivity) -> Presence {
    match activity {
        FaceActivity::Thinking | FaceActivity::Working | FaceActivity::Delegating | FaceActivity::Responding => {
            Presence::Working
        }
        FaceActivity::Waiting => Presence::Approval,
        FaceActivity::Blocked => Presence::Blocked,
        _ => Presence::None,
    }
}

/// `legendName`: the face's name under it, said the way the app says it beside a face, in sentence case.
pub fn legend_name(activity: FaceActivity) -> String {
    let word = face_label(activity);
    let mut chars = word.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// The button's label: "A teammate's face: {name}. Show the next one."
pub fn legend_label(activity: FaceActivity) -> String {
    format!("A teammate's face: {}. Show the next one.", legend_name(activity))
}

/// The row's words beside it (Screens.tsx 3216-3220): its eyes say it too only while Terminal faces is on.
pub fn legend_lede(terminal_faces: bool) -> &'static str {
    if terminal_faces {
        "A teammate's face says what it is doing, and its eyes say it too. This one shows each in turn; click it for the next."
    } else {
        "A teammate's face says what it is doing. This one shows each in turn; click it for the next. With Terminal faces on, its eyes say it too."
    }
}

/// Where the legend is: which step (counting on for ever, as `at` does), and since when it has been shown (window
/// clock, seconds). A press, or the step's time running out, moves it on and starts the next step's time.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LegendClock {
    pub at: usize,
    pub since: Option<f64>,
}

impl LegendClock {
    pub fn shown(&self) -> LegendStep {
        FACE_LEGEND[self.at % FACE_LEGEND.len()]
    }

    /// A press: the next face, whatever the motion setting. Its time starts at the next tick.
    pub fn press(&mut self) {
        self.at += 1;
        self.since = None;
    }

    /// The clock at `now`. While it rests (`paused`), it stops where it is, its time and all: its step is not counted
    /// on, and it starts its full time again once it goes on (the timer is cleared and set afresh, as the effect
    /// does). True when the step changed.
    pub fn tick(&mut self, now: f64, paused: bool) -> bool {
        if paused {
            self.since = None;
            return false;
        }
        let Some(since) = self.since else {
            self.since = Some(now);
            return false;
        };
        // A microsecond's grace: 12.6 - 10.0 is 2.5999… in floating point.
        if now - since + 1e-6 >= self.shown().seconds {
            self.at += 1;
            self.since = Some(now);
            return true;
        }
        false
    }

    /// Seconds until the next step is due, while it is going.
    pub fn due_in(&self, now: f64) -> Option<f64> {
        self.since.map(|since| (self.shown().seconds - (now - since)).max(0.0))
    }
}

#[cfg(test)]
#[path = "face_legend_tests.rs"]
mod tests;
