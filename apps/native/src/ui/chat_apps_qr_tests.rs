//! The Telegram pairing link as a code a phone's camera reads (TelegramSettings.tsx, PhoneSettings.tsx `QrCode`),
//! drawn headless through `chat_apps_section::section` with no bot behind it: the QR / link step only, apart from
//! setting up a live bot.
use super::*;
use locust_core::chat_apps::{ChatAppBot, ChatAppChat, ChatAppLink};
use rune::{Cx, Env, Runtime};

const LATER: &str = "2099-01-01T00:00:00Z";
const URL: &str = "https://t.me/tides_bot?start=abcdefghijklmnop";

fn view(s: &State, _: &mut Cx<State>) -> El {
    col().w_full().child(section(s, &Palette::new(true)))
}

struct Drawn {
    words: String,
    /// The label of each picture drawn: a QR code is one, labelled for a screen reader.
    pictures: Vec<String>,
    /// The squares of the first code drawn, as a phone's camera reads them: dark or not, row by row.
    squares: Vec<Vec<bool>>,
}

fn drawn(set: impl FnOnce(&mut ChatAppsState)) -> Drawn {
    let mut s = State::default();
    set(&mut s.chat_apps);
    let mut rt = Runtime::new(s, view, (900.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let code = all.iter().position(|e| e.role == "img");
    let mut squares = Vec::new();
    if let Some(at) = code {
        // The code's runs: each row drawn as a line 4 points high holding its runs, a run filled black dark and one
        // left white light. The line itself is the code's whole width; the runs are narrower.
        let [fx, fy, fw, fh] = all[at].rect;
        let width = fw - 2.0 * 16.0;
        let mut rows: Vec<(f32, Vec<(f32, f32, bool)>)> = Vec::new();
        for e in &all[at + 1..] {
            let [x, y, w, h] = e.rect;
            let inside = x >= fx && y >= fy && x + w <= fx + fw + 0.5 && y + h <= fy + fh + 0.5;
            if !inside || (h - 4.0).abs() > 0.01 || w >= width - 0.5 {
                continue;
            }
            let dark = e.fill == rune::Color::BLACK;
            match rows.iter_mut().find(|(row_y, _)| (*row_y - y).abs() < 0.01) {
                Some((_, runs)) => runs.push((x, w, dark)),
                None => rows.push((y, vec![(x, w, dark)])),
            }
        }
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (_, mut runs) in rows {
            runs.sort_by(|a, b| a.0.total_cmp(&b.0));
            squares.push(runs.into_iter().flat_map(|(_, w, dark)| std::iter::repeat_n(dark, (w / 4.0).round() as usize)).collect());
        }
    }
    Drawn {
        words: all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n"),
        pictures: all.iter().filter(|e| e.role == "img").filter_map(|e| e.label.clone()).collect(),
        squares,
    }
}

fn bot() -> ChatAppBot {
    ChatAppBot { username: "tides_bot".into(), name: "Tides".into(), invite: None, team: None, open: None }
}

// case: walks Telegram from BotFather to the link that pairs a chat
#[test]
fn the_telegram_pairing_link_is_also_a_code_for_a_phones_camera_and_none_shows_before_there_is_a_link() {
    // Off, with the token step open: no code.
    let off = drawn(|c| c.telegram_open = true);
    assert!(off.words.contains("Open BotFather"));
    assert!(off.pictures.is_empty(), "{:?}", off.pictures);

    // A bot kept and no link yet: no code either, only the button that makes a link.
    let no_link = drawn(|c| c.telegram = ChatAppSettings { connected: true, bot: Some(bot()), ..Default::default() });
    assert!(no_link.words.contains("Make a pairing link"));
    assert!(no_link.pictures.is_empty(), "{:?}", no_link.pictures);

    // Pairing: the link as a code, with the step that says to scan it, and the link's own buttons.
    let pairing = drawn(|c| {
        c.telegram = ChatAppSettings {
            connected: true,
            bot: Some(bot()),
            link: Some(ChatAppLink { url: URL.into(), until: LATER.into() }),
            ..Default::default()
        }
    });
    assert_eq!(pairing.pictures, ["A code for your phone\u{2019}s camera. It opens your bot in Telegram to pair your chat."]);
    for words in [
        "3. Scan the code with your phone, or open the link here, and press Start. It pairs one chat, once, within ten minutes.",
        "Open in Telegram",
        "Copy the link",
        "Reading @tides_bot\u{2019}s messages.",
    ] {
        assert!(pairing.words.contains(words), "{words} in {}", pairing.words);
    }
    // The code drawn is the link's own, square for square (locust_core::qr, PhoneSettings.tsx's generator).
    let code = locust_core::qr::qr_code(URL).expect("a code for the link");
    let expected: Vec<Vec<bool>> = (0..code.size).map(|r| (0..code.size).map(|c| code.is_dark(r, c)).collect()).collect();
    assert_eq!(pairing.squares, expected, "the squares drawn are the link's code");

    // Paired, and pairing another chat: a code for the new link, saying whose place it takes.
    let again = drawn(|c| {
        c.telegram = ChatAppSettings {
            connected: true,
            bot: Some(bot()),
            chat: Some(ChatAppChat { name: "Sam".into(), username: Some("sam_here".into()), paired_at: "2026-11-18T00:00:00Z".into() }),
            link: Some(ChatAppLink { url: URL.into(), until: LATER.into() }),
            ..Default::default()
        }
    });
    assert_eq!(again.pictures, ["A code for your phone\u{2019}s camera. It pairs another Telegram chat in place of this one."]);
    assert!(again.words.contains("The chat that presses Start with this link takes the place of Sam\u{2019}s, within ten minutes."), "{}", again.words);
    assert!(again.words.contains("Open in Telegram"));

    // Paired with no new link: no code.
    let paired = drawn(|c| {
        c.telegram = ChatAppSettings {
            connected: true,
            bot: Some(bot()),
            chat: Some(ChatAppChat { name: "Sam".into(), username: Some("sam_here".into()), paired_at: "2026-11-18T00:00:00Z".into() }),
            ..Default::default()
        }
    });
    assert!(paired.pictures.is_empty(), "{:?}", paired.pictures);
}
