# Locust

<p align="center"><a href="https://locust.lol"><img src="docs/assets/locust-banner.png" alt="Locust: AI teammates, on the models you pick" width="100%"></a></p>

<p align="center">
<a href="https://github.com/StopBanning/locust/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/StopBanning/locust?label=release"></a>
<a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-blue"></a>
<img alt="Windows, Mac and Linux" src="https://img.shields.io/badge/runs%20on-Windows%20%7C%20Mac%20%7C%20Linux-informational">
</p>

**Your AI coding agents, as one team on your own computer.** Locust runs Claude
Code, Codex, Cursor, OpenCode, GitHub Copilot, Muse Code, Antigravity and Kilo
side by side, each teammate on the agent and model you pick, under your own
accounts. You watch every step, approve what matters, and keep a record of
everything they did. Free and open source (MIT), with no Locust account and no
telemetry.

**From your phone, or any browser.** Pair your phone once, or sign in with
GitHub at [app.locust.lol/desk](https://app.locust.lol/desk/), and your team is
there wherever you are, while Locust runs on your computer. What goes between
them is sealed end to end: the relay in the middle cannot read it.

**[Download for Windows, Mac or Linux](https://locust.lol)** ·
[Releases](https://github.com/StopBanning/locust/releases) ·
[What's new](CHANGELOG.md)

If Locust is useful to you, a star on this repository helps other people find it.

<table>
<tr>
<td><img src="docs/assets/shots/01-home.png" alt="Locust home: a team of six, each teammate on its own AI and model, and Connected accounts: 8 ready"><br><sub>Your team of six, each teammate on its own AI and model. Connected accounts: 8 ready.</sub></td>
<td><img src="docs/assets/shots/02-model-picker.png" alt="The model picker, grouped by agent, with Opus 5.5 on Claude Code marked active and Antigravity’s models listed above it"><br><sub>One picker for the models your own accounts offer, grouped by agent, with the one in use marked.</sub></td>
</tr>
<tr>
<td><img src="docs/assets/shots/03-run-in-progress.png" alt="A teammate working in Edit mode with a four-step plan card, step 1 under way"><br><sub>A run in progress: a four-step plan, step 1 under way, the live line naming the step and its time.</sub></td>
<td><img src="docs/assets/shots/04-approval.png" alt="An Approve each card showing the exact command, where it runs, and Approve once, Always allow this session or Deny"><br><sub>Approve each: before a command runs you see the exact command, where it runs, and what Locust can and cannot tell about it.</sub></td>
</tr>
<tr>
<td><img src="docs/assets/shots/05-hand-off.png" alt="A hand-off chain: Atlas on Codex reads the code and writes VERDICT: APPROVED on Wren’s diagnosis"><br><sub>A hand-off chain: Wren (OpenCode) diagnosed the bug; Atlas (Codex) read the code himself and wrote VERDICT: APPROVED.</sub></td>
<td><img src="docs/assets/shots/06-finished.png" alt="The finished run: 4 of 4 steps, two files edited, four tests passing"><br><sub>The finished fix: 2 files edited, a test file added, four tests passing, in 33 s on a free model.</sub></td>
</tr>
<tr>
<td><img src="docs/assets/shots/07-blind-compare.png" alt="A blind compare: Model A and Model B answer the same ask side by side with names hidden, and a judge model can be asked"><br><sub>Blind compare: two models on the same ask, names hidden until you keep one. A judge model can be asked which it would keep.</sub></td>
<td><img src="docs/assets/shots/09-board.png" alt="The Board: conversations in columns by what they need from you, one under Needs you and one under Done"><br><sub>The Board: every conversation by what it needs from you.</sub></td>
</tr>
</table>

<sub>Screenshots from the packaged app on a fresh demo profile (0.838.0; the hand-off chain and the Blind compare from 0.619.0): real runs, no mocks, taken by `_tools/drive-promo-shots.mjs`. The models shown are what that machine’s accounts offer; yours come from your own accounts.</sub>

**Locust** puts Claude Code, Codex, Cursor, OpenCode, GitHub Copilot,
Antigravity, Muse Code and, in Preview, Kilo side by side under one roof. You give a teammate a
mission; it runs on your machine, under your own provider accounts, in a
read-only sandbox unless you say otherwise. Every event is written to an
append-only local ledger before it reaches the screen, so what you are shown
is what was recorded, and a mission survives a restart.

## Install

**Download the latest build** at [locust.lol](https://locust.lol), for Windows,
Mac and Linux, or from [Releases](https://github.com/StopBanning/locust/releases),
which has every build's notes and the same files with their SHA-256 sums. The
Windows and Mac builds are not signed by a publisher yet
([why, and the signing policy](docs/CODE-SIGNING.md)), so each asks once:

- **Windows** warns that the installer comes from an unknown publisher: choose
  **More info**, then **Run anyway**.
- **Mac** refuses the first open, since Apple has not checked the app. Close
  that message, open **System Settings › Privacy & Security**, and press
  **Open Anyway** beside Locust.
- **Linux** comes three ways. The **.deb** (Ubuntu, Debian, Mint) and the
  **.rpm** (Fedora, openSUSE) install like any package: open the file, or run
  `sudo apt install ./Locust-*.deb` or `sudo dnf install ./Locust-*.rpm`. The
  **AppImage** runs on any other Linux and updates itself: allow it to run as
  a program (`chmod +x Locust-*.AppImage`), and on Ubuntu 22.04 or later
  install FUSE first (`sudo apt install libfuse2t64`, or `libfuse2` before
  24.04).

Locust is developed in this repository, and each release is cut from `main`.
Issues are welcome here; code changes are by invitation (see CONTRIBUTING.md).

## Why this exists

Most agent products hide the runtime, the model, the provider, the permissions
and the failure state behind one opaque "agent". Locust separates them and
makes every one of those decisions inspectable -- and, where it matters,
yours to make.

```text
You ──► a teammate (name, role, face; its own runtime, model and mode)
          │
          ├─ Claude Code ──── the CLI you installed and signed into
          ├─ Codex CLI ─────── "
          ├─ Cursor Agent ──── "
          ├─ OpenCode ──────── "
          ├─ Copilot CLI ───── "
          ├─ Antigravity ───── the IDE's own agent, through its CLI
          ├─ Muse Code ─────── "
          └─ Kilo ──────────── its free Auto Free model needs no account (Preview)
          │
          ▼
        one append-only ledger per conversation, written before the screen is
```

There is no model of Locust's own and no account of Locust's own. Each runtime
runs its own tools; Locust asks you before an action where the runtime offers a
place to ask, says so where it cannot, and records what happened either way.

## Repository map

- `apps/desktop` — the Electron + React app: main process (runtimes, ledger, approvals, routines, updates), preload bridge, renderer.
- `packages/runtime-adapters` — finding installed CLIs without reading their credentials, the process transport, and one event normalizer per runtime, each built from streams measured off the real CLI.
- `packages/mission-store` — the versioned, append-only mission ledger and its reader.
- `attic/contracts`, `attic/runtime-core` — an earlier design (routing contracts, fallback policy, a hand-off state machine). Nothing in the app imports them; they are out of the workspace and kept for history.
- `_tools` — the release tools and the drives: scripts that launch the built app over CDP, do what a person does, and keep screenshots as the record.
- `_smoke` — live smokes against the real CLIs, run by hand.
- `docs` — `ARCHITECTURE.md` (the September design, with a note on what was built instead), `CODE-SIGNING.md`, the roadmap documents (history), and the assets.

## What works now

As of 0.620 (2026-10-05). Everything here runs in the built app and is checked
by drives against the packaged build, not by unit tests alone.

**Seven runtimes, side by side.** Claude Code, Codex CLI, Cursor Agent,
OpenCode, Copilot CLI, Antigravity and Muse Code each own real conversations
end to end. Each is found on your machine without reading its credential
files, reports whether it is signed in, and offers the models its own CLI
names -- nothing is offered that the installed CLI did not list. Gemini CLI is
found and signed into like the others but refused: Google serves the CLI to
API keys and enterprise licences only, and its stream has never been measured,
so Locust says so rather than start a process whose output nobody can read.
Kilo joined in 0.835, in Preview: a new conversation starts on its free Auto
Free model, which needs no account.

**Teammates.** A teammate is a name, a role, a look and routing defaults: its
runtime, model, reasoning effort and mode. Up to 64 teammates; up to 8
conversations running at once, each with its own process and its own ledger
writer. A face moves only while its teammate is working.

**Five modes, and cards before consequences.** Ask (read-only), Plan, Accept
edits, Approve each and Auto. In Approve each an approval card says the exact
command and the folder it runs in, and a command that reaches beyond its own
run -- `taskkill /IM python.exe`, `kill -9 -1`, a shutdown -- is named for
what it does and is asked about every time. Approve once and Always are
session-scoped; a rule you choose to save ("Yes, and don't ask again") is
listed in Settings with how many cards it answered, and can be removed one at
a time or all at once. A saved rule that says no wins over an Always, and the
record names who decided each card: you, your rule, or your earlier Always.
Where a runtime offers no place to ask (Cursor and Muse never do; Antigravity
reports its refusals after the fact), the card is not pretended; the record
says what happened. Which AI agents ask first, in which modes, and so what
Locust can stop, is in [docs/WHAT-LOCUST-CAN-STOP.md](docs/WHAT-LOCUST-CAN-STOP.md),
written from the same list Settings > AI agents shows. Every connection Locust
itself makes -- its updates, the agents it installs, the pet gallery, a
previewed page's libraries -- is in [docs/NETWORK.md](docs/NETWORK.md); there
is no telemetry.

**The record.** Every conversation is an append-only ledger: each event,
each card and its answer, each hand-off, written before the screen shows it.
A conversation can be saved as a Markdown record that says what it holds and,
in so many words, what it does not. Completed, failed, cancelled and
interrupted runs come back after a restart with their integrity reported
truthfully.

**Teams.** Teammates hand work to each other through a product-owned channel:
a run ends with a share per named recipient, the host checks the name against
the roster, and the recipient's next turn sees it quoted as a claim, dated and
attributed. Hand-off chains run a routine's steps across teammates on
different runtimes, with a checker that must approve before the run counts.
Rooms hold a conversation several teammates take part in.

**Routines.** A routine runs a teammate on a schedule -- every N hours,
daily, weekly, once, when files in a folder change, or when its private link
is called -- and can keep running
in the background after the window is closed.

**The Board.** Every conversation in columns by what it needs from you: Needs
you, Working, Ready to look at, Done -- across every runtime at once. It opens
from the bottom bar, beside Settings.

**Scout.** A teammate that keeps watch, off until you turn it on: every few
minutes it looks at your pull requests and their checks, the reviews asked of
you, your main branch's checks, a teammate waiting on you and a turn that
failed, and tells you on Home what needs you -- in Locust's own words, with its
take and the next step drafted for the right teammate. It never acts by
itself. It reads with a free model unless you give it another.

**Talk.** A voice call with ChatGPT's own voice, beside the chat box's
microphone, placed by the Codex CLI with your ChatGPT sign-in (Windows, for
now). The voice reads your folder with read-only tools, uses your connectors
for quick things -- one that would send, change or buy something waits for
your Allow in the call -- and hands bigger work to a teammate after saying back
who will do what. Pressed in a teammate's conversation, Talk is a call with that
teammate: the voice speaks as them, in the voice you gave them, and what you ask
goes into their conversation, where they do it on their own agent. The voices
are the nine ChatGPT offers. Ctrl+Alt+Space starts Talk from any app, and each
call sums up what it handed out: who has each to-do, and how it's going.

**Other apps.** Your other AI apps can ask your teammates through a local MCP
server that is off until you turn it on, listens on 127.0.0.1 only, and starts
turns in Ask mode unless you say otherwise. The Claude app is added in one
press (a copy of its settings is kept first); Claude Code, Codex and OpenClaw
get the command or config entry to paste. Through OpenClaw, your chat apps
(WhatsApp, Telegram, iMessage, Slack) can ask the team too. From anywhere,
Claude on the web and Claude Duet add Locust as a custom connector at one
address on Locust's relay: each app is let in only when you allow it on a card
on your computer, which makes and checks its keys, and it can pick a model and
effort for a turn but never a higher mode. An app can wait for a reply, stop a
turn it started and, once you let it see your conversations, carry one whose
plan ran out on to another agent. It reaches Locust only while Locust is
running, so Keep it running leaves Locust in the tray when you close its window
and starts it when you sign in.

**Away from your computer.** Pair your phone once and it reaches Locust from
any network through Locust's relay, every message locked on your own devices:
it shows your conversations, answers approval cards and can buzz when something
needs you. `app.locust.lol/desk` opens the whole window in any browser, driven
by your computer, and a Telegram bot of your own carries messages to your
teammates. Each is off until you turn it on.

**Links.** A link like `locust://new?prompt=Explain%20this%20project` opens
Locust on a new conversation with that message in the box, so a README or a
page can hand someone a prompt. It is never sent: the box says it came from a
link, and anything in it a person could not see -- Unicode tag characters,
direction overrides, zero-width marks -- is taken out first. Only the message
travels; no folder, model or file. The installed app claims these links; a copy
a test runs never does.

**Compare.** The same mission on two or three models at once, each in its own
copy of the folder, side by side; keep the one you like. Blind hides the names
until you choose. ([Three models building the same game,
blind](https://locust.lol/arena/).)

**Updates.** Locust updates itself from its own downloads at dl.locust.lol, on
Windows with a differential download, and every release carries Windows, Mac
(Apple silicon and Intel) and Linux builds. The Linux AppImage replaces itself;
a .deb or .rpm, which belongs to your package manager, says when a new version
is out and hands you the new package. Settings says which build you are on.
The Windows and Mac builds are not signed yet; see the policy below.

What is still open, in the order the authors would take it, is in
`CHANGELOG.md`'s most recent entries and the issues here. The roadmap documents
under `docs/` are the September plan and are kept as history, not as a promise.

## Run locally

Requirements: Node.js 22.22+ and pnpm 11.

```powershell
cd Locust
pnpm install
pnpm dev
```

`pnpm dev` starts the Electron application with the React renderer in development mode. The install script downloads the Electron runtime when needed.

Run all checks:

```powershell
pnpm check
```

This builds every workspace package, runs TypeScript checks, and runs the complete test suite (about 13,800 tests across the desktop app, the adapters, the ledger and the relay, as of 0.835).

Green tests are not the evidence here. Each package carries a mutation
control (`test/mutation-control.mjs`) that breaks one behaviour at a time and
requires the NAMED test to fail, rejecting any mutation that stops the file
running -- a red suite caused by a broken file proves nothing about any test
in it. Run them with `node test/mutation-control.mjs`
from a package directory. The drives in `_tools/drive-*.mjs` are the other
half: each launches the packaged app, does what a person does, and keeps its
screenshots; a drive that cannot fail on the build before the fix is not
trusted.

## Working on it from another agent session

Read `AGENTS.md` and `PROJECT.md`. `docs/CROSS_TASK_CONTEXT.md` carries the
conventions an agent session is expected to keep; `CHANGELOG.md` is the record
of what has shipped, release by release, written for the person using the app.

## Code signing policy

Locust's Windows installer is not signed yet, so Windows warns when you
install it. The policy Locust follows to get it signed, and will follow once
it is, including who approves each release and what is never collected, is in
[docs/CODE-SIGNING.md](docs/CODE-SIGNING.md).

## Current scope

Locust runs on your own computer, on the CLIs you installed and the accounts
you signed into. There is no account with Locust and no telemetry. The one
service Locust runs is its relay at `app.locust.lol`, used only once you turn
on your phone, a browser or a routine link ([docs/NETWORK.md](docs/NETWORK.md)).
Cloud execution, if it ever comes, comes after the local approval
and recovery model has proved itself in use.
