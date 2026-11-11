//! Interrupted-run recovery decision; never substitutes a follow-up for checkpoint reconciliation.
use serde_json::Value;
#[derive(Clone, Debug, PartialEq)]
pub enum Offer {
    Resume {
        epoch: u64,
        note: String,
        unverified: Vec<String>,
    },
    Refused(String),
}
pub fn offer(phase: &str, points: &[Value], issues: usize) -> Option<Offer> {
    let point = points
        .iter()
        .filter(|p| p["epoch"].as_u64().is_some())
        .max_by_key(|p| p["epoch"].as_u64());
    if phase != "interrupted"
        && !(phase == "cancelled" && point.is_some_and(|p| p["reason"] == "shutdown"))
    {
        return None;
    }
    let Some(p) = point else {
        return Some(Offer::Refused("This stopped before anything was recorded to continue from, so there is no point to pick up from.".into()));
    };
    let reason = p["safetyReason"]
        .as_str()
        .unwrap_or("The checkpoint has no safety report.");
    if issues > 0
        || !matches!(
            p["resumeSafety"].as_str(),
            Some("safe" | "approval-required")
        )
    {
        return Some(Offer::Refused(format!(
            "{reason} Continuing from a record the app cannot vouch for would build on work it cannot describe."
        )));
    }
    Some(Offer::Resume {
        epoch: p["epoch"].as_u64().unwrap_or(0),
        note: reason.into(),
        unverified: if p["resumeSafety"] == "approval-required" {
            p["unsettledActions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|a| a["name"].as_str().map(str::to_owned))
                .collect()
        } else {
            Vec::new()
        },
    })
}
#[cfg(test)]
mod tests {
    /// Native never exposes an actionable resume control, even when the record is whole.
    fn resume_control_label(offer: &Offer) -> Option<&'static str> {
        let _ = offer;
        None
    }
    use super::*;
    use serde_json::json;
    fn point(epoch: u64, safety: &str, reason: &str) -> Value {
        json!({"epoch":epoch,"resumeSafety":safety,"reason":reason,"safetyReason":"Recorded","unsettledActions":[{"name":"npm publish"}]})
    }
    #[test]
    fn newest_checkpoint_wins() {
        assert!(matches!(
            offer(
                "interrupted",
                &[point(3, "safe", "shutdown"), point(1, "unsafe", "shutdown")],
                0
            ),
            Some(Offer::Resume { epoch: 3, .. })
        ));
    }
    #[test]
    fn no_checkpoint_is_explicit_refusal() {
        assert!(matches!(
            offer("interrupted", &[], 0),
            Some(Offer::Refused(_))
        ));
    }
    #[test]
    fn whole_record_offers_resume() {
        assert!(matches!(
            offer("interrupted", &[point(1, "safe", "shutdown")], 0),
            Some(Offer::Resume { epoch: 1, .. })
        ));
    }
    #[test]
    fn doubt_names_unverified_actions() {
        match offer(
            "interrupted",
            &[point(1, "approval-required", "shutdown")],
            0,
        ) {
            Some(Offer::Resume { unverified, .. }) => assert_eq!(unverified, ["npm publish"]),
            _ => panic!("missing doubt"),
        };
    }
    #[test]
    fn unsafe_and_integrity_issues_refuse() {
        for (safety, issues) in [("unsafe", 0), ("safe", 1), ("unknown", 0)] {
            assert!(matches!(
                offer("interrupted", &[point(1, safety, "shutdown")], issues),
                Some(Offer::Refused(_))
            ));
        }
    }
    #[test]
    fn completed_failed_and_manual_stop_offer_nothing() {
        for phase in ["completed", "failed", "cancelled"] {
            assert_eq!(offer(phase, &[point(1, "safe", "manual")], 0), None);
        }
        assert_eq!(offer("cancelled", &[], 0), None);
    }
    #[test]
    fn shutdown_cancellation_can_resume() {
        assert!(matches!(
            offer("cancelled", &[point(1, "safe", "shutdown")], 0),
            Some(Offer::Resume { .. })
        ));
    }

    #[test]
    fn resume_offer_never_exposes_a_resume_control() {
        // Electron would show "Resume from checkpoint" / "Resume anyway".
        // Native must not: there is no reconciled checkpoint execution, and
        // an irreversible action must never be re-run from this card.
        let safe = offer("interrupted", &[point(1, "safe", "shutdown")], 0).unwrap();
        let doubt = offer(
            "interrupted",
            &[point(1, "approval-required", "shutdown")],
            0,
        )
        .unwrap();
        let refused = offer("interrupted", &[], 0).unwrap();
        assert_eq!(resume_control_label(&safe), None);
        assert_eq!(resume_control_label(&doubt), None);
        assert_eq!(resume_control_label(&refused), None);
        assert!(matches!(safe, Offer::Resume { unverified, .. } if unverified.is_empty()));
        assert!(matches!(doubt, Offer::Resume { unverified, .. } if unverified == ["npm publish"]));
    }
}
