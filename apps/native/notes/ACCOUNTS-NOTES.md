# Named Codex & Claude Accounts (`exec/native-accounts`, PARITY row 63)

## Summary of Changes

### Core (`apps/native/core/src/`)
- **`agent_accounts.rs`**: Added `root(&self)` plus a process-wide registry (`register`, `registered`, `shared`) keyed by native data directory so `native`, `runtime_terminal`, and the UI share live sign-in / busy state (`account_in_use`).
- **`route_at_start.rs`** & **`lib.rs`**: Ported Electron `route-at-start.ts` (`route_at_start`, `composer_route_for`, `pick_key_for`, `start_as`, `as_reviewer`) mapping `{runtime, model, account}` into composer picks and turn origins.
- **`command_guard.rs`**: Added `codex_config_for_home(folder, codex_home)` so Codex inspects `other_codex_hooks` under the named account's `CODEX_HOME` instead of leaking or reading the machine home when an account is active.
- **`ledger_write.rs`**: Added `resolved_route` (`resolvedRouteId`) written on `mission.created` and every `mission.event` (`{runtime}:{model}-account:{id|default}`).
- **`engine.rs`**, **`engine_process.rs`**, **`people.rs`**: Added `account: Option<String>` to `Teammate`, serialized `teammate.account` and `origin.account` (`Option<Option<String>>`) across the out-of-process engine RPC, and validated `route.account` via `agent_accounts::is_account_id` on `save_teammate_id`.
- **`engine_api.rs`**: Exposed `account_in_use(&self, agent: Agent, account_id: &str) -> bool` wired to `native::account_in_use`.
- **`native.rs`**:
  - Enabled teammates on named Codex/Claude accounts in `supported`, `route_of` (rejecting accounts on non-Claude/Codex runtimes), and `parse_roster`; cleared `account` on `carry(...)`.
  - Added `Origin.account: Option<Option<String>>`, `Prior.account: Option<String>` (parsed from `resolvedRouteId`), `RUNNING_ACCOUNTS` tracking + `Registered` cleanup + `account_in_use`, `accounts_for`, and `account_launch_env`.
  - Wired `send_from` & `run_once`: `org_policy::turn_refusal(..., effective_account.is_some())`, `account_launch_env`, cold `route_switch` when `prior.account != effective_account`, launch env application (`CODEX_HOME`/`CODEX_SQLITE_HOME` or `CLAUDE_CONFIG_DIR` with blanked API/OAuth keys), `codex_home` forwarded to `run_turn` -> `codex_config_for_home`, and `resolvedRouteId` / `account` recorded in `extra`.
- **`usage_ledger.rs`**: Added `reading_of_warning`, `limited_runtimes_from`, `limited_runtimes`, and `usage_windows_from` merged into `usage_windows`, keyed by `agent_accounts::usage_key(adapter, account_from_route(resolvedRouteId))`.
- **`runtime_terminal.rs`**: Added `mac_script_with_env` (emitting `export NAME='value'` only for valid shell identifiers) and `open_with_env`.

### UI (`apps/native/src/`)
- **`ui/agent_accounts_card.rs`**: Wired `account_in_use` callback (`engine_api::api().account_in_use(agent, id)`) and shared registry; `reload` falls back cleanly to reading existing `agent-accounts/accounts.json` when no live manager is registered yet; added `#[cfg(test)] test_guard()` (`TEST_LOCK` mutex + cleanup on drop) for headless test isolation.
- **`ui/people_panel.rs`**: Added `Panel.account`, populated in `edit()`, saved or cleared (`Value::Null` when machine sign-in or non-Claude/Codex runtime) in `build_entry_from_form()`, rendered "Account" choices (`team-account-default` and `team-account-<id>`, hidden when org policy disables `accounts` unless already set on the teammate), and reloaded accounts in `reload()`.
- **`ui/app_state.rs`**, **`main.rs`**, **`ui/thread_view.rs`**: Added `TurnPick.account: Option<Option<String>>` and `State.limited_runtimes`; updated composer chip label, `model_picker` (`Codex CLI · Work` / `GPT · Work` groups with `SIGN IN` / `AT LIMIT` badges and explicit machine-default `Some(None)` vs named account `Some(Some(id))` overrides, hidden when org policy sets `accounts: false`), `open_models`, and `send`.
- **`ui/compare_view.rs`**: Added `account: Option<String>` to `ComparePick` and `ModelChoice`; populated added accounts in `available_choices`, slot menus (`compare-slot-model-menu`), column creation, judge/draft origins, reruns, and tune flows.
- **`ui/terminal_panel.rs`**: Passed `account_launch_env` (resolved from `resolvedRouteId`) to `runtime_terminal::open_with_env`.

## Special-Case Checks & Mutation Results

1. **Agent validation (`core/src/agent_accounts.rs` `launch_env`)**
   - Temporarily disabled `if found.agent.as_str() != runtime` check in `launch_env`.
   - Caught by `native::tests::named_codex_and_claude_accounts_launch_with_own_home_and_fail_cleanly` (panicked at `core/src/native/tests.rs:2033:26` because assigning a Claude account to a Codex turn succeeded instead of returning `OTHER_AGENT`). Reverted cleanly.
