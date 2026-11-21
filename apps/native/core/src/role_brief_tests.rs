//! Electron's `role-brief.test.ts` (5 cases): a role is a brief, not a label. Until 2026-09-17 the roster line said
//! "Wren (Code & Migrations)" and nothing told Wren what that asked of it.
use super::*;

/// Wren's brief for a turn, with Booty beside it; `role` is the record's preset (None: no role on the record).
fn compose(role: Option<&str>, title: Option<&str>) -> String {
    let root = std::env::temp_dir().join(format!("locust-role-brief-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut wren = serde_json::json!({"teammateId": "tm_w", "name": "Wren"});
    if let Some(role) = role {
        wren["role"] = role.into();
    }
    if let Some(title) = title {
        wren["roleTitle"] = title.into();
    }
    let booty = serde_json::json!({"teammateId": "tm_b", "name": "Booty", "role": "Research & Briefs"});
    std::fs::write(
        store.electron().join("teammates.json"),
        serde_json::json!({"schemaVersion": 1, "teammates": [wren, booty], "missionOwners": {}, "settings": {}}).to_string(),
    )
    .unwrap();
    let (briefed, _) = crate::people::turn_brief(&store, "tm_w", &root.join("shop"), "Look at the repo.", None, Some("codex"), &[], 0).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    briefed
}

// case: every preset has a brief, and so does Custom
#[test]
fn every_preset_has_a_brief_and_so_does_custom() {
    for role in TEAMMATE_ROLES {
        let said = role_brief(role, Some("release manager")).unwrap_or_default();
        assert!(said.starts_with("Your role is "), "{role}: {said}");
    }
    // Every word of each preset, as Electron says it.
    let presets = [
        ("Code & Migrations", "Your role is code: read before you change, keep the change to what was asked, run what proves it, and report what you verified and what you did not."),
        ("Research & Briefs", "Your role is research: read the sources and name them, keep what they say apart from what you conclude, and write the brief so it stands on its own."),
        ("Ops & Scheduling", "Your role is operations, the work that repeats: say what runs, when, and what would fail with nobody noticing, and keep every change reversible."),
        ("Docs & QA", "Your role is writing and checking: match the words to the code, say what is untested, and never call a thing done that you did not see work."),
        ("Data & Reporting", "Your role is figures: every number you report names where it came from, the arithmetic comes from the data and not from memory, and the summary is one a person can act on."),
    ];
    for (role, said) in presets {
        assert_eq!(role_brief(role, Some(role)).as_deref(), Some(said), "{role}");
    }
    assert_eq!(role_brief("Something else", Some("release manager")), None);
}

// case: builds a Custom brief around the title the person typed
#[test]
fn builds_a_custom_brief_around_the_title_the_person_typed() {
    let said = role_brief("Custom", Some("release manager")).unwrap_or_default();
    assert_eq!(
        said,
        "Your role is release manager, in the person's own words: do the work that title describes the way a capable colleague with it would, bring what someone in that role would know, and say plainly when an ask falls outside it rather than stretching to cover it. When asked who you are or what your role is, answer with that title, not with the name of the program you run in."
    );
    assert!(said.contains("say plainly when an ask falls outside it"));
    // The title is trimmed into the sentence.
    assert!(role_brief("Custom", Some("  release manager ")).unwrap_or_default().starts_with("Your role is release manager, in"));
    // No title yet: the shared part alone, never "Your role is Custom".
    let shared = "Your role is the one the person set you up for: do what is asked the way a capable colleague would, and say plainly when an ask falls outside what you can do here rather than stretching to cover it.";
    assert_eq!(role_brief("Custom", Some("Custom")).as_deref(), Some(shared));
    assert_eq!(role_brief("Custom", Some("   ")).as_deref(), Some(shared));
    assert_eq!(role_brief("Custom", None).as_deref(), Some(shared));
    assert!(!role_brief("Custom", None).unwrap_or_default().contains("Custom"));
}

// case: the chief of staff is told to route, to say what was delegated, and to report back in one message
#[test]
fn the_chief_of_staff_is_told_to_route_and_report_back_in_one_message() {
    let said = role_brief("Chief of Staff", None).unwrap_or_default();
    assert_eq!(
        said,
        "Your role is to run the team, not to do all of the work yourself. When the person asks for something a teammate's role fits, brief that teammate through the share block the way you would brief a senior colleague: what is needed and why, what you already know, the end state and what done looks like. Tell the person what you delegated and to whom. When a reply comes back, report the result to the person as one message that stands on its own: the answer, what is still open, and what you would do next. Do the work yourself only when no teammate's role fits or the brief would take longer than the task."
    );
    assert!(said.contains("brief that teammate through the share block"));
    assert!(said.contains("what is needed and why, what you already know, the end state and what done looks like"));
    assert!(said.contains("Tell the person what you delegated and to whom"));
    assert!(said.contains("one message that stands on its own"));
    // And when to do it themselves, so the role is not a rule against working.
    assert!(said.contains("Do the work yourself only when"));
}

// case: goes into the prompt ahead of the roster line it refers to, in the stable prefix
#[test]
fn goes_into_the_prompt_ahead_of_the_roster_line_it_refers_to() {
    let prompt = compose(Some("Chief of Staff"), None);
    let role = prompt.find("Your role is to run the team").expect("the role is in the brief");
    let roster = prompt.find("Teammates in this workspace besides you").expect("the roster is in the brief");
    let ask = prompt.find("Look at the repo.").expect("the ask is in the brief");
    assert!(roster > role, "{prompt}");
    assert!(ask > roster, "{prompt}");
    // First of everything: nothing standing comes before the role.
    assert!(prompt.starts_with("Your role is to run the team"), "{prompt}");
}

// case: puts the Custom title into the prompt, and says nothing when the preset is unknown
#[test]
fn puts_the_custom_title_into_the_prompt_and_says_nothing_when_the_preset_is_unknown() {
    assert!(compose(Some("Custom"), Some("release manager")).contains("Your role is release manager"));
    assert!(!compose(None, None).contains("Your role is"));
    assert!(!compose(Some("Something else"), None).contains("Your role is"));
}
