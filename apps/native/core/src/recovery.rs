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
/// The checkpoint a resume would start from: the newest epoch written.
pub fn resume_point(points: &[Value]) -> Option<&Value> {
    points
        .iter()
        .filter(|p| p["epoch"].as_u64().is_some())
        .max_by_key(|p| p["epoch"].as_u64())
}

pub fn offer(phase: &str, points: &[Value], issues: usize) -> Option<Offer> {
    let point = resume_point(points);
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
/// ResumeCard.tsx: the control a Resume offer shows, in Electron's words: "Resume from checkpoint", or "Resume anyway"
/// when an action never reported back (the caution beside it, `RESUME_CAUTION`). A refusal has none.
pub fn resume_control(offer: &Offer) -> Option<&'static str> {
    match offer {
        Offer::Resume { unverified, .. } if unverified.is_empty() => Some("Resume from checkpoint"),
        Offer::Resume { .. } => Some("Resume anyway"),
        Offer::Refused(_) => None,
    }
}

/// ResumeCard.tsx: said beside "Resume anyway".
pub const RESUME_CAUTION: &str = "Check those first — the next run is told to verify them before building on them.";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn point(epoch: u64, safety: &str, reason: &str) -> Value {
        json!({"epoch":epoch,"resumeSafety":safety,"reason":reason,"safetyReason":"Recorded","unsettledActions":[{"name":"npm publish"}]})
    }
    // case: takes the newest, not the first
    #[test]
    fn takes_the_newest_not_the_first() {
        let points = [
            point(1, "unsafe", "shutdown"),
            point(3, "safe", "shutdown"),
            point(2, "unsafe", "shutdown"),
        ];
        assert_eq!(resume_point(&points).and_then(|p| p["epoch"].as_u64()), Some(3));
        assert!(matches!(
            offer("interrupted", &points, 0),
            Some(Offer::Resume { epoch: 3, .. })
        ));
    }

    // case: has nothing to offer when none were written
    #[test]
    fn has_nothing_to_offer_when_none_were_written() {
        assert_eq!(resume_point(&[]), None);
    }

    // case: offers a resume when the record is whole
    #[test]
    fn offers_a_resume_when_the_record_is_whole() {
        assert!(matches!(
            offer("interrupted", &[point(1, "safe", "shutdown")], 0),
            Some(Offer::Resume { epoch: 1, unverified, .. }) if unverified.is_empty()
        ));
    }

    // case: offers it WITH the doubt named when an action never reported back
    #[test]
    fn offers_it_with_the_doubt_named_when_an_action_never_reported_back() {
        match offer(
            "interrupted",
            &[point(1, "approval-required", "shutdown")],
            0,
        ) {
            Some(Offer::Resume { unverified, .. }) => assert_eq!(unverified, ["npm publish"]),
            _ => panic!("missing doubt"),
        };
    }

    // case: refuses when the ledger cannot be trusted to say what happened
    #[test]
    fn refuses_when_the_ledger_cannot_be_trusted_to_say_what_happened() {
        assert!(matches!(
            offer("interrupted", &[point(1, "unsafe", "shutdown")], 0),
            Some(Offer::Refused(_))
        ));
        assert!(matches!(
            offer("interrupted", &[point(1, "unknown", "shutdown")], 0),
            Some(Offer::Refused(_))
        ));
    }

    // case: refuses when recovery reported an issue, whatever the checkpoint concluded
    #[test]
    fn refuses_when_recovery_reported_an_issue_whatever_the_checkpoint_concluded() {
        assert!(matches!(
            offer("interrupted", &[point(1, "safe", "shutdown")], 1),
            Some(Offer::Refused(_))
        ));
    }

    // case: says so plainly when nothing was written before the stop
    #[test]
    fn says_so_plainly_when_nothing_was_written_before_the_stop() {
        match offer("interrupted", &[], 0) {
            Some(Offer::Refused(note)) => {
                assert!(note.contains("no point to pick up from"), "{note}");
            }
            other => panic!("expected Refused, got {other:?}"),
        }
    }

    // case: offers nothing for a %s mission
    #[test]
    fn offers_nothing_for_a_completed_or_failed_mission() {
        for phase in ["completed", "failed"] {
            assert_eq!(offer(phase, &[point(1, "safe", "shutdown")], 0), None);
        }
    }

    // case: offers a resume for a run the app closed underneath
    #[test]
    fn offers_a_resume_for_a_run_the_app_closed_underneath() {
        assert!(matches!(
            offer("cancelled", &[point(1, "safe", "shutdown")], 0),
            Some(Offer::Resume { epoch: 1, .. })
        ));
    }

    // case: offers nothing to someone who pressed Stop
    #[test]
    fn offers_nothing_to_someone_who_pressed_stop() {
        assert_eq!(offer("cancelled", &[point(1, "safe", "manual")], 0), None);
    }

    // case: offers nothing for a cancelled run that wrote no checkpoint at all
    #[test]
    fn offers_nothing_for_a_cancelled_run_that_wrote_no_checkpoint_at_all() {
        assert_eq!(offer("cancelled", &[], 0), None);
    }

    #[test]
    fn transcript_apply_computes_recovery_offer_from_newest_checkpoint() {
        let mut transcript = crate::ledger::Transcript::default();
        transcript.apply(vec![
            json!({ "event": { "sequence": 1, "type": "run.started", "payload": {} } }),
            json!({ "recordType": "mission.checkpoint", "checkpoint": point(1, "unsafe", "shutdown") }),
            json!({ "recordType": "mission.checkpoint", "checkpoint": point(3, "safe", "shutdown") }),
            json!({ "recordType": "mission.checkpoint", "checkpoint": point(2, "unsafe", "shutdown") }),
            json!({ "event": { "sequence": 2, "type": "run.cancelled", "payload": {} } }),
        ]);
        assert!(matches!(
            transcript.recovery,
            Some(Offer::Resume { epoch: 3, .. })
        ));
    }

    /// ResumeCard.tsx: "Resume from checkpoint" for a whole record, "Resume anyway" beside the caution when an action
    /// never reported back, and no button for a refusal (a refusal is information, not a resume you cannot click yet).
    #[test]
    fn the_control_is_resume_from_checkpoint_or_resume_anyway_and_a_refusal_has_none() {
        let safe = offer("cancelled", &[point(1, "safe", "shutdown")], 0).unwrap();
        let doubt = offer("cancelled", &[point(1, "approval-required", "shutdown")], 0).unwrap();
        let refused = offer("cancelled", &[point(1, "unsafe", "shutdown")], 0).unwrap();
        assert_eq!(resume_control(&safe), Some("Resume from checkpoint"));
        assert_eq!(resume_control(&doubt), Some("Resume anyway"));
        assert_eq!(resume_control(&refused), None);
        assert!(matches!(doubt, Offer::Resume { unverified, .. } if unverified == ["npm publish"]));
        assert_eq!(RESUME_CAUTION, "Check those first — the next run is told to verify them before building on them.");
    }
}
