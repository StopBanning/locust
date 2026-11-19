//! chat-apps-say-each-step-in-settings.test.tsx, drawn headless through `chat_apps_section::section`: Discord, Slack
//! and Telegram in that order, each off with Set up; each app's steps from where its token is made to the code or link
//! that pairs, the token in a field that never shows it; who is paired and where the words go; and, once paired, the
//! switch that tells you there when something needs you.
use super::*;
use locust_core::chat_apps::{ChatAppBot, ChatAppChat, ChatAppCode, ChatAppLink};
use rune::{Cx, Env, Runtime};

const LATER: &str = "2099-01-01T00:00:00Z";

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(section(s, &Palette::new(true)))
}

struct Drawn {
    words: String,
    in_order: Vec<String>,
    secure_fields: Vec<String>,
    switches: Vec<(String, bool)>,
}

fn drawn(set: impl FnOnce(&mut ChatAppsState)) -> Drawn {
    let mut s = State::default();
    set(&mut s.chat_apps);
    let mut rt = Runtime::new(s, view, (900.0, 4000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let look = crate::look::controls::Look::new(true);
    Drawn {
        words: all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n"),
        in_order: all.iter().filter_map(|e| e.text.clone()).collect(),
        secure_fields: all.iter().filter(|e| e.role == "textbox" || e.key.as_deref().is_some_and(|k| k.ends_with("-token"))).filter_map(|e| e.label.clone()).collect(),
        switches: all.iter().filter(|e| e.role == "switch").filter_map(|e| e.label.clone().map(|l| (l, e.fill == look.c("switch-on")))).collect(),
    }
}

fn discord_bot() -> ChatAppBot {
    ChatAppBot {
        username: "Locust#4242".into(),
        name: "Locust".into(),
        invite: Some("https://discord.com/oauth2/authorize?client_id=1".into()),
        team: None,
        open: Some("https://discord.com/users/1".into()),
    }
}

fn sam(username: &str) -> ChatAppChat {
    ChatAppChat { name: "Sam".into(), username: Some(username.into()), paired_at: "2026-11-18T00:00:00Z".into() }
}

#[test]
fn names_discord_slack_and_telegram_in_that_order_each_off_with_set_up() {
    let d = drawn(|_| {});
    assert!(d.words.contains("Write to your teammates from Discord, Slack or Telegram"), "{}", d.words);
    let at = |name: &str| d.in_order.iter().position(|t| t == name).unwrap_or_else(|| panic!("{name} not drawn"));
    assert!(at("Discord") < at("Slack") && at("Slack") < at("Telegram"));
    assert_eq!(d.in_order.iter().filter(|t| *t == "Set up").count(), 3);
    assert!(d.secure_fields.is_empty(), "off and not opened, no steps show: {:?}", d.secure_fields);
}

#[test]
fn walks_discord_from_the_developer_portal_to_the_code_sent_to_the_bot() {
    let off = drawn(|c| c.discord_open = true);
    assert!(off.words.contains("Open the Developer Portal"));
    assert!(off.secure_fields.iter().any(|l| l == "Your bot\u{2019}s token from Discord"), "{:?}", off.secure_fields);
    assert!(off.words.contains("kept encrypted on this computer and sent only to Discord"));

    let pairing = drawn(|c| {
        c.discord = ChatAppSettings {
            connected: true,
            bot: Some(discord_bot()),
            code: Some(ChatAppCode { code: "47291836".into(), until: LATER.into() }),
            ..Default::default()
        }
    });
    for words in [
        "Not paired yet",
        "Connected to Locust#4242. It reads only its direct messages.",
        "Add the bot to a server",
        "4729 1836",
        "Copy the code",
        "Open the bot in Discord",
    ] {
        assert!(pairing.words.contains(words), "{words} in {}", pairing.words);
    }
    assert!(pairing.secure_fields.is_empty());

    let paired = drawn(|c| {
        c.discord = ChatAppSettings {
            connected: true,
            bot: Some(ChatAppBot { invite: None, open: None, ..discord_bot() }),
            chat: Some(sam("sam_here")),
            ..Default::default()
        }
    });
    for words in [
        "On \u{00b7} Sam",
        "Sam (@sam_here)",
        "Pair another account",
        "type / to pick a teammate",
        "Discord\u{2019}s servers can read what you and your teammates write there",
        "Approval cards there offer Allow once and Deny; Always stays here.",
    ] {
        assert!(paired.words.contains(words), "{words} in {}", paired.words);
    }
}

#[test]
fn walks_slack_from_the_app_it_makes_in_one_step_to_its_two_tokens_and_the_code() {
    let off = drawn(|c| c.slack_open = true);
    assert!(off.words.contains("Create the app in Slack"));
    for label in ["The bot\u{2019}s token from Slack", "The app\u{2019}s own token from Slack"] {
        assert!(off.secure_fields.iter().any(|l| l == label), "{label} in {:?}", off.secure_fields);
    }
    assert!(off.words.contains("connections:write"), "{}", off.words);
    assert!(locust_core::chat_slack_fmt::slack_manifest_link().starts_with("https://api.slack.com/apps?new_app=1&manifest_json="));

    let tides = |open: Option<&str>| ChatAppBot {
        username: "locust".into(),
        name: "locust".into(),
        invite: None,
        team: Some("Tides".into()),
        open: open.map(str::to_owned),
    };
    let pairing = drawn(|c| {
        c.slack = ChatAppSettings {
            connected: true,
            bot: Some(tides(Some("https://slack.com/app_redirect?app=A1&team=T1"))),
            code: Some(ChatAppCode { code: "12345678".into(), until: LATER.into() }),
            ..Default::default()
        }
    });
    for words in ["Connected to the Locust app in Tides.", "1234 5678", "Open Locust in Slack"] {
        assert!(pairing.words.contains(words), "{words} in {}", pairing.words);
    }

    let paired = drawn(|c| {
        c.slack = ChatAppSettings {
            connected: false,
            bot: Some(tides(None)),
            chat: Some(sam("sam")),
            message: Some("Slack could not be reached. Locust keeps trying.".into()),
            ..Default::default()
        }
    });
    for words in ["Paired in Tides \u{00b7} not connected now", "Slack could not be reached. Locust keeps trying.", "\u{201c}Ada: check the build\u{201d}"] {
        assert!(paired.words.contains(words), "{words} in {}", paired.words);
    }
}

#[test]
fn walks_telegram_from_botfather_to_the_link_that_pairs_a_chat() {
    let off = drawn(|c| c.telegram_open = true);
    assert!(off.words.contains("Open BotFather"));
    assert!(off.secure_fields.iter().any(|l| l == "Your bot\u{2019}s token from BotFather"), "{:?}", off.secure_fields);
    assert!(off.words.contains("kept encrypted on this computer and sent only to Telegram"));

    let bot = ChatAppBot { username: "tides_bot".into(), name: "Tides".into(), invite: None, team: None, open: None };
    let pairing = drawn(|c| {
        c.telegram = ChatAppSettings {
            connected: true,
            bot: Some(bot.clone()),
            link: Some(ChatAppLink { url: "https://t.me/tides_bot?start=abcdefghijklmnop".into(), until: LATER.into() }),
            ..Default::default()
        }
    });
    for words in ["Open in Telegram", "Copy the link", "Reading @tides_bot\u{2019}s messages."] {
        assert!(pairing.words.contains(words), "{words} in {}", pairing.words);
    }

    let paired = drawn(|c| c.telegram = ChatAppSettings { connected: true, bot: Some(bot.clone()), chat: Some(sam("sam_here")), ..Default::default() });
    for words in ["Sam (@sam_here)", "Pair another chat", "Telegram\u{2019}s servers can read what you and your teammates write there"] {
        assert!(paired.words.contains(words), "{words} in {}", paired.words);
    }
}

#[test]
fn offers_once_paired_to_tell_you_there_when_something_needs_you_off_until_turned_on_saying_what_is_sent_0_837() {
    let off = drawn(|c| c.discord = ChatAppSettings { connected: true, bot: Some(discord_bot()), chat: Some(sam("sam_here")), ..Default::default() });
    assert!(off.switches.contains(&("Tell me in Discord when something needs me".to_owned(), false)), "{:?}", off.switches);
    assert!(off.words.contains("Discord\u{2019}s servers can read who is asking, what the card asks to do and the first line of its file or command"));
    assert!(off.words.contains("by your phone\u{2019}s rules: merged, a few a day, held in quiet hours"));

    let tides = ChatAppBot { username: "locust".into(), name: "locust".into(), invite: None, team: Some("Tides".into()), open: None };
    let on = drawn(|c| c.slack = ChatAppSettings { connected: true, bot: Some(tides), chat: Some(sam("sam_here")), alerts: Some(true), ..Default::default() });
    assert!(on.switches.contains(&("Tell me in Slack when something needs me".to_owned(), true)), "{:?}", on.switches);

    let bot = ChatAppBot { username: "tides_bot".into(), name: "Tides".into(), invite: None, team: None, open: None };
    let telegram = drawn(|c| c.telegram = ChatAppSettings { connected: true, bot: Some(bot), chat: Some(sam("sam_here")), ..Default::default() });
    assert!(telegram.switches.iter().any(|(l, _)| l == "Tell me in Telegram when something needs me"));

    // Not paired yet, there is nobody to tell.
    let unpaired = drawn(|c| c.discord = ChatAppSettings { connected: true, bot: Some(discord_bot()), ..Default::default() });
    assert!(!unpaired.words.contains("when something needs me"));
    assert!(!unpaired.switches.iter().any(|(l, _)| l.contains("when something needs me")));
}

#[test]
fn a_token_typed_into_its_field_is_never_drawn() {
    let typed = "MTIzNDU2Nzg5.never-on-screen.abc";
    let d = drawn(|c| {
        c.discord_open = true;
        c.discord_token.replace_text(typed);
    });
    assert!(!d.words.contains(typed), "{}", d.words);
    assert!(!d.words.contains("never-on-screen"));
}
