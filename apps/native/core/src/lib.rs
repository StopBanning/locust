//! Locust's engine: everything the app does that is not drawing. It must never depend on the UI toolkit;
//! `cargo tree -p locust-core` shows no Rune.

pub mod agents;
pub mod path_title;
pub mod antigravity_cascade;
pub mod antigravity_usage;
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
pub mod chat_apps;
pub mod chat_discord;
pub mod chat_discord_fmt;
pub mod chat_host;
pub mod chat_http;
pub mod chat_link;
pub mod chat_slack;
pub mod chat_slack_fmt;
pub mod chat_telegram;
pub mod chat_telegram_fmt;
#[cfg(test)]
mod chat_link_tests;
pub mod claude_app;
pub mod locust_link;
pub mod reply_page;
pub mod remote_control;
pub mod remote_mcp;
pub mod agent_accounts;
pub mod agent_install;
pub mod teammate_keys;
pub mod text_files;
pub mod undone_note;
pub mod tool_fold;
pub mod step_pictures;
pub mod speech_runs;
#[cfg(test)]
mod thinking_dots_tests;
#[cfg(test)]
mod model_limit_bar_tests;
#[cfg(test)]
mod thin_parts_tests;
#[cfg(test)]
mod first_hour_tests;
pub mod mcp_names;
pub mod usage_words;
pub mod agent_card;
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
pub mod cursor_config;
pub mod cursor_config_allow;
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
#[cfg(test)]
mod free_start_tests;
pub mod context_usage;
pub mod desk;
pub mod disk_observation;
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
pub mod limits;
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
pub mod phone_alerts;
pub mod phone_calls;
pub mod phone_host;
pub mod phone_page;
pub mod phone_server;
pub mod png_text;
pub mod profile_backup;
pub mod protocol_tags;
pub mod pull_requests;
pub mod qr;
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
pub mod routine_hooks;
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
#[cfg(test)]
mod shell_wrapper_tests;
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
#[cfg(test)]
mod rail_clock_tests;
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
#[cfg(test)]
mod tool_words_tests;
#[cfg(test)]
mod tool_vocabulary_tests;
pub mod activity_trace;
pub mod conversation_turns;

pub mod peer_thread;

pub mod signal_rail;

#[cfg(test)]
mod assistant_text_tests;

#[cfg(test)]
mod changed_file_tests;
#[cfg(test)]
mod copilot_effort_levels_tests;

#[cfg(test)]
mod typed_prompt_tests;

#[cfg(test)]
mod handoff_brief_tests;

#[cfg(test)]
mod leave_out_brief_tests;

#[cfg(test)]
mod file_name_brief_tests;

#[cfg(test)]
mod switch_brief_tests;

#[cfg(test)]
mod provider_carry_on_tests;

#[cfg(test)]
mod plan_carry_on_tests;

#[cfg(test)]
mod workspace_brief_tests;

#[cfg(test)]
mod mission_mode_tests;

#[cfg(test)]
mod handover_note_tests;

#[cfg(test)]
mod route_at_start_tests;

#[cfg(test)]
mod model_lists_tests;

#[cfg(test)]
mod model_catalog_tests;

#[cfg(test)]
mod usage_readings_kept_tests;

#[cfg(test)]
mod mission_history_tests;

#[cfg(all(test, unix))]
mod permission_host_tests;

#[cfg(test)]
mod approval_channel_tests;

#[cfg(test)]
mod copilot_permission_tests;

#[cfg(test)]
mod approvals_from_the_window_tests;

// Real post-checkout hooks are made executable the Unix way.
#[cfg(all(test, unix))]
mod worktree_checkout_tests;

#[cfg(test)]
mod install_lock_tests;

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

pub mod face_status;
pub mod face_legend;
pub mod look_picker;
pub mod pet_library;
pub mod pet_gallery;
#[cfg(test)]
mod pet_gallery_tests;
pub mod pets;
pub mod pet_motion;
pub mod pet_puppets;
#[cfg(test)]
mod pet_library_tests;
#[cfg(test)]
mod pet_record_tests;
#[cfg(test)]
mod routine_lines_tests;
#[cfg(test)]
mod block_in_code_tests;
#[cfg(test)]
mod compare_points_tests;
#[cfg(test)]
mod example_file_tests;
#[cfg(test)]
mod notes_for_a_question_tests;
#[cfg(test)]
mod approval_data_tests;
#[cfg(test)]
mod question_not_a_card_tests;
#[cfg(test)]
mod room_task_claims_tests;
#[cfg(test)]
mod step_budget_tests;
#[cfg(test)]
mod torn_tag_tests;
#[cfg(test)]
mod effort_choices_tests;
#[cfg(test)]
mod every_runtime_ledger_tests;
#[cfg(test)]
mod mission_store_records_tests;
#[cfg(test)]
mod checkpoint_reconcile_tests;
#[cfg(test)]
mod mission_store_ledger_tests;
#[cfg(test)]
mod schema_compat_tests;
#[cfg(test)]
mod shared_rules_tests;
pub mod cover_activity;

#[cfg(test)]
mod every_teammate_is_a_bot_tests;
#[cfg(test)]
mod face_ledger_tests;
#[cfg(test)]
mod cost_sentences_tests;
#[cfg(test)]
mod route_name_tests;
#[cfg(test)]
mod free_model_order_tests;
#[cfg(test)]
mod finished_toast_tests;
#[cfg(test)]
mod claude_route_name_tests;
#[cfg(test)]
mod handoff_route_tests;
#[cfg(test)]
mod compare_changes_line_tests;

#[cfg(test)]
pub(crate) mod load_waits;

#[cfg(test)]
mod renderer_eye_tests;

#[cfg(test)]
mod renderer_work_faces_tests;
#[cfg(test)]
mod screen_eye_weight_tests;
#[cfg(test)]
mod screen_holds_eyes_tests;

pub mod teammate_reach;

pub mod teammate_work;

pub mod teammate_month;
#[cfg(test)]
mod unknown_record_tests;
#[cfg(test)]
mod earlier_peer_only_tests;
#[cfg(test)]
mod finished_message_tests;
#[cfg(test)]
#[path = "long_turn_tests.rs"]
mod long_turn_tests;
#[cfg(test)]
#[path = "recorded_picker_tests.rs"]
mod recorded_picker_tests;
#[cfg(test)]
#[path = "recorded_routes_tests.rs"]
mod recorded_routes_tests;
#[cfg(test)]
mod recorded_words_tests;
#[cfg(test)]
mod recorded_cost_tests;
#[cfg(test)]
mod recorded_failure_tests;
#[cfg(test)]
mod cancelled_run_cost_tests;
#[cfg(test)]
mod next_steps_cards_tests;
pub mod window_bounds;
#[cfg(test)]
mod window_bounds_tests;
pub mod workspace_launch;
#[cfg(test)]
mod workspace_launch_tests;
#[cfg(test)]
mod slow_agent_tests;
#[cfg(test)]
mod live_line_tests;
// Reasoning effort in words and as Claude Code's slider (exec/native-picker).
pub mod effort_levels;
// The model picker's rows: order, fold, free tag, the free model to offer (exec/native-picker).
pub mod route_rows;
pub mod runtime_order;
pub mod steps_fold;
#[cfg(test)]
mod steps_fold_tests;
pub mod thread_words;
#[cfg(test)]
mod thread_words_tests;
#[cfg(test)]
mod runtime_order_tests;
