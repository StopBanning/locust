# Agent discovery and the AI agents screen (12 Nov 2026)

Plan Part 3, "Port next" #3, on `exec/native-engine`. Built in four parts.

## What was built

1. **The screen's words** (`core/src/agents.rs`): `row_status` (routeRowStatus), `install_command`,
   `install_sentence`, `sign_in_command` and `integration_of`.
   - `scripts/agents-goldens.mjs` runs Electron's own functions over 396 made-up discovery results: 9 agents, 11
     states, active or not, at a usage limit or not.
   - The Rust matches every row and every install line. Changing one word fails 128 rows.
2. **The discovery decision** (`agents::discover_one`): runtime-adapters' discovery plus main's publicStatus.
   - Each agent's version, help and sign-in checks, and the features its help must name.
   - Readiness from the version for planned agents, and an OpenCode list that answers readiness.
   - A refused OpenCode 3, Muse's credentials file (its presence only, never its contents), and the agent's own words
     for "signed out".
   - `scripts/discovery-goldens.mjs` runs Electron's discovery with a made-up runner over 72 scenarios, using the
     real help texts this Mac's agents print (`parity/agents/*-help.txt`, with one home path replaced). Every
     decision matches.
   - Taking Codex out of the sign-in-check agents fails exactly the scenario that depends on it.
3. **The real run** (`agents::discover`): every agent checked at once, each check bounded at 10 s
   (PROBE_TIMEOUT_MS) and ended when it runs over.
   - Antigravity is found as Electron's antigravity-host finds it. A model list means ready, an answer without one
     means signed out, and silence means not confirmed.
   - `locust-native --agents` prints the result.
   - On this Mac: Codex, Claude Code, Cursor, Copilot and Antigravity are ready; OpenCode is ready with no account;
     Kilo is in preview; Gemini is planned; Muse is not installed.
4. **The screen** (`src/ui/agents_section.rs`): Settings › AI agents › "AI agents & accounts".
   - Discovery runs in the background when Settings opens.
   - Rows are ordered usable first (RUNTIME_BAND, then the name).
   - Each row shows its tag and version, and Electron's sentence when it is not plainly ready.
   - A NOT INSTALLED or SIGN IN row shows the line to run, with Copy (Electron's InstallCommand).
   - There is a Check again button.

## Where it differs from Electron, and why

- **No Install, Sign in or Update buttons yet.** Electron's buttons run npm or open a terminal on the computer.
  - Until this window runs them, it shows the line, with Copy, as Electron does when its Install cannot run.
  - So the NOT INSTALLED sentence's "Press Install" reads "Run the line below", and the nothing-installed lede says
    "run its line below". Everything else is Electron's word for word.
- **The lede leaves out** "Claude Code and Codex can also use accounts you add here": named accounts are not built.

## How it was checked

- `agents_tests.rs`:
  - every screen row and install line against Electron's answers;
  - every discovery decision against Electron's;
  - a real `sleep 5` cut off at 0.2 s and read as timed out;
  - Antigravity's three outcomes.
- **Window test** `the_ai_agents_page_lists_each_agent_usable_first_with_what_to_do`:
  - the order and the tags;
  - nothing extra said on a ready row;
  - the sign-in and install lines, with Copy;
  - no "Press Install";
  - the checking line while nothing has been found yet.
- **Frames:** `--snap` of the `settings-agents` and `settings-agents-light` scenes. The first frame showed the tag
  pills stretched on rows with a sentence; fixed with `self_start`.
- The full suite passed, and so did `cargo build --release`.

## Left

- Install, Sign in and Update buttons (they run on the computer).
- Capability reporting for ACP agents.
- The runtime setup line (connectors and hooks read from each agent's files).
- Claude Code's remote-control switch.

## Install (13 Nov)

- `core/src/agent_install.rs` ports `main/runtime-installer.ts`: the row's own `npm install -g` line, split from the
  string the row shows; one install at a time; six failures in words; a permission failure off Windows tried once into
  `~/.npm-global` (already on the PATH Locust adopts at launch); a clean exit followed by looking for the command again.
- npm is spawned with no shell (Electron needs one for `npm.cmd` on Windows); arguments are still checked for shell
  syntax, so the guard holds for a future package name read from elsewhere.
- No bundled npm: this app carries no Node. A machine without npm gets "npm could not be started" and the line to run.
- The display name goes into the words ("OpenCode installed, but...") where Electron passes the id.
- Update (runtime-updates.ts) is next: it reuses `Installer::install` with a version.
