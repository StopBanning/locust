# What Locust sends over the network

**Conversation references from GitHub.** Only when you choose a repository, Locust asks your own `gh repo clone`
for a shallow copy from `github.com`, or uses `git clone --depth 1` over HTTPS for a public repository if gh cannot
clone it. Private repositories use gh's existing sign-in; Locust never reads a token or signs you in. A copy is
limited to 500 MB including its git data. Opening the conversation after an hour refreshes its copy, unless a run
is still using it. Old and released copies move to Locust's own `reference-trash` folder, not permanent deletion.
Local reference folders are not cloned or uploaded. As with the project folder, an agent may send what it reads
to its model's service. References do not narrow the mode: Auto can change them, and its chips say so.

**Links in the terminal.** Clicking a web link in the person's terminal opens it in their browser, through the
main process, for HTTP and HTTPS only. Terminal output stays in this window and is never sent to a model or
written into a conversation. Commands the person types use their own environment and can make their own
connections, as they can in any terminal; Locust does not proxy or audit those connections.

Locust sends no analytics or automatic uploads of your work. Optional OpenAI
voice typing sends recordings only after you choose it and allow it. It has no
analytics and no usage counts, and when it crashes, the crash dump stays on
this machine (nothing is uploaded). The AI agents it runs
connect to their own services, with the accounts you signed them in with:
that is the work you asked for. This page lists every connection Locust
itself makes, when it makes it, and what it sends, so you can check it.

It is held to the code by `apps/desktop/src/main/the-network-is-written-down.test.ts`:
a new place in Locust's main process that reaches the network fails that test
until it is listed here.

## Locust's own connections

