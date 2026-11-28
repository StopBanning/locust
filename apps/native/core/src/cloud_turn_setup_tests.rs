use super::*;

const MISSION: &str = "mission_setup";
const CLAUDE: &str = "2.1.296";
const OPEN_CODE: &str = "1.18.27";
const COMPUTER: &str = "locust-hearth-x7q9";

fn prepare(
    turns: &CloudTurns<'_, FakeComputers>,
    folder: &Path,
    agent: Option<(CloudAgent, Option<&str>)>,
    open_code_version: Option<&str>,
    set_up: bool,
) -> CloudPrepared {
    turns.prepare(
        PrepareInput {
            mission_id: MISSION,
            folder,
            follows: None,
            open_code_version,
            agent,
            set_up,
        },
        &mut |_| {},
    )
}

fn refused(ready: CloudPrepared) -> String {
    match ready {
        CloudPrepared::Err {
            message,
            setup: None,
            sign_in: None,
        } => message,
        other => panic!("expected ordinary refusal, got {other:?}"),
    }
}

#[test]
fn cloud_setup_refuses_a_non_ascii_version_before_contacting_the_computer() {
    with_world(
        FakeOptions::default_github(),
        |folder, turns, computers, told, _| {
            for version in ["٢.١.٢٩٦", "2.1.296-preview", "latest"] {
                let result = prepare(
                    turns,
                    folder,
                    Some((CloudAgent::Claude, Some(version))),
                    None,
                    true,
                );
                assert!(refused(result).contains("which Claude Code"));
            }
            assert!(computers.calls.borrow().is_empty());
            assert!(told.borrow().is_empty());
            assert!(turns.of(MISSION).is_none());
        },
    );
}

#[test]
// case: readies a turn of an agent that is signed in there, and hands back one that is not
fn readies_a_turn_of_an_agent_that_is_signed_in_there_and_hands_back_one_that_is_not() {
    with_world(
        FakeOptions::with_existing("Shutdown"),
        |folder, turns, computers, told, _| {
            computers.signed_in.set(false);
            let result = prepare(
                turns,
                folder,
                Some((CloudAgent::Claude, Some(CLAUDE))),
                None,
                false,
            );
            assert_eq!(
                result,
                CloudPrepared::Err {
                    message: "Claude Code is not signed in on your cloud computer yet.".into(),
                    setup: None,
                    sign_in: Some(crate::cloud_turns::CloudSignIn {
                        computer: COMPUTER.into(),
                        repo: "acme/hearth".into(),
                        agent: CloudAgent::Claude
                    })
                }
            );
            assert_eq!(
                computers.calls.borrow().as_slice(),
                ["ready", "claude@2.1.296"]
            );
            assert!(told.borrow().is_empty());
            assert!(turns.of(MISSION).is_none());

            computers.signed_in.set(true);
            let result = prepare(
                turns,
                folder,
                Some((CloudAgent::Claude, Some(CLAUDE))),
                None,
                false,
            );
            assert!(
                matches!(result, CloudPrepared::Ok { codespace, bin: Some(bin), .. }
            if codespace == COMPUTER && bin == "/home/codespace/.locust/npm/bin/claude")
            );
            assert_eq!(computers.signed_checks.get(), 2);
            assert!(
                computers
                    .calls
                    .borrow()
                    .iter()
                    .any(|call| call.starts_with("bring:"))
            );
            assert!(turns.of(MISSION).is_some());

            let missing = prepare(turns, folder, Some((CloudAgent::Codex, None)), None, false);
            assert_eq!(
                refused(missing),
                "Locust could not tell which Codex is installed here, so it cannot put the same one on your cloud computer."
            );
        },
    );
}

#[test]
fn cloud_setup_rejects_provider_version_mismatches_for_both_installers() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, told, _| {
            *computers.install_answer.borrow_mut() = Some(AgentInstalled {
                version: "2.1.200".into(),
                bin: "/home/codespace/.locust/npm/bin/claude".into(),
            });
            let claude = prepare(
                turns,
                folder,
                Some((CloudAgent::Claude, Some(CLAUDE))),
                None,
                false,
            );
            assert!(refused(claude).contains("Claude Code could not be installed"));
            *computers.open_code_version.borrow_mut() = Some("1.0.0".into());
            let open_code = prepare(turns, folder, None, Some(OPEN_CODE), false);
            assert!(refused(open_code).contains("OpenCode could not be installed"));
            assert_eq!(computers.signed_checks.get(), 0);
            assert!(
                !computers
                    .calls
                    .borrow()
                    .iter()
                    .any(|call| call.starts_with("bring:"))
            );
            assert!(told.borrow().is_empty());
            assert!(turns.of(MISSION).is_none());
        },
    );
}

#[test]
fn cloud_setup_requires_the_whole_path_to_the_installed_agent() {
    with_world(
        FakeOptions::with_existing("Available"),
        |folder, turns, computers, told, _| {
            for bin in ["~/.locust/npm/bin/claude", "/home/a b/claude"] {
                *computers.install_answer.borrow_mut() = Some(AgentInstalled {
                    version: CLAUDE.into(),
                    bin: bin.into(),
                });
                let result = prepare(
                    turns,
                    folder,
                    Some((CloudAgent::Claude, Some(CLAUDE))),
                    None,
                    false,
                );
                assert_eq!(
                    refused(result),
                    "That is not where Locust put Claude Code there."
                );
            }
            assert_eq!(computers.signed_checks.get(), 0);
            assert!(
                !computers
                    .calls
                    .borrow()
                    .iter()
                    .any(|call| call.starts_with("bring:"))
            );
            assert!(told.borrow().is_empty());
            assert!(turns.of(MISSION).is_none());
        },
    );
}
