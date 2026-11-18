//! Settings › General › "Chat apps" (Electron `ChatAppsSettings.tsx`, `ChatAppRow.tsx`, `TelegramSettings.tsx`,
//! 0.831-0.832): Discord, Slack, and Telegram rows, each walking from token setup to pairing and alerts.

use super::*;
use locust_core::chat_apps::{ChatAppId, ChatAppSettings, SlackTokens, spaced_code};
use locust_core::chat_discord_fmt::DISCORD_DEVELOPER_PORTAL;
use locust_core::chat_host;
use locust_core::chat_link::TokenInput;
use locust_core::chat_slack_fmt::slack_manifest_link;

#[derive(Default)]
pub(super) struct ChatAppsState {
    pub discord: ChatAppSettings,
    pub slack: ChatAppSettings,
    pub telegram: ChatAppSettings,
    pub discord_open: bool,
    pub slack_open: bool,
    pub telegram_open: bool,
    pub discord_token: TextEdit,
    pub slack_bot_token: TextEdit,
    pub slack_app_token: TextEdit,
    pub telegram_token: TextEdit,
    pub copied: Option<ChatAppId>,
    pub busy: bool,
    pub managed: bool,
}

impl ChatAppsState {
    pub fn settings_for(&self, app: ChatAppId) -> &ChatAppSettings {
        match app {
            ChatAppId::Discord => &self.discord,
            ChatAppId::Slack => &self.slack,
            ChatAppId::Telegram => &self.telegram,
        }
    }

    pub fn set_settings(&mut self, app: ChatAppId, next: ChatAppSettings) {
        match app {
            ChatAppId::Discord => self.discord = next,
            ChatAppId::Slack => self.slack = next,
            ChatAppId::Telegram => self.telegram = next,
        }
    }

    pub fn is_open(&self, app: ChatAppId) -> bool {
        match app {
            ChatAppId::Discord => self.discord_open,
            ChatAppId::Slack => self.slack_open,
            ChatAppId::Telegram => self.telegram_open,
        }
    }

    pub fn toggle_open(&mut self, app: ChatAppId) {
        match app {
            ChatAppId::Discord => self.discord_open = !self.discord_open,
            ChatAppId::Slack => self.slack_open = !self.slack_open,
            ChatAppId::Telegram => self.telegram_open = !self.telegram_open,
        }
    }
}

pub(super) fn reload(s: &mut State) {
    s.chat_apps.managed = chat_host::managed_refusal().is_some();
    s.chat_apps.discord = chat_host::settings(ChatAppId::Discord);
    s.chat_apps.slack = chat_host::settings(ChatAppId::Slack);
    s.chat_apps.telegram = chat_host::settings(ChatAppId::Telegram);
}

pub(super) fn chat_status(state: &ChatAppSettings) -> String {
    if state.bot.is_none() {
        return "Off".into();
    }
    match &state.chat {
        None => {
            if state.connected {
                "Not paired yet".into()
            } else {
                "Connecting\u{2026}".into()
            }
        }
        Some(chat) => {
            if state.connected {
                format!("On \u{00b7} {}", chat.name)
            } else {
                format!("On \u{00b7} {} \u{00b7} not connected now", chat.name)
            }
        }
    }
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let l = crate::look::controls::Look::new(p.dark);
    let mut box_ = col()
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .child(text("Chat apps".to_owned()).font(type_::TITLE).color(p.text));

    if s.chat_apps.managed {
        return box_.child(
            text("Managed by your organization.".to_owned())
                .font(type_::LABEL)
                .color(p.secondary)
                .wrap(),
        );
    }

    box_ = box_
        .child(
            text(
                "Write to your teammates from Discord, Slack or Telegram, get their answers there, and answer their approval cards, while Locust is open on this computer.".to_owned(),
            )
            .font(type_::LABEL)
            .color(p.secondary)
            .wrap(),
        )
        .child(app_row(s, p, &l, ChatAppId::Discord))
        .child(app_row(s, p, &l, ChatAppId::Slack))
        .child(app_row(s, p, &l, ChatAppId::Telegram));

    box_
}

fn btn(l: &crate::look::controls::Look, label: &str, disabled: bool) -> El {
    crate::look::controls::button(l, label, crate::look::controls::Tone::Neutral, disabled)
}

