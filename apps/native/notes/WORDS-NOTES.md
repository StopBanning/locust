# Words 1: Settings says each switch in one line; Team's "Last run" tells the truth

Lane `cloud/words-1`, from `exec/native-engine` at 3ee084ac. Two rows: review change 6 (approved by the owner) and one
data fault on the Team screen.

## Row 1: each switch in one line, the rest under "More" (review change 6)

The review: "Settings explains every switch in a paragraph … three to five lines per switch, each starting 'On.' or
'Off.', which the switch already shows. Change: one line saying what the switch does. The rules and exceptions go behind
'More'."

What changed, once for every switch below:

- The words under a switch's name are one line saying what the switch does, the same whichever way it is set.
- Under that line is **More** (`settings_panel::switch_more`): a button (`more-<key>`, role button, label "More", says
  whether it is open) that stays shut until pressed. Pressed, it shows the old paragraph (`more-<key>-text`) for the
  state the switch is in, with only its first word gone where that word was "On." or "Off." Nothing else was cut: every
  fact the paragraph had is under More, word for word. Which folds are open is kept in `settings_panel.folds` under
  `more-<key>`, so each opens on its own and stays as left while Settings is open.
- A state that is news rather than a setting is still said out loud, not folded: Your phone's "On, but this computer is
  not on a network Locust can find …", Remote Control while starting, stopping, ended or failed, and Without asking's
  "Pick an agent above first …" (why the switch cannot be pressed yet).
- The fold says "More", as the review names it. Electron's `More` component (Screens.tsx:1396) says "How it works"
  on its summary; Rust keeps that label for the section-level folds (`how_it_works::fold`), so a switch's own fold and its
  section's explanation do not read as the same thing twice (Auto mode has both).

| Electron file:line | Rust (where) | Old Rust text | New line (always shown) | Under More |
| --- | --- | --- | --- | --- |
| LocustMcpSettings.tsx:56, :57 | settings_panel.rs `door_section_at`, `DOOR_LINE`, `door_more` | "On. Other apps on this computer can ask teammates, run your routines and read replies. {New turns use each teammate's own mode \| New turns are Ask mode (read only)}, with no automatic teammate handoffs. Locust must be running; your AI account usage and monthly limits still apply." / "Off. No local server is listening." | Lets other apps on this computer ask your teammates, run your routines and read replies. | the old text, "On."/"Off." dropped |
| LocustMcpSettings.tsx:66, :67 | settings_panel.rs, `OWN_MODE_LINE`, `own_mode_more` | "On. A turn another app starts runs in the mode the teammate is set to here, or the mode and folder the app asks for, so it can edit files. A folder must be one Locust knows or a worktree of one. {Approval cards …} Auto runs only if Auto is on in Settings." / "Off. Every turn another app starts is Ask mode (read only), whatever mode the teammate is set to." | Lets a turn another app starts use the teammate's own mode, so it can edit files. | the old text, "On."/"Off." dropped |
| LocustMcpSettings.tsx:80, :81 | settings_panel.rs, `EDITOR_ANSWERS_LINE`, `_ON`, `_OFF` | "On. An editor that starts a turn can approve or deny that turn’s cards there. Questions are still answered here, and no app can answer a card for a turn it did not start." / "Off. Cards for a turn an editor starts wait in this window, and only you answer them." | Lets an editor approve or deny the cards for a turn it started. | the old text, "On."/"Off." dropped |
| LocustMcpSettings.tsx:90, :91 | settings_panel.rs, `CONVERSATIONS_LINE`, `_ON`, `_OFF` | "On. An app can list, read, continue and stop your conversations, and carry one whose plan ran out on to another AI agent, by the same rules as its own: no higher mode, and cards answered only here." / "Off. An app sees only the conversations it started. A turn it started that runs out can still be carried on to another AI agent." | Lets an app see and continue your conversations, not only the ones it started. | the old text, "On."/"Off." dropped |
| RemoteMcpSettings.tsx:60, :61 | remote_mcp_card.rs, `LINE`, `LEDE_ON`, `LEDE_OFF` | "On. Claude on the web, Claude Duet and other apps that take a remote MCP server can use your team through Locust’s relay, by the same rules as an app on this computer. Each one connects only after you allow it on a card here." / "Off. Turn on to use your team from Claude on the web, Claude Duet, or any app that takes a remote MCP server." | Lets Claude on the web, Claude Duet and other apps that take a remote MCP server use your team. | the old text, "On."/"Off." dropped |
| PhoneSettings.tsx:109, :111, :114 | phone_section.rs, `LINE`, `OFF`, `ON_ANYWHERE`, `ON` | "Off. Turn it on to check on your teammates from your phone, answer approval cards and send the next message." / "On, from anywhere. Your phone reaches Locust through Locust’s relay. Every message is locked …" / "On. Scan the code with your phone’s camera … Your phone must be on the same Wi-Fi as this computer." | Check on your teammates from your phone, answer approval cards and send the next message. | the old text, "Off."/"On." dropped ("On, from anywhere." becomes "From anywhere.", the radio beside it says which). `NO_NETWORK` (:113) stays said. |
| ScoutSetting.tsx:232, :233 | scout_setting.rs `SCOUT_LINE`; words still core's `scout::view::setting_on_lede` / `SCOUT_OFF_LEDE` | "On. Scout looks every N minutes while Locust runs, and tells you on Home what needs you, with the next step drafted. It never sends, merges or changes anything by itself.[ It reads with M.]" / "Off. Turn it on and Scout joins your team: it watches your pull requests, their checks and reviews, and tells you on Home what needs you." | Scout watches your pull requests, their checks and reviews, and tells you on Home what needs you. | core's words with "On."/"Off." dropped at the screen (`without_state`) |
| RemoteControlSetting.tsx:45, :49 | remote_control_card.rs `LINE`; words still core's `remote_control::says` + `NEEDS_SIGN_IN` | "On. Start a session in this folder from claude.ai or the Claude app; what Claude Code prints is below. It stops when you switch it off or quit Locust. It needs your claude.ai sign-in; …" / "Off. When on, you can start Claude Code sessions in this folder from claude.ai while Locust is open, until you switch it off or quit Locust. It needs your claude.ai sign-in; API keys do not work. Team and Enterprise owners must allow Remote Control." | Lets you start Claude Code sessions in this folder from claude.ai or the Claude app while Locust is open. | core's words with "On."/"Off." dropped, then the sign-in sentence. Other phases unchanged (said, role status). |
| YourAccountSettings.tsx:99 | your_account_section.rs `SYNC_OFF` | "Off. Your copy stays as it was; turning sync on again needs no new code." | (already there) "Signed in with GitHub as …. Keep a copy of this Locust on Locust’s relay." | Your copy stays as it was; turning sync on again needs no new code. |
| Screens.tsx:2907, :2908 | settings_panel.rs `AUTO_LINE`, `AUTO_ON`, `AUTO_OFF` | "On. A run started in Auto goes ahead without asking and is not kept to the project folder." / "Off. Teammates may change files inside the project folder and nowhere else." | Lets a run started in Auto go ahead without asking, outside the project folder too. | the old text, "On."/"Off." dropped; the section's "How it works" stays below |
| Screens.tsx:2779, :2780 | settings_panel.rs `WITHOUT_ASKING_LINE`, `_ON`, `_OFF`, `_PICK_FIRST` | "On. Only for messages you send yourself, never a comparison, a routine or a reply between teammates." / "Off. The card waits for your press." | Carries the work on by itself instead of waiting for your press. | the old text, "On."/"Off." dropped |

