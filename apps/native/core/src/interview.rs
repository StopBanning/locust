//! A NEW TEAMMATE INTERVIEWS YOU (`shared/interview.ts`, `InstructionsCard.tsx`).
//!
//! Saved with "Let it interview you", the teammate's first conversation opens with the person's ask to be interviewed,
//! in Ask for that first turn. The teammate asks a few short questions one at a time, then writes its standing
//! instructions in a `<locust-instructions>` block. Nothing is kept until the person presses Save, and a block
//! anywhere outside an interview of the person's own making is only words.

use regex::Regex;
use std::sync::OnceLock;

pub const INSTRUCTIONS_TAG: &str = "locust-instructions";
pub const INTERVIEW_OPENING: &str =
    "Interview me so you can write your own standing instructions.";
pub const MAX_TEAMMATE_INSTRUCTIONS: usize = 4000;

/// The person's first message of an interview: what is asked, and how the answer must come back.
pub fn interview_prompt(name: &str, role: &str) -> String {
    [
        INTERVIEW_OPENING.to_owned(),
        format!(
            "You are {name}, {role}, a teammate in Locust. Ask me one short question at a time and wait for my answer: what I want from you, the work you will do, how I like answers, and what you must never do. Five questions at most."
        ),
        "Only ask: use no tool, read no file and make no plan while you interview me, and do not start the work itself.".to_owned(),
        format!(
            "When you know enough, or when I say I am done, write your standing instructions to yourself, in the second person, under {} characters, inside <{INSTRUCTIONS_TAG}> and </{INSTRUCTIONS_TAG}> on lines of their own. Put no password, key or other secret in them.",
            MAX_TEAMMATE_INSTRUCTIONS / 2
        ),
    ]
    .join("\n\n")
}

/// Whether a conversation that began with these words is an interview.
pub fn is_interview(first_prompt: Option<&str>) -> bool {
    first_prompt.is_some_and(|p| p.trim_start().starts_with(INTERVIEW_OPENING))
}

fn block_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?s)<locust-instructions\s*>([\s\S]*?)</locust-instructions>")
            .expect("valid interview block regex")
    })
}

/// The last instructions block outside code in a message, trimmed and bounded; nothing when there is none.
pub fn instructions_block(text: &str) -> Option<String> {
    let blocks = crate::protocol_tags::blocks_outside_code(text, block_regex());
    let (_, _, last) = blocks.into_iter().last()?;
    let words = last.trim();
    if words.is_empty() {
        return None;
    }
    if words.encode_utf16().count() > MAX_TEAMMATE_INSTRUCTIONS {
        let mut units = 0usize;
        let bounded: String = words
            .chars()
            .take_while(|c| {
                units += c.len_utf16();
                units <= MAX_TEAMMATE_INSTRUCTIONS
            })
            .collect();
        Some(bounded.trim_end().to_owned())
    } else {
        Some(words.to_owned())
    }
}

/// Why these instructions are not saved: a key in them, which the teammate's model would be given every turn.
pub fn instructions_refusal(name: &str, text: &str) -> Option<String> {
    crate::secrets::secret_in(text)
        .map(|kind| crate::secrets::instructions_secret_refusal(name, kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    const WRITTEN: &str =
        "You are Ada. Answer in three bullet points at most.\nAsk before changing a file outside docs/.";

    #[test]
    fn begins_with_persons_ask_and_tells_teammate_how_to_answer() {
        let prompt = interview_prompt("Ada", "Research & Briefs");
        assert!(is_interview(Some(&prompt)));
        assert!(prompt.contains("You are Ada, Research & Briefs"));
        assert!(prompt.contains("one short question at a time"));
        assert!(prompt.contains("use no tool, read no file and make no plan"));
        assert!(prompt.contains("or when I say I am done"));
        assert!(prompt.contains(&format!("<{INSTRUCTIONS_TAG}>")));
        assert!(prompt.contains("no password, key or other secret"));
        assert!(!is_interview(Some("Write the release notes")));
        assert!(!is_interview(None));
    }

    #[test]
    fn reads_last_block_outside_code_trimmed_and_bounded() {
        assert_eq!(
            instructions_block(&format!(
                "Here they are.\n<{INSTRUCTIONS_TAG}>\n  {WRITTEN}  \n</{INSTRUCTIONS_TAG}>"
            ))
            .as_deref(),
            Some(WRITTEN)
        );
        assert_eq!(
            instructions_block(&format!(
                "<{INSTRUCTIONS_TAG}>first</{INSTRUCTIONS_TAG}> then <{INSTRUCTIONS_TAG}>second</{INSTRUCTIONS_TAG}>"
            ))
            .as_deref(),
            Some("second")
        );
        assert_eq!(
            instructions_block("```\n<locust-instructions>an example</locust-instructions>\n```"),
            None
        );
        assert_eq!(
            instructions_block(&format!("<{INSTRUCTIONS_TAG}>   </{INSTRUCTIONS_TAG}>")),
            None
        );
        let over = format!(
            "<{INSTRUCTIONS_TAG}>{}</{INSTRUCTIONS_TAG}>",
            "x".repeat(MAX_TEAMMATE_INSTRUCTIONS + 50)
        );
        assert_eq!(
            instructions_block(&over).unwrap().encode_utf16().count(),
            MAX_TEAMMATE_INSTRUCTIONS
        );
        assert!(
            !crate::protocol_tags::defang_protocol_blocks(&format!(
                "<{INSTRUCTIONS_TAG}>x</{INSTRUCTIONS_TAG}>"
            ))
            .contains(&format!("<{INSTRUCTIONS_TAG}>"))
        );
    }

    #[test]
    fn refuses_instructions_that_hold_a_key_before_saving() {
        assert_eq!(
            instructions_refusal(
                "Ada",
                "Use the key sk-ant-abcdefghijklmnopqrstuvwxyz0123 for the API." // public-guard: allow
            )
            .as_deref(),
            Some(
                "They hold an Anthropic API key, and Ada’s model is given them on every turn, so they were not saved. Take it out first."
            )
        );
        assert!(
            instructions_refusal("Ada", "The database password is hunter2hunter2.")
                .unwrap()
                .contains("They hold a password")
        );
        assert_eq!(instructions_refusal("Ada", WRITTEN), None);
    }
}
