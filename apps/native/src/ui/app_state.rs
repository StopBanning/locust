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

pub(super) struct State {
    pub(super) routines: crate::routines_panel::Panel,
    pub(super) browser: crate::browser_panel::Panel,
    pub(super) terminal: crate::terminal_panel::Panel,
    pub(super) changes: crate::changes_panel::Panel,
    pub(super) changes_job: Latest<crate::changes_panel::ReadResult>,
    /// Undo of a turn, run off the window's thread.
    pub(super) undo_job: Latest<crate::changes_panel::UndoAnswer>,
    pub(super) commit: crate::commit_panel::Panel,
    pub(super) commit_read_job: Latest<crate::commit_panel::ReadAnswer>,
    pub(super) commit_job: Latest<crate::commit_panel::CommitAnswer>,
    pub(super) commit_draft_job: Latest<crate::commit_panel::DraftAnswer>,
    pub(super) changes_repo_job: Latest<(std::path::PathBuf, bool)>,
    pub(super) changes_repo_root: Option<std::path::PathBuf>,
    pub(super) changes_repo: Option<(std::path::PathBuf, bool)>,
    pub(super) pr: crate::pull_request_chip::PrState,
    pub(super) side_chat: Option<crate::side_chat::SideChatState>,
    pub(super) review: Option<crate::review_changes::Review>,
    pub(super) github: crate::github_section::Card,
    pub(super) claude_app: crate::claude_app_card::Card,
    /// The box holds a message a locust:// link brought, not yet sent or cleared.
    pub(super) from_link: bool,
    /// Claude Code's Remote Control server, as last read while Settings was open.
    pub(super) remote_control: Option<locust_core::remote_control::RemoteControlState>,
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
    /// The conversation whose run the person stopped from this window.
    pub(super) stopped_by_you: Option<String>,
    pub(super) own_models: crate::own_models_card::Card,
    pub(super) own_models_job: Latest<crate::own_models_card::TestAnswer>,
    /// Why a sign-in window could not be opened.
    pub(super) agents_sign_in_said: Option<String>,
    /// Sign-ins a test pressed, kept instead of opened.
    pub(super) agents_signed_in: Vec<String>,
    /// The agent being installed from its row, npm's latest line, and how the last install ended.
    pub(super) agents_install: crate::agents_section::Installing,
    pub(super) agents_install_job: Latest<crate::agents_section::InstallAnswer>,
    /// Teammates' key names, the key card's boxes, and the change in flight (teammate_keys).
    pub(super) keys: crate::key_card::Keys,
    pub(super) keys_job: Latest<crate::key_card::Answer>,
    /// Codex and Copilot kept current: what the service says, and the job that asks it (agents_section.rs).
    pub(super) agent_updates: crate::agents_section::Keeping,
    pub(super) agent_updates_job: Latest<crate::agents_section::UpdatesAnswer>,
    pub(super) diff_notes: crate::diff_notes_ui::Notes,
    pub(super) issues: crate::issue_list::Picker,
    pub(super) issues_job: Latest<crate::issue_list::Answer>,
    pub(super) connectors: crate::connectors_section::Card,
    pub(super) connectors_job: Latest<Vec<locust_core::connector_reader::Connector>>,
    pub(super) github_job: Latest<crate::github_section::Answer>,
    pub(super) claude_app_job: Latest<crate::claude_app_card::Answer>,
    pub(super) review_job: Latest<crate::review_changes::Answer>,
    pub(super) selection_ask: Option<crate::side_chat::SelectionAskState>,
    pub(super) recent_hover: Option<String>,
    pub(super) people_panel: crate::people_panel::Panel,
    pub(super) settings_panel: crate::settings_panel::Panel,
    /// The agent (and model) a stopped turn's card offers to carry on with: the person's pick, else the first ready.
    pub(super) carry_offer: Option<(String, Option<String>)>,
    pub(super) usage_panel: crate::usage_panel::Panel,
    pub(super) keyboard_sheet: crate::keyboard_sheet::Panel,
    pub(super) save_record: crate::save_record::Panel,
    pub(super) import_dialog: crate::import_dialog::Panel,
    pub(super) feedback_dialog: crate::feedback_dialog::Panel,
    pub(super) quit_guard: crate::quit_guard_dialog::Panel,
    pub(super) add_menu_open: bool,
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
    pub(super) draft: TextEdit,
    pub(super) picks: HashMap<String, TurnPick>,
    pub(super) models: HashMap<String, Vec<engine::Models>>,
    pub(super) limited_runtimes: HashMap<String, String>,
    pub(super) model_menu: Option<String>,
    /// What the person typed in the model picker's search.
    pub(super) model_query: TextEdit,
    /// The conversation's reference folders on screen, the conversation they were read for, and the chips' menus.
    pub(super) reference_items: Vec<locust_core::references::Item>,
    pub(super) references_key: String,
    pub(super) reference_menu: bool,
    pub(super) reference_picker: bool,
    pub(super) reference_note: Option<String>,
    /// Whether Locust's window is the one in front; the turns and cards already told; finishes gathering.
    pub(super) window_focused: bool,
    pub(super) told_cards: std::collections::HashSet<String>,
    pub(super) live_turns: std::collections::HashSet<String>,
    pub(super) finishes: Vec<(String, String)>,
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
    /// The kit gallery scene (look::gallery) instead of the app.
    pub(super) gallery: bool,
    /// Each teammate's look (hue, seeded parts, picked bot), read beside the roster (native::load_looks).
    pub(super) looks: std::collections::HashMap<String, locust_core::native::FaceLook>,
    /// The icon gallery scene (look::gallery::icons_view).
    pub(super) icon_gallery: bool,
    pub(super) activity_open: HashMap<String, bool>,
    pub(super) recent: Vec<ConversationRow>,
    pub(super) recent_error: Option<String>,
    pub(super) opening_teammate: Option<String>,
    pub(super) opening_conversation: Option<String>,
    pub(super) waiting_target: Option<(String, String)>,
    pub(super) recents: Latest<Result<RecentBatch, String>>,
    pub(super) ledger_summaries: HashMap<String, ledger::Summary>,
    pub(super) ledger_available: bool,
    pub(super) ledger: Option<LedgerPane>,
    pub(super) ledger_job: Latest<LedgerAnswer>,
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
    /// Permission mode for Shift+Tab / slash (Electron mode chip).
    pub(super) mode: String,
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
    pub(super) find_open: bool,
    pub(super) find_query: TextEdit,
    pub(super) prompt_rail: prompt_rail::PromptRailState,
    pub(super) thread_away: bool,
    pub(super) jump_hover: bool,
    pub(super) find_hits: Vec<thread_find::FindHit>,
    pub(super) find_index: usize,
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
    pub(super) group_menu_id: Option<String>,
    pub(super) group_remove_confirm: bool,
    pub(super) folded_groups: std::collections::HashSet<String>,
    pub(super) group_settings: crate::group_settings_dialog::Panel,
    pub(super) file_view: Option<(reply_actions::FileTarget, Result<String, String>)>,
    /// Last Copy press feedback (`Copied` / `Not copied`), keyed by body.
    pub(super) copy_feedback: Option<reply_actions::CopyFeedback>,
    /// ReplyDiagram Code tab is selected when true; keyed by fence source hash.
    pub(super) diagram_code_tab: HashMap<u64, bool>,
    /// Decodes math/PDF/attachment PNGs for Markdown images (set in setup).
    pub(super) image_cache: Option<std::sync::Arc<rune::ImageCache>>,
    /// Suggested tasks the person put away (`NextSteps.tsx` `gone`).
    pub(super) next_steps_gone: std::collections::HashSet<String>,
    /// Last handed-file reveal refusal, keyed by message scope.
    pub(super) handed_file_refusal: Option<(String, String)>,
    /// Post-edit checks currently running on the engine: `mission_id -> command`.
    pub(super) running_checks: HashMap<String, String>,
    /// Expanded "What Locust sent" disclosures on `CheckFixLine` items.
    pub(super) open_check_fix_prompts: std::collections::HashSet<String>,
    pub(super) last_restore: Option<locust_core::profile_backup::RestoreOutcome>,
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
    pub(super) mission_owners: HashMap<String, String>,
    pub(super) relay_live: native::RelayLiveState,
    pub(super) relay_hop_cap: u32,
    pub(super) left_out_by_you: Vec<String>,
    pub(super) edit: crate::edit_message::Panel,
    pub(super) exchange_open: Option<String>,
    pub(super) exchange_stopping: bool,
    pub(super) peer_thread_open: HashMap<String, bool>,
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
            browser: crate::browser_panel::Panel::default(),
            terminal: crate::terminal_panel::Panel::default(),
            settings_panel: crate::settings_panel::Panel::default(),
            carry_offer: None,
            usage_panel: crate::usage_panel::Panel::default(),
            keyboard_sheet: crate::keyboard_sheet::Panel::default(),
            save_record: crate::save_record::Panel::default(),
            import_dialog: crate::import_dialog::Panel::default(),
            feedback_dialog: crate::feedback_dialog::Panel::default(),
            quit_guard: crate::quit_guard_dialog::Panel::default(),
            add_menu_open: false,
            session_places: None,
            home: crate::home_panel::Panel::default(),
            keychain_asking: false,
            changes: crate::changes_panel::Panel::default(),
            changes_job: Latest::default(),
            undo_job: Latest::default(),
            commit: crate::commit_panel::Panel::default(),
            commit_read_job: Latest::default(),
            commit_job: Latest::default(),
            commit_draft_job: Latest::default(),
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
            agent_accounts: Default::default(),
            link_refusal: None,
            opened_links: Vec::new(),
            question_answers: Default::default(),
            usage_line: None,
            usage_line_job: Latest::default(),
            stopped_by_you: None,
            own_models: Default::default(),
            own_models_job: Latest::default(),
            agents_sign_in_said: None,
            agents_signed_in: Vec::new(),
            agents_install: Default::default(),
            agents_install_job: Latest::default(),
            keys: Default::default(),
            keys_job: Latest::default(),
            agent_updates: Default::default(),
            agent_updates_job: Latest::default(),
            diff_notes: Default::default(),
            issues: Default::default(),
            issues_job: Latest::default(),
            connectors: Default::default(),
            connectors_job: Latest::default(),
            github_job: Latest::default(),
            claude_app_job: Latest::default(),
            review_job: Latest::default(),
            selection_ask: None,
            teammates: Vec::new(),
            selected: 0,
            conversations: HashMap::new(),
            draft: TextEdit::new(""),
            picks: HashMap::new(),
            models: HashMap::new(),
            limited_runtimes: HashMap::new(),
            model_menu: None,
            model_query: TextEdit::default(),
            reference_items: Vec::new(),
            references_key: String::new(),
            reference_menu: false,
            reference_picker: false,
            reference_note: None,
            window_focused: true,
            told_cards: std::collections::HashSet::new(),
            live_turns: std::collections::HashSet::new(),
            finishes: Vec::new(),
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
            gallery: false,
            looks: std::collections::HashMap::new(),
            icon_gallery: false,
            activity_open: HashMap::new(),
            recent: Vec::new(),
            recent_error: None,
            opening_teammate: None,
            opening_conversation: None,
            waiting_target: None,
            recents: Latest::default(),
            ledger_summaries: HashMap::new(),
            ledger_available: false,
            ledger: None,
            ledger_job: Latest::default(),
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
            mode: "ask".into(),
            nothing_installed: false,
            selected_ready: true,
            route_can_run: true,
            data: None,
            off_the_queue: false,
            scene_popup: ComposerScenePopup::None,
            org: org::OrgState::default(),
            sidebar_query: TextEdit::new(""),
            find_open: false,
            find_query: TextEdit::new(""),
            prompt_rail: prompt_rail::PromptRailState::default(),
            thread_away: false,
            jump_hover: false,
            find_hits: Vec::new(),
            find_index: 0,
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
            group_menu_id: None,
            group_remove_confirm: false,
            folded_groups: std::collections::HashSet::new(),
            group_settings: crate::group_settings_dialog::Panel::default(),
            file_view: None,
            copy_feedback: None,
            diagram_code_tab: HashMap::new(),
            image_cache: None,
            next_steps_gone: std::collections::HashSet::new(),
            handed_file_refusal: None,
            running_checks: HashMap::new(),
            open_check_fix_prompts: std::collections::HashSet::new(),
            last_restore: None,
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
            mission_owners: HashMap::new(),
            relay_live: native::RelayLiveState::default(),
            relay_hop_cap: locust_core::workspace_settings::DEFAULT_RELAY_HOP_CAP,
            left_out_by_you: Vec::new(),
            edit: crate::edit_message::Panel::default(),
            exchange_open: None,
            exchange_stopping: false,
            peer_thread_open: HashMap::new(),
        }
    }
}
