//! LIVE2 #1: `--scene math` draws the question, prose, and math equations visibly inside the conversation viewport.
use super::*;
use rune::{Env, Runtime};
use std::sync::Arc;

#[test]
fn math_scene_draws_question_answer_and_equations_on_screen() {
    let mut s = State::default();
    s.image_cache = Some(Arc::new(rune::ImageCache::new(Arc::new(|| {}))));
    crate::scenes::math_scene(&mut s);

    let mut rt = Runtime::new(s, crate::view, (1200.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);

    let elements = rt.elements();
    let msgs_rect = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("messages"))
        .map(|e| e.rect)
        .expect("messages viewport");
    let visible = |r: [f32; 4]| {
        r[2] > 0.0
            && r[3] > 0.0
            && r[1] + r[3] > msgs_rect[1]
            && r[1] < msgs_rect[1] + msgs_rect[3]
    };

    let question = elements
        .iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.contains("Show the linear algebra.")));
    assert!(
        question.is_some_and(|e| visible(e.rect)),
        "question must be visible in viewport {:?}, got {:?}",
        msgs_rect,
        question.map(|e| e.rect)
    );

    let prose = elements
        .iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.contains("The first coefficient")));
    assert!(
        prose.is_some_and(|e| visible(e.rect)),
        "answer prose must be visible in viewport {:?}, got {:?}",
        msgs_rect,
        prose.map(|e| e.rect)
    );

    let tail = elements
        .iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.contains("Money like $5 and $10 stays prose.")));
    assert!(
        tail.is_some_and(|e| visible(e.rect)),
        "trailing prose must be visible in viewport {:?}, got {:?}",
        msgs_rect,
        tail.map(|e| e.rect)
    );

    let reply_item = elements
        .iter()
        .find(|e| e.key.as_deref() == Some("synthetic-ledger/ledger-item-1"))
        .expect("assistant reply item");
    assert!(
        reply_item.rect[3] < 450.0,
        "reply item must not inflate with phantom blank height, got {:?}",
        reply_item.rect
    );

    let math_imgs: Vec<_> = elements
        .iter()
        .filter(|e| e.role == "image" && e.key.as_deref().is_some_and(|k| k == "b1" || k == "b3"))
        .collect();
    assert_eq!(math_imgs.len(), 2, "both inline and display math images must be present");
    for img in &math_imgs {
        assert!(visible(img.rect), "math image {:?} must be visible in viewport {:?}", img.rect, msgs_rect);
    }
    // 2x retina PNGs are drawn at half their pixel dimensions, not double size.
    assert!(math_imgs[0].rect[2] < 70.0 && math_imgs[0].rect[3] < 24.0, "inline math too large: {:?}", math_imgs[0].rect);

    // Wide equations clamp to max_w preserving aspect ratio, and zero dimensions are rejected.
    assert_eq!(crate::reply_actions::math_image_size([0, 40], 840.0), None);
    assert_eq!(crate::reply_actions::math_image_size([40, 0], 840.0), None);
    assert_eq!(crate::reply_actions::math_image_size([200, 100], 840.0), Some((100.0, 50.0)));
    assert_eq!(crate::reply_actions::math_image_size([2000, 200], 500.0), Some((500.0, 50.0)));
}
