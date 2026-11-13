//! Feedback and private report dialog (`renderer/src/components/FeedbackDialog.tsx`).
//!
//! Never sends anything without the person's press. Scrubs secrets before any report leaves.

use std::path::PathBuf;

use locust_core::report_problem::{
    self, FeedbackReport, MAX_DESCRIPTION, ReportFacts, SUPPORT_ADDRESS,
};
use rune::{El, FileDialogOptions, TextEdit, Tx, col, el, row, text, text_input};
use rune_kit::Checkbox;

use crate::look::controls::{Look, button, ghost_button, primary_button, Tone};
use crate::{Palette, State, type_};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeedbackSent {
    Issue(String),
    Email(String),
    File(String),
}

#[derive(Default)]
pub(super) struct Panel {
    pub open: bool,
    pub description: TextEdit,
    pub conversation: Option<String>,
    pub include_conversation: bool,
    pub saved_path: Option<String>,
    pub error: Option<String>,
    pub last_sent: Option<FeedbackSent>,
    /// Test hook: when true, Send/Email record `last_sent` without launching the OS browser/mail app.
    pub suppress_os_open: bool,
}

pub(super) fn open(s: &mut State, conversation: Option<String>) {
    let suppress = s.feedback_dialog.suppress_os_open;
    s.feedback_dialog = Panel {
        open: true,
        description: TextEdit::default(),
        conversation,
        include_conversation: false,
        saved_path: None,
        error: None,
        last_sent: None,
        suppress_os_open: suppress,
    };
}

pub(super) fn close(s: &mut State) {
    s.feedback_dialog.open = false;
}

pub(super) fn current_facts() -> ReportFacts {
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        other => other,
    };
    ReportFacts {
        version: env!("CARGO_PKG_VERSION").into(),
        release: String::new(),
        arch: arch.into(),
        platform: Some(platform.into()),
    }
}

fn current_report(s: &State) -> FeedbackReport {
    let text = s.feedback_dialog.description.text.trim();
    let trimmed: String = text.chars().take(MAX_DESCRIPTION).collect();
    let conversation = if s.feedback_dialog.include_conversation {
        s.feedback_dialog.conversation.clone()
    } else {
        None
    };
    FeedbackReport {
        description: trimmed,
        conversation,
    }
}

fn ready(s: &State) -> bool {
    !s.feedback_dialog.description.text.trim().is_empty()
}

fn open_external(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let cmd = ("open", vec![url]);
    #[cfg(target_os = "windows")]
    let cmd = ("cmd", vec!["/c", "start", "", url]);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let cmd = ("xdg-open", vec![url]);
    std::process::Command::new(cmd.0)
        .args(&cmd.1)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("{e}"))
}

fn downloads_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(|h| PathBuf::from(h).join("Downloads"))
        .filter(|p| p.is_dir())
}

pub(super) fn send_issue(s: &mut State) {
    if !ready(s) {
        return;
    }
    let url = report_problem::feedback_url(&current_facts(), &current_report(s));
    s.feedback_dialog.error = None;
    s.feedback_dialog.last_sent = Some(FeedbackSent::Issue(url.clone()));
    if !s.feedback_dialog.suppress_os_open {
        if let Err(_) = open_external(&url) {
            s.feedback_dialog.error =
                Some("Your browser could not be opened. What you wrote is still here.".into());
            return;
        }
        s.feedback_dialog.open = false;
    }
}

pub(super) fn send_email(s: &mut State) {
    if !ready(s) {
        return;
    }
    let url = report_problem::support_mailto_url(&current_facts(), &current_report(s));
    s.feedback_dialog.error = None;
    s.feedback_dialog.last_sent = Some(FeedbackSent::Email(url.clone()));
    if !s.feedback_dialog.suppress_os_open {
        if let Err(_) = open_external(&url) {
            s.feedback_dialog.error =
                Some("Your mail app could not be opened. What you wrote is still here.".into());
            return;
        }
        s.feedback_dialog.open = false;
    }
}

pub(super) fn save_file(s: &mut State, tx: &mut Tx) {
    if !ready(s) {
        return;
    }
    let content = report_problem::report_file_text(&current_facts(), &current_report(s));
    if s.feedback_dialog.suppress_os_open {
        s.feedback_dialog.last_sent = Some(FeedbackSent::File(content));
        return;
    }
    let options = FileDialogOptions {
        title: "Save report".into(),
        directory: downloads_dir(),
        file_name: Some("locust-report.md".into()),
        ..Default::default()
    }
    .filter("Markdown", &["md", "txt"]);
    tx.save_file(options, move |s: &mut State, answer, _tx| match answer {
        Ok(None) => {}
        Ok(Some(path)) => {
            if std::fs::write(&path, &content).is_ok() {
                let shown = path.display().to_string();
                s.feedback_dialog.saved_path = Some(shown.clone());
                s.feedback_dialog.last_sent = Some(FeedbackSent::File(shown));
                s.feedback_dialog.error = None;
            } else {
                s.feedback_dialog.error = Some(
                    "The report could not be saved. What you wrote is still here; Email or Send still work."
                        .into(),
                );
            }
        }
        Err(_) => {
            s.feedback_dialog.error =
                Some("The report could not be saved. What you wrote is still here.".into());
        }
    });
}

