//! Electron's `a-claude-route-names-its-version.test.tsx`, the cases Rust builds: `claudeModelName`
//! (`models::claude_model_name`), with Electron's values.
use crate::models::claude_model_name;

#[test]
fn spells_family_and_version_the_way_anthropic_writes_them() {
    assert_eq!(claude_model_name("claude-opus-5-5").as_deref(), Some("Opus 5.5"));
    assert_eq!(claude_model_name("claude-fable-5-1").as_deref(), Some("Fable 5.1"));
    assert_eq!(claude_model_name("claude-sonnet-5").as_deref(), Some("Sonnet 5"));
    assert_eq!(claude_model_name("claude-haiku-4-5-20251001").as_deref(), Some("Haiku 4.5"));
    assert_eq!(claude_model_name("claude-opus-4-1-20250805-v1").as_deref(), Some("Opus 4.1"));
    assert_eq!(claude_model_name("claude-opus-4-20250514").as_deref(), Some("Opus 4"));
}

#[test]
fn guesses_at_nothing_it_does_not_recognise() {
    assert_eq!(claude_model_name("opus"), None);
    assert_eq!(claude_model_name("claude-fable-5-mythos-5"), None);
    assert_eq!(claude_model_name("gpt-6"), None);
}