2. **Platform macOS terminal script (`core/src/runtime_terminal.rs` `mac_script_with_env`)**
   - Temporarily disabled `export KEY='val'` generation in `mac_script_with_env`.
   - Caught by `runtime_terminal::import_tests::opens_an_existing_conversation_in_a_terminal_with_the_recorded_account_home` (panicked at `core/src/runtime_terminal.rs:645:9` because `CODEX_HOME`, `CODEX_SQLITE_HOME`, and blanked API keys were absent from the generated `/bin/zsh -l` script). Reverted cleanly.
3. **Protocol / `resolvedRouteId` & cold switch (`core/src/native.rs` `send_from`)**
   - Temporarily dropped `|| prior.account != effective_account` from the `route_switch` condition.
   - Caught by `native::tests::named_codex_and_claude_accounts_launch_with_own_home_and_fail_cleanly` (panicked at `core/src/native/tests.rs:1993:10` because switching from Work account to machine default tried to resume instead of starting a cold `route-switch` ledger). Reverted cleanly.
4. **Account kind (`machine default` vs `named`, `core/src/native.rs` `send_from`)**
   - Temporarily changed `Origin.account = Some(None)` handling so an explicit machine-default override fell back to `job.teammate.account`.
   - Caught by `native::tests::named_codex_and_claude_accounts_launch_with_own_home_and_fail_cleanly` (panicked at `core/src/native/tests.rs:1993:10` because overriding to machine default stayed on the teammate's saved `codex_work` account instead of switching to default). Reverted cleanly.

## Electron Account Tests Inventory

- **Dedicated account tests in `apps/desktop/src/`**:
  - `main/an-account-sign-in-belongs-to-its-agent.test.ts` -> **carried** (`core/src/agent_accounts_tests.rs`).
  - `main/an-added-account-has-only-its-own-home.test.ts` -> **partial (12 of 13)** across `agent_accounts_tests.rs`, `route_at_start.rs`, `native::tests::named_codex_and_claude_accounts_launch_with_own_home_and_fail_cleanly`, `usage_ledger::tests::keeps_usage_and_exhausted_limits_separate_for_each_account`, `runtime_terminal::import_tests::opens_an_existing_conversation_in_a_terminal_with_the_recorded_account_home`, `compare_window_tests::compares_the_same_agent_and_model_on_two_distinct_accounts_and_keeps_both_across_reload`, and `window_drive_tests.rs`.
    - **Omitted item (1 of 13)**: case #12 (*"starts, watches and stops background work on the same account"*) depends on `claude-background` (`main/claude-background.ts`), which is not yet ported natively (tracked in `parity/rows.tsv` row 128).
  - `renderer/src/an-added-account-is-named-in-the-picker.test.tsx` -> **carried** (`src/ui/tests.rs` `an_added_account_is_named_in_the_picker_and_respects_policy_and_limits`).
- **Related cross-feature tests mentioning accounts**:
  - `main/a-mac-opens-its-own-terminal.test.ts`: account env/home terminal launch variant now covered by `runtime_terminal::import_tests::opens_an_existing_conversation_in_a_terminal_with_the_recorded_account_home`.
  - `main/route-at-start.test.ts`: account-inheritance and explicit default override logic carried in `core/src/route_at_start.rs` (`keeps_the_saved_account_for_cli_or_editor_starts_and_an_explicit_default_for_a_picker_override`); full 17-case non-account suite remains `waiting` in `parity/tests.tsv`.

## Judgement Calls

- **`Origin.account: Option<Option<String>>` & `TurnPick.account: Option<Option<String>>`**: Modeled as three states (`None` = use teammate's saved account; `Some(None)` = explicit "this computer's sign-in" override for this turn; `Some(Some(id))` = explicit named account override). This cleanly distinguishes inheriting a teammate's saved account from overriding back to machine default, including across `carry(...)` and `engine_process` JSON serialization (`Origin.account` / `teammate.account`).
- **`resolvedRouteId` format**: Recorded `"{runtime}:{model}-account:{id|default}"` on `mission.created` and every `mission.event` (`ledger_write.rs` + `native.rs`), matching Electron's `-account:<id>` convention so `usage_ledger`, `prior_of`, and `terminal_panel` read account provenance directly without duplicating metadata logic.
- **Test isolation for shared `agent_accounts_card` global**: Added `agent_accounts_card::test_guard()` (`Mutex` + `Drop` cleanup) and explicit array format (`[{"id","runtime","name","state"}]`) in UI/window tests so parallel test execution does not leak global account registries or policy state across tests.
- **PARITY row 63 marked `done` (`live=no`)**: All UI, engine, Compare, usage, terminal, and policy paths for named accounts and explicit selection are implemented and verified with fake CLIs/headless windows; the sole omitted test case (`claude-background`) belongs to row 128 (`Claude Code remote control and background access`), and no live run against real `claude`/`codex` binaries was executed in this task.

## Review (13 Nov)

The scripted review passed: 1,295 tests, no warnings, and the three window tests alone and together three times.
Checked by hand:
- no account means the launch is exactly as it was;
- an account run's environment is applied after the runtime builds its command, so nothing sets the person's keys
  again;
- an account of another agent, or a removed one, refuses with words and never falls back to this computer's sign-in;
- the picker and the people panel read no files while drawing.

Nothing to fix. One note for later: `resolvedRouteId` is written as `<runtime>:<model>-account:<id|default>`. That is
only read by this app, and `account_from_route` ignores `default`, so it is harmless.
