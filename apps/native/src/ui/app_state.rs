use super::*;

pub(super) fn color(value: &str) -> Color {
    // Palette::new supplies only valid hex source literals.
    rune::color::hex(value).expect("valid Locust colour")
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum StatusTone {
    Muted,
    Green,
    Amber,
    Red,
}

pub(super) struct Palette {
    pub(super) dark: bool,
    pub(super) bg: Color,
    pub(super) sidebar: Color,
    pub(super) raised: Color,
    pub(super) selected: Color,
    pub(super) your_bubble: Color,
    pub(super) field: Color,
    pub(super) hairline: Color,
    pub(super) text: Color,
    pub(super) secondary: Color,
    pub(super) muted: Color,
    pub(super) green: Color,
    pub(super) amber: Color,
    pub(super) red: Color,
}

impl Palette {
    pub(super) fn new(dark: bool) -> Self {
        let tokens = if dark {
            [
                "#121415", "#17191a", "#191c1d", "#1e2223", "#23282a", "#1a1d1e", "#f2efe9",
                "#c7c3bc", "#8b9190", "#7fc97f", "#e9b949", "#e4685d",
            ]
        } else {
            [
                "#faf9f6", "#f1f0ec", "#ffffff", "#e7e6e1", "#ebeae5", "#ffffff", "#1c1e1d",
                "#3a3e3d", "#5f6563", "#2e8540", "#b8790a", "#c9443a",
            ]
        };
        let [
            bg,
            sidebar,
            raised,
            selected,
            your_bubble,
            field,
            text,
            secondary,
            muted,
            green,
            amber,
            red,
        ] = tokens.map(color);

        Self {
            dark,
            bg,
            sidebar,
            raised,
            selected,
            your_bubble,
            field,
            hairline: if dark {
                Color::WHITE.alpha(0.06)
            } else {
                text.alpha(0.08)
            },
            text,
            secondary,
            muted,
            green,
            amber,
            red,
        }
    }

    pub(super) fn status(&self, tone: StatusTone) -> Color {
        match tone {
            StatusTone::Muted => self.muted,
            StatusTone::Green => self.green,
            StatusTone::Amber => self.amber,
            StatusTone::Red => self.red,
        }
    }
}

#[derive(Default)]
pub(super) struct Conversation {
    pub(super) conversation_id: Option<String>,
    pub(super) turns: Vec<(String, String)>,
}

// Tag each flight with its teammate ID, conversation ID, header, and tip so stale results are dropped.
pub(super) struct Flight {
    pub(super) teammate_id: String,
    pub(super) conversation_id: Option<String>,
    pub(super) header: Option<Teammate>,
    pub(super) after_tip: Option<String>,
    // Some only until start/send lands; reply failures must not restore sent text.
    pub(super) message: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct TurnPick {
    pub(super) model: String,
    pub(super) effort: Option<String>,
    /// Another agent than the teammate's own, picked for this turn: the reply runs there (a route switch, with the
    /// hand-over brief), as Electron's RoutePicker offers every installed agent.
    pub(super) runtime: Option<String>,
    /// Explicit account pick (`Some(None)` = this computer's sign-in, `Some(Some(id))` = added account).
    pub(super) account: Option<Option<String>>,
}

pub(super) struct OwnTurn {
    pub(super) conversation: String,
    pub(super) tip: Option<String>,
    pub(super) before: Option<String>,
}

/// Runtime catalogues are kept as listed so a model can return after a later successful run.
pub(super) struct ModelCatalogues {
    listed: HashMap<String, Vec<engine::Models>>,
    visible: HashMap<String, Vec<engine::Models>>,
    retired: std::collections::HashSet<String>,
}

impl Default for ModelCatalogues {
    fn default() -> Self {
        Self {
            listed: HashMap::new(),
            visible: HashMap::new(),
            retired: locust_core::connectors_off::retired_free_models(&[]),
        }
    }
}

impl std::ops::Deref for ModelCatalogues {
    type Target = HashMap<String, Vec<engine::Models>>;

    fn deref(&self) -> &Self::Target {
        &self.visible
    }
}

impl ModelCatalogues {
    pub(super) fn insert(&mut self, id: String, models: Vec<engine::Models>) {
        self.visible.insert(id.clone(), self.visible_models(models.clone()));
        self.listed.insert(id, models);
        self.remember_names();
    }

    /// The chip and every row name a model as its runtime spells it (`rememberOwnModels` over every catalogue read).
    fn remember_names(&self) {
        locust_core::teammate_tooltip::remember_own_models(self.listed.values().flatten().flat_map(|a| {
            a.models.iter().map(move |m| (a.runtime.as_str(), m.id.as_str(), m.name.as_str()))
        }));
    }

    pub(super) fn remove(&mut self, id: &str) {
        self.visible.remove(id);
        self.listed.remove(id);
        self.remember_names();
    }

    pub(super) fn update_retired(&mut self, retired: std::collections::HashSet<String>) {
        if self.retired != retired {
            self.retired = retired;
            self.visible = self.listed.iter().map(|(id, models)| {
                (id.clone(), self.visible_models(models.clone()))
            }).collect();
        }
    }

    fn visible_models(&self, mut models: Vec<engine::Models>) -> Vec<engine::Models> {
        for agent in &mut models {
            if agent.runtime == locust_core::connectors_off::FREE_START_RUNTIME {
                agent.models.retain(|model| !self.retired.contains(&model.id));
            }
        }
        models
    }

    #[cfg(test)]
    pub(super) fn get_mut(&mut self, id: &str) -> Option<&mut Vec<engine::Models>> {
        self.visible.get_mut(id)
    }
}

pub(super) struct State {
    pub(super) scout: crate::scout_cards::Panel,
    pub(super) background_runs: crate::background_runs::Panel,
    pub(super) cloud: crate::claude_cloud_panel::Cloud,
    pub(super) routines: crate::routines_panel::Panel,
    pub(super) browser: crate::browser_panel::Panel,
    pub(super) page_points: crate::page_points_ui::PagePoints,
    pub(super) terminal: crate::terminal_panel::Panel,
    pub(super) changes: crate::changes_panel::Panel,
    pub(super) changes_job: Latest<crate::changes_panel::ReadResult>,
    /// Undo of a turn, run off the window's thread.
    pub(super) undo_job: Latest<crate::changes_panel::UndoAnswer>,
    /// What each finished turn's foot may offer to put back (TurnUndo), read off the window's thread.
    pub(super) turn_undo: crate::turn_undo::Panel,
    pub(super) turn_undo_job: Latest<crate::turn_undo::Answer>,
    /// The tagged teammates' runs, started off the window's thread (`tag_send`).
    pub(super) tag_job: Latest<locust_core::tagging::Tagged>,
    pub(super) commit: crate::commit_panel::Panel,
    pub(super) commit_read_job: Latest<crate::commit_panel::ReadAnswer>,
    pub(super) commit_job: Latest<crate::commit_panel::CommitAnswer>,
    pub(super) commit_draft_job: Latest<crate::commit_panel::DraftAnswer>,
    pub(super) walk_job: Latest<crate::changes_panel::WalkAnswer>,
    pub(super) changes_repo_job: Latest<(std::path::PathBuf, bool)>,
    pub(super) changes_repo_root: Option<std::path::PathBuf>,
    pub(super) changes_repo: Option<(std::path::PathBuf, bool)>,
    pub(super) pr: crate::pull_request_chip::PrState,
    pub(super) side_chat: Option<crate::side_chat::SideChatState>,
    pub(super) review: Option<crate::review_changes::Review>,
    pub(super) chat_apps: crate::chat_apps_section::ChatAppsState,
    pub(super) github: crate::github_section::Card,
    pub(super) claude_app: crate::claude_app_card::Card,
    /// The box holds a message a locust:// link brought, not yet sent or cleared.
    pub(super) from_link: bool,
    /// Claude Code's Remote Control server, as last read while Settings was open.
    pub(super) remote_control: Option<locust_core::remote_control::RemoteControlState>,
    /// Other apps from anywhere (Electron remote-mcp.ts): the switch, the apps with keys, a sign-in waiting on its card.
    pub(super) remote_mcp: Option<locust_core::remote_mcp::RemoteMcpState>,
    pub(super) agent_accounts: crate::agent_accounts_card::Card,
    /// The reply link last refused, and why (said beside it).
    pub(super) link_refusal: Option<(String, String)>,
    /// Web links a test pressed, kept instead of opened.
    pub(super) opened_links: Vec<String>,
    /// What the person has chosen and typed on a question card.
    pub(super) question_answers: crate::question_card::Answers,
    /// Home's usage line: the last 30 days, read when Home opens.
    pub(super) usage_line: Option<locust_core::usage::UsageSummary>,
    pub(super) usage_line_job: Latest<Option<locust_core::usage::UsageSummary>>,
    /// Every account's latest usage reading (`usage_ledger::usage_windows`), for the model chip's card.
    pub(super) usage_readings: HashMap<String, String>,
    pub(super) usage_readings_job: Latest<HashMap<String, String>>,
    /// The pointer is on the model chip.
    pub(super) chip_hover: bool,
    pub(super) hover: crate::hover::HoverState,
    /// The conversation whose run the person stopped from this window.
    pub(super) stopped_by_you: Option<String>,
    pub(super) own_models: crate::own_models_card::Card,
    pub(super) own_models_job: Latest<crate::own_models_card::TestAnswer>,
    /// Why a sign-in window could not be opened.
    pub(super) agents_sign_in_said: Option<String>,
    /// Sign-ins a test pressed, kept instead of opened.
    pub(super) agents_signed_in: Vec<String>,
    /// Where each row's sign-in button stands (SignInButton): asking before signing in again, or its window open.
    pub(super) agents_sign_in_rows: HashMap<String, crate::agents_section::SignInRow>,
    /// The agent being installed from its row, npm's latest line, and how the last install ended.
    pub(super) agents_install: crate::agents_section::Installing,
    pub(super) agents_install_job: Latest<crate::agents_section::InstallAnswer>,
    /// Teammates' key names, the key card's boxes, and the change in flight (teammate_keys).
    pub(super) keys: crate::key_card::Keys,
    pub(super) keys_job: Latest<crate::key_card::Answer>,
    /// Each runtime's own slash commands, as the engine kept them, and the context ring's menu.
    pub(super) runtime_commands: crate::runtime_commands_ui::Commands,
    pub(super) runtime_commands_job: Latest<std::collections::BTreeMap<String, Vec<locust_core::runtime_commands::CommandInfo>>>,
    /// Codex and Copilot kept current: what the service says, and the job that asks it (agents_section.rs).
    pub(super) agent_updates: crate::agents_section::Keeping,
    pub(super) agent_updates_job: Latest<crate::agents_section::UpdatesAnswer>,
    pub(super) diff_notes: crate::diff_notes_ui::Notes,
    pub(super) issues: crate::issue_list::Picker,
    pub(super) issues_job: Latest<crate::issue_list::Answer>,
    pub(super) connectors: crate::connectors_section::Card,
    pub(super) connectors_job: Latest<Vec<locust_core::connector_reader::Connector>>,
    pub(super) github_job: Latest<crate::github_section::Answer>,
    /// Settings › Teammates › Skills from GitHub: the shelf, a look and what is kept.
    pub(super) skills_from_github: crate::skills_from_github::Shelf,
    pub(super) skills_job: Latest<crate::skills_from_github::Answer>,
    pub(super) claude_app_job: Latest<crate::claude_app_card::Answer>,
    pub(super) review_job: Latest<crate::review_changes::Answer>,
    pub(super) selection_ask: Option<crate::side_chat::SelectionAskState>,
    pub(super) recent_hover: Option<String>,
    pub(super) people_panel: crate::people_panel::Panel,
    /// The Conversations screen (Ctrl 1).
    pub(super) missions: crate::missions_screen::Screen,
    /// The Pets row of the look picker (PetPicks.tsx).
    pub(super) pet_picks: crate::pet_picks::PetPicks,
    /// Worn pets drawn as faces: their sheets, cut into cells (PetSprite.tsx).
    pub(super) pet_faces: crate::pet_faces::PetFaces,
    pub(super) settings_panel: crate::settings_panel::Panel,
    /// The agent (and model) a stopped turn's card offers to carry on with: the person's pick, else the first ready.
    pub(super) carry_offer: Option<(String, Option<String>)>,
    pub(super) usage_panel: crate::usage_panel::Panel,
    pub(super) keyboard_sheet: crate::keyboard_sheet::Panel,
    pub(super) save_record: crate::save_record::Panel,
    pub(super) import_dialog: crate::import_dialog::Panel,
    pub(super) feedback_dialog: crate::feedback_dialog::Panel,
    pub(super) quit_guard: crate::quit_guard_dialog::Panel,
    /// Where the sidebar/rail `+` button was laid out, so the add menu hangs from it.
    pub(super) add_menu_at: [f32; 4],
    pub(super) session_places: Option<locust_core::session_import::SessionPlaces>,
    pub(super) home: crate::home_panel::Panel,
    /// Amber macOS keychain note — never reads the keychain itself.
    pub(super) keychain_asking: bool,
    // Which engine this window runs on, shown in the status line. None until the team has loaded.
    pub(super) engine: Option<Kind>,
    pub(super) teammates: Vec<Teammate>,
    pub(super) selected: usize,
    // Fix: histories and conversation IDs survive team reordering and reloads.
    pub(super) conversations: HashMap<String, Conversation>,
    pub(super) folder_question: Option<crate::folder_confirmation::Question>,
    pub(super) draft: TextEdit,
    /// A tracker status in a reply being changed: its strip of other statuses is open (reply_actions::StatusChange).
    pub(super) status_change: Option<crate::reply_actions::StatusChange>,
    pub(super) picks: HashMap<String, TurnPick>,
    pub(super) models: ModelCatalogues,
    pub(super) limited_runtimes: HashMap<String, String>,
    pub(super) model_menu: Option<String>,
    /// What the person typed in the model picker's search.
    pub(super) model_query: TextEdit,
    /// Where the effort thumb is while the pointer holds it (effort_slider.rs); None when it rests on its stop.
    pub(super) effort_held: Option<f32>,
    /// Whether the model picker's Older versions fold is open.
    pub(super) older_models_open: bool,
    /// The conversation's reference folders on screen, the conversation they were read for, and the chips' menus.
    pub(super) reference_items: Vec<locust_core::references::Item>,
    pub(super) references_key: String,
    pub(super) reference_menu: bool,
    pub(super) reference_picker: bool,
    pub(super) reference_note: Option<String>,
    /// The picker's GitHub box and the downloads it starts (reference_chips::Work).
    pub(super) reference_work: crate::reference_chips::Work,
    /// Whether Locust's window is the one in front; the turns and cards already told; finishes gathering.
    pub(super) window_focused: bool,
    /// When motion rests (useCoverActivity's rule): made at the first poll, fed by `cover_watch::observe`.
    pub(super) cover: Option<locust_core::cover_activity::CoverActivity>,
    /// The platform's last input (`rune::last_input`) the cover has been told of.
    pub(super) cover_input_seen: Option<rune::time::Instant>,
    pub(super) told_cards: std::collections::HashSet<String>,
    pub(super) live_turns: std::collections::HashSet<String>,
    pub(super) finishes: Vec<(String, String)>,
    /// Each room's changes, held two minutes from its first and then said as one toast (attention.ts `roomChanged`).
    pub(super) room_toasts: locust_core::attention::RoomToasts,
    pub(super) finish_ids: Vec<String>,
    /// A conversation to open once the recent list has it (a pressed notification's), and until when to keep looking.
    pub(super) open_when_listed: Option<(String, std::time::Instant)>,
    pub(super) finishes_due: Option<std::time::Instant>,
    /// Settings › Appearance › Sidebar (`auto`, `wide`, `compact`), whether this window drew the rail last, and the
    /// rail's card: whose it is, whether a press pinned it, where the pointer is, and when it left.
    /// The turns running or waiting in this process at the last look: when the set changes (a turn another app started
    /// through the door, one ending), the recent list is read again so the sidebar and Board show it.
    pub(super) seen_live: Vec<String>,
    /// The mode chosen on the message box for the turns sent from here (Electron's composer mode); None is the
    /// teammate's own. Picking another teammate goes back to theirs.
    pub(super) composer_mode: Option<String>,
    pub(super) mode_menu: bool,
    /// The message box's + menu (Attach files, Choose a folder).
    pub(super) plus_menu: bool,
    pub(super) layout_pref: String,
    pub(super) layout_compact: std::cell::Cell<bool>,
    pub(super) rail_open: Option<String>,
    pub(super) rail_pinned: bool,
    pub(super) rail_over_face: Option<String>,
    pub(super) rail_over_card: bool,
    pub(super) rail_left_at: Option<std::time::Instant>,
    pub(super) model_job: Latest<(String, Result<Vec<engine::Models>, String>)>,
    pub(super) own_turn: Option<OwnTurn>,
    pub(super) stopping: Latest<(String, Result<String, String>)>,
    pub(super) status: String,
    pub(super) status_tone: StatusTone,
    pub(super) load_error: Option<String>,
    pub(super) flight: Option<Flight>,
    pub(super) demo: bool,
    // None follows the system; snapshot scenes explicitly choose an appearance.
    pub(super) appearance: Option<bool>,
    /// The window is on a Mac's glass (window_glass.rs): set once by the real window, never in a headless run.
    pub(super) mac_glass: bool,
    /// The kit gallery scene (look::gallery) instead of the app.
    pub(super) gallery: bool,
    /// Each teammate's look (hue, seeded parts, picked bot, pet), read beside the roster (native::load_looks).
    pub(super) looks: std::collections::HashMap<String, locust_core::native::FaceLook>,
    /// Rig animation scheduler for visible bots (steps only when active/transitioning; 0 wakes at rest).
    pub(super) bot_stage: crate::look::bot_stage::BotStage,
    /// Last observed system reduced-motion setting from `Cx::is_reduced_motion()`.
    pub(super) reduced_motion: std::cell::Cell<bool>,
    /// The icon gallery scene (look::gallery::icons_view).
    pub(super) icon_gallery: bool,
    pub(super) activity_open: HashMap<String, bool>,
    pub(super) recent: Vec<ConversationRow>,
    pub(super) recent_error: Option<String>,
    pub(super) opening_teammate: Option<String>,
    pub(super) opening_conversation: Option<String>,
    pub(super) waiting_target: Option<(String, String)>,
    pub(super) recents: Latest<Result<RecentBatch, String>>,
    pub(super) recent_route_keys: Vec<String>,
    pub(super) resolved_model_names: HashMap<String, String>,
    pub(super) ledger_summaries: HashMap<String, ledger::Summary>,
    pub(super) ledger_available: bool,
    pub(super) ledger: Option<LedgerPane>,
    pub(super) ledger_job: Latest<LedgerAnswer>,
    pub(super) calls: crate::call_words::ThreadCalls,
    pub(super) voice: crate::voice_panel::Panel,
    pub(super) opening: Latest<Result<Reply, String>>,
    pub(super) team: Latest<Result<(Kind, Vec<Teammate>), String>>,
    pub(super) sending: Latest<Result<Started, String>>,
    pub(super) waiting: Latest<Result<Reply, String>>,
    /// First card waiting on the person in this window (Approve each).
    pub(super) pending_approval: Option<approval::Request>,
    /// Visible refusal when answering after the turn ended.
    pub(super) approval_notice: Option<String>,
    pub(super) approval_shown: std::time::Instant,
    pub(super) approval_denying: bool,
    pub(super) approval_reason: TextEdit,
    // --- M4b composer (Electron Composer.tsx) ---
    pub(super) drafts: composer::Drafts,
    pub(super) attached: Vec<String>,
    pub(super) tagged: Vec<String>,
    pub(super) queue_rows: Vec<composer::QueuedRow>,
    pub(super) queue_error: Option<String>,
    pub(super) queue_send_now: Option<String>,
    /// The queued row the queue just sent, until its send lands: refused as busy, it goes back in line (retried_after_busy).
    pub(super) queue_in_flight: Option<composer::QueuedRow>,
    pub(super) composer_note: Option<String>,
    pub(super) slash_at: usize,
    pub(super) at_at: usize,
    pub(super) at_dismissed: Option<String>,
    pub(super) project_files: Vec<String>,
    pub(super) files_partial: bool,
    pub(super) files_job: Latest<(Vec<String>, bool)>,
    pub(super) files_root: Option<std::path::PathBuf>,
    pub(super) files_asked: Option<std::time::Instant>,
    pub(super) sent_history: Vec<String>,
    pub(super) recall_at: isize,
    /// Scene / discovery flags for placeholders and disabled-send.
    pub(super) nothing_installed: bool,
    pub(super) selected_ready: bool,
    pub(super) route_can_run: bool,
    /// Native data folder for queue + pasted attachments (never Electron's).
    pub(super) data: Option<store::Store>,
    /// Off-queue edit strip (Composer: Edit took words back into the box).
    pub(super) off_the_queue: bool,
    /// Popup forced open for snap scenes.
    pub(super) scene_popup: ComposerScenePopup,
    // --- P4 conversation organization ---
    pub(super) org: org::OrgState,
    pub(super) sidebar_query: TextEdit,
    /// Which conversations the sidebar lists, and whether its menu is open (Sidebar.tsx `sourceFilter`).
    pub(super) sidebar_source: crate::conversation_list::ConversationSource,
    pub(super) sidebar_source_open: bool,
    /// The source select's highlight while open (Select.tsx `highlighted`), and the letters typed into it with when.
    pub(super) sidebar_source_active: i64,
    pub(super) sidebar_source_typed: (String, Option<std::time::Instant>),
    /// How many rows of the long ungrouped conversation list are drawn (ListWindow.tsx `listBudget`).
    pub(super) sidebar_list_budget: usize,
    pub(super) find_open: bool,
    pub(super) find_query: TextEdit,
    pub(super) prompt_rail: prompt_rail::PromptRailState,
    pub(super) thread_away: bool,
    pub(super) jump_hover: bool,
    pub(super) find_hits: Vec<thread_find::FindHit>,
    pub(super) find_index: usize,
    /// The query `find_index` was stepped for: a changed query starts again at its first hit.
    pub(super) find_hits_query: String,
    pub(super) find_pieces: Vec<String>,
    pub(super) find_earlier_turns: usize,
    pub(super) find_show_earlier: bool,
    pub(super) palette_open: bool,
    pub(super) palette_query: TextEdit,
    pub(super) palette_index: usize,
    pub(super) palette_actions: Vec<command_palette::PaletteAction>,
    pub(super) beside: Option<beside::BesideState>,
    pub(super) rename_id: Option<String>,
    pub(super) rename_draft: TextEdit,
    pub(super) org_menu_id: Option<String>,
    /// The right-click menu open over the window (context_menu.rs).
    pub(super) row_menu: Option<crate::context_menu::MenuState>,
    /// Where the conversation header's `⋯` was laid out, so its menu opens under it.
    pub(super) header_menu_at: [f32; 4],
    pub(super) group_menu_id: Option<String>,
    pub(super) group_remove_confirm: bool,
    pub(super) folded_groups: std::collections::HashSet<String>,
    pub(super) group_settings: crate::group_settings_dialog::Panel,
    pub(super) file_view: Option<(reply_actions::FileTarget, Result<String, String>)>,
    /// Current zoom and pan of a picture open in the file viewer (ZoomableImage.tsx `view`).
    pub(super) viewer_image_view: Option<crate::step_pictures::ImageView>,
    /// Last Copy press feedback (`Copied` / `Not copied`), keyed by body.
    pub(super) copy_feedback: Option<reply_actions::CopyFeedback>,
    /// ReplyDiagram Code tab is selected when true; keyed by fence source hash.
    pub(super) diagram_code_tab: HashMap<u64, bool>,
    /// Decodes math/PDF/attachment PNGs for Markdown images (set in setup).
    pub(super) image_cache: Option<std::sync::Arc<rune::ImageCache>>,
    /// Pictures a step or reply draws, by (conversation folder, path as written): the handle keeps the decode alive
    /// between frames, and None remembers a refusal so the disk is asked once (`step_pictures.rs`).
    pub(super) thread_pictures: std::cell::RefCell<HashMap<(std::path::PathBuf, String), Option<rune::Image>>>,
    /// The last answer to whether the idle teammate's folder holds nothing yet: the folder, when it was asked, and the
    /// answer (idle_teammate::folder_holds_nothing_yet).
    pub(super) folder_empty_seen: std::cell::RefCell<Option<(std::path::PathBuf, std::time::Instant, bool)>>,
    /// Suggested tasks the person put away (`NextSteps.tsx` `gone`).
    pub(super) next_steps_gone: std::collections::HashSet<String>,
    /// Last handed-file reveal refusal, keyed by message scope.
    pub(super) handed_file_refusal: Option<(String, String)>,
    /// Post-edit checks currently running on the engine: `mission_id -> command`.
    pub(super) running_checks: HashMap<String, String>,
    /// Expanded "What Locust sent" disclosures on `CheckFixLine` items.
    pub(super) open_check_fix_prompts: std::collections::HashSet<String>,
    pub(super) last_restore: Option<locust_core::profile_backup::RestoreOutcome>,
    /// "Find your folders on this computer": the restored folders missing here, and what was chosen for each.
    pub(super) restore_folders: crate::restored_folders_card::Finding,
    pub(super) inspector_open: bool,
    pub(super) inspector_tab: crate::inspector_panel::Tab,
    pub(super) inspector_turn: Option<usize>,
    pub(super) connector_names: Vec<String>,
    pub(super) teammate_connectors: HashMap<String, Vec<String>>,
    pub(super) dropped_problem: Option<(String, String)>,
    pub(super) setup_card: Option<crate::setup_card::SetupCard>,
    pub(super) instructions_card: crate::instructions_card::InstructionsCardState,
    pub(super) tune_card: Option<crate::tune_teammate::TuneCardState>,
    pub(super) compare: crate::compare_view::CompareState,
    pub(super) benchmarks_open: bool,
    pub(super) rooms: crate::room_screen::RoomsState,
    pub(super) workroom_messages: HashMap<String, locust_core::workroom::Message>,
    /// Handoffs heard from the workroom, held for their moment (`handoff_glances.rs`).
    pub(super) handoff_glances: crate::handoff_glances::HandoffGlances,
    /// Live runs the list was read too early to know, read again for (`live_list.rs`).
    pub(super) unlisted_runs: crate::live_list::UnlistedRuns,
    /// What each live run is doing and who just finished, for the faces (`teammate_faces.rs`).
    pub(super) live_faces: crate::teammate_faces::LiveFaces,
    pub(super) mission_owners: HashMap<String, String>,
    pub(super) relay_live: native::RelayLiveState,
    pub(super) relay_hop_cap: u32,
    /// How many conversations the records keep, when more than the list shows (native::MAX_LISTED_MISSIONS).
    pub(super) recent_total: Option<usize>,
    /// What the records say about their own damage, under the list (native::ListDamage).
    pub(super) recent_damage: native::ListDamage,
    pub(super) left_out_by_you: Vec<String>,
    /// The handoff preview under the box, as the send would compose it, and what it was composed for (the
    /// conversation, the runtime the reply goes to, the leave-outs, the draft's size), so it is read again only when
    /// one of those changes (handoff_divider::refresh_preview).
    pub(super) handoff_preview: Option<(String, Option<String>)>,
    pub(super) edit: crate::edit_message::Panel,
    pub(super) exchange_open: Option<String>,
    pub(super) exchange_stopping: bool,
    pub(super) earlier_turn_budgets: HashMap<String, usize>,
    pub(super) peer_thread_open: HashMap<String, bool>,
    /// A Stop pressed while the run was still starting, remembered against its key until the start names it.
    pub(super) stop_when_named: Option<String>,
    pub(super) elsewhere_cover: crate::elsewhere_cover::ElsewhereCover,
    /// The sidebar list narrowed to one teammate's conversations, from that face's card (Sidebar.tsx `faceFilter`).
    pub(super) sidebar_face_filter: Option<String>,
    /// "Your phone" From anywhere settings (PhoneAnywhereSettings).
    pub(super) phone_anywhere: Option<locust_core::phone_anywhere::PhoneAnywhereSettings>,
    /// Live "From anywhere" relay link, owned by the window while enabled (`phone_section::PhoneAnywhereLink`).
    pub(super) phone_anywhere_link: crate::phone_section::PhoneAnywhereLink,
    /// When phone is off, whether the person picked "Only on this Wi-Fi" instead of "From anywhere".
    pub(super) phone_picked_wifi: bool,
    /// Startup switches for Remote MCP ("Use from anywhere" Keep it running, Electron StayingReachable).
    pub(super) remote_mcp_stay: Option<crate::remote_mcp_card::StayingReachable>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum ComposerScenePopup {
    #[default]
    None,
    Slash,
    Mention,
    Queue,
    Blocked,
}

impl Default for State {
    fn default() -> Self {
        Self {
            engine: None,
            recent_hover: None,
            routines: crate::routines_panel::Panel::default(),
            people_panel: crate::people_panel::Panel::default(),
            missions: crate::missions_screen::Screen::default(),
            pet_picks: crate::pet_picks::PetPicks::default(),
            pet_faces: crate::pet_faces::PetFaces::default(),
            browser: crate::browser_panel::Panel::default(),
            page_points: Default::default(),
            terminal: crate::terminal_panel::Panel::default(),
            settings_panel: crate::settings_panel::Panel::default(),
            chat_apps: Default::default(),
            carry_offer: None,
            usage_panel: crate::usage_panel::Panel::default(),
            keyboard_sheet: crate::keyboard_sheet::Panel::default(),
            save_record: crate::save_record::Panel::default(),
            import_dialog: crate::import_dialog::Panel::default(),
            feedback_dialog: crate::feedback_dialog::Panel::default(),
            quit_guard: crate::quit_guard_dialog::Panel::default(),
            add_menu_at: [0.0; 4],
            session_places: None,
            home: crate::home_panel::Panel::default(),
            keychain_asking: false,
            changes: crate::changes_panel::Panel::default(),
            changes_job: Latest::default(),
            undo_job: Latest::default(),
            turn_undo: crate::turn_undo::Panel::default(),
            turn_undo_job: Latest::default(),
            tag_job: Latest::default(),
            commit: crate::commit_panel::Panel::default(),
            commit_read_job: Latest::default(),
            commit_job: Latest::default(),
            commit_draft_job: Latest::default(),
            walk_job: Latest::default(),
            changes_repo_job: Latest::default(),
            changes_repo_root: None,
            changes_repo: None,
            pr: crate::pull_request_chip::PrState::default(),
            side_chat: None,
            review: None,
            github: Default::default(),
            claude_app: Default::default(),
            from_link: false,
            remote_control: None,
            remote_mcp: None,
            agent_accounts: Default::default(),
            link_refusal: None,
            opened_links: Vec::new(),
            question_answers: Default::default(),
            usage_line: None,
            usage_line_job: Latest::default(),
            usage_readings: HashMap::new(),
            usage_readings_job: Latest::default(),
            chip_hover: false,
            hover: Default::default(),
            stopped_by_you: None,
            own_models: Default::default(),
            own_models_job: Latest::default(),
            agents_sign_in_said: None,
            agents_signed_in: Vec::new(),
            agents_sign_in_rows: HashMap::new(),
            agents_install: Default::default(),
            agents_install_job: Latest::default(),
            keys: Default::default(),
            keys_job: Latest::default(),
            runtime_commands: Default::default(),
            runtime_commands_job: Latest::default(),
            agent_updates: Default::default(),
            agent_updates_job: Latest::default(),
            diff_notes: Default::default(),
            issues: Default::default(),
            issues_job: Latest::default(),
            connectors: Default::default(),
            connectors_job: Latest::default(),
            github_job: Latest::default(),
            skills_from_github: Default::default(),
            skills_job: Latest::default(),
            claude_app_job: Latest::default(),
            review_job: Latest::default(),
            selection_ask: None,
            teammates: Vec::new(),
            selected: 0,
            conversations: HashMap::new(),
            folder_question: None,
            draft: TextEdit::new(""),
            status_change: None,
            picks: HashMap::new(),
            models: ModelCatalogues::default(),
            limited_runtimes: HashMap::new(),
            model_menu: None,
            model_query: TextEdit::default(),
            effort_held: None,
            older_models_open: false,
            reference_items: Vec::new(),
            references_key: String::new(),
            reference_menu: false,
            reference_picker: false,
            reference_note: None,
            reference_work: Default::default(),
            window_focused: true,
            cover: None,
            cover_input_seen: None,
            told_cards: std::collections::HashSet::new(),
            live_turns: std::collections::HashSet::new(),
            finishes: Vec::new(),
            room_toasts: Default::default(),
            finish_ids: Vec::new(),
            open_when_listed: None,
            finishes_due: None,
            seen_live: Vec::new(),
            composer_mode: None,
            mode_menu: false,
            plus_menu: false,
            layout_pref: "auto".into(),
            layout_compact: std::cell::Cell::new(false),
            rail_open: None,
            rail_pinned: false,
            rail_over_face: None,
            rail_over_card: false,
            rail_left_at: None,
            model_job: Latest::default(),
            own_turn: None,
            stopping: Latest::default(),
            status: "Loading teammates…".into(),
            status_tone: StatusTone::Muted,
            load_error: None,
            flight: None,
            demo: false,
            appearance: None,
            mac_glass: false,
            gallery: false,
            looks: std::collections::HashMap::new(),
            bot_stage: crate::look::bot_stage::BotStage::default(),
            reduced_motion: std::cell::Cell::new(false),
            icon_gallery: false,
            activity_open: HashMap::new(),
            recent: Vec::new(),
            recent_error: None,
            opening_teammate: None,
            opening_conversation: None,
            waiting_target: None,
            recents: Latest::default(),
            ledger_summaries: HashMap::new(),
            resolved_model_names: HashMap::new(),
            recent_route_keys: Vec::new(),
            ledger_available: false,
            ledger: None,
            ledger_job: Latest::default(),
            calls: crate::call_words::ThreadCalls::default(),
            voice: crate::voice_panel::Panel::default(),
            opening: Latest::default(),
            team: Latest::default(),
            sending: Latest::default(),
            waiting: Latest::default(),
            pending_approval: None,
            approval_notice: None,
            approval_shown: std::time::Instant::now(),
            approval_denying: false,
            approval_reason: TextEdit::new(""),
            drafts: composer::Drafts::default(),
            attached: Vec::new(),
            tagged: Vec::new(),
            queue_rows: Vec::new(),
            queue_error: None,
            queue_send_now: None,
            queue_in_flight: None,
            composer_note: None,
            slash_at: 0,
            at_at: 0,
            at_dismissed: None,
            project_files: Vec::new(),
            files_partial: false,
            files_job: Latest::default(),
            files_root: None,
            files_asked: None,
            sent_history: Vec::new(),
            recall_at: -1,
            nothing_installed: false,
            selected_ready: true,
            route_can_run: true,
            data: None,
            off_the_queue: false,
            scene_popup: ComposerScenePopup::None,
            org: org::OrgState::default(),
            sidebar_query: TextEdit::new(""),
            sidebar_source: Default::default(),
            sidebar_source_open: false,
            sidebar_source_active: -1,
            sidebar_source_typed: (String::new(), None),
            sidebar_list_budget: crate::sidebar::LIST_PAGE,
            find_open: false,
            find_query: TextEdit::new(""),
            prompt_rail: prompt_rail::PromptRailState::default(),
            thread_away: false,
            jump_hover: false,
            find_hits: Vec::new(),
            find_index: 0,
            find_hits_query: String::new(),
            find_pieces: Vec::new(),
            find_earlier_turns: 0,
            find_show_earlier: false,
            palette_open: false,
            palette_query: TextEdit::new(""),
            palette_index: 0,
            palette_actions: Vec::new(),
            beside: None,
            rename_id: None,
            rename_draft: TextEdit::new(""),
            org_menu_id: None,
            row_menu: None,
            header_menu_at: [0.0; 4],
            group_menu_id: None,
            group_remove_confirm: false,
            folded_groups: std::collections::HashSet::new(),
            group_settings: crate::group_settings_dialog::Panel::default(),
            file_view: None,
            viewer_image_view: None,
            copy_feedback: None,
            diagram_code_tab: HashMap::new(),
            image_cache: None,
            thread_pictures: std::cell::RefCell::new(HashMap::new()),
            folder_empty_seen: std::cell::RefCell::new(None),
            next_steps_gone: std::collections::HashSet::new(),
            handed_file_refusal: None,
            running_checks: HashMap::new(),
            open_check_fix_prompts: std::collections::HashSet::new(),
            last_restore: None,
            restore_folders: Default::default(),
            inspector_open: false,
            inspector_tab: crate::inspector_panel::Tab::Activity,
            inspector_turn: None,
            connector_names: Vec::new(),
            teammate_connectors: HashMap::new(),
            dropped_problem: None,
            setup_card: None,
            instructions_card: crate::instructions_card::InstructionsCardState::default(),
            tune_card: None,
            compare: crate::compare_view::CompareState::default(),
            benchmarks_open: false,
            rooms: crate::room_screen::RoomsState::new(),
            workroom_messages: HashMap::new(),
            handoff_glances: Default::default(),
            unlisted_runs: Default::default(),
            live_faces: Default::default(),
            mission_owners: HashMap::new(),
            relay_live: native::RelayLiveState::default(),
            relay_hop_cap: locust_core::workspace_settings::DEFAULT_RELAY_HOP_CAP,
            recent_total: None,
            recent_damage: native::ListDamage::default(),
            left_out_by_you: Vec::new(),
            handoff_preview: None,
            edit: crate::edit_message::Panel::default(),
            exchange_open: None,
            exchange_stopping: false,
            earlier_turn_budgets: HashMap::new(),
            peer_thread_open: HashMap::new(),
            scout: crate::scout_cards::Panel::default(),
            stop_when_named: None,
            elsewhere_cover: crate::elsewhere_cover::ElsewhereCover::default(),
            sidebar_face_filter: None,
            background_runs: crate::background_runs::Panel::default(),
            phone_anywhere: None,
            phone_anywhere_link: crate::phone_section::PhoneAnywhereLink::default(),
            phone_picked_wifi: false,
            remote_mcp_stay: None,
            cloud: crate::claude_cloud_panel::Cloud::default(),
        }
    }
}
