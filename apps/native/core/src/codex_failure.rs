//! What a Codex failure message says went wrong (Electron `codex-events.ts` `failureKind`, shared with the app-server
//! transport). The app-server's `codexErrorInfo` decides first; when it names nothing, the message's own words do.
//! Codex's words for a sign-in that can no longer be refreshed named no pattern, so the failure read as "unknown" and
//! the card offered no sign-in (QA-2026-09-29 round 2, N11).

use crate::invariants::regex;

const QUOTA: &[&str] = &[
    r"(?i)\byou(?:'ve| have) hit your usage limit\b",
    r"(?i)\b(?:usage|quota) limit (?:has been )?(?:reached|exceeded)\b",
    r"(?i)\b(?:quota|usage allowance) (?:is )?exhausted\b",
    r"(?i)\binsufficient_quota\b",
];
const RATE_LIMIT: &[&str] = &[r"(?i)\brate[_ -]?limit(?:ed| exceeded)?\b", r"(?i)\btoo many requests\b", r"(?i)\bhttp\s*429\b"];
const AUTHENTICATION: &[&str] = &[
    r"(?i)\bauthentication (?:failed|required)\b",
    r"(?i)\bunauthorized\b",
    r"(?i)\binvalid api key\b",
    r"(?i)\blogin (?:required|expired)\b",
    r"(?i)\baccess token could not be refreshed\b",
    r"(?i)\bsign in again\b",
];
const SAFETY: &[&str] = &[r"(?i)\bsafety (?:policy|refusal|block)\b", r"(?i)\bblocked by policy\b", r"(?i)\bcontent policy\b"];

fn any(patterns: &[&'static str], message: &str) -> bool {
    patterns.iter().any(|p| regex(p).is_match(message))
}

/// The kind a Codex failure message names: a route limit first, then a failed sign-in, then a safety block.
pub fn failure_kind(message: Option<&str>) -> &'static str {
    let Some(message) = message else { return "unknown" };
    if any(QUOTA, message) {
        return "quota-exhausted";
    }
    if any(RATE_LIMIT, message) {
        return "temporary-rate-limit";
    }
    if any(AUTHENTICATION, message) {
        return "authentication-failed";
    }
    if any(SAFETY, message) {
        return "safety-blocked";
    }
    "unknown"
}

/// The app-server's failed turn (app-server-events.ts): `codexErrorInfo` by its v2 names, else the message's words.
pub fn app_server_failure_kind(info: &str, message: &str) -> &'static str {
    match info {
        "usageLimitExceeded" => "quota-exhausted",
        "rateLimitExceeded" => "temporary-rate-limit",
        "unauthorized" => "authentication-failed",
        "cyberPolicy" | "misalignmentPolicyViolation" => "safety-blocked",
        _ => failure_kind(Some(message)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- packages/runtime-adapters/test/an-expired-sign-in-is-said.test.ts ---

    #[test]
    fn codexs_words_for_an_expired_sign_in_read_as_a_failed_sign_in() {
        assert_eq!(failure_kind(Some("Your access token could not be refreshed. Please log out and sign in again.")), "authentication-failed");
        assert_eq!(failure_kind(Some("Your access token could not be refreshed because your refresh token was already used.")), "authentication-failed");
    }

    #[test]
    fn and_other_failures_do_not() {
        assert_ne!(failure_kind(Some("The model returned an unexpected response.")), "authentication-failed");
    }

    // The corners of the patterns, each pinned.

    #[test]
    fn a_route_limit_is_said_before_anything_else_and_only_in_its_own_words() {
        assert_eq!(failure_kind(Some("You've hit your usage limit. Try again at 3:49 AM.")), "quota-exhausted");
        assert_eq!(failure_kind(Some("usage limit has been reached")), "quota-exhausted");
        assert_eq!(failure_kind(Some("quota is exhausted")), "quota-exhausted");
        assert_eq!(failure_kind(Some("insufficient_quota")), "quota-exhausted");
        assert_eq!(failure_kind(Some("Rate-limited; HTTP 429, unauthorized")), "temporary-rate-limit", "a limit outranks the rest");
        assert_eq!(failure_kind(Some("Too many requests")), "temporary-rate-limit");
        // A quota merely mentioned is not one used up (native read any "quota" as exhausted).
        assert_eq!(failure_kind(Some("Your quota resets tomorrow; the model returned nothing.")), "unknown");
    }

    #[test]
    fn a_failed_sign_in_and_a_safety_block_are_heard_in_each_of_their_words() {
        assert_eq!(failure_kind(Some("Your session ended. Please sign in again.")), "authentication-failed", "its own words alone");
        for words in ["Authentication required", "authentication failed", "401 Unauthorized", "Invalid API key", "login expired", "Login required"] {
            assert_eq!(failure_kind(Some(words)), "authentication-failed", "{words}");
        }
        for words in ["safety policy", "blocked by policy", "violates the content policy", "safety refusal"] {
            assert_eq!(failure_kind(Some(words)), "safety-blocked", "{words}");
        }
        assert_eq!(failure_kind(None), "unknown");
    }

    #[test]
    fn the_app_servers_error_info_decides_first_and_its_words_when_it_names_nothing() {
        assert_eq!(app_server_failure_kind("usageLimitExceeded", "whatever"), "quota-exhausted");
        assert_eq!(app_server_failure_kind("rateLimitExceeded", "whatever"), "temporary-rate-limit");
        assert_eq!(app_server_failure_kind("unauthorized", "whatever"), "authentication-failed");
        assert_eq!(app_server_failure_kind("cyberPolicy", "whatever"), "safety-blocked");
        assert_eq!(app_server_failure_kind("misalignmentPolicyViolation", "whatever"), "safety-blocked");
        assert_eq!(app_server_failure_kind("", "Your access token could not be refreshed."), "authentication-failed");
        assert_eq!(app_server_failure_kind("other", "nothing in particular"), "unknown");
    }
}

#[cfg(test)]
mod app_server_tests {
    use crate::runtime::events::Normalizer;
    use serde_json::json;

    #[test]
    fn a_failed_turn_with_no_error_info_reads_its_kind_from_codexs_words() {
        let mut n = Normalizer::default();
        let out = n.accept("error", &json!({"error":{"message":"Your access token could not be refreshed. Please log out and sign in again."},"willRetry":false}));
        let failed = out.iter().find(|e| e.kind == "run.failed").expect("the turn failed");
        assert_eq!(failed.payload["kind"], "authentication-failed");
        let mut n = Normalizer::default();
        let out = n.accept("error", &json!({"error":{"message":"Your quota resets tomorrow.","codexErrorInfo":"other"},"willRetry":false}));
        assert_eq!(out.iter().find(|e| e.kind == "run.failed").unwrap().payload["kind"], "unknown", "a quota mentioned is not used up");
    }
}
