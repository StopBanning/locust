//! "How it works" (Screens.tsx `More`): the explanation a setting used to open with, folded under one line. Closed
//! until pressed; a chevron and the words in sentence case, as the Settings pane says them
//! (`.lc-settings__pane .lc-settings__more > summary { text-transform: none }`), then the text. Which folds are open
//! is kept by key, so each setting's fold opens on its own.

use super::*;
use crate::settings_pages::HOW_IT_WORKS;

/// The folds that are open, by key.
#[derive(Default)]
pub(crate) struct Folds {
    open: std::collections::HashSet<String>,
}

impl Folds {
    pub fn is_open(&self, key: &str) -> bool {
        self.open.contains(key)
    }

    pub fn toggle(&mut self, key: &str) {
        if !self.open.remove(key) {
            self.open.insert(key.to_owned());
        }
    }
}

/// The fold for `key`: its summary (`how-it-works-<key>`, a button that says whether it is open) and, once open, its
/// paragraphs (`how-it-works-<key>-text`). `toggle` flips the key in whichever `Folds` the screen keeps.
pub(crate) fn fold(
    l: &crate::look::controls::Look,
    folds: &Folds,
    key: &str,
    paragraphs: &[&str],
    toggle: impl Fn(&mut State) + 'static,
) -> El {
    let open = folds.is_open(key);
    let chevron = if open { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
    let summary = row()
        .key(format!("how-it-works-{key}"))
        .role("button")
        .label(HOW_IT_WORKS)
        .expanded(open)
        .focusable(true)
        .focus_ring(2.0, 2.0, l.c("focus-ring"))
        .gap(6.0)
        .center_y()
        .child(crate::look::controls::glyph(l, chevron, 10.0, Some(l.c("text-muted"))))
        .child(text(HOW_IT_WORKS).font(l.ty.body).color(l.c("text-muted")))
        .on_press(toggle);
    let mut body = col().w_full().gap(4.0).px(10.0).pb(10.0).child(summary);
    if open {
        let mut words = col().key(format!("how-it-works-{key}-text")).w_full().gap(12.0).pt(12.0);
        for paragraph in paragraphs {
            words = words.child(text((*paragraph).to_owned()).font(l.ty.body).color(l.c("text-muted")).wrap());
        }
        body = body.child(words);
    }
    body
}

/// Electron's `<More>` under each section Rust draws, in Electron's order (`Screens.tsx`), by fold key: the section's
/// heading and the paragraphs, word for word from Screens.tsx (entities read as the characters they name, lines joined
/// as JSX joins them). Where Rust does not yet have part of a section (Project folder's folder change, Connectors'
/// ask-first switch), the fold says plainly what is not here yet rather than describing a feature the app does not have.
pub(crate) const FOLDS: &[(&str, &str, &[&str])] = &[
    (
        "report-a-problem",
        "Report a problem",
        &[
            "The log records what happened, never what was said. Crashes, a window that stopped answering, and the version you were on — no messages, no file contents, and nothing from a conversation. That is what makes it safe to send.",
            "It is capped, and rolls over once. Nothing in it leaves this machine unless you send it.",
        ],
    ),
    (
        "project-folder",
        "Project folder",
        &[
            "Changing the project folder from Settings is not here yet.",
            "A notes file named LOCUST.md at the folder’s root is given to every teammate, on every AI agent, before each run. A teammate with Own copy on works in its own copy of the folder (a git worktree of its repository), kept under .locust/worktrees. Each turn it finishes is saved on its branch as a commit; Review changes in its conversation shows them and lands them on your branch as one commit of yours. Removing one here keeps its branch.",
        ],
    ),
    (
        "between-teammates",
        "Between teammates",
        &[
            "When a teammate writes to another, the other can answer on their own: Locust starts a run for them with the message as its brief, and their answer starts the sender’s next turn, so it lands in the thread that asked. They keep going while a reply helps finish the work, and stop when one has nothing more to say, or when the budget below is spent.",
            "Each teammate answers on their own route — their runtime, model and mode, not the sender’s -- which is how two models end up on one piece of work. Switch replies off to make messages wait for you instead.",
            "A teammate does one thing at a time, so a message that arrives while they are working waits until that run ends. A sender can mark a message urgent when waiting would make it useless -- “stop, I am editing that file” is the case it exists for. Letting that stop the recipient part-way throws away whatever they had in flight, so it is off until you turn it on; their unfinished work stays in their own conversation either way.",
        ],
    ),
    (
        "agents",
        "AI agents & accounts",
        &[
            "Locust never pools subscriptions or proxies your requests. Under each AI agent is what it has set up for itself — connectors (MCP servers) and hooks, read from its own files — so a tool a teammate reaches for, or a script that runs mid-run, is never a surprise. Locust changes nothing there.",
        ],
    ),
    (
        "own-models",
        "Your own models",
        &[
            "Anything that speaks the OpenAI chat API works: a company’s own endpoint, vLLM, Ollama’s /v1, LM Studio. Teammates reach it through OpenCode, so OpenCode needs to be installed; it is the model’s name, not OpenCode’s, that the model list and the chat bar show.",
            "A key is kept only as your system encrypts it for your account, and never shown again; it is sent to that address and nowhere else. Test asks the address which models it serves, and sends the model one request capped at a single word to see whether it can use tools — so a mistyped name, or a model that can only chat, is caught before a teammate tries it. A model set to chat only talks with its teammates and reads and changes no files.",
        ],
    ),
    (
        "github",
        "GitHub",
        &[
            "Locust uses the GitHub account the GitHub CLI (gh) is signed in to, the same one you use in a terminal. It keeps no token of its own. Sign in here and Locust starts gh’s own sign-in: it shows a one-time code, you enter it on GitHub’s page in your browser, and gh keeps the sign-in where it always does.",
            "Once signed in, Commit in a conversation can also push and open a pull request. To sign out, run gh auth logout in a terminal.",
        ],
    ),
    (
        "connectors",
        "Connectors",
        &[
            "A connector is not on this machine: it acts on the service it reaches, so no permission mode governs it. By default a teammate may use whichever connectors your own Claude Code has, the way you can — the question was answered when you connected them.",
            "The switch to stop a run and ask before every connector call is not here yet.",
        ],
    ),
    (
        "add-connector",
        "Add a connector",
        &[
            "Claude Code, Codex, Gemini CLI, Copilot, OpenCode and Antigravity each keep their own list of connectors, in their own file, and each has a command that adds one. Locust runs that command for you, the way you would type it, and never edits their files itself. The list above is Claude Code’s.",
            "An agent that already has a connector by that name keeps its own, untouched: it may hold a key Locust never saw. One that cannot say whether it has one is left alone. Undo takes it back out with each agent’s own command; OpenCode has none, so it says where its copy is kept.",
        ],
    ),
    (
        "plan-runs-out",
        "When an agent cannot go on",
        &[
            "Wherever the work goes on, Locust carries it there with a summary of what was done and what was left unsettled, and a line in the conversation says where it moved and why.",
            "HAND OFF — Pick another AI agent from the model menu while it is running. It is stopped there and carried on with a summary of what was done.",
            "CONTINUE ELSEWHERE — After a run has stopped, pick another model and reply. The next turn starts there with a summary of what was done, and your reply as its first instruction.",
            "CARRY ON — When a plan runs out, one press on the stopped run’s card carries the work on with the agent picked above, or, with Without asking on, it goes on by itself. Claude Code and Codex say when their plan runs out; an agent that does not say so is not carried on. The same goes for a sign-in that expired, a provider that is down or no network, but only when the run had not used a single tool yet, so nothing is done twice.",
        ],
    ),
    (
        "auto-mode",
        "Auto mode",
        &[
            "Every other mode confines a run to the folder you chose for it, and a runtime that wants to touch anything outside it is refused. Auto is the mode that does not: a run started in Auto goes ahead without asking and may read and change files anywhere this computer lets you, including outside the folder.",
            "It exists because some work genuinely lives in more than one place. Auto is always in the composer's permission menu and picking it there is what switches it on, so this is the same decision seen from the other side: what is on now, and the way to take it back. It is checked again each time a run starts — switching it off here stops the next run, including one a teammate or a routine was about to start — and it is never the mode a malformed or missing choice falls back to.",
        ],
    ),
    (
        "plans",
        "Plans",
        &[
            "Codex, Cursor and OpenCode each have a tool for keeping a todo list, and the board in a room draws whatever they put in it. This asks them to use it: add the steps once they are known, mark one in progress, mark it done when it is done.",
            "It is off by default because it is not free — the bookkeeping costs tokens and changes how a teammate narrates itself. It is worth switching on when you are watching the board rather than reading every line.",
            "Claude Code is never asked. It has no such tool, so the request would be an instruction it cannot follow and the board would stay empty with no explanation. This switch does nothing for a Claude Code teammate, in either position.",
        ],
    ),
    (
        "own-skills",
        "Your skills",
        &[
            "A Claude Code teammate can use the skills in the folder's .claude/skills, the way Claude Code does. Your own, in .claude/skills in your home folder, were written for your own sessions, so they are lent only when this is on. It is read each time a run starts.",
            "A skill is instructions. It uses only the tools the teammate's mode already gives it, so a teammate in Ask that uses a skill still changes nothing. In Auto a teammate runs as you and finds every skill you have, whichever way this is set.",
            "Codex, OpenCode, Copilot and Antigravity find your own skills themselves, in the folders they already read in your own sessions; this switch is for Claude Code. Claude Code, Codex, OpenCode, Copilot, Antigravity and Cursor teammates get their own skills from a tune and the ones you keep from GitHub.",
        ],
    ),
    (
        "skills-from-github",
        "Skills from GitHub",
        &[
            "Look lists every skill in the repository and every file each one carries, before anything is downloaded. Files that run, such as scripts and programs, are named. Only the skills you tick are kept, exactly as they are at the commit you looked at. Nothing changes when the repository does, until you look for changes and keep again.",
            "A skill is instructions a teammate follows, and its scripts run with the teammate's mode. In Ask they cannot change anything; in Auto a teammate can run them without asking. Keep skills from people you trust, as you would install a program.",
            "Only public repositories are read, and Locust sends GitHub nothing about you.",
        ],
    ),
    (
        "memory",
        "What your team remembers",
        &[
            "The way Claude Code and Cursor do, managed from here. A teammate writes one by ending a reply with it; every teammate in the folder reads what is kept. Each memory says who wrote it, where, and from which conversation, and you can edit, switch off, or remove any of them.",
        ],
    ),
    (
        "reply-size",
        "Reply text size",
        &[
            "A teammate's reply is the one thing on this screen you read rather than scan, so it is set in its own face and sized on its own. Your own messages and a teammate's question cards grow with it; the rest of the app stays where it is whatever you pick here.",
            "The line length follows the size rather than staying put, so a bigger reply gets a wider paragraph and about the same number of words to a line. Measured, not assumed: roughly 74 characters at Standard and 72 at Large.",
        ],
    ),
    (
        "boot-screen",
        "Boot screen",
        &[
            "Finding the AI agents on this machine takes as long as it takes — each one is a real command and some of them are slow to answer. The screen shows that happening rather than a spinner standing in for it, and every line on it is something the app actually read.",
            "It only ever covers the empty middle of the window. The sidebar, the folder and the message box stay where they are, so you can pick a teammate or start typing without waiting for it, and clicking anywhere on it puts it away.",
        ],
    ),
    (
        "phone",
        "Your phone",
        &[
            "From anywhere, this computer keeps one connection open to Locust’s relay, and your phone reaches it there from any network. Each message is locked on one side and opened on the other, with keys only this computer and your phone hold, so the relay passes along what it cannot read. Only on this Wi-Fi, Locust opens one small page on this computer for your home network instead, and nothing goes through anyone else’s server. Either way, a message you send from your phone is your next message in that conversation, on the same teammate and mode, as if you typed it here, and turning this off unpairs every phone.",
        ],
    ),
    (
        "chat-apps",
        "Chat apps",
        &[
            "Each app is your own: a bot you make in Discord or Telegram, or an app you make in Slack, and its token stays encrypted on this computer. While Locust is open, it keeps one connection out to that app -- Discord’s gateway, Slack’s Socket Mode, or Telegram’s long requests -- so nothing on this computer listens for it. Only the account that paired, by the code or the link, is answered; anyone else who finds the bot is ignored. A message there is your next message to that teammate, on its own model and mode, as if you typed it here, and you see it here too. A card from a turn started there comes to the chat with Allow once and Deny. A message that arrives while Locust is closed starts nothing when it opens: the chat says so, and you send it again.",
        ],
    ),
    (
        "updates",
        "Updates",
        &[
            "It never installs one while a teammate is working: restarting then would cut the run off and leave its record without a receipt.",
        ],
    ),
    (
        "privacy",
        "Privacy & local data",
        &[
            "Nothing here is ever deleted on a timer. Turns go when you ask, after you have been shown exactly what would go, and a turn an ongoing conversation continues from is kept even when it is old.",
        ],
    ),
];

/// The words of the fold `key` (nothing for a key that has none).
pub(crate) fn words(key: &str) -> &'static [&'static str] {
    FOLDS.iter().find(|(k, _, _)| *k == key).map(|(_, _, words)| *words).unwrap_or(&[])
}
