//! a-runtime-mark-wears-its-colour-as-a-token.test.ts (4 cases): the marks' colours are tokens; Claude's mark alone is
//! painted by runtime (Gemini's gradient is in its drawing); an account's usage bars are quiet, amber from 80% and red
//! when spent; and a mark whose runtime cannot take work greys with its words at half strength (fixed 20 Nov: Rust
//! greyed it but drew it at full strength). Colours read from the drawn icon (`text_color`).
use super::*;

fn drawn(runtime: &'static str, muted: bool) -> rune::ElementInfo {
    let mut rt = rune::Runtime::new(
        (runtime, muted),
        |(runtime, muted), _| {
            let l = Look::new(true);
            row().children(runtime_mark(&l, runtime, 16.0, l.c("text-secondary"), None, *muted).map(|m| m.key("mark")))
        },
        (200.0, 100.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("mark")).expect("a mark is drawn")
}

// case: is a token, for every mark whose colour is part of the mark
#[test]
fn each_marks_colour_is_a_token() {
    for declared in ["--lc-mark-claude: #d97757;", "--lc-mark-gemini-violet: #9168c0;", "--lc-mark-gemini-blue: #5684d1;", "--lc-mark-gemini-cyan: #1ba1e3;"] {
        assert!(super::super::TOKENS_CSS.contains(declared), "{declared}");
    }
    assert!(Look::new(true).c("mark-claude").a > 0.99, "the token resolves to a colour");
}

// case: is painted by runtime for Claude alone: Gemini carries its gradient in the mark
#[test]
fn claude_alone_is_painted_by_runtime() {
    let l = Look::new(true);
    assert_eq!(drawn("claude", false).text_color, Some(l.c("mark-claude")));
    for other in ["codex", "cursor", "opencode", "gemini"] {
        if crate::look::controls::mark_drawn(other, false).is_some() {
            assert_eq!(drawn(other, false).text_color, Some(l.c("text-secondary")), "{other} wears its words' colour");
        }
    }
}

// case: draws an account's usage bars in the app's own tones: quiet, amber from 80%, red when spent (0.389)
#[test]
fn usage_bars_are_quiet_amber_from_80_and_red_when_spent() {
    use locust_core::agent_card::tone_of;
    let l = Look::new(true);
    assert_eq!(crate::agent_card_ui::bar_colour(&l, tone_of(79)), l.c("text-secondary"));
    assert_eq!(crate::agent_card_ui::bar_colour(&l, tone_of(80)), l.c("amber"));
    assert_eq!(crate::agent_card_ui::bar_colour(&l, tone_of(100)), l.c("red"));
}

// case: goes grey with its words when the runtime cannot take work, brand colour and all
#[test]
fn a_muted_mark_greys_with_its_words_at_half_strength() {
    let l = Look::new(true);
    let muted = drawn("claude", true);
    assert_eq!(muted.text_color, Some(l.c("text-secondary")), "Claude's orange gives way to the words' colour");
    assert!((muted.opacity - 0.5).abs() < 0.01, "at half strength: {}", muted.opacity);
    assert!((drawn("claude", false).opacity - 1.0).abs() < 0.01);
}
