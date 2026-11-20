//! Electron's `a-quiet-runtime-is-asked-to-say-what-it-does.test.ts` (3 cases): Antigravity worked 23 minutes with
//! nothing on screen but "Working...", so its runs are asked to say in a sentence what each step is.
use super::*;

fn brief(who: &str, runtime: &str) -> String {
    let root = std::env::temp_dir().join(format!("locust-narrate-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::write(
        store.electron().join("teammates.json"),
        serde_json::json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_flash","name":"Flash","role":"Code & Migrations"}],"missionOwners":{},"settings":{}}).to_string(),
    )
    .unwrap();
    let (briefed, _) = crate::people::turn_brief(&store, who, &root.join("shop"), "Review it.", None, Some(runtime), &[], 0).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    briefed
}

// case: goes to Antigravity, and not to runtimes that narrate on their own
#[test]
fn goes_to_antigravity_and_not_to_runtimes_that_narrate_on_their_own() {
    assert!(runtime_narrates_when_asked("antigravity"));
    for runtime in ["claude", "codex", "cursor", "opencode", "copilot"] {
        assert!(!runtime_narrates_when_asked(runtime), "{runtime}");
    }
}

// case: is in a run nobody owns when asked for, and not when not
#[test]
fn is_in_a_run_nobody_owns_when_asked_for_and_not_when_not() {
    assert!(brief("", "antigravity").contains(&narrate_section()));
    assert!(!brief("", "claude").contains(&narrate_section()));
}

// case: is in a teammate run when asked for
#[test]
fn is_in_a_teammate_run_when_asked_for() {
    assert!(brief("tm_flash", "antigravity").contains(&narrate_section()));
    assert!(!brief("tm_flash", "codex").contains(&narrate_section()));
}
