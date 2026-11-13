//! The context ring beside the composer: how full the model's context is, drawn from the engine's reading.
use super::*;

pub(super) fn control(s: &State, p: &Palette) -> Option<El> {
    let reading = s.ledger.as_ref()?.snapshot.context.as_ref()?;
    let tone = if reading.percent >= 80 {
        p.amber
    } else {
        p.muted
    };
    let mut path = rune::Path::new().move_to([7.0, 7.0]).line_to([7.0, 2.5]);
    for n in 0..=reading.percent {
        let angle = n as f32 / 100.0 * std::f32::consts::TAU;
        path = path.line_to([7.0 + 4.5 * angle.sin(), 7.0 - 4.5 * angle.cos()]);
    }
    let shape = rune::Shape::new().fill_path(&path.close(), tone);
    let ring = row()
            .key("context-ring")
            .gap(6.0)
            .center_y()
            .child(
                rune::canvas(shape)
                    .size(14.0, 14.0)
                    .radius(7.0)
                    .border(1.0, p.muted),
            )
            .child(
                text(reading.sentence())
                    .font(type_::LABEL)
                    .color(tone)
                    .wrap_anywhere(),
            );
    // Pressing the ring opens what it says, with Compact now beneath (0.797), where the runtime can compact.
    if crate::runtime_commands_ui::offer(s).is_none() {
        return Some(ring);
    }
    let ring = ring.role("button").label("Context").on_press(|s: &mut State| s.runtime_commands.ring_open = !s.runtime_commands.ring_open);
    Some(match crate::runtime_commands_ui::menu(s, p) {
        Some(menu) => col().gap(6.0).child(menu).child(ring),
        None => ring,
    })
}
