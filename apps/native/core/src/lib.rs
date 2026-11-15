//! Locust's engine: everything the app does that is not drawing. It must never depend on the UI toolkit;
//! `cargo tree -p locust-core` shows no Rune.

pub mod agents;
pub mod approval;
pub mod approval_patch;
pub mod mode_facts;
pub mod approval_rule_store;
pub mod approval_rules;
pub mod attachments;
pub mod attention;
pub mod away;
pub mod benchmark;
pub mod browser;
pub mod browser_tools;
pub mod carry_on;
pub mod claude_app;
pub mod locust_link;
pub mod reply_page;
pub mod remote_control;
pub mod agent_accounts;
pub mod agent_install;
pub mod teammate_keys;
pub mod text_files;
pub mod undone_note;
pub mod tool_fold;
pub mod mcp_names;
pub mod usage_words;
pub mod path_words;
pub mod edit_earlier;
pub mod agent_updates;
pub mod published_links;
pub mod workroom;
pub mod peer_share;
pub mod briefing;
pub mod peer_exchange;
pub mod acp_agent;
pub mod cancellation;
pub mod cursor_ignore;
pub mod own_models;
pub mod endpoint_test;
pub mod changes;
pub mod checkpoints;
pub mod command_guard;
pub mod command_plain;
pub mod command_reach;
pub mod commit_draft;
pub mod compare;
pub mod compare_copies;
pub mod compare_judge;
pub mod compare_reruns;
pub mod compare_score;
pub mod compact_now;
pub mod connector_reader;
pub mod connectors_off;
pub mod context_usage;
pub mod desk;
pub mod door;
pub mod door_tools;
pub mod edit_check;
pub mod engine;
pub mod engine_api;
pub mod engine_process;
pub mod folder_commit;
pub mod diff_notes;
pub mod folders;
pub mod github_account;
pub mod github_issues;
pub mod groups;
pub mod hand_off;
pub mod image_files;
pub mod inspector;
pub mod interview;
pub mod invariants;
pub mod keep_awake;
pub mod kilo_models;
pub mod ledger;
pub mod ledger_write;
pub mod login_item;
pub mod mission_export;
pub mod models;
pub mod named_folder;
pub mod native;
pub mod org_policy;
pub mod office;
pub mod own_command;
pub mod page_preview;
pub mod pdf;
pub mod people;
pub mod png_text;
pub mod profile_backup;
pub mod protocol_tags;
pub mod pull_requests;
pub mod quit_guard;
pub mod recovery;
pub mod references;
pub mod relay;
pub mod report_problem;
pub mod room_history;
pub mod room_store;
pub mod room_task;
pub mod room_tasks;
pub mod rooms;
pub mod retention;
pub mod route_at_start;
pub mod routine_copy;
pub mod routine_file;
pub mod routine_files;
pub mod routine_host;
pub mod routine_inputs;
pub mod routine_io;
pub mod routine_recovery;
pub mod routine_runner;
pub mod routine_templates;
pub mod routines;
pub mod run_cli;
pub mod runtime;
pub mod runtime_commands;
pub mod runtime_terminal;
pub mod scout;
pub mod secrets;
pub mod session_import;
pub mod shell_path;
pub mod shell_words;
pub mod provider_error;
pub mod failure_words;
pub mod memory_tidy;
pub mod codex_failure;
pub mod side_question;
pub mod skill_runtimes;
pub mod skills;
pub mod spend;
pub mod store;
pub mod team_card;
pub mod teammate_setup;
pub mod teammate_skills;
pub mod terminal;
pub mod time_marker;
pub mod trash;
pub mod tune;
pub mod tune_lesson_store;
#[cfg(any(test, feature = "test-hooks"))]
pub mod test_isolation;
pub mod usage;
pub mod usage_ledger;
pub mod workspace_settings;
pub mod worktrees;

#[cfg(test)]
#[path = "helper_calls_tests.rs"]
mod helper_calls_tests;

pub mod thread_view;

pub mod command_outcomes;

pub mod activity_summary;

pub mod activity_entries;
pub mod activity_trace;
pub mod conversation_turns;

pub mod peer_thread;

pub mod signal_rail;

#[cfg(test)]
mod assistant_text_tests;

#[cfg(test)]
mod changed_file_tests;

#[cfg(test)]
mod typed_prompt_tests;

#[cfg(test)]
mod plan_payload_tests;

#[cfg(test)]
mod plan_presentation_tests;

#[cfg(test)]
mod peer_only_reply_tests;

#[cfg(test)]
mod late_tool_target_tests;

#[cfg(test)]
mod runtime_helper_tests;

#[cfg(test)]
mod peer_message_tests;

#[cfg(test)]
mod runtime_notice_tests;

pub mod model_alias;

#[cfg(test)]
mod reply_session_tests;

#[cfg(test)]
mod turn_words_tests;

pub mod recent_routes;

#[cfg(test)]
mod thread_marker_case_tests;

pub mod relay_title;
pub mod decision_standing;

#[cfg(test)]
mod waiting_clock_tests;

pub mod bot_spec;

pub mod bot_rig;

pub mod buddy_rig;

pub mod buddy_moves;

pub mod teammate_tooltip;

pub mod git_quiet;

#[cfg(test)]
pub(crate) mod load_waits;