Not changed, and why:

- **Not in the Rust app yet**, so nothing to change: TalkShortcutSetting.tsx:42 ("Off. Turn it on to start Talk …"),
  Screens.tsx:2673 (connectors ask-first; Rust's Connectors fold says that switch is not here yet), :2867 (max effort),
  :3178 (screen faces), :3233 (fur), :3314 (composer shader), NewTeammateDialog.tsx:917, :945 (the new-teammate dialog,
  not Settings).
- **Boot screen** (Screens.tsx:3442, settings_panel.rs `boot_screen_line`): three choices, not a switch; "Off." names the
  choice picked, which nothing else on the line says. Left as it is (and `settings_read_like_claudes_tests` pins it).
- **Core files this lane does not own**: `core/src/desk.rs:680, :688` (the browser desk's Settings ledes for Swarm and
  Auto mode) still open with "Off."; `core/src/scout/view.rs` and `core/src/remote_control.rs` still hold the long words
  with their "On."/"Off." (the window drops the state word when it shows them under More). A later lane that owns core
  can move the state word out at the source and delete `without_state`.
- Between teammates' two switches ("Automatic replies: off. …") and Plans / Your skills already say one short line.

Existing tests that read the old paragraphs now open the switch's More first (the words are all still there):
`settings_door_tests.rs` (Electron's `<More>` kept its words in the page whether open or not), `settings_switches_tests`,
`remote_control_card_tests` (`off_it_reads_as_one_switch…` now checks the one line and the folded state words),
`window_drive_tests` (Remote Control), `remote_mcp_card` and `phone_section` tests (the "Off."/"On, from anywhere."
prefixes they quoted are gone).

Tests: `src/ui/settings_switch_more_tests.rs` (11), drawn through the whole Settings window.

## Breaks

Each break in `notes/words-breaks/*.entries` (`path|||old ==> new`, from apps/native) was applied alone, the tests run,
the named test seen failing, and the file restored. Results are in the lane's mailbox report.
