//! Electron's `packages/runtime-adapters/test/a-claude-run-that-lingers-after-its-result-completed.test.ts`, through
//! the caller: the Claude runtime (claude.rs `settle`) fills the normalizer's process evidence -- the exit code and
//! whether the host ended the run for what it left running after its result -- and asks `finish` for the end.
use super::*;

fn result() -> Value {
    json!({"type":"result","subtype":"success","is_error":false,"result":"index.html is finished.","session_id":"s1"})
}

fn ended(saw_result: bool, exit_code: i32, ended_after_result: bool, forced: bool) -> String {
    let mut claude = ClaudeNormalizer::default();
    if saw_result {
        claude.accept(&result());
    }
    claude.evidence.exit_code = Some(exit_code);
    claude.evidence.ended_after_result = ended_after_result;
    claude.evidence.forced_termination_attempted = Some(forced);
    claude.finish(false, "").kind
}

#[test]
fn is_completed_when_the_runner_ended_it_after_the_result() {
    // case: is completed when the runner ended it after the result
    assert_eq!(ended(true, 1, true, true), "run.completed");
}

#[test]
fn is_still_failed_when_it_exited_non_zero_on_its_own() {
    // case: is still failed when it exited non-zero on its own
    assert_eq!(ended(true, 1, false, false), "run.failed");
}

#[test]
fn and_ending_it_without_a_result_is_never_a_completion() {
    // case: and ending it without a result is never a completion
    assert_eq!(ended(false, 1, true, false), "run.failed");
}
