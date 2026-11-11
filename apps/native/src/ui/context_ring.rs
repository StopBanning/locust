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
    Some(
        row()
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
            ),
    )
}
