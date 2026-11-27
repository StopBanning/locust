//! A live drawing (`canvas_live`) paints its latest contents each frame, with no commit: an app moves a
//! drawing from `poll` without the view running again.
use super::*;
use crate::{LiveShape, Shape, canvas_live};

fn bar(w: f32) -> Shape {
    Shape::new().fill(vec![[0.0, 0.0], [w, 0.0], [w, 4.0], [0.0, 4.0]], Color::WHITE)
}

fn rects<S: 'static>(rt: &Runtime<S>) -> Vec<[f32; 4]> {
    rt.instances.iter().map(|i| i.rect).collect()
}

fn moves_with_no_commit(cached: bool) {
    struct S { live: LiveShape, cached: bool, builds: std::cell::Cell<u32> }
    fn view(s: &S, _: &mut Cx<S>) -> El {
        s.builds.set(s.builds.get() + 1);
        let face = canvas_live(&s.live).key("face").size(40.0, 40.0);
        let holder = el().key("holder").size(60.0, 60.0).child(face);
        col().child(if s.cached { holder.cache() } else { holder })
    }
    let live = LiveShape::new(bar(10.0));
    let mut rt = Runtime::new(S { live: live.clone(), cached, builds: Default::default() }, view, (100.0, 100.0), Env::new(true, 1.0));
    for _ in 0..5 {
        rt.frame(1.0 / 60.0);
    }
    let (commits, builds, before) = (rt.stats.commits, rt.state.builds.get(), rects(&rt));
    assert!(live.set(bar(30.0)), "a new drawing");
    assert!(!live.set(bar(30.0)), "the same drawing again changes nothing");
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.stats.commits, commits, "no commit");
    assert_eq!(rt.state.builds.get(), builds, "the view did not run");
    assert_ne!(rects(&rt), before, "the frame painted the new drawing");
    // A rebuild for another reason keeps painting what it holds now.
    let now = rects(&rt);
    rt.update(|_, _| {});
    rt.frame(1.0 / 60.0);
    assert_eq!(rects(&rt), now);
}

#[test]
fn a_live_drawing_moves_with_no_commit() {
    moves_with_no_commit(false);
}

#[test]
fn a_live_drawing_under_a_cached_layer_moves_with_no_commit() {
    moves_with_no_commit(true);
}
