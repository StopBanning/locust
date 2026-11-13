//! The share block: how a teammate's run says "another teammate needs this" (Electron `shared/peer-share.ts`). A
//! one-prompt, one-transcript runtime has no side channel, so the decision travels in the transcript as a tag the host
//! finds without guessing: `<locust-share to="Name">text</locust-share>`. Only the host reads these: it checks the name
//! against the roster, bounds the text, attributes it, and posts it to the workroom. Nothing in a block grants
//! anything; it is a claim from one agent to another, shown as one.

use crate::invariants::regex;
use crate::protocol_tags::{blocks_outside_code, defang_protocol_blocks, strip_blocks_outside_code};

pub const SHARE_TAG: &str = "locust-share";
/// About 1,000 words: a brief a colleague can act on, still whole beside the briefing.
pub const MAX_SHARE_TEXT_LENGTH: usize = 6_000;
pub const MAX_SHARES_PER_MISSION: usize = 4;

const BLOCK: &str = r"<locust-share\s+([^<>]{1,200}?)\s*>([\s\S]*?)</locust-share>";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareBlock {
    pub to: String,
    pub text: String,
    /// `when="later"`: telling, not asking; the message waits for the recipient's next run.
    pub defer: bool,
    /// `when="now"`: asks to be taken before the recipient finishes. A request; the person's switch decides.
    pub urgent: bool,
    /// `wants="answer"`, or a message that ends by asking.
    pub wants_answer: bool,
}

