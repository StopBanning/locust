//! A shape drawn again with the same outline is not triangulated again (speed-1: a window of drawn faces clipped every
//! outline's ears on each paint, most of its CPU while one face moved), and a new outline is never painted with
//! another's triangles.
use rune::render::{Instance, KIND_TRIANGLE};
use rune::{Color, Cx, El, Env, Runtime, Shape, canvas, el};

struct S {
    points: Vec<[f32; 2]>,
    tick: u32,
}

fn view(s: &S, _: &mut Cx<S>) -> El {
    // A new shade each tick, so the shape is a new one and the canvas paints again.
    let shade = if s.tick % 2 == 0 { 1.0 } else { 0.9 };
    let shape = Shape::new().view_box(200.0, 200.0).fill(s.points.clone(), Color::WHITE.alpha(shade));
    el().size(200.0, 200.0).child(canvas(shape).size(200.0, 200.0))
}

fn star(n: usize) -> Vec<[f32; 2]> {
    (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let r = if i % 2 == 0 { 90.0 } else { 40.0 };
            [100.0 + r * a.cos(), 100.0 + r * a.sin()]
        })
        .collect()
}

fn triangles(instances: &[Instance]) -> Vec<&Instance> {
    instances.iter().filter(|i| i.params[1] == KIND_TRIANGLE).collect()
}

/// This thread's CPU time in seconds: other work on the machine does not count.
fn cpu() -> f64 {
    let mut t = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut t) };
    t.tv_sec as f64 + t.tv_nsec as f64 * 1e-9
}

#[test]
fn an_outline_painted_again_is_not_triangulated_again() {
    let n = 1600;
    let mut rt = Runtime::new(S { points: star(n), tick: 0 }, view, (200.0, 200.0), Env::new(true, 1.0));
    let t = cpu();
    rt.frame(0.0);
    let first = cpu() - t;
    assert_eq!(triangles(&rt.instances).len(), n - 2, "the star in full");
    let t = cpu();
    for tick in 1..=20 {
        rt.update(|s, _| s.tick = tick);
        rt.frame(f64::from(tick) / 60.0);
        assert_eq!(triangles(&rt.instances).len(), n - 2, "painted again, in full");
    }
    let again = cpu() - t;
    assert!(again < first * 4.0, "20 paints of the same outline took {again:.4} s against {first:.4} s for the first");
}

#[test]
fn a_new_outline_with_as_many_points_gets_its_own_triangles() {
    let square = vec![[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
    // Four points too, dented in at its first: the square's diagonal from the second to the fourth lies outside it.
    let dart = vec![[30.0, 50.0], [0.0, 0.0], [100.0, 50.0], [0.0, 100.0]];
    let mut rt = Runtime::new(S { points: square, tick: 0 }, view, (200.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.update(|s, _| s.points = dart);
    rt.frame(1.0 / 60.0);
    let area: f32 = triangles(&rt.instances)
        .iter()
        .map(|i| {
            let (a, b, c) = ([i.radii[0], i.radii[1]], [i.radii[2], i.radii[3]], [i.uv[0], i.uv[1]]);
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5
        })
        .sum();
    assert!((area - 3500.0).abs() < 1.0, "the dart's own area, not the square's triangles over its points: {area}");
}
