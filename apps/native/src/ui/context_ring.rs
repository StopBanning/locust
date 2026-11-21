//! The context ring beside the composer: how full the model's context is, drawn from the engine's reading.
use super::*;

pub(super) fn control(s: &State, p: &Palette) -> Option<El> {
    let reading = s.ledger.as_ref()?.snapshot.context.as_ref()?;
    let tone = if reading.percent >= 80 {
        p.amber
    } else {
        p.muted
    };
    let mut shape = rune::Shape::new();
    if let Some(path) = locust_core::context_usage::pie_slice_path(reading.percent as f64, 7.0, 4.5) {
        for points in locust_core::bot_spec::parse_svg_subpaths(&path) {
            let mut path = rune::Path::new();
            for (index, point) in points.iter().enumerate() {
                let point = [point[0] as f32, point[1] as f32];
                path = if index == 0 { path.move_to(point) } else { path.line_to(point) };
            }
            shape = shape.fill_path(&path.close(), tone);
        }
    }
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
                text(locust_core::context_usage::ring_sentence(reading, s.ledger.as_ref().and_then(|ledger| ledger.snapshot.cost.as_ref())))
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


#[cfg(test)]
mod renderer_tests {
    use super::*;

    #[test]
    fn the_drawn_ring_uses_the_conversations_cost_and_hides_subscription_only_cost() {
        let mut s = State::default();
        s.ledger = Some(LedgerPane {
            cold_tip: None,
            header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(),
                runtime: Some("claude".into()), model: None, account: None, busy: false,
                runs_in: "ask".into(), folder: None },
            conversation: Conversation { conversation_id: Some("m".into()), turns: vec![] },
            items: vec![], dir: std::path::PathBuf::new(), worker: None,
            snapshot: ledger::Snapshot {
                context: Some(locust_core::context_usage::Reading { used: 236757, window: 1000000, percent: 24 }),
                cost: Some(locust_core::benchmark::RunCost { plan: true, ..Default::default() }),
                ..Default::default()
            },
            due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
        });
        let mut rt = rune::Runtime::new(s, |s, _| control(s, &Palette::new(true)).unwrap(),
            (600.0, 100.0), rune::Env::new(true, 1.0));
        rt.frame(0.0);
        let words = |rt: &rune::Runtime<State>| rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join(" ");
        assert_eq!(words(&rt), "Context: 237k of 1.0M used, 24%");
        rt.update(|s, _| s.ledger.as_mut().unwrap().snapshot.cost.as_mut().unwrap().usd = Some(0.4));
        rt.frame(0.0);
        assert_eq!(words(&rt), "Context: 237k of 1.0M used, 24% — This conversation: $0.40");
    }
}
