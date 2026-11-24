//! The lockup powering on: PoweredLockup.tsx and its keyframes in shell.css (`lcLockupPower`, `lcLockupRelight`,
//! `lcLockupGlow`, `lcLockupTube`, `lcLockupSweep`), as values of the time since a lighting began. Home draws them on
//! the cover's glass (`ui/home_panel.rs::cover`) and steps them from its own poll; nothing here keeps a clock.

/// Which lighting is playing: the power-on from dark, or a relight from lit (`is-powering`, `is-relighting`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lighting {
    Power,
    Relight,
}

impl Lighting {
    /// How long it plays: its own animation's length, the one whose end clears the class (`LIGHTINGS`).
    pub fn seconds(self) -> f64 {
        match self {
            Lighting::Power => 2.6,
            Lighting::Relight => 2.2,
        }
    }
}

/// One frame of the lockup's lighting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LockupFrame {
    /// The whole lockup's opacity (`lcLockupPower` / `lcLockupRelight`).
    pub ink: f32,
    /// The lime copy laid over it (`lcLockupGlow` on `.lc-lockup__lit`).
    pub lit: f32,
    /// The tube's scanlines and sweep (`lcLockupTube`).
    pub tube: f32,
    /// Where the sweep is, as a share of its own height (`lcLockupSweep`'s translateY: -1.1 to 2.4).
    pub sweep: f32,
}

/// The finished lockup: what reduced motion draws, and what a lighting ends on.
pub const STILL: LockupFrame = LockupFrame { ink: 1.0, lit: 0.0, tube: 0.0, sweep: -1.1 };

const POWER: &[(f64, f64)] = &[(0.0, 0.0), (0.05, 1.0), (0.08, 0.3), (0.11, 1.0), (0.16, 0.72), (0.19, 1.0), (1.0, 1.0)];
const RELIGHT: &[(f64, f64)] = &[(0.0, 1.0), (0.10, 0.8), (0.14, 1.0), (1.0, 1.0)];
const GLOW: &[(f64, f64)] = &[(0.0, 0.0), (0.08, 0.0), (0.19, 1.0), (0.50, 1.0), (1.0, 0.0)];
const TUBE: &[(f64, f64)] = &[(0.0, 0.0), (0.06, 1.0), (0.58, 1.0), (1.0, 0.0)];
const TUBE_SECONDS: f64 = 2.4;
const SWEEP_DELAY: f64 = 0.25;
const SWEEP_SECONDS: f64 = 1.2;

/// CSS `ease-out`, cubic-bezier(0, 0, 0.58, 1), applied within each keyframe's own span as CSS applies it.
fn ease_out(x: f64) -> f64 {
    let (x1, x2) = (0.0, 0.58);
    let (y1, y2) = (0.0, 1.0);
    let bez = |t: f64, a: f64, b: f64| 3.0 * (1.0 - t) * (1.0 - t) * t * a + 3.0 * (1.0 - t) * t * t * b + t * t * t;
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if bez(mid, x1, x2) < x { lo = mid } else { hi = mid }
    }
    bez((lo + hi) / 2.0, y1, y2)
}

fn keyframes(frames: &[(f64, f64)], at: f64) -> f64 {
    let at = at.clamp(0.0, 1.0);
    for pair in frames.windows(2) {
        let ((a, va), (b, vb)) = (pair[0], pair[1]);
        if at <= b {
            let span = b - a;
            let x = if span <= 0.0 { 1.0 } else { (at - a) / span };
            return va + (vb - va) * ease_out(x);
        }
    }
    frames.last().map_or(1.0, |f| f.1)
}

/// The lighting `age` seconds in, or `None` once it has ended (and Home stops asking for frames).
pub fn lockup_frame(kind: Lighting, age: f64) -> Option<LockupFrame> {
    let length = kind.seconds();
    if !(0.0..length).contains(&age) {
        return None;
    }
    let ink = keyframes(if kind == Lighting::Power { POWER } else { RELIGHT }, age / length);
    let sweep_at = ((age - SWEEP_DELAY) / SWEEP_SECONDS).clamp(0.0, 1.0);
    Some(LockupFrame {
        ink: ink as f32,
        lit: keyframes(GLOW, age / length) as f32,
        tube: keyframes(TUBE, age / TUBE_SECONDS) as f32,
        sweep: (-1.1 + 3.5 * sweep_at) as f32,
    })
}

/// The wait before the next relight, in milliseconds: 7 to 10 seconds after the last one ended (`nextLightingDelay`).
/// `random` is a draw in 0..1; one out of range still lands inside the window.
pub fn next_lighting_delay(random: f64) -> u64 {
    7_000 + (random.clamp(0.0, 1.0) * 3_000.0).round() as u64
}

/// A draw in 0..1 for `next_lighting_delay`, from the clock's nanoseconds: enough that two waits differ.
pub fn clock_draw() -> f64 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.subsec_nanos());
    f64::from(nanos % 1_000_000) / 1_000_000.0
}

#[cfg(test)]
#[path = "lockup_tests.rs"]
mod tests;
