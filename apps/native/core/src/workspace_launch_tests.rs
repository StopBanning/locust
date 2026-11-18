//! Workspace resolution parity tests (Electron `main/workspace.test.ts` and
//! `main/a-launch-from-an-agents-settings-folder-is-not-a-workspace.test.ts`).

use crate::workspace_launch::{
    ResolveOptions, WorkspaceResolution, WorkspaceSource, is_inside_directory,
    not_a_teammate_folder, read_remembered_workspace, resolve_workspace_path, workspace_id_for,
    write_remembered_workspace,
};
use serde_json::Value;

const INSTALL: &str = "C:\\Users\\colin\\AppData\\Local\\Programs\\Locust";

#[test]
fn is_the_folder_the_app_was_launched_from() {
    let argv = vec!["Locust.exe".into()];
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &argv,
            cwd: "C:\\work\\pebble",
            install_directory: Some(INSTALL),
            remembered: None,
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\work\\pebble".into()),
            source: WorkspaceSource::LaunchFolder,
        }
    );
}

#[test]
fn is_never_the_install_folder() {
    let argv = vec!["Locust.exe".into()];
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &argv,
            cwd: INSTALL,
            install_directory: Some(INSTALL),
            remembered: None,
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: None,
            source: WorkspaceSource::None,
        }
    );
}

#[test]
fn treats_a_folder_under_the_install_folder_the_same_whatever_its_case() {
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &[],
            cwd: "c:\\users\\COLIN\\appdata\\local\\programs\\locust\\resources",
            install_directory: Some(INSTALL),
            remembered: None,
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        })
        .source,
        WorkspaceSource::None
    );
    assert!(!is_inside_directory("/opt/Locust", Some("/opt/locust"), false));
    assert!(is_inside_directory("/opt/locust/resources", Some("/opt/locust"), false));
    assert!(!is_inside_directory(
        "C:\\Users\\colin\\AppData\\Local\\Programs\\Locust2",
        Some(INSTALL),
        true
    ));
}

#[test]
fn falls_back_to_the_remembered_folder_when_launched_from_install_folder() {
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &[],
            cwd: INSTALL,
            install_directory: Some(INSTALL),
            remembered: Some("C:\\work\\pebble"),
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\work\\pebble".into()),
            source: WorkspaceSource::Remembered,
        }
    );
}

#[test]
fn launch_from_a_real_folder_beats_remembered() {
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &[],
            cwd: "C:\\work\\otter",
            install_directory: Some(INSTALL),
            remembered: Some("C:\\work\\pebble"),
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\work\\otter".into()),
            source: WorkspaceSource::LaunchFolder,
        }
    );
}

#[test]
fn explicit_workspace_argument_wins_over_everything() {
    let argv = vec!["Locust.exe".into(), "--workspace=C:\\work\\pebble".into()];
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &argv,
            cwd: "C:\\work\\otter",
            install_directory: Some(INSTALL),
            remembered: Some("C:\\work\\heron"),
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\work\\pebble".into()),
            source: WorkspaceSource::Argument,
        }
    );
}

#[test]
fn ignores_relative_or_empty_workspace_argument_and_remembered_inside_install() {
    let rel = vec!["--workspace=pebble".into()];
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &rel,
            cwd: "C:\\work\\otter",
            install_directory: Some(INSTALL),
            remembered: None,
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\work\\otter".into()),
            source: WorkspaceSource::LaunchFolder,
        }
    );
    let empty = vec!["--workspace=".into()];
    let bad_rem = format!("{INSTALL}\\resources");
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &empty,
            cwd: INSTALL,
            install_directory: Some(INSTALL),
            remembered: Some(&bad_rem),
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: None,
            source: WorkspaceSource::None,
        }
    );
}

#[test]
fn development_build_with_no_install_dir_takes_any_launch_folder() {
    let argv = vec!["electron".into(), ".".into()];
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &argv,
            cwd: "C:\\repo\\apps\\desktop",
            install_directory: None,
            remembered: Some("C:\\work\\pebble"),
            default_workspace: None,
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\repo\\apps\\desktop".into()),
            source: WorkspaceSource::LaunchFolder,
        }
    );
}

#[test]
fn makes_do_with_default_folder_when_nothing_chosen_and_precedence_holds() {
    let argv = vec!["Locust.exe".into()];
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &argv,
            cwd: INSTALL,
            install_directory: Some(INSTALL),
            remembered: None,
            default_workspace: Some("C:\\Users\\colin\\Documents\\Locust"),
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\Users\\colin\\Documents\\Locust".into()),
            source: WorkspaceSource::Default,
        }
    );
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &argv,
            cwd: INSTALL,
            install_directory: Some(INSTALL),
            remembered: Some("C:\\work\\pebble"),
            default_workspace: Some("C:\\Users\\colin\\Documents\\Locust"),
            win32: true,
            home_directory: None,
            system_directory: None,
        }),
        WorkspaceResolution {
            path: Some("C:\\work\\pebble".into()),
            source: WorkspaceSource::Remembered,
        }
    );
}

#[test]
fn never_defaults_into_install_folder_and_ignores_relative_default() {
    let argv = vec!["Locust.exe".into()];
    let inside = format!("{INSTALL}\\workspace");
    for bad in [&inside, "Documents\\Locust"] {
        assert_eq!(
            resolve_workspace_path(&ResolveOptions {
                argv: &argv,
                cwd: INSTALL,
                install_directory: Some(INSTALL),
                remembered: None,
                default_workspace: Some(bad),
                win32: true,
                home_directory: None,
                system_directory: None,
            }),
            WorkspaceResolution {
                path: None,
                source: WorkspaceSource::None,
            }
        );
    }
}

