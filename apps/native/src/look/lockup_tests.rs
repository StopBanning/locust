//! The lockup's keyframes, read against shell.css's numbers (`lcLockupPower`, `lcLockupRelight`, `lcLockupGlow`,
//! `lcLockupTube`, `lcLockupSweep`). The drawn cases are in ui/home_lockup_tests.rs.
use super::*;

fn samples(kind: Lighting) -> Vec<LockupFrame> {
    (0..260).filter_map(|i| lockup_frame(kind, f64::from(i) / 100.0)).collect()
}

#[test]
fn the_power_on_starts_dark_flickers_and_ends_lit() {
    let first = lockup_frame(Lighting::Power, 0.0).unwrap();
    assert!(first.ink.abs() < 1e-6 && first.lit.abs() < 1e-6 && first.tube.abs() < 1e-6, "0% {{ opacity: 0 }}: {first:?}");
    // 8%: the flicker down to 0.3, 5% and 11%: lit.
    assert!((lockup_frame(Lighting::Power, 0.08 * 2.6).unwrap().ink - 0.3).abs() < 1e-3);
    assert!((lockup_frame(Lighting::Power, 0.11 * 2.6).unwrap().ink - 1.0).abs() < 1e-3);
    // 19% to 50%: the lime copy is wholly lit.
    assert!((lockup_frame(Lighting::Power, 0.3 * 2.6).unwrap().lit - 1.0).abs() < 1e-3);
    let last = lockup_frame(Lighting::Power, 2.599).unwrap();
    assert!(last.ink > 0.999 && last.lit < 0.01, "{last:?}");
    assert_eq!(lockup_frame(Lighting::Power, 2.6), None, "2.6 s, and it has ended");
}

#[test]
fn a_relight_never_dips_under_0_7_and_ends_at_2_2_s() {
    let all = samples(Lighting::Relight);
    assert!(all.iter().all(|f| f.ink >= 0.7), "{:?}", all.iter().map(|f| f.ink).fold(1.0_f32, f32::min));
    assert!(all.iter().any(|f| f.ink < 0.85), "it does flicker: 10% {{ opacity: 0.8 }}");
    assert_eq!(lockup_frame(Lighting::Relight, 2.2), None);
}

#[test]
fn the_sweep_waits_a_quarter_second_then_passes_down_in_1_2_s() {
    assert!((lockup_frame(Lighting::Power, 0.2).unwrap().sweep + 1.1).abs() < 1e-4, "translateY(-110%) before it starts");
    assert!((lockup_frame(Lighting::Power, 1.45).unwrap().sweep - 2.4).abs() < 1e-4, "translateY(240%) at its end");
    // The tube is lit by 6% of 2.4 s and out at 2.4 s.
    assert!((lockup_frame(Lighting::Power, 0.06 * 2.4).unwrap().tube - 1.0).abs() < 1e-3);
    assert!(lockup_frame(Lighting::Power, 2.45).unwrap().tube < 1e-3);
}
