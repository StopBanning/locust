//! Electron's `the-stop-button-is-round-and-centred.test.ts` (4 cases, 3 applicable in Rune): the stop button is a
//! borderless round disc whose 1px ring and 50px stop-glow sit inside the edge; it holds a 14×14 rounded square inside
//! the 40×40 seat of the send button; and every round composer button glyph (the 16px send arrow in its 40px 1px-bordered
//! disc, the 14px stop square in its 40px borderless disc, and the 16px plus in its even button) is even in an even box so
//! it lands on whole pixels. Electron's third case checks `<Beam className="lc-stopbeam">` and its `--beam-*-opacity`
//! hooks; Rune draws no package `<Beam>` around the stop button (`the-beams-are-mono-and-quiet.test.tsx`).
use super::controls::*;
use super::*;
use rune::row;

fn drawn_controls(dark: bool) -> Vec<rune::ElementInfo> {
    let mut rt = rune::Runtime::new(
        dark,
        |&dark, _| {
            let l = Look::new(dark);
            row()
                .child(send_circle(&l, SendState::Ready, "Send").key("send"))
                .child(send_circle(&l, SendState::Stop, "Stop").key("stop"))
                .child(head_icon(&icons::PLUS, "Attach files or choose a folder", "Attach files").build(l.k).key("plus"))
        },
        (300.0, 100.0),
        rune::Env::new(dark, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
}

fn by_key<'a>(all: &'a [rune::ElementInfo], key: &str) -> &'a rune::ElementInfo {
    all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element {key}"))
}

fn send_circle_source() -> &'static str {
    let source = include_str!("controls.rs");
    let at = source.find("pub fn send_circle(").expect("send_circle");
    &source[at..at + source[at..].find("\n}\n").unwrap() + 2]
}

// case: is a disc with no border, its ring and glow drawn inside the edge
#[test]
fn is_a_disc_with_no_border_its_ring_and_glow_drawn_inside_the_edge() {
    let body = send_circle_source();
    let stop_arm = body.split("SendState::Stop =>").nth(3).expect("stop styling arm");
    let stop_arm = stop_arm.split("_ =>").next().expect("stop arm before fallback");
    assert!(!stop_arm.contains(".border("), "the stop disc draws no border: {stop_arm}");
    assert!(stop_arm.contains(".radius(999.0)"), "a pill/disc radius: {stop_arm}");
    assert!(stop_arm.contains(r#"ring(1.0, l.c("border-card"))"#), "inset 1px card ring: {stop_arm}");
    assert!(
        stop_arm.contains(r#"Shadow { blur: STOP_GLOW_BLUR, color: l.c("stop-glow"), inset: true"#),
        "inset stop-glow shadow: {stop_arm}"
    );
    assert_eq!(STOP_GLOW_BLUR, 50.0);
    for dark in [true, false] {
        let l = Look::new(dark);
        let all = drawn_controls(dark);
        let stop = by_key(&all, "stop");
        assert_eq!(stop.radius, 999.0);
        assert_eq!(stop.fill, l.c("circle-bg"));
        assert!(l.c("stop-glow").a > 0.0, "stop-glow token resolves in dark={dark}");
    }
}

// case: holds libraries.dev's square, scaled to the send's seat -- 12 of 36 is about 14 of 40
#[test]
fn holds_libraries_devs_square_scaled_to_the_sends_seat_12_of_36_is_about_14_of_40() {
    assert_eq!(SEND_SIZE, 40.0);
    assert_eq!(STOP_SQUARE_SIZE, 14.0);
    assert_eq!(STOP_SQUARE_RADIUS, 3.0);
    let l = Look::new(true);
    let all = drawn_controls(true);
    let send = by_key(&all, "send");
    let stop = by_key(&all, "stop");
    let square = by_key(&all, "stop-square");
    assert_eq!((send.rect[2], send.rect[3]), (40.0, 40.0));
    assert_eq!((stop.rect[2], stop.rect[3]), (40.0, 40.0));
    assert_eq!((square.rect[2], square.rect[3]), (14.0, 14.0));
    assert_eq!(square.radius, 3.0);
    assert_eq!(square.fill, l.c("text-secondary"));
}

// case: is even in an even box, so it lands on whole pixels wherever the row does
#[test]
fn is_even_in_an_even_box_so_it_lands_on_whole_pixels_wherever_the_row_does() {
    // Send: the 16px arrow inside the 40px disc's 1px border.
    assert_eq!(SEND_ARROW_SIZE, 16.0);
    assert_eq!(SEND_BORDER, 1.0);
    assert_eq!((SEND_SIZE - 2.0 * SEND_BORDER - SEND_ARROW_SIZE) % 2.0, 0.0);
    // Stop: the 14px square inside the borderless 40px disc.
    assert_eq!((SEND_SIZE - STOP_SQUARE_SIZE) % 2.0, 0.0);

    let all = drawn_controls(true);
    let send = by_key(&all, "send");
    let arrow = by_key(&all, "send-arrow");
    assert_eq!((arrow.rect[2], arrow.rect[3]), (16.0, 16.0));
    assert_eq!((send.rect[2] - 2.0 * SEND_BORDER - arrow.rect[2]) % 2.0, 0.0);
    assert_eq!(arrow.rect[0] - send.rect[0], 12.0);
    assert_eq!(arrow.rect[1] - send.rect[1], 12.0);

    let stop = by_key(&all, "stop");
    let square = by_key(&all, "stop-square");
    assert_eq!((stop.rect[2] - square.rect[2]) % 2.0, 0.0);
    assert_eq!(square.rect[0] - stop.rect[0], 13.0);
    assert_eq!(square.rect[1] - stop.rect[1], 13.0);

    // The +: an even glyph inside an even button, landing on whole pixels from its button's edge.
    let plus = by_key(&all, "plus");
    let plus_at = all.iter().position(|e| e.key.as_deref() == Some("plus")).unwrap();
    let plus_icon = all.iter().find(|e| e.parent == Some(plus_at) && e.key.as_deref() == Some("icon")).expect("plus icon");
    assert_eq!(plus.rect[2] % 2.0, 0.0, "plus box width {}", plus.rect[2]);
    assert_eq!(plus_icon.rect[2] % 2.0, 0.0, "plus glyph width {}", plus_icon.rect[2]);
    assert_eq!((plus.rect[2] - plus_icon.rect[2]) % 2.0, 0.0);
    assert_eq!((plus_icon.rect[0] - plus.rect[0]).fract(), 0.0);
    assert_eq!((plus_icon.rect[1] - plus.rect[1]).fract(), 0.0);
}