| What | Where | When |
| --- | --- | --- |
| Its own updates (Windows) | `dl.locust.lol` (Locust's own downloads, on Cloudflare) | 8 seconds after it starts, then every hour while it is open, and when you press Check now |
| Its own updates (Mac) | `dl.locust.lol/mac-releases.json`, then the disk image you choose | When it checks, and when you choose to update |
| Installing an AI agent | the npm registry (`registry.npmjs.org`, or the one your npm is set to use) | When you press Install for OpenCode, Claude Code, Codex CLI or Copilot CLI |
| Keeping Codex CLI and Copilot CLI current | the npm registry, and `dl.locust.lol/runtime-canary.json` | 45 seconds after the installed Locust starts, then at most every 6 hours |
| The pet gallery | `openpets.dev`, addresses under `/pets/` only | When you open the gallery, and when you take a pet |
| Testing one of your own models | the address you gave it in Settings > Your own models | When you press Test |
| Skills from GitHub | `api.github.com` and `raw.githubusercontent.com`, for the public repository you name | When you press Look, Look for changes, or Keep in Settings > Teammates |
| Voice typing's one-time files | `github.com/ggml-org/whisper.cpp`, `huggingface.co/ggerganov/whisper.cpp`, and their download CDNs | On Windows x64, only when you press Download in the microphone's first-use prompt |
| OpenAI voice typing | `https://api.openai.com/v1/audio/transcriptions` | Only after choosing Your OpenAI account, saving your API key, allowing audio upload, and recording with the microphone |
| A web page a teammate made, open in the preview | eight public hosts, for libraries and fonts only | While the page is open |
| A voice call (Talk, beside the microphone) | OpenAI's voice service, at the media address the call's answer names | On Windows, only while a call you started is on |
| Scout's looks (a teammate that keeps watch) | GitHub, through your own GitHub CLI; then Scout's own model, through its agent (free: OpenCode's) | Only after you turn Scout on, every 10, 30 or 60 minutes while Locust runs, and its model only when something is new |
| Pictures on a GitHub issue | `github.com/user-attachments` and `githubusercontent.com` only | When you choose an issue, so its pictures can sit in the chat box |
| Your phone, from anywhere | Locust's relay at `app.locust.lol` (on Cloudflare): one WebSocket this computer opens to it | Only after you turn on Your phone with From anywhere, for as long as it stays on |
| Scout while your computer is off | GitHub, through your own GitHub CLI, to open one pull request and set one secret; then, from GitHub's own runner and not this computer, the relay at `app.locust.lol` | Only after you turn on Look while my computer is off and merge its pull request; the workflow runs every hour or two, and does nothing more while Locust is open |
| Locust in a browser | Nothing new from this computer: the relay link Your phone already keeps; the browser loads the window from Locust's relay | Only for a paired device you let open the full window |
| Your teammates in Telegram | `api.telegram.org` (Telegram's Bot API), over HTTPS, for your own bot only | Only after you paste your bot's token in Settings > General > Telegram, and then for as long as Locust is open |
| Your cloud computer (a teammate's turn sent with Cloud, or with Cloud computer) | GitHub, through your own GitHub CLI: Codespaces, and the codespace itself over `gh codespace ssh`; from there, the teammate's model, through its agent (OpenCode, Claude Code or Codex) | Only after you pick Cloud for an OpenCode teammate, or Cloud computer for a Claude Code or Codex teammate, and press Set it up, and then for each turn you send there |

**Pictures on a GitHub issue.** When you choose an issue, Locust reads its text through your own `gh` and fetches at most four pictures, each from `github.com/user-attachments` or `githubusercontent.com`. A redirect is followed only while the next address stays on those hosts. The fetch sends no token. `gh` keeps the sign-in it already has, and Locust never prints it.

**Its own updates.** A plain request to Locust's own downloads for the files of
the newest release: `latest.yml`, then the parts of the installer that changed.
No account, nothing that identifies you beyond what any download shows (your IP
address, to Cloudflare). A check is never made while a teammate is working; it
is tried again 15 minutes later. A version already downloaded and waiting does
not stop the hourly check: a newer one replaces it. A copy of Locust that is not
the installed one never updates itself.

**Installing and updating AI agents.** Locust runs npm, and npm downloads the
package: `opencode-ai`, `@anthropic-ai/claude-code`, `@openai/codex` or
`@github/copilot`. Cursor Agent, Muse Code and Antigravity are installed from
their makers' pages, in your browser. For Codex CLI and Copilot CLI, the
installed Locust also asks npm for their newest version (one `npm view` each)
and reads the verdict file above, which can hold a version back. The look runs
whatever "Update Codex CLI and Copilot CLI on their own" (Settings > AI agents)
says; the switch decides only whether a version 12 hours old is installed on
its own. Claude Code, OpenCode and Cursor Agent keep themselves current, with
their own connections.

**The pet gallery.** The catalog of pets and the files of a pet you take, over
https, from `openpets.dev` under `/pets/` and nowhere else, with no redirects
followed. What was read is kept on this machine and used again.

**Testing one of your own models.** Test asks the address you gave which
models it serves (`/models`), then sends the model one chat request, capped at
a single word and carrying one tool that does nothing, to see whether it can
use tools. A key you gave goes with both, to that address and nowhere else.
Nothing of your work is in either request. A teammate's runs on the model go
through OpenCode, below.

**Skills from GitHub.** Look asks GitHub's API for the repository you named,
the commit its branch points to, and the list of its files at that commit,
then reads each skill's SKILL.md for its description. Keep downloads the files
of the skills you ticked, at that same commit, and checks each against the
file id GitHub's list gives it. No sign-in is asked for or sent, so only public
repositories can be read, and GitHub allows 60 looks an hour from one address.
Redirects are followed only to those two hosts. Nothing is checked again on its
own: a kept skill changes only when you look for changes and keep again.

**Voice typing's one-time files.** Download retrieves the pinned Windows x64 CPU
archive `b5454/whisper-bin-x64.zip` and the selected pinned model:
Fast's `ggml-tiny.en-q5_1.bin` (32,166,155 bytes) or Accurate's
`ggml-base.en-q5_1.bin` (59,721,011 bytes). Both use
revision `5359861c739e955e79d9a303bcbc70fb988958b1`, following their HTTPS download
redirects. Every asset has a SHA-256 in the code. The checked runtime and model
stay in this profile's `voice/` folder. No account, API key, recording, transcript,
or work goes in these requests; the servers see what any file download reveals,
including your IP address. No connection is made before Download, and local
transcription makes no speech-service request. A cancelled download is restarted
cleanly when you choose Download again. Audio files are transient, removed after
transcription or cancellation; no recognition process is retained at rest.

**OpenAI voice typing.** This is off by default. Settings > General says that
audio goes to OpenAI; the first recording asks permission once. Main sends a
multipart WAV, `model=gpt-4o-transcribe`, `language=en`, and `response_format=json`
to `https://api.openai.com/v1/audio/transcriptions`, authenticated with the
person's own API key. No redirects are followed. No draft, conversation, or
project files are included. OpenAI API charges apply; a ChatGPT subscription
does not include API usage. The saved key is encrypted by Windows for this
account and never returned to the window or included in logs or profile backups.
Only the transcript or a plain, sanitized error comes back to the window.
Cancelling aborts the request and discards late text, but cannot recall audio
already sent to OpenAI. No upload occurs before consent, no request is made at
rest, and nothing falls back to OpenAI from either local choice.

**A voice call.** Talk, beside the chat box's microphone, starts a call with
ChatGPT's own voice. Locust does not place it: Codex CLI does, with the ChatGPT
sign-in you gave Codex (Locust never sees it), over Codex's own app server
(`thread/realtime/start`, WebRTC, realtime v3). The window makes the WebRTC offer
from your microphone and plays the voice that comes back, so for as long as the
call lasts your voice goes from this window to OpenAI's voice service and its
voice comes back the same way; what was said comes back from Codex as text. The
call's Codex works in your folder on a thread of its own: read-only, not kept in
Codex's history, reading the folder through Locust's own read-only tools, and
running no command. Your connectors are the call's too (0.742): ChatGPT's own,
and each add-on of yours that is a service at an address, reached by Codex as it
reaches them for any of your runs. One that only reads answers at once; one that
would send, change or buy something waits for your Allow in the call's strip.
Add-ons your computer runs as programs, plugins and hooks stay switched off for
that thread. Nothing is sent before you press Talk, and End, leaving the
conversation, or closing Locust ends it. Talk's keys (0.755, Ctrl+Alt+Space unless you change
them) are a press of Talk from any app: Windows tells Locust they were pressed,
and nothing leaves this computer until the call they start does. A call's
summary of its to-dos is drawn from what this window already knows, and sends
nothing.

**Scout (0.756).** Off until you press Turn on Scout. Then, every 10 minutes
while Locust runs (or 30 or 60, as you set it), Locust looks for itself: your
open pull requests, the reviews asked of you, the default branch's checks and
the issues open on the folder's repository, with your own GitHub CLI in the
folder (`gh pr list`, `gh run list`, `gh repo view`, `gh issue list` -- what
that sign-in can read, as Commit and the pull request chip already do), and
its own records of a teammate waiting on you or a turn that
failed. That look sends nothing anywhere but GitHub. Only when it finds
something new does Scout's model read it: one turn, in Ask mode, on the agent
and model Scout is set to -- Space Bunny through OpenCode when Locust adds it,
a free model whose provider may keep what it is sent -- carrying the new facts
(pull request titles and numbers, a new issue's title and who opened it, check names, which teammate waits on you
and for how long -- never what it waits to run -- a failed turn's error and
what it was asked) and your teammates'
names and roles. Change Scout's model like any teammate's, and its looks go
there instead. The "Your /locust comments" watch reads `gh api user` once for the signed-in login and one repository issue-comments
page per look with `since`; it accepts only that person's newly created asks. For each ask it reads the issue or pull
request text and recent comments through `gh api`, and marks that context as other people's words in the draft.
A named teammate needs no Scout model turn; otherwise Scout may suggest the teammate, with the person's last teammate
in that folder as the fallback. The answer stays on a card until the person presses Post it: then `gh api` posts exactly
the reviewed text as their comment. An attempt is recorded before posting, and an uncertain result is never retried.
Scout sends no message, merges, pushes or changes anything on its own: every action on its cards is your press.

**Your cloud computer (0.761).** Off until you pick Cloud in the chat-type menu
for an OpenCode teammate and press Set it up. Locust then asks GitHub, with your
own GitHub CLI, to make a codespace for the folder's repository on your GitHub
account (`gh codespace create`: 2 CPUs and 8 GB, stops itself after 10 idle
minutes, deleted after a week unused) and installs OpenCode on it from npm. For
each turn you send with Cloud, Locust sends that codespace this folder as it is
-- commits not pushed, edits not committed, files not added, as one git bundle
over `gh codespace ssh` -- and starts OpenCode's own server there, reached
through that ssh connection's tunnel: Locust picks a free port on `127.0.0.1`
here and the tunnel forwards it to the server there. Your words go to that
server, and from it to the teammate's model; its change is read back over the
same connection. The agent there runs without the codespace's GitHub sign-in,
so nothing it does reaches GitHub; its change comes into this folder only when
you press Apply. GitHub counts the codespace's hours (60 free a month at this
size) and charges for more only if your GitHub account has a way to pay.
Locust holds no GitHub token: gh keeps it where gh keeps it. Since 0.762, files
you attach to a message there (a picture, a file from outside the folder, a PDF
Locust opened) go to the codespace over the same `gh codespace ssh`, and only
those. And when you press Open a pull request on a change waiting there, Locust
itself -- never the agent -- commits just that change on a new branch with the
codespace's own GitHub sign-in, pushes it to the repository, and opens the pull
request with your GitHub CLI here (`gh pr create`).

**Claude Code and Codex on your cloud computer (0.768).** Off until you pick
Cloud computer for a Claude Code or Codex teammate. Locust installs that agent
on the codespace from npm (`@anthropic-ai/claude-code` or `@openai/codex`, the
version this computer has), and each is signed in there by you, once: when you
press Sign in, Locust starts the agent's own sign-in on the codespace, which asks
to open its maker's page -- `claude.com`, or `auth.openai.com`; Locust opens
nothing else -- and Locust opens that page in your browser. The page answers
to a port on `localhost`, one the agent picks (Codex: 1455). Locust
checks that port is free here, carries that one port to the agent there over a
second `gh codespace ssh`, and asks the agent's own server there once, through
it, that the way is open before your browser goes to the page. You click
Authorize; your browser and the agent do the rest. Locust never sees the
sign-in, which stays on the codespace and goes when it is deleted. A turn there
then goes as an OpenCode turn there does, over the same `gh codespace ssh`:
Codex's app server or Claude Code's own input and output, and the agent's own
connection to its maker from the codespace.

**Hearing a voice (0.751).** Hear it, beside a voice picker, plays that voice
saying hello. The first time for each voice it is a call like Talk's, a few
seconds long, with nothing of yours in it: no microphone (the window sends a
"Hello?" made by Windows' own speech on this computer), no folder tools, no
connectors and no add-ons. The greeting that comes back is kept on this
computer, in Locust's own folder, and every play after that is the kept one,
sending nothing.

**A web page in the preview.** A page a teammate made runs inside Locust. It may
load libraries, stylesheets and fonts -- GET and HEAD over https only -- from
`cdnjs.cloudflare.com`, `cdn.jsdelivr.net`, `unpkg.com`, `esm.sh`,
`cdn.tailwindcss.com`, `code.jquery.com`, `fonts.googleapis.com` and
`fonts.gstatic.com`. Everything else it asks of the web is refused, so what it
reads of your folder stays on this machine.

## What stays on this machine

- **Locust's own window loads nothing from the network** in the installed app:
  one filter refuses every request it makes, apart from a previewed page's,
  above, and a voice call's audio while a call you started is on (above). On
  Windows its own main frame may request microphone audio for voice typing
  after a mic press, and for a voice call after you press Talk. Cameras,
  preview frames, location, and all other device and web permissions remain
  refused.
- **Locust's permission host listens on `127.0.0.1` only.** Claude Code asks it
  before a connector call or a command, and each run gets its own token. Locust
  also reads Antigravity's own local server, at `127.0.0.1`.
- **Locust's browser bridge listens on `127.0.0.1` only.** The in-window Browser
  panel runs in its own isolated Chromium session (`persist:locust-browser`),
  separate from your Chrome profile and cookies. When a teammate runs on an
  agent that accepts per-run MCP tools, a per-run stdio bridge forwards browser
  tool calls to this loopback listener with a token minted for that run only.
  Nothing the browser shows reaches a model unless a teammate's tool call asked
  for it in that turn, and password, payment, submit, send, download, and
  cross-origin navigations ask on a card in the conversation first.
- **Let your other AI apps use Locust** (Settings > General) is off by default.
  When enabled, a separate listener binds only to `127.0.0.1` and
  authenticates the token before reading the request body. The local stdio
  bridge reads this profile's account-only connection file and forwards tool
  calls over HTTP; it never starts Locust or sends the token off the machine.
  Turning the switch off closes the listener and revokes the token; enabling
  it again mints a new one. Teammate conversations started through it run in
  Ask mode (read only), use the agents' own accounts, apply Locust's monthly
  limits, and appear in history marked as started from another app. Shares
  remain visible in Locust, but never start automatic teammate handoffs. Listing
  teammates, replies and background runs is read only. Running turns are not
  cancelled when the listener is switched off. An app you add carries what it
  is asked to that listener from wherever it was asked: with OpenClaw's line
  (0.754), a message you send OpenClaw from a chat app on your phone reaches
  your teammates, through OpenClaw's own connections, not Locust's.
- **The locust command and ACP editors** (`packages/cli`) use that same opt-in
  loopback listener and only Locust's connection file. No new port or external
  destination is introduced. The private CLI API grants a per-session
  capability for a new conversation; an editor can cancel that conversation,
  and answer its own current action's approval once only if you turned on Let
  editors answer cards for their own turns (off by default; every app
  connected to Locust holds the key a session is opened with). The existing MCP bridge
  does not advertise those methods or answer approvals. Questions stay in
  Locust. Editor disconnects and lost heartbeat leases cancel owned work;
  neither prompts nor uncertain requests are automatically retried. The
  editor supplies text, never programs or an MCP server to execute. Turns
  preserve the local server's Ask/own-mode choice and the app's existing
  permission and monthly-limit guards.
- **Your phone** (Settings > General) is off by default, and it is the one
  listener that is not on `127.0.0.1` only. When enabled, Locust listens for
  your home network on port 47740 (any free port when that one is taken), and
  Windows may ask you once whether it can. It serves one page, which holds
  nothing; everything that page shows or does needs the key in the code you
  scanned, sent with each request and checked before the request body is read.
  An address that sends 20 wrong keys in ten minutes is refused for a while,
  and a request that names this computer by anything but its address is
  refused. It is plain HTTP on your own network, with no server in between:
  use it on a network you trust. New code mints a new key and unpairs every
  phone; turning the switch off closes the listener and forgets the key.
  This is the Only on this Wi-Fi choice; From anywhere, below, listens to nothing.
- **Your phone, from anywhere** (Settings > General, the first choice when you
  turn Your phone on) listens to nothing. Locust opens one WebSocket out to its
  relay at `app.locust.lol` and keeps it open while this is on, so a phone you
  paired can reach it from any network. Every request and answer is sealed on
  one side and opened on the other (an X25519 key exchange, then AES-256-GCM,
  with a key for each direction), and those keys never leave your computer and
  the phone: the relay passes along what it cannot read. It keeps only hashes
  of the tokens it lets in, and writes no log of its own of who connected or
  when. A code pairs one phone, once, within ten minutes. Remove a phone in
  Settings and it stops at once; turning this off forgets your computer's keys
  and has the relay delete its room. The relay's code is in this repository
  (`apps/relay`), so anyone can run their own and start Locust with
  `LOCUST_RELAY` set to its address.
  With alerts turned on, on the phone, an alert goes from your computer
  through the relay to that phone's browser push service (Apple's, Google's,
  Mozilla's or Microsoft's), encrypted on your computer for that browser alone
  (the Web Push standard, RFC 8291). The relay signs it and passes it on, and
  posts to no other address; neither it nor the push service can read it. The
  alert says who and what kind, never a command or a message's words.
- **Locust in a browser** (Settings > General > Your phone: Full window, on a
  paired device) adds no connection of this computer's own. The browser opens
  `app.locust.lol/desk`, which asks this computer, through the relay link it
  already keeps, which Locust it runs; the relay then serves that release's own
  window, read from the zip the release put on Locust's downloads
  (`dl.locust.lol/window/`), so a browser never runs code a computer sent it.
  Every call the window makes goes sealed through the same link, and this
  computer answers only those `apps/desktop/src/shared/desk-calls.ts` lets a
  browser make; signing in, permissions, spending and the terminal stay here.
- **Your teammates in Telegram** (Settings > General > Telegram) listens to
  nothing. With your own bot's token from Telegram's @BotFather, Locust asks
  `api.telegram.org` for the bot's new messages, one long request at a time,
  and answers through the same bot; no other address is ever sent the token,
  since it is part of each request's address. The token is kept as the
  operating system encrypts it (safeStorage), and Settings shows only the
  bot's name. Only the one chat that pressed Start with your pairing link is
  answered: a link pairs one chat, once, within ten minutes, and anyone else
  who writes to the bot is ignored. A message there starts your next turn with
  that teammate, on its own model and mode; a card from that turn goes to the
  chat with Allow once and Deny, never Always, and shows its one-line summary,
  not the whole command. A bot's chat is not end-to-end encrypted, so Telegram's
  servers can read what is written there: Locust takes out anything shaped
  like a key (the same scrub a saved record gets) before it sends an answer or
  a card, and Settings says so. A message that arrived while Locust was closed
  starts nothing when it opens. Remove forgets the token and the chat.
- **Scout's away look** (Settings > General, under Scout: Look while my
  computer is off) runs on GitHub, not on your computer. Turning it on, Locust
  uses your own `gh` to open one pull request in the folder's repository, adding
  a workflow and the one file it runs (`apps/desktop/src/main/scout-away-look.ts`,
  built into `apps/desktop/resources/scout-away.mjs`), and to set the
  repository's LOCUST_SCOUT secret on gh's standard input, never its command
  line. Once merged, the workflow reads the repository with its own read-only
  token and posts to the relay over HTTPS, sealed with a key the relay never
  has; with Locust open on your computer it asks the relay that, and stops.
  Turning it off deletes the secret and turns the workflow off.
- **The spellchecker downloads no dictionary.**
- **Memory recall** runs a small model that ships with Locust.
- **A report reaches Locust's makers only when you send it.** Send feedback,
  and Report a problem in Settings, let you choose: open a public GitHub issue
  in your browser, open your mail app with the report addressed to Locust's
  makers, or save it as a file.
- **Links** (the Locust website, an agent's install page) open in your
  browser, when you click them. A link inside a previewed page asks you first,
  since an address can carry what the page read.

## The AI agents' own connections

OpenCode 1 uses its existing local server connection. OpenCode 2 uses its
Agent Client Protocol subprocess over standard input and output. To fill the
model picker, Locust opens an empty temporary session, reads the models it
advertises and stops the process; it sends no prompt and offers no filesystem,
terminal or MCP access during that read. OpenCode may make its own startup
connections to discover providers and models. Its sign-ins stay with OpenCode.

Each AI agent connects to its own service with the account you signed it in
with, and sends what that service needs to do the work: your messages, and the
files and command output the agent reads. Codex CLI talks to OpenAI, Claude
Code to Anthropic, Copilot CLI to GitHub, Cursor Agent to Cursor, Antigravity
to Google, Muse Code to Meta, and OpenCode to the provider of the model you
pick -- for one of your own models, the address you gave it in Settings > Your
own models. An agent you added in Settings > AI agents > Your own agents connects
wherever its makers built it to, with whatever account you signed it in with;
Locust starts it on this computer and talks to it over its standard input and
output only. A connector a teammate uses is called by its agent, at the service
that connector names. A Claude Code teammate can also search the web and open
web pages: in Auto on its own, and in every other mode only after you approve
each one on its card, which shows the search words or the page's whole address.
Codex CLI, Copilot CLI, OpenCode and Muse Code search the web and open pages
with their own tools in every mode. Cursor Agent and Antigravity do only in
Auto: outside it their own rules refuse those tools. Each teammate's Activity
panel says which of these its run may do, and whether it asks you first.

Locust does not sit between an agent and its service: it reads what the agent
reports on this machine as it works, and nothing of its network traffic. It
never sees the agents' sign-ins: each agent keeps its own. A key you give Locust for one of
your own models is kept on this machine, encrypted by the system, sent by Test
to that model's address, and handed only to OpenCode, for that model.