#[test]
fn workspace_id_is_deterministic_and_hex32() {
    let a = workspace_id_for("C:\\work\\pebble");
    assert_eq!(a, workspace_id_for("C:\\work\\pebble"));
    assert_ne!(a, workspace_id_for("C:\\work\\otter"));
    assert!(a.starts_with("ws_"));
    assert_eq!(a.len(), 35);
    assert!(a[3..].chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')));
}

#[test]
fn remembered_folder_round_trips_and_handles_missing_or_malformed() {
    let dir = std::env::temp_dir().join(format!("locust-ws-launch-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("workspace.json");
    write_remembered_workspace(&file, "C:\\work\\pebble").unwrap();
    assert_eq!(read_remembered_workspace(&file).as_deref(), Some("C:\\work\\pebble"));
    let parsed: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(parsed, serde_json::json!({ "schemaVersion": 1, "path": "C:\\work\\pebble" }));

    assert_eq!(read_remembered_workspace(&dir.join("missing.json")), None);
    let bad = dir.join("bad.json");
    std::fs::write(&bad, "{not json").unwrap();
    assert_eq!(read_remembered_workspace(&bad), None);
    let empty = dir.join("empty.json");
    std::fs::write(&empty, r#"{"schemaVersion":1,"path":""}"#).unwrap();
    assert_eq!(read_remembered_workspace(&empty), None);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn launch_folder_no_teammate_should_work_in_falls_back() {
    let home = "C:\\Users\\person";
    let install = "C:\\Users\\person\\AppData\\Local\\Programs\\Locust";
    let remembered = "C:\\Users\\person\\Documents\\Codex\\project";
    let default_ws = "C:\\Users\\person\\Documents\\Locust";
    let sys = "C:\\Windows";

    for cwd in [
        "C:\\Users\\person\\.claude",
        "C:\\Users\\person\\.claude\\projects",
        "C:\\Users\\person\\.codex",
        "C:\\Users\\person\\.cursor",
        "C:\\Users\\person\\.gemini",
        "C:\\Users\\person\\.copilot",
        "C:\\Users\\person",
        "C:\\Users",
        "C:\\Windows\\System32",
        "c:\\users\\PERSON\\.Claude",
    ] {
        assert_eq!(
            resolve_workspace_path(&ResolveOptions {
                argv: &[],
                cwd,
                install_directory: Some(install),
                remembered: Some(remembered),
                default_workspace: Some(default_ws),
                win32: true,
                home_directory: Some(home),
                system_directory: Some(sys),
            }),
            WorkspaceResolution {
                path: Some(remembered.into()),
                source: WorkspaceSource::Remembered,
            },
            "expected fallback for {cwd}"
        );
    }

    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &[],
            cwd: "C:\\Users\\person\\.claude",
            install_directory: Some(install),
            remembered: None,
            default_workspace: Some(default_ws),
            win32: true,
            home_directory: Some(home),
            system_directory: Some(sys),
        }),
        WorkspaceResolution {
            path: Some(default_ws.into()),
            source: WorkspaceSource::Default,
        }
    );

    assert_eq!(
        not_a_teammate_folder("C:\\Users\\person\\.claude", Some(home), true, None).as_deref(),
        Some(".claude, where the agents keep their own settings")
    );
    assert_eq!(
        not_a_teammate_folder(home, Some(home), true, None).as_deref(),
        Some("your home folder")
    );
}

#[test]
fn launch_folder_that_is_a_project_is_still_the_workspace_and_arg_wins() {
    let home = "C:\\Users\\person";
    let install = "C:\\Users\\person\\AppData\\Local\\Programs\\Locust";
    let remembered = "C:\\Users\\person\\Documents\\Codex\\project";
    let default_ws = "C:\\Users\\person\\Documents\\Locust";
    let sys = "C:\\Windows";

    for cwd in [
        "C:\\Users\\person\\Documents\\Codex\\locust",
        "C:\\Users\\person\\.claude-projects",
        "C:\\Users\\person\\work",
        "C:\\Users\\person\\.locust\\compare\\cmp_1-a",
    ] {
        assert_eq!(
            resolve_workspace_path(&ResolveOptions {
                argv: &[],
                cwd,
                install_directory: Some(install),
                remembered: Some(remembered),
                default_workspace: Some(default_ws),
                win32: true,
                home_directory: Some(home),
                system_directory: Some(sys),
            }),
            WorkspaceResolution {
                path: Some(cwd.into()),
                source: WorkspaceSource::LaunchFolder,
            }
        );
    }

    let argv = vec!["--workspace=C:\\Users\\person\\.claude".into()];
    assert_eq!(
        resolve_workspace_path(&ResolveOptions {
            argv: &argv,
            cwd: "C:\\x",
            install_directory: Some(install),
            remembered: Some(remembered),
            default_workspace: Some(default_ws),
            win32: true,
            home_directory: Some(home),
            system_directory: Some(sys),
        }),
        WorkspaceResolution {
            path: Some("C:\\Users\\person\\.claude".into()),
            source: WorkspaceSource::Argument,
        }
    );
}
