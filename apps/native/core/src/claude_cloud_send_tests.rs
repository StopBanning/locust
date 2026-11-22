use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::{
    ClaudeCloud, ClaudeLaunch, CloudPlatform, CloudSendResult, ExecOptions, ExecReply,
    UNCONFIRMED_SEND_MESSAGE,
};

const SESSION_ID: &str = "session_0123456789";

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn send_with(
    reply: ExecReply,
    platform: CloudPlatform,
) -> (CloudSendResult, Vec<Vec<String>>) {
    let dir = TempDir::new("locust-cloud-json");
    let store_path = dir.path.join("sessions.json");
    let root_str = dir.path.to_string_lossy().to_string();
    std::fs::write(
        &store_path,
        json!({
            "sessions": [{
                "id": "cc_abcdef",
                "folder": root_str,
                "prompt": "Test",
                "startedAt": "2026-10-01T12:00:00.000Z",
                "sessionId": SESSION_ID,
            }]
        })
        .to_string(),
    )
    .expect("write sessions");

    let calls: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
    let cloud = ClaudeCloud::new(&store_path, platform, || {
        Some(ClaudeLaunch {
            discovered_path: "fake-claude".into(),
            executable_path: "fake-claude".into(),
            prefix_args: Vec::new(),
            env: None,
        })
    })
    .with_exec(|_file: &str, args: &[String], _opts: &ExecOptions| {
        calls.borrow_mut().push(args.to_vec());
        reply.clone()
    });
    let result = cloud.send("cc_abcdef", "More");
    drop(cloud);
    (result, calls.into_inner())
}

#[test]
// case: A cloud send requests JSON and accepts the final success on %s.
fn a_cloud_send_requests_json_and_accepts_the_final_success_on_s() {
    for platform in [CloudPlatform::Win32, CloudPlatform::Darwin] {
        let (result, calls) = send_with(
            ExecReply {
                code: 0,
                output: format!(
                    "Notice\n{{\"ok\":true,\"session_id\":\"{SESSION_ID}\",\"url\":\"https://claude.ai/code/{SESSION_ID}\"}}\n"
                ),
                stdout: None,
            },
            platform,
        );
        assert_eq!(result, CloudSendResult::Ok);
        assert!(calls[0].join(" ").contains("--output-format json"));
    }
}

#[test]
// case: A success receipt is trusted whatever form its session id takes, so a sent message is never offered again.
fn a_success_receipt_is_trusted_whatever_form_its_session_id_takes() {
    let (res1, _) = send_with(
        ExecReply {
            code: 0,
            output: "{\"ok\":true,\"session_id\":\"cse_other_form\"}\n".into(),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert_eq!(res1, CloudSendResult::Ok);

    let (res2, _) = send_with(
        ExecReply {
            code: 0,
            output: "{\"ok\":true}\n".into(),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert_eq!(res2, CloudSendResult::Ok);
}

#[test]
// case: A failed JSON send displays %s verbatim.
fn a_failed_json_send_displays_s_verbatim() {
    for error in ["Session archived", "Session not found"] {
        let (res, _) = send_with(
            ExecReply {
                code: 1,
                output: json!({ "ok": false, "session_id": SESSION_ID, "error": error })
                    .to_string(),
                stdout: None,
            },
            CloudPlatform::Win32,
        );
        assert_eq!(
            res,
            CloudSendResult::Err {
                message: error.into(),
            }
        );
    }
}

#[test]
// case: A cloud send uses the last JSON answer and never accepts terminal success words.
fn a_cloud_send_uses_the_last_json_answer_and_never_accepts_terminal_success_words() {
    let (res1, _) = send_with(
        ExecReply {
            code: 0,
            output: format!(
                "{{\"ok\":true,\"session_id\":\"{SESSION_ID}\"}}\n{{\"ok\":false,\"session_id\":\"{SESSION_ID}\",\"error\":\"Archived\"}}\n"
            ),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert_eq!(
        res1,
        CloudSendResult::Err {
            message: "Archived".into(),
        }
    );

    let (res2, _) = send_with(
        ExecReply {
            code: 0,
            output: "Sent to cloud session".into(),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert!(matches!(res2, CloudSendResult::Err { .. }));

    let (res3, _) = send_with(
        ExecReply {
            code: 1,
            output: json!({ "ok": true, "session_id": SESSION_ID }).to_string(),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert!(matches!(res3, CloudSendResult::Err { .. }));
}

#[test]
// case: A send nobody confirmed is said to be unconfirmed, never "not sent", so it is not sent twice (0.568).
fn a_send_nobody_confirmed_is_said_to_be_unconfirmed_never_not_sent() {
    let expected = CloudSendResult::Err {
        message: UNCONFIRMED_SEND_MESSAGE.into(),
    };
    let (res1, _) = send_with(
        ExecReply {
            code: 0,
            output: "Sent to cloud session".into(),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert_eq!(res1, expected);

    let (res2, _) = send_with(
        ExecReply {
            code: 1,
            output: json!({ "ok": true, "session_id": SESSION_ID }).to_string(),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert_eq!(res2, expected);

    let (res3, _) = send_with(
        ExecReply {
            code: 0,
            output: json!({ "ok": true, "session_id": SESSION_ID }).to_string(),
            stdout: Some(String::new()),
        },
        CloudPlatform::Win32,
    );
    assert_eq!(res3, expected);

    let (res4, _) = send_with(
        ExecReply {
            code: -1,
            output: String::new(),
            stdout: None,
        },
        CloudPlatform::Win32,
    );
    assert_eq!(res4, expected);
}

#[test]
// case: A configuration failure keeps the Error fallback and stderr cannot forge success.
fn a_configuration_failure_keeps_the_error_fallback_and_stderr_cannot_forge_success() {
    let (res1, _) = send_with(
        ExecReply {
            code: 1,
            output: "Error: Sign in first\n".into(),
            stdout: Some(String::new()),
        },
        CloudPlatform::Win32,
    );
    assert_eq!(
        res1,
        CloudSendResult::Err {
            message: "Claude Code did not send that: Sign in first".into(),
        }
    );

    let (res2, _) = send_with(
        ExecReply {
            code: 0,
            output: json!({ "ok": true, "session_id": SESSION_ID }).to_string(),
            stdout: Some(String::new()),
        },
        CloudPlatform::Win32,
    );
    assert!(matches!(res2, CloudSendResult::Err { .. }));
}