fn app_row(s: &State, p: &Palette, l: &crate::look::controls::Look, app: ChatAppId) -> El {
    let state = s.chat_apps.settings_for(app);
    let has_bot = state.bot.is_some();
    let open = s.chat_apps.is_open(app);

    let mut head = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            col()
                .grow(1.0)
                .gap(2.0)
                .child(text(app.display_name().to_owned()).font(type_::BODY).color(p.text))
                .child(text(chat_status(state)).font(type_::LABEL).color(p.secondary)),
        );

    if !has_bot {
        let label = if open { "Hide" } else { "Set up" };
        head = head.child(
            btn(l, label, false)
                .key(format!("chat-app-toggle-{}", app.as_str()))
                .on_press(move |s: &mut State| {
                    s.chat_apps.toggle_open(app);
                }),
        );
    }

    let mut card = col()
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .fill(p.field)
        .radius(8.0)
        .child(head);

    if let Some(ref msg) = state.message {
        card = card.child(text(msg.clone()).font(type_::LABEL).color(p.secondary).wrap());
    }

    if has_bot || open {
        card = card.child(app_body(s, p, l, app, state));
    }

    card
}

fn app_body(
    s: &State,
    p: &Palette,
    l: &crate::look::controls::Look,
    app: ChatAppId,
    state: &ChatAppSettings,
) -> El {
    match (&state.bot, &state.chat) {
        (None, _) => token_steps(s, p, l, app),
        (Some(bot), None) => pairing_steps(s, p, l, app, state, bot),
        (Some(bot), Some(chat)) => paired_steps(s, p, l, app, state, bot, chat),
    }
}

