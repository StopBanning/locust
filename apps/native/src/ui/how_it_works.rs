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

/// Electron's `<More>` under each section Rust draws, by fold key: the section's heading and the paragraphs, word for
/// word from Screens.tsx (entities read as the characters they name, lines joined as JSX joins them).
pub(crate) const FOLDS: &[(&str, &str, &[&str])] = &[
    (
        "auto-mode",
        "Auto mode",
        &[
            "Every other mode confines a run to the folder you chose for it, and a runtime that wants to touch anything outside it is refused. Auto is the mode that does not: a run started in Auto goes ahead without asking and may read and change files anywhere this computer lets you, including outside the folder.",
            "It exists because some work genuinely lives in more than one place. Auto is always in the composer's permission menu and picking it there is what switches it on, so this is the same decision seen from the other side: what is on now, and the way to take it back. It is checked again each time a run starts — switching it off here stops the next run, including one a teammate or a routine was about to start — and it is never the mode a malformed or missing choice falls back to.",
        ],
    ),
    (
        "report-a-problem",
        "Report a problem",
        &[
            "The log records what happened, never what was said. Crashes, a window that stopped answering, and the version you were on — no messages, no file contents, and nothing from a conversation. That is what makes it safe to send.",
            "It is capped, and rolls over once. Nothing in it leaves this machine unless you send it.",
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
        "memory",
        "What your team remembers",
        &[
            "The way Claude Code and Cursor do, managed from here. A teammate writes one by ending a reply with it; every teammate in the folder reads what is kept. Each memory says who wrote it, where, and from which conversation, and you can edit, switch off, or remove any of them.",
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
        "privacy",
        "Privacy & local data",
        &[
            "Nothing here is ever deleted on a timer. Turns go when you ask, after you have been shown exactly what would go, and a turn an ongoing conversation continues from is kept even when it is old.",
        ],
    ),
    (
        "updates",
        "Updates",
        &[
            "It never installs one while a teammate is working: restarting then would cut the run off and leave its record without a receipt.",
        ],
    ),
];

/// The words of the fold `key` (nothing for a key that has none).
pub(crate) fn words(key: &str) -> &'static [&'static str] {
    FOLDS.iter().find(|(k, _, _)| *k == key).map(|(_, _, words)| *words).unwrap_or(&[])
}