pub(super) fn settings_section(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let log_path = s
        .data
        .as_ref()
        .map(|d| d.root().join("locust-errors.log").display().to_string())
        .unwrap_or_default();
    col()
        .w_full()
        .gap(6.0)
        .padding(10.0)
        .fill(p.field)
        .radius(6.0)
        .child(text("Report a problem".to_owned()).font(type_::TITLE).color(p.text))
        .child(
            text("Every mission is recorded to an append-only ledger on this machine. Nothing is uploaded. The error log records what happened, never what was said.".to_owned())
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        )
        .child(
            row()
                .gap(8.0)
                .child(
                    ghost_button(&l, "Show the log")
                        .key("settings-show-log")
                        .on_press(move |s: &mut State| {
                            if !log_path.is_empty() {
                                s.settings_panel.error = format!("Error log: {log_path}");
                            }
                        }),
                )
                .child(
                    primary_button(&l, "Send feedback", false)
                        .key("settings-send-feedback")
                        .on_press(|s: &mut State| {
                            open(s, None);
                        }),
                ),
        )
}

pub(super) fn view(s: &State, p: &Palette) -> El {
    let l = Look::new(p.dark);
    let can_send = ready(s);
    let facts = current_facts();
    let claim = report_problem::report_claim(
        s.feedback_dialog.include_conversation,
        s.feedback_dialog.conversation.is_some(),
        facts.platform.as_deref(),
    );

    let mut body = col()
        .w(540.0)
        .gap(10.0)
        .padding(16.0)
        .fill(p.raised)
        .radius(10.0)
        .child(
            text("Send feedback".to_owned())
                .font(type_::TITLE)
                .color(p.text),
        )
        .child(
            text_input(&s.feedback_dialog.description, type_::BODY)
                .key("feedback-description")
                .w_full()
                .padding(8.0)
                .fill(p.field)
                .radius(6.0)
                .color(p.text)
                .placeholder("Describe the issue", p.muted)
                .on_edit(|s: &mut State| &mut s.feedback_dialog.description),
        );

    if s.feedback_dialog.conversation.is_some() {
        body = body.child(
            Checkbox::new(s.feedback_dialog.include_conversation)
                .label("Include this conversation")
                .on_toggle(|s: &mut State| {
                    s.feedback_dialog.include_conversation =
                        !s.feedback_dialog.include_conversation;
                })
                .build(l.k)
                .key("feedback-include-conversation"),
        );
    }

    body = body.child(
        text(claim)
            .font(type_::LABEL)
            .color(p.secondary)
            .wrap(),
    );

    if let Some(err) = &s.feedback_dialog.error {
        body = body.child(
            text(err.clone())
                .font(type_::LABEL)
                .color(p.amber)
                .wrap(),
        );
    }

    if let Some(saved) = &s.feedback_dialog.saved_path {
        body = body.child(
            text(format!(
                "Saved to {saved}. Send it to {SUPPORT_ADDRESS}, or attach it to an issue."
            ))
            .font(type_::LABEL)
            .color(p.text)
            .wrap(),
        );
    }

    body = body.child(
        text(format!(
            "Send opens a public GitHub issue (an account is needed). Email opens your mail app to {SUPPORT_ADDRESS}, which only Locust’s makers read; a long conversation may not fit an email, and Save as a file keeps all of it."
        ))
        .font(type_::LABEL)
        .color(p.secondary)
        .wrap(),
    );

    body.child(
        row()
            .w_full()
            .gap(8.0)
            .child(
                button(&l, "Cancel", Tone::Neutral, false)
                    .key("feedback-cancel")
                    .on_press(|s: &mut State| close(s)),
            )
            .child(el().grow(1.0))
            .child(
                button(&l, "Save as a file", Tone::Neutral, !can_send)
                    .key("feedback-save")
                    .on_press_tx(|s: &mut State, tx| save_file(s, tx)),
            )
            .child(
                button(&l, "Email", Tone::Neutral, !can_send)
                    .key("feedback-email")
                    .on_press(|s: &mut State| send_email(s)),
            )
            .child(
                primary_button(&l, "Send", !can_send)
                    .key("feedback-send")
                    .on_press(|s: &mut State| send_issue(s)),
            ),
    )
}