/// One attribute of an open tag, in either quoting style, any order.
fn attributes_of(open: &str) -> std::collections::HashMap<String, String> {
    regex(r#"(?i)([a-z-]{1,20})=(?:"([^"<>\n]{0,60})"|'([^'<>\n]{0,60})')"#)
        .captures_iter(open)
        .map(|c| (c[1].to_ascii_lowercase(), c.get(2).or_else(|| c.get(3)).map_or("", |m| m.as_str()).to_owned()))
        .collect()
}

/// `Gem (Custom)` to `Gem`: the roster prints the role; the parser wants the name. Only a trailing role goes.
fn without_role(name: &str) -> String {
    regex(r"\s*\([^()]*\)\s*$").replace(name, "").trim().to_owned()
}

/// Complete, well-formed blocks in transcript order, none inside code. Empty bodies are dropped.
pub fn parse_share_blocks(text: &str) -> Vec<ShareBlock> {
    let block = regex(BLOCK);
    blocks_outside_code(text, &block)
        .into_iter()
        .filter_map(|(start, _, _)| {
            let c = block.captures_at(text, start)?;
            let attributes = attributes_of(&c[1]);
            let to = without_role(attributes.get("to").map_or("", String::as_str));
            let body = c[2].trim().to_owned();
            if to.is_empty() || body.is_empty() {
                return None;
            }
            // Exactly one word means it: `when="soon"` gets the ordinary treatment.
            let when = attributes.get("when").map(|w| w.trim().to_ascii_lowercase()).unwrap_or_default();
            let wants_answer = attributes.get("wants").is_some_and(|w| w.trim().eq_ignore_ascii_case("answer")) || body.trim_end().ends_with('?');
            Some(ShareBlock { to, text: body, defer: when == "later", urgent: when == "now", wants_answer })
        })
        .collect()
}

/// The transcript without its share blocks: what was shared shows in the peer card, never twice.
pub fn strip_share_blocks(text: &str) -> String {
    let stripped = strip_blocks_outside_code(text, &regex(BLOCK));
    regex(r"\n{3,}").replace_all(&stripped, "\n\n").trim_end().to_owned()
}

/// A received message quoted into another agent's prompt cannot carry a block the recipient could echo back and have
/// acted on under its own name: every tag the host parses is defanged.
pub fn sanitize_inbound(text: &str) -> String {
    defang_protocol_blocks(text)
}

/// Text as the workroom takes it: stray control characters removed rather than refusing the whole share, and a hard
/// cut with a marker, never a silent one.
pub fn bounded_share_text(text: &str) -> String {
    let clean: String = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|c| !matches!(*c as u32, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f))
        .collect::<String>()
        .trim()
        .to_owned();
    let units: Vec<u16> = clean.encode_utf16().collect();
    if units.len() <= MAX_SHARE_TEXT_LENGTH {
        return clean;
    }
    format!("{}\u{2026}", String::from_utf16_lossy(&units[..MAX_SHARE_TEXT_LENGTH - 1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = "Watch the to= attribute.";
    fn plain(to: &str) -> ShareBlock {
        ShareBlock { to: to.into(), text: BODY.into(), defer: false, urgent: false, wants_answer: false }
    }

    // Electron's shared/peer-share.test.ts.
    #[test]
    fn reads_the_ordinary_one_single_quotes_and_the_roster_line_as_printed() {
        assert_eq!(parse_share_blocks(&format!("<locust-share to=\"Gem\">{BODY}</locust-share>")), vec![plain("Gem")]);
        assert_eq!(parse_share_blocks(&format!("<locust-share to='Gem'>{BODY}</locust-share>")), vec![plain("Gem")]);
        assert_eq!(parse_share_blocks(&format!("<locust-share to=\"Gem (Custom)\">{BODY}</locust-share>")), vec![plain("Gem")]);
        assert_eq!(parse_share_blocks(&format!("<locust-share to='Wren (Code & Migrations)'>{BODY}</locust-share>")), vec![plain("Wren")]);
    }

    #[test]
    fn keeps_a_name_with_brackets_in_the_middle_and_drops_a_block_without_recipient_or_body() {
        assert_eq!(parse_share_blocks(&format!("<locust-share to=\"Gem (2) Prime\">{BODY}</locust-share>"))[0].to, "Gem (2) Prime");
        assert!(parse_share_blocks("<locust-share to=\"\">hello</locust-share>").is_empty());
        assert!(parse_share_blocks("<locust-share to=\"Gem\"></locust-share>").is_empty());
    }

    #[test]
    fn a_share_that_asks_to_be_taken_now_is_that_one_word_in_either_order() {
        assert!(parse_share_blocks(&format!("<locust-share to=\"Gem\" when=\"now\">{BODY}</locust-share>"))[0].urgent);
        let other_order = &parse_share_blocks(&format!("<locust-share when='now' to='Gem'>{BODY}</locust-share>"))[0];
        assert!(other_order.urgent && other_order.to == "Gem");
        for when in ["soon", "urgent", "asap", "immediately", ""] {
            assert!(!parse_share_blocks(&format!("<locust-share to=\"Gem\" when=\"{when}\">{BODY}</locust-share>"))[0].urgent, "{when}");
        }
        assert!(parse_share_blocks(&format!("<locust-share when=\"now\">{BODY}</locust-share>")).is_empty());
        assert!(parse_share_blocks(&format!("<locust-share to=\"Gem\" when=\"later\">{BODY}</locust-share>"))[0].defer);
    }

    #[test]
    fn strips_every_form_it_can_read_so_nothing_is_shown_twice() {
        for tag in ["to=\"Gem\"", "to='Gem'", "to=\"Gem (Custom)\"", "to=\"Gem\" when=\"now\""] {
            assert_eq!(strip_share_blocks(&format!("Here is the answer.\n<locust-share {tag}>{BODY}</locust-share>")), "Here is the answer.", "{tag}");
        }
    }

    #[test]
    fn a_block_inside_code_is_code_and_a_question_wants_an_answer() {
        assert!(parse_share_blocks("Use it like `<locust-share to=\"Gem\">hi</locust-share>`.").is_empty());
        assert!(parse_share_blocks("<locust-share to=\"Gem\">Which port does it use?</locust-share>")[0].wants_answer);
        assert!(parse_share_blocks("<locust-share to=\"Gem\" wants=\"answer\">Port notes.</locust-share>")[0].wants_answer);
    }

    #[test]
    fn bounds_text_with_a_marker_and_defangs_quoted_tags() {
        assert_eq!(bounded_share_text("  a\u{7}b\r\nc  "), "ab\nc");
        let long = bounded_share_text(&"x".repeat(MAX_SHARE_TEXT_LENGTH + 10));
        assert_eq!(long.encode_utf16().count(), MAX_SHARE_TEXT_LENGTH);
        assert!(long.ends_with('\u{2026}'));
        assert!(!sanitize_inbound("<locust-share to=\"Gem\">x</locust-share>").contains("<locust-share"));
    }
}
