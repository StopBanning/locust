//! Electron `what-a-teammate-published-is-a-card.test.tsx` (inline published-link cards, R25 / B15),
//! drawn headless through `reply_actions::reply_in_thread`, which calls `published_cards::cards`.
//! Also includes regression checks for already-wired copy, save-copy and Markdown file-reader actions.
use super::*;
use locust_core::published_links::{NOT_OPENED, published_in, published_of};
use rune::{Env, Runtime};

thread_local! {
    static REPLY_BODY: std::cell::Cell<&'static str> = const { std::cell::Cell::new("") };
}

fn draw_reply(body: &'static str) -> Runtime<State> {
    REPLY_BODY.with(|b| b.set(body));
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    let mut rt = Runtime::new(
        s,
        |s, _| {
            let body = REPLY_BODY.with(|b| b.get());
            crate::reply_actions::reply_in_thread(s, body, false, true, &Palette::new(false))
        },
        (900.0, 900.0),
        Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn texts_under<T>(rt: &Runtime<T>, key: &str) -> Vec<String> {
    let els = rt.elements();
    let Some(root) = els.iter().find(|e| e.key.as_deref() == Some(key)) else {
        return Vec::new();
    };
    let prefix = format!("{}/", root.path);
    els.iter()
        .filter(|e| !e.exiting && (e.path == root.path || e.path.starts_with(&prefix)))
        .filter_map(|e| e.text.clone())
        .collect()
}

fn press<T: 'static>(rt: &mut Runtime<T>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("element {key} not drawn"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
}

// case: draws a card each, with Open, and nothing for a reply that names none
#[test]
fn draws_a_card_each_with_open_and_nothing_for_a_reply_that_names_none() {
    // Visible before fixture: a reply naming only an ordinary URL draws no published card.
    let before = draw_reply("Nothing to see at https://example.com");
    assert!(before.find("published").is_none(), "no card container when reply names no published link");
    assert!(before.find("published-0").is_none(), "no card when reply names no published link");

    // Visible after fixture: a reply naming a Claude artifact draws a card with its title, kind · host,
    // full URL hover (Electron PublishedCards.tsx title={item.url}), and Open.
    let url = "https://claude.ai/artifact/pricing-page-AbCdEfGhIjKlMnOpQrStUv";
    let mut after = draw_reply("The page: https://claude.ai/artifact/pricing-page-AbCdEfGhIjKlMnOpQrStUv");
    let group = after
        .elements()
        .into_iter()
        .find(|e| !e.exiting && e.key.as_deref() == Some("published"))
        .expect("published card container drawn");
    assert_eq!(group.label.as_deref(), Some("What this reply links to"));

    let card_el = after
        .elements()
        .into_iter()
        .find(|e| !e.exiting && e.key.as_deref() == Some("published-0"))
        .expect("published-0 card drawn");
    assert_eq!(card_el.role, "button");
    assert_eq!(card_el.label.as_deref(), Some(url));
    assert_eq!(card_el.help.as_deref(), Some(url));

    let card_texts = texts_under(&after, "published-0");
    assert!(card_texts.iter().any(|t| t == "Pricing page"), "{card_texts:?}");
    assert!(card_texts.iter().any(|t| t == "Claude artifact \u{b7} claude.ai"), "{card_texts:?}");
    assert!(card_texts.iter().any(|t| t == "Open"), "{card_texts:?}");

    // Pressing the card opens the link through open_web_link.
    press(&mut after, "published-0");
    assert_eq!(after.state.opened_links, vec![url.to_owned()]);

    // A refusal for that card's URL is shown beside it with role="status".
    after.update(move |s, _| {
        s.link_refusal = Some((url.to_owned(), NOT_OPENED.into()));
    });
    after.frame(0.016);
    assert!(
        after
            .elements()
            .into_iter()
            .any(|e| !e.exiting && e.role == "status" && e.text.as_deref() == Some(NOT_OPENED)),
        "refusal shown beside the card"
    );
}

// case: is still a card, named as written, and drawing it throws nothing
#[test]
fn is_still_a_card_named_as_written_and_drawing_it_throws_nothing() {
    let notion = published_of("https://www.notion.so/Launch-plan-100%-done", None).expect("notion card");
    assert_eq!((notion.kind, notion.title.as_str()), ("Notion page", "Launch plan 100% done"));
    let figma = published_of("https://www.figma.com/design/AbC123/Cart-%C3", None).expect("figma card");
    assert_eq!((figma.kind, figma.title.as_str()), ("Figma file", "Cart %C3"));

    let reply = "Done: https://www.notion.so/Launch-plan-100%-done";
    assert_eq!(published_in(reply).len(), 1);

    // Visible before/after fixture: plain text has no card; stray-% Notion URL draws its card without panicking.
    let before = draw_reply("Working on it.");
    assert!(before.find("published").is_none());

    let after = draw_reply(reply);
    let card_el = after
        .elements()
        .into_iter()
        .find(|e| !e.exiting && e.key.as_deref() == Some("published-0"))
        .expect("stray-% card drawn");
    assert_eq!(card_el.help.as_deref(), Some("https://www.notion.so/Launch-plan-100%-done"));

    let card_texts = texts_under(&after, "published-0");
    assert!(card_texts.iter().any(|t| t == "Launch plan 100% done"), "{card_texts:?}");
    assert!(card_texts.iter().any(|t| t == "Notion page \u{b7} www.notion.so"), "{card_texts:?}");
    assert!(card_texts.iter().any(|t| t == "Open"), "{card_texts:?}");
}

// Regression check (B15): already-wired copy, Save a copy, and Markdown rendering stay intact.
#[test]
fn b15_regression_copy_save_and_markdown_actions_stay_wired() {
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    s.teammates.push(Teammate {
        id: "tm".into(),
        name: "Wren".into(),
        role: String::new(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some("/work".into()),
    });
    s.conversations.insert("tm".into(), Default::default());
    s.file_view = Some((
        crate::reply_actions::FileTarget {
            path: "/work/docs/plan.md".into(),
            start: 0,
            end: 0,
        },
        Ok("# Plan\n\nShip on Tuesday.\n".into()),
    ));
    let mut rt = Runtime::new(
        s,
        |s, _| crate::reply_actions::viewer_on(s, &Palette::new(false), "darwin", &[]).unwrap_or_else(col),
        (900.0, 1200.0),
        Env::new(false, 1.0),
    );
    rt.frame(0.0);
    // Markdown renders as prose, Save a copy is present, and Copy path copies the path.
    assert!(rt.find("file-reader/prose").is_some(), "Markdown renders as prose");
    assert!(rt.find("file-reader/save").is_some(), "Save a copy control is wired");
    assert!(rt.find("file-reader/menu/CopyPath").is_some(), "Copy path control is wired");
    press(&mut rt, "file-reader/menu/CopyPath");
    assert_eq!(rt.copy_request.as_deref(), Some("/work/docs/plan.md"));
}
