use super::*;
use serde_json::Value;
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub used: u64,
    pub window: u64,
    pub percent: u64,
}
impl Reading {
    pub fn from_usage(v: &Value) -> Option<Self> {
        let used = v.get("contextTokens")?.as_u64()?;
        let window = v.get("contextWindow")?.as_u64()?;
        if used == 0 || window == 0 {
            return None;
        }
        Some(Self {
            used,
            window,
            percent: ((used as f64 / window as f64 * 100.0).round() as u64).min(100),
        })
    }
    pub fn sentence(&self) -> String {
        format!(
            "Context: {} of {} used, {}%",
            compact_count(self.used),
            compact_count(self.window),
            self.percent
        )
    }
}

/// Electron `usageView.compactCount`: 1.1B, 3.1M, 410k, 920.
pub fn compact_count(value: u64) -> String {
    const UNITS: [(u64, &str); 3] = [(1_000_000_000, "B"), (1_000_000, "M"), (1_000, "k")];
    for (index, (size, unit)) in UNITS.iter().enumerate() {
        if value < *size {
            continue;
        }
        let scaled = value as f64 / *size as f64;
        let text = if scaled >= 9.95 {
            format!("{scaled:.0}")
        } else {
            format!("{scaled:.1}")
        };
        if text == "1000"
            && let Some((larger_size, larger_unit)) =
                index.checked_sub(1).and_then(|i| UNITS.get(i)).copied()
        {
            let _ = larger_size;
            return format!("1.0{larger_unit}");
        }
        return format!("{text}{unit}");
    }
    value.to_string()
}
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
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn only_runtime_measured_context_is_shown() {
        assert!(Reading::from_usage(&json!({"inputTokens":100,"contextWindow":200})).is_none());
        assert!(Reading::from_usage(&json!({"contextTokens":100,"contextWindow":0})).is_none());
        let r =
            Reading::from_usage(&json!({"contextTokens":164000,"contextWindow":200000})).unwrap();
        assert_eq!(r.sentence(), "Context: 164k of 200k used, 82%");
        assert_eq!(
            Reading::from_usage(&json!({"contextTokens":300,"contextWindow":100}))
                .unwrap()
                .percent,
            100
        );
    }

    #[test]
    fn compact_count_matches_electron_words() {
        assert_eq!(compact_count(920), "920");
        assert_eq!(compact_count(1_000), "1.0k");
        assert_eq!(compact_count(1_500), "1.5k");
        assert_eq!(compact_count(410_000), "410k");
        assert_eq!(compact_count(999_500), "1.0M");
        assert_eq!(compact_count(1_100_000), "1.1M");
        assert_eq!(compact_count(9_950_000), "10M");
        assert_eq!(compact_count(999_600_000), "1.0B");
        assert_eq!(compact_count(1_133_000_000), "1.1B");
    }
}

#[cfg(test)]
mod ledger_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn compaction_is_a_line_and_invalidates_context() {
        let mut t = ledger::Transcript::default();
        let event = |n, kind: &str, payload: Value| json!({"recordType":"mission.event","event":{"sequence":n,"type":kind,"payload":payload}});
        t.apply(vec![event(
            1,
            "run.completed",
            json!({"usage":{"contextTokens":164000,"contextWindow":200000}}),
        )]);
        assert_eq!(t.context.as_ref().unwrap().percent, 82);
        t.apply(vec![event(2,"adapter.diagnostic",json!({"code":"opencode.context_compacted","level":"info","message":"The conversation outgrew the model's context, so OpenCode summarized it and carried on from the summary."}))]);
        assert!(t.context.is_none());
        assert!(
            matches!(&t.items[..],[ledger::Item::Note {level,text}] if level.is_empty() && text.contains("OpenCode summarized"))
        );
        t.apply(vec![event(
            3,
            "run.completed",
            json!({"usage":{"contextTokens":10,"contextWindow":100}}),
        )]);
        assert_eq!(t.context.as_ref().unwrap().percent, 10);
    }
}