fn open_url(url: &str) {
    if cfg!(test) || !url.starts_with("https://") {
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
    #[cfg(not(target_os = "macos"))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn token_steps(s: &State, p: &Palette, l: &crate::look::controls::Look, app: ChatAppId) -> El {
    let mut body = col().w_full().gap(6.0);
    match app {
        ChatAppId::Discord => {
            body = body
                .child(text("1. In Discord\u{2019}s Developer Portal, press New Application, name it (Locust will do), and press Create.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                .child(btn(l, "Open the Developer Portal", false).on_press(|_: &mut State| {
                    open_url(DISCORD_DEVELOPER_PORTAL);
                }))
                .child(text("2. On its Bot page, press Reset Token (Discord may ask for your password) and copy the token. Paste it here. It is kept encrypted on this computer and sent only to Discord.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                .child(
                    row()
                        .w_full()
                        .gap(6.0)
                        .center_y()
                        .child(token_input(&s.chat_apps.discord_token, p, "MTI\u{2026}", "chat-discord-token", "Your bot\u{2019}s token from Discord", |s| &mut s.chat_apps.discord_token))
                        .child(btn(l, "Connect", s.chat_apps.busy || s.chat_apps.discord_token.text.trim().is_empty()).on_press(|s: &mut State| {
                            let tok = s.chat_apps.discord_token.text.clone();
                            s.chat_apps.discord_token = TextEdit::default();
                            let next = chat_host::set_token(ChatAppId::Discord, TokenInput::Single(tok));
                            s.chat_apps.set_settings(ChatAppId::Discord, next);
                        })),
                );
        }
        ChatAppId::Slack => {
            let link = slack_manifest_link();
            body = body
                .child(text("1. Make the app in Slack. This opens Slack with everything filled in: pick your workspace, press Next, then Create.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                .child(btn(l, "Create the app in Slack", false).on_press(move |_: &mut State| {
                    open_url(&link);
                }))
                .child(text("2. In the app\u{2019}s settings, open Install App, install it to your workspace, and press Allow. Copy the Bot User OAuth Token and paste it here.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                .child(token_input(&s.chat_apps.slack_bot_token, p, "xoxb-\u{2026}", "chat-slack-bot-token", "The bot\u{2019}s token from Slack", |s| &mut s.chat_apps.slack_bot_token))
                .child(text("3. Then open Basic Information. Under App-Level Tokens, press Generate Token and Scopes, add the scope connections:write, and press Generate. Paste that token here. Both are kept encrypted on this computer and sent only to Slack.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                .child(
                    row()
                        .w_full()
                        .gap(6.0)
                        .center_y()
                        .child(token_input(&s.chat_apps.slack_app_token, p, "xapp-\u{2026}", "chat-slack-app-token", "The app\u{2019}s own token from Slack", |s| &mut s.chat_apps.slack_app_token))
                        .child(btn(
                            l,
                            "Connect",
                            s.chat_apps.busy
                                || s.chat_apps.slack_bot_token.text.trim().is_empty()
                                || s.chat_apps.slack_app_token.text.trim().is_empty(),
                        ).on_press(|s: &mut State| {
                            let bot = s.chat_apps.slack_bot_token.text.clone();
                            let app_tok = s.chat_apps.slack_app_token.text.clone();
                            s.chat_apps.slack_bot_token = TextEdit::default();
                            s.chat_apps.slack_app_token = TextEdit::default();
                            let next = chat_host::set_token(ChatAppId::Slack, TokenInput::Slack(SlackTokens { bot, app: app_tok }));
                            s.chat_apps.set_settings(ChatAppId::Slack, next);
                        })),
                );
        }
        ChatAppId::Telegram => {
            body = body
                .child(text("1. In Telegram, open @BotFather and send /newbot. Give your bot any name; BotFather answers with its token.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                .child(btn(l, "Open BotFather", false).on_press(|_: &mut State| {
                    open_url("https://t.me/BotFather");
                }))
                .child(text("2. Paste the token here. It is kept encrypted on this computer and sent only to Telegram.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                .child(
                    row()
                        .w_full()
                        .gap(6.0)
                        .center_y()
                        .child(token_input(&s.chat_apps.telegram_token, p, "123456789:AAH\u{2026}", "chat-telegram-token", "Your bot\u{2019}s token from BotFather", |s| &mut s.chat_apps.telegram_token))
                        .child(btn(l, "Connect", s.chat_apps.busy || s.chat_apps.telegram_token.text.trim().is_empty()).on_press(|s: &mut State| {
                            let tok = s.chat_apps.telegram_token.text.clone();
                            s.chat_apps.telegram_token = TextEdit::default();
                            let next = chat_host::set_token(ChatAppId::Telegram, TokenInput::Single(tok));
                            s.chat_apps.set_settings(ChatAppId::Telegram, next);
                        })),
                );
        }
    }
    body
}

fn token_input(
    edit: &TextEdit,
    p: &Palette,
    placeholder: &str,
    key: &str,
    label: &str,
    pick: fn(&mut State) -> &mut TextEdit,
) -> El {
    text_input(edit, type_::BODY)
        .key(key)
        .single_line()
        .label(label)
        .placeholder(placeholder, p.secondary)
        .grow(1.0)
        .h(32.0)
        .padding(6.0)
        .radius(6.0)
        .fill(p.bg)
        .color(p.text)
        .on_edit(pick)
}

fn pairing_steps(
    s: &State,
    p: &Palette,
    l: &crate::look::controls::Look,
    app: ChatAppId,
    state: &ChatAppSettings,
    bot: &locust_core::chat_apps::ChatAppBot,
) -> El {
    let mut body = col().w_full().gap(6.0);
    match app {
        ChatAppId::Discord => {
            let status_line = if state.connected {
                format!("Connected to {}. It reads only its direct messages.", bot.username)
            } else {
                "Connecting to Discord\u{2026}".into()
            };
            body = body
                .child(text(status_line).font(type_::LABEL).color(p.secondary).wrap())
                .child(text("3. Add the bot to a server of yours. If you have none, make one first: in Discord, the + under your servers, then Create My Own.".to_owned()).font(type_::LABEL).color(p.secondary).wrap());
            if let Some(ref invite) = bot.invite {
                let inv = invite.clone();
                body = body.child(btn(l, "Add the bot to a server", false).on_press(move |_: &mut State| {
                    open_url(&inv);
                }));
            }
            body = body.child(text("4. In that server, click the bot in the members list and send it this code in a message. It pairs your account, once, within ten minutes.".to_owned()).font(type_::LABEL).color(p.secondary).wrap());
            if let Some(ref code) = state.code {
                let c = code.code.clone();
                let copied = s.chat_apps.copied == Some(ChatAppId::Discord);
                body = body.child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(text(spaced_code(&c)).font(type_::TITLE).color(p.text))
                        .child(btn(l, if copied { "Copied" } else { "Copy the code" }, false).on_press_tx(move |s: &mut State, tx| {
                            crate::reply_actions::request_copy(tx, c.clone());
                            s.chat_apps.copied = Some(ChatAppId::Discord);
                        })),
                );
            } else {
                body = body.child(btn(l, "Make a pairing code", false).on_press(|s: &mut State| {
                    let next = chat_host::new_link(ChatAppId::Discord);
                    s.chat_apps.set_settings(ChatAppId::Discord, next);
                }));
            }
            let mut btns = row().gap(8.0);
            if let Some(ref open) = bot.open {
                let u = open.clone();
                btns = btns.child(btn(l, "Open the bot in Discord", false).on_press(move |_: &mut State| {
                    open_url(&u);
                }));
            }
            btns = btns.child(btn(l, "Remove the bot", false).on_press(|s: &mut State| {
                let next = chat_host::remove(ChatAppId::Discord);
                s.chat_apps.set_settings(ChatAppId::Discord, next);
            }));
            body = body.child(btns);
        }
        ChatAppId::Slack => {
            let where_team = bot.team.as_deref().unwrap_or("your workspace");
            let status_line = if state.connected {
                format!("Connected to the Locust app in {where_team}.")
            } else {
                "Connecting to Slack\u{2026}".into()
            };
            body = body
                .child(text(status_line).font(type_::LABEL).color(p.secondary).wrap())
                .child(text("4. In Slack, open Locust under Apps in the sidebar, or search for it, and send it this code. It pairs your account, once, within ten minutes.".to_owned()).font(type_::LABEL).color(p.secondary).wrap());
            if let Some(ref code) = state.code {
                let c = code.code.clone();
                let copied = s.chat_apps.copied == Some(ChatAppId::Slack);
                body = body.child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(text(spaced_code(&c)).font(type_::TITLE).color(p.text))
                        .child(btn(l, if copied { "Copied" } else { "Copy the code" }, false).on_press_tx(move |s: &mut State, tx| {
                            crate::reply_actions::request_copy(tx, c.clone());
                            s.chat_apps.copied = Some(ChatAppId::Slack);
                        })),
                );
            } else {
                body = body.child(btn(l, "Make a pairing code", false).on_press(|s: &mut State| {
                    let next = chat_host::new_link(ChatAppId::Slack);
                    s.chat_apps.set_settings(ChatAppId::Slack, next);
                }));
            }
            let mut btns = row().gap(8.0);
            if let Some(ref open) = bot.open {
                let u = open.clone();
                btns = btns.child(btn(l, "Open Locust in Slack", false).on_press(move |_: &mut State| {
                    open_url(&u);
                }));
            }
            btns = btns.child(btn(l, "Remove the app", false).on_press(|s: &mut State| {
                let next = chat_host::remove(ChatAppId::Slack);
                s.chat_apps.set_settings(ChatAppId::Slack, next);
            }));
            body = body.child(btns);
        }
        ChatAppId::Telegram => {
            let status_line = if state.connected {
                format!("Reading @{}\u{2019}s messages.", bot.username)
            } else {
                "Connecting to Telegram\u{2026}".into()
            };
            body = body.child(text(status_line).font(type_::LABEL).color(p.secondary).wrap());
            if let Some(ref link) = state.link {
                let u1 = link.url.clone();
                let u2 = link.url.clone();
                let copied = s.chat_apps.copied == Some(ChatAppId::Telegram);
                body = body
                    .child(text("3. Open the link and press Start. It pairs one chat, once, within ten minutes.".to_owned()).font(type_::LABEL).color(p.secondary).wrap())
                    .child(
                        row()
                            .gap(8.0)
                            .child(btn(l, "Open in Telegram", false).on_press(move |_: &mut State| {
                                open_url(&u1);
                            }))
                            .child(btn(l, if copied { "Copied" } else { "Copy the link" }, false).on_press_tx(move |s: &mut State, tx| {
                                crate::reply_actions::request_copy(tx, u2.clone());
                                s.chat_apps.copied = Some(ChatAppId::Telegram);
                            })),
                    );
            } else {
                body = body.child(btn(l, "Make a pairing link", false).on_press(|s: &mut State| {
                    let next = chat_host::new_link(ChatAppId::Telegram);
                    s.chat_apps.set_settings(ChatAppId::Telegram, next);
                }));
            }
            body = body.child(btn(l, "Remove the bot", false).on_press(|s: &mut State| {
                let next = chat_host::remove(ChatAppId::Telegram);
                s.chat_apps.set_settings(ChatAppId::Telegram, next);
            }));
        }
    }
    body
}

fn paired_steps(
    _s: &State,
    p: &Palette,
    l: &crate::look::controls::Look,
    app: ChatAppId,
    state: &ChatAppSettings,
    bot: &locust_core::chat_apps::ChatAppBot,
    chat: &locust_core::chat_apps::ChatAppChat,
) -> El {
    let who = match &chat.username {
        Some(u) => format!("{} (@{u})", chat.name),
        None => chat.name.clone(),
    };
    let paired_label = match app {
        ChatAppId::Discord => format!("Paired with {}", bot.username),
        ChatAppId::Slack => format!("Paired in {}", bot.team.as_deref().unwrap_or("your workspace")),
        ChatAppId::Telegram => format!("Paired with @{}", bot.username),
    };
    let paired_state = if state.connected {
        paired_label
    } else {
        format!("{paired_label} \u{00b7} not connected now")
    };
    let another_label = if app == ChatAppId::Telegram { "Pair another chat" } else { "Pair another account" };

    let mut body = col().w_full().gap(6.0).child(
        row()
            .w_full()
            .center_y()
            .gap(8.0)
            .child(
                col()
                    .grow(1.0)
                    .gap(2.0)
                    .child(text(who).font(type_::BODY).color(p.text))
                    .child(text(paired_state).font(type_::LABEL).color(p.secondary)),
            )
            .child(btn(l, another_label, false).on_press(move |s: &mut State| {
                let next = chat_host::new_link(app);
                s.chat_apps.set_settings(app, next);
            }))
            .child(btn(l, "Remove", false).on_press(move |s: &mut State| {
                let next = chat_host::remove(app);
                s.chat_apps.set_settings(app, next);
            })),
    );

    if let Some(ref code) = state.code {
        body = body.child(text(format!("Send {} to take {}\u{2019}s place, within ten minutes.", spaced_code(&code.code), chat.name)).font(type_::LABEL).color(p.secondary).wrap());
    } else if let Some(ref link) = state.link {
        let u = link.url.clone();
        body = body.child(btn(l, "Open in Telegram", false).on_press(move |_: &mut State| {
            open_url(&u);
        }));
    }

    let note = match app {
        ChatAppId::Discord => "In your messages with the bot, type / to pick a teammate, or write a name and a colon: \u{201c}Ada: check the build\u{201d}. Discord\u{2019}s servers can read what you and your teammates write there. Locust takes out anything shaped like a key before it sends, but keep secrets out of it. Approval cards there offer Allow once and Deny; Always stays here.".to_owned(),
        ChatAppId::Slack => "In Slack, write to the Locust app with a teammate\u{2019}s name and a colon: \u{201c}Ada: check the build\u{201d}. Reply in an answer\u{2019}s thread to carry on that conversation. Slack\u{2019}s servers can read what you and your teammates write there. Locust takes out anything shaped like a key before it sends, but keep secrets out of it. Approval cards there offer Allow once and Deny; Always stays here.".to_owned(),
        ChatAppId::Telegram => format!("In Telegram, write /name and a message to @{}. Telegram\u{2019}s servers can read what you and your teammates write there: a bot\u{2019}s chat is not end-to-end encrypted. Locust takes out anything shaped like a key before it sends, but keep secrets out of it. Approval cards there offer Allow once and Deny; Always stays here.", bot.username),
    };
    body = body.child(text(note).font(type_::LABEL).color(p.secondary).wrap());

    let alerts_on = state.alerts == Some(true);
    body = body.child(
        row()
            .w_full()
            .center_y()
            .gap(10.0)
            .child(
                col()
                    .grow(1.0)
                    .gap(2.0)
                    .child(text("Tell me here when something needs me".to_owned()).font(type_::BODY).color(p.text))
                    .child(
                        text(format!(
                            "While you are away from this window, the cards that wait on you come here with Allow once and Deny, and so do Scout\u{2019}s urgent finds and a teammate that hits its limit, by your phone\u{2019}s rules: merged, a few a day, held in quiet hours. {}\u{2019}s servers can read who is asking, what the card asks to do and the first line of its file or command, with anything shaped like a key taken out.",
                            app.display_name()
                        ))
                        .font(type_::LABEL)
                        .color(p.secondary)
                        .wrap(),
                    ),
            )
            .child(
                crate::look::controls::switch(l, alerts_on, &format!("Tell me in {} when something needs me", app.display_name()), false)
                    .on_press(move |s: &mut State| {
                        let next = chat_host::set_alerts(app, !alerts_on);
                        s.chat_apps.set_settings(app, next);
                    }),
            ),
    );
    body
}

pub(super) fn scene(s: &mut State) {
    s.demo = true;
    s.appearance = Some(true);
    s.settings_panel.open = true;
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
    s.chat_apps.discord = ChatAppSettings {
        connected: true,
        bot: Some(locust_core::chat_apps::ChatAppBot {
            username: "Locust#4242".into(),
            name: "Locust".into(),
            invite: Some("https://discord.com/oauth2/authorize?client_id=1".into()),
            team: None,
            open: Some("https://discord.com/users/1".into()),
        }),
        code: Some(locust_core::chat_apps::ChatAppCode {
            code: "47291836".into(),
            until: "2026-11-18T04:00:00Z".into(),
        }),
        ..Default::default()
    };
    s.chat_apps.slack = ChatAppSettings {
        connected: true,
        bot: Some(locust_core::chat_apps::ChatAppBot {
            username: "locust".into(),
            name: "locust".into(),
            invite: None,
            team: Some("Tides".into()),
            open: None,
        }),
        chat: Some(locust_core::chat_apps::ChatAppChat {
            name: "Sam".into(),
            username: Some("sam_here".into()),
            paired_at: "2026-11-17T00:00:00Z".into(),
        }),
        alerts: Some(true),
        ..Default::default()
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::chat_apps::{ChatAppBot, ChatAppChat, ChatAppCode, ChatAppLink};

    #[test]
    fn chat_status_matches_electron_words() {
        assert_eq!(chat_status(&ChatAppSettings::default()), "Off");
        let bot = ChatAppBot { username: "Locust#4242".into(), name: "Locust".into(), invite: None, team: None, open: None };
        assert_eq!(
            chat_status(&ChatAppSettings { bot: Some(bot.clone()), connected: false, ..Default::default() }),
            "Connecting\u{2026}"
        );
        assert_eq!(
            chat_status(&ChatAppSettings { bot: Some(bot.clone()), connected: true, ..Default::default() }),
            "Not paired yet"
        );
        let chat = ChatAppChat { name: "Sam".into(), username: Some("sam_here".into()), paired_at: "2026-11-17T00:00:00Z".into() };
        assert_eq!(
            chat_status(&ChatAppSettings { bot: Some(bot.clone()), chat: Some(chat.clone()), connected: true, ..Default::default() }),
            "On \u{00b7} Sam"
        );
        assert_eq!(
            chat_status(&ChatAppSettings { bot: Some(bot), chat: Some(chat), connected: false, ..Default::default() }),
            "On \u{00b7} Sam \u{00b7} not connected now"
        );
    }

    #[test]
    fn builds_section_in_discord_slack_telegram_order_and_all_states() {
        let mut s = State::default();
        s.chat_apps.discord = ChatAppSettings {
            connected: true,
            bot: Some(ChatAppBot { username: "Locust#4242".into(), name: "Locust".into(), invite: Some("https://discord.com".into()), team: None, open: None }),
            code: Some(ChatAppCode { code: "47291836".into(), until: "2026-11-17T01:00:00Z".into() }),
            ..Default::default()
        };
        s.chat_apps.slack = ChatAppSettings {
            connected: true,
            bot: Some(ChatAppBot { username: "locust".into(), name: "locust".into(), invite: None, team: Some("Tides".into()), open: None }),
            chat: Some(ChatAppChat { name: "Sam".into(), username: Some("sam".into()), paired_at: "2026-11-17T00:00:00Z".into() }),
            alerts: Some(true),
            ..Default::default()
        };
        s.chat_apps.telegram = ChatAppSettings {
            connected: true,
            bot: Some(ChatAppBot { username: "tides_bot".into(), name: "Tides".into(), invite: None, team: None, open: None }),
            link: Some(ChatAppLink { url: "https://t.me/tides_bot?start=abc".into(), until: "2026-11-17T01:00:00Z".into() }),
            ..Default::default()
        };
        let p = Palette::new(true);
        let _el = section(&s, &p);
    }
}
