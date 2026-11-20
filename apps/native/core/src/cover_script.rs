//! The title screen's three, each with a part of its own (Electron `coverScript.ts`, 0.562): the worker types in bursts,
//! finishes with green carets and a hop, looks over at the ghost, waits on you a moment in amber, and gets back to it;
//! the thinker looks about with round eyes, has an idea, looks across at the worker. Each plays on a loop of its own
//! length from its own start, so the two awake ones are almost never wearing the same face. The sleeper sleeps.
//!
//! Every beat is a state a teammate really shows, in the colour it really uses, so the cover teaches the faces as it
//! plays. `beat_at` also says how long until the next beat, so Home draws the cover again only at a change.

use crate::bot_spec::Phosphor;

/// Which way a beat looks for its first moments: across the machine, to a neighbour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glance {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoverBeat {
    /// Names the beat, so a change of beat can be told from a repeat.
    pub name: &'static str,
    pub seconds: f64,
    /// `None`: a screen's resting bars.
    pub eyes: Option<[&'static str; 2]>,
    pub phosphor: Phosphor,
    /// Hops once as the beat starts.
    pub hop: bool,
    pub glance: Option<Glance>,
    /// Locust's waiting-on-you colour.
    pub waiting: bool,
}

const fn beat(name: &'static str, seconds: f64, eyes: Option<[&'static str; 2]>, phosphor: Phosphor) -> CoverBeat {
    CoverBeat { name, seconds, eyes, phosphor, hop: false, glance: None, waiting: false }
}

const TYPING: [&str; 2] = [">", "▮"];
const CARETS: [&str; 2] = ["^", "^"];
const DOTS: [&str; 2] = ["•", "•"];

pub const WORKER: [CoverBeat; 5] = [
    beat("typing", 6.5, Some(TYPING), Phosphor::Cyan),
    CoverBeat { hop: true, ..beat("done", 1.8, Some(CARETS), Phosphor::Green) },
    CoverBeat { glance: Some(Glance::Left), ..beat("looks-over", 1.7, None, Phosphor::Cyan) },
    CoverBeat { waiting: true, ..beat("waiting", 3.2, None, Phosphor::Amber) },
    beat("back-to-it", 3.8, Some(TYPING), Phosphor::Cyan),
];

pub const THINKER: [CoverBeat; 4] = [
    beat("thinking", 5.5, Some(DOTS), Phosphor::Cyan),
    beat("idea", 1.6, Some(CARETS), Phosphor::Green),
    CoverBeat { glance: Some(Glance::Right), ..beat("looks-across", 2.4, None, Phosphor::Cyan) },
    beat("thinking-again", 3.5, Some(DOTS), Phosphor::Cyan),
];

pub const SLEEPING: CoverBeat = beat("asleep", f64::INFINITY, Some(["c", "c"]), Phosphor::Cyan);
pub const WATCHING: CoverBeat = beat("watching", f64::INFINITY, None, Phosphor::Cyan);
pub const PLEASED: CoverBeat = CoverBeat { hop: true, ..beat("pleased", 1.5, Some(CARETS), Phosphor::Green) };

/// What a still title screen shows (reduced motion, or before the agents have answered): each at its first beat.
pub const STILL_WORKER: CoverBeat = WORKER[0];
pub const STILL_THINKER: CoverBeat = THINKER[0];

/// Where the worker and the thinker start in their loops, so the two are never in step.
pub const WORKER_OFFSET: f64 = 0.0;
pub const THINKER_OFFSET: f64 = 4.1;

/// How long a loop of beats takes.
pub fn loop_seconds(beats: &[CoverBeat]) -> f64 {
    beats.iter().map(|b| b.seconds).sum()
}

/// The beat a loop is on `elapsed` seconds in, its index, and how long until the next one.
pub fn beat_at(beats: &[CoverBeat], elapsed: f64) -> (CoverBeat, usize, f64) {
    let period = loop_seconds(beats);
    let mut into = elapsed.rem_euclid(period);
    for (index, b) in beats.iter().enumerate() {
        if into < b.seconds {
            return (*b, index, b.seconds - into);
        }
        into -= b.seconds;
    }
    let last = beats.len() - 1;
    (beats[last], last, 0.001)
}

/// The cover's clock: seconds of play, banked while it is held so a cover that rests picks up where it stopped.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CoverClock {
    banked: f64,
    resumed_at: Option<f64>,
}

impl CoverClock {
    /// Tells the clock whether the cover plays at `now` (in the caller's seconds). Starts or stops it as that changes.
    pub fn set_playing(&mut self, playing: bool, now: f64) {
        match (playing, self.resumed_at) {
            (true, None) => self.resumed_at = Some(now),
            (false, Some(at)) => {
                self.banked += (now - at).max(0.0);
                self.resumed_at = None;
            }
            _ => {}
        }
    }

    /// Seconds played by `now`.
    pub fn elapsed(&self, now: f64) -> f64 {
        self.banked + self.resumed_at.map_or(0.0, |at| (now - at).max(0.0))
    }

    /// A clock that has played this long and is held, for a cover drawn at a moment of its loop.
    pub fn at(seconds: f64) -> Self {
        Self { banked: seconds, resumed_at: None }
    }
}

/// The worker's and the thinker's beats at `elapsed`, and how long until either changes: the one wake the cover needs.
pub fn cover_beats(elapsed: f64) -> (CoverBeat, CoverBeat, f64) {
    let (worker, _, w_left) = beat_at(&WORKER, elapsed + WORKER_OFFSET);
    let (thinker, _, t_left) = beat_at(&THINKER, elapsed + THINKER_OFFSET);
    (worker, thinker, w_left.min(t_left))
}

#[cfg(test)]
#[path = "cover_script_tests.rs"]
mod tests;
