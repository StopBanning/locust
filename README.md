# Locust

<p align="center"><a href="https://locust.lol"><img src="docs/assets/locust-banner.png" alt="Locust: AI teammates, on the models you pick" width="100%"></a></p>

<p align="center">
<a href="https://github.com/StopBanning/locust/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/StopBanning/locust?label=release"></a>
<a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-blue"></a>
<img alt="Windows, Mac and Linux" src="https://img.shields.io/badge/runs%20on-Windows%20%7C%20Mac%20%7C%20Linux-informational">
</p>

**Your AI coding agents, as one team on your own computer.**

Locust runs Claude Code, Codex, Cursor, OpenCode, GitHub Copilot, Antigravity,
Muse Code and Kilo side by side. Each teammate has a name and a role, and runs
on the agent and model you pick, under your own accounts. You watch every step,
approve what matters, undo a turn you don't like, and keep a record of
everything they did. Reach the team from your phone or any browser while
Locust runs at home.

Free and open source (MIT). No account needed, no model of Locust's own, no
telemetry. Sign in with GitHub if you want a sealed copy of your Locust kept for
your next computer.

**[Download for Windows, Mac or Linux](https://locust.lol)** ·
[Releases](https://github.com/StopBanning/locust/releases) ·
[What's new](CHANGELOG.md) ·
[Ready-made teams](https://github.com/StopBanning/locust-teams)

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
<td><img src="docs/assets/shots/05-hand-off.png" alt="A hand-off chain: Atlas on Codex reads the code and writes VERDICT: APPROVED on Wren’s diagnosis"><br><sub>A hand-off: Wren (OpenCode) diagnosed the bug; Atlas (Codex) read the code himself and wrote VERDICT: APPROVED.</sub></td>
<td><img src="docs/assets/shots/06-finished.png" alt="The finished run: 4 of 4 steps, two files edited, four tests passing"><br><sub>The finished fix: 2 files edited, a test file added, four tests passing, in 33 s on a free model.</sub></td>
</tr>
<tr>
<td><img src="docs/assets/shots/07-blind-compare.png" alt="A blind compare: Model A and Model B answer the same ask side by side with names hidden, and a judge model can be asked"><br><sub>Blind compare: two models on the same ask, names hidden until you keep one.</sub></td>
<td><img src="docs/assets/shots/09-board.png" alt="The Board: conversations in columns by what they need from you, one under Needs you and one under Done"><br><sub>The Board: every conversation by what it needs from you.</sub></td>
</tr>
</table>

<sub>Screenshots from the packaged app on a fresh demo profile (0.838.0; the hand-off and the Blind compare from 0.619.0): real runs, no mocks, taken by `_tools/drive-promo-shots.mjs`. The models shown are what that machine's accounts offer; yours come from your own accounts.</sub>

## The trending skills, on all your agents

Skills are folders of instructions an agent follows. Most skill packs are
written for one agent. Keep a pack once in Locust and every teammate gets it,
whichever agent it runs on.

Open **Settings › Teammates › Skills from GitHub**. The shelf has the
most-starred packs (Matt Pocock's skills, Superpowers, Anthropic's skills,
Impeccable, Claude-Mem) and a **New this week** row of the packs climbing
fastest right now (tstack, Unslop, Beautiful Rust, Build Any Mac App). Or paste
any public repository. Locust lists every skill and names every file in it
that runs before anything is kept, and keeps the exact commit you looked at.

## What your team does

- **Works side by side, on any agent.** Up to 64 teammates, 8 conversations
  running at once, each on its own agent and model. A teammate can hand work
  to another, and a checker on a different agent can be asked to approve it.
- **Asks before it acts.** Five modes, from Ask (read-only) to Auto. In
  Approve each, a card shows the exact command and folder before it runs.
  Where an agent offers no place to ask, Locust says so instead of
  pretending ([what Locust can stop](docs/WHAT-LOCUST-CAN-STOP.md)).
- **Keeps a record, and can take a turn back.** Every event is written to a
  local, append-only record before the screen shows it, so a conversation
  survives a restart. **Undo these changes** puts back the files a turn
  changed.
- **Keeps working on a schedule.** Routines run a teammate every few hours,
  daily, weekly, when files change or when a private link is called, and
  carry on in the background after the window is closed.
- **Keeps watch.** Scout, off until you turn it on, reads your pull requests,
  checks and reviews and tells you what needs you, with the next step drafted
  for the right teammate. It never acts by itself.
- **Follows you.** Pair your phone once, or sign in with GitHub at
  [app.locust.lol/desk](https://app.locust.lol/desk/), to answer approval
  cards and talk to your team from anywhere. Messages are sealed on your own
  devices; the relay in the middle cannot read them. Discord, Slack and
  Telegram work too.
- **Talks.** Talk is a voice call with ChatGPT's own voice, through your
  signed-in Codex. It reads your folder and hands bigger work to a teammate.
- **Compares models.** The same task on two or three models at once, each in
  its own copy of the folder. Blind hides the names until you keep one
  ([three models building the same game](https://locust.lol/arena/)).
- **Shares a team as a picture.** **Share team › Save image** puts your
  teammates and their routines inside a PNG; **Add team from image** brings
  them in, nothing started. [Ready-made teams](https://github.com/StopBanning/locust-teams).
- **Answers your other apps.** A local MCP server, off until you turn it on,
  lets the Claude app, Claude Code, Codex and OpenClaw ask your teammates.
  Claude on the web reaches it through the relay, once you allow it on your
  computer.

## How Locust compares

| | What it is | How Locust differs |
| --- | --- | --- |
| **Claude Code, Codex** and the other agents' own apps | One company's agent, on that company's models. | Locust runs them as teammates in one window, next to six other agents, so you don't choose one. Each keeps its own sign-in and its own tools. |
| **Orkas** | A desktop team led by its own Commander model, which hands parts of a task to built-in specialists and CLI agents. | Every agent you bring leads its own conversation; no model of Locust's own sits above them. No sign-in, and your phone, a browser and chat apps reach the team in the open-source app itself. |
| **Orca** | A worktree, a terminal and a browser for each task, across many coding CLIs. | Teammates with roles, approval cards before commands, a record of every step and Undo. Built for people who would rather not live in a terminal. |
| **Paperclip** | A self-hosted web control plane for "an AI company": org chart, budgets, heartbeats. | A desktop app with nothing to host. Your computer runs the agents; the relay only carries sealed messages when you turn it on. |
| **CrewAI, LangChain** | Code frameworks for building agents into your own program. | Nothing to write. You direct real agents by chatting with them. |

**Locust is for you if** you already use more than one AI agent, want to pick
the model for each job, and want to see and approve what they do.

**It isn't for you if** you want one chatbot, or a library to build agents into
your own program.

## How it works

```text
You ──► a teammate (name, role, face; its own agent, model and mode)
          │
          ├─ Claude Code ──── the CLI you installed and signed into
          ├─ Codex ─────────── "
          ├─ Cursor ────────── "
          ├─ OpenCode ──────── "
          ├─ GitHub Copilot ── "
          ├─ Antigravity ───── the IDE's own agent, through its CLI
          ├─ Muse Code ─────── "
          └─ Kilo ──────────── its free model needs no account (Preview)
          │
          ▼
        one append-only record per conversation, written before the screen is
```

- **Agents.** Locust finds each installed CLI without reading its credential
  files, offers only the models that CLI lists, and turns each one's own
  output into one shape of event. One reader per agent, each built from
  streams measured off the real CLI:
  [packages/runtime-adapters](packages/runtime-adapters/src).
- **The record.** Every event, card and answer is appended to the
  conversation's ledger before it is drawn:
  [packages/mission-store](packages/mission-store/src).
- **Approvals.** A card waits for you, or for a rule you saved, and the record
  names who decided:
  [approval-channel.ts](apps/desktop/src/main/approval-channel.ts),
  [approval-rule-store.ts](apps/desktop/src/main/approval-rule-store.ts).
- **Routines.** Schedules, file triggers and links:
  [routine-runner.ts](apps/desktop/src/main/routine-runner.ts).
- **Skills from GitHub.** Listed before kept, pinned to a commit:
  [skill-library.ts](apps/desktop/src/main/skill-library.ts).
- **Team pictures.** A team inside a PNG:
  [team-card.ts](apps/desktop/src/shared/team-card.ts).
- **From anywhere.** X25519 and AES-GCM between your devices; the relay is a
  Cloudflare Worker that only forwards:
  [relay-crypto.ts](apps/desktop/src/main/relay-crypto.ts),
  [apps/relay](apps/relay/src).

Every connection Locust itself makes is listed in [docs/NETWORK.md](docs/NETWORK.md).

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

Locust updates itself from its own downloads at dl.locust.lol. A .deb or .rpm,
which belongs to your package manager, says when a new version is out and
hands you the new package.

## Run locally

Requirements: Node.js 22.22+ and pnpm 11.

```bash
cd locust
pnpm install
pnpm dev
```

`pnpm dev` starts the Electron app with the React renderer in development
mode. `pnpm check` builds every package, runs the TypeScript checks and the
whole test suite (about 14,100 tests across the desktop app, the adapters, the
ledger and the relay, as of 0.844).

Green tests are not the evidence here. Each package carries a mutation control
(`test/mutation-control.mjs`) that breaks one behaviour at a time and requires
the named test to fail. The drives in `_tools/drive-*.mjs` are the other half:
each launches the built app, does what a person does, and keeps its
screenshots.

## Repository map

- `apps/desktop`: the Electron + React app. The main process (agents, ledger, approvals, routines, updates), the preload bridge and the renderer.
- `apps/relay`: the Cloudflare Worker behind phone, browser and app access. It forwards sealed messages.
- `packages/runtime-adapters`: finding installed CLIs, the process transport, and one event reader per agent.
- `packages/mission-store`: the versioned, append-only ledger and its reader.
- `_tools`: the release tools and the drives.
- `docs`: [NETWORK.md](docs/NETWORK.md), [WHAT-LOCUST-CAN-STOP.md](docs/WHAT-LOCUST-CAN-STOP.md), [CODE-SIGNING.md](docs/CODE-SIGNING.md), the architecture notes and the assets.

## Contributing

Locust is developed in this repository, and each release is cut from `main`.
Issues are welcome; code changes are by invitation (see
[CONTRIBUTING.md](CONTRIBUTING.md)). An agent session working here reads
`AGENTS.md`, `PROJECT.md` and `docs/CROSS_TASK_CONTEXT.md` first.

## License

[MIT](LICENSE)
