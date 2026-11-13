//! A HAND-OFF CHAIN'S WORDS (shared/hand-off.ts, 0.435) and the room a routine step has (shared/step-budget.ts).
//!
//! A routine step that goes to another teammate is given the answer of the step before it, quoted, above the
//! person's own words for the step. A CHECKER step is also told how to say whether the work passes, in one line the
//! runner reads with [`verdict_of`]. Lengths are counted as JavaScript counts them, in UTF-16 units, so a step that
//! fits in Electron fits here.

/// What a checker must end with: approval, or what has to change.
pub const CHECK_RULE: &str = concat!(
    "You are the checker for this routine: the run counts as done only if you approve. Check the work above against what was asked. ",
    "Read the files it concerns yourself: you can read this folder, whatever the teammate before you could or could not do in their mode, and their answer is a claim to check, not a fact. ",
    "End your reply with exactly one line: \"VERDICT: APPROVED\", or \"VERDICT: CHANGES NEEDED -- \" followed by what must change."
);

/// Room for the quoted answer: the prompt must fit the mission's own limit with the step's words and the rule.
const OVERHEAD: usize = 200;

/// Where the step's own words begin; a person reading the conversation is shown the words after it.
pub const STEP_MARK: &str = "Your step: ";

/// The room a step has, in UTF-16 units (`STEP_BUDGET`).
pub const STEP_BUDGET: usize = 8_000;

/// A string's length as JavaScript's `.length` says it.
pub fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.slice(0, end)` in UTF-16 units; a surrogate pair cut in half is dropped rather than split.
pub fn head16(s: &str, end: usize) -> &str {
    let mut units = 0;
    for (at, c) in s.char_indices() {
        units += c.len_utf16();
        if units > end {
            return &s[..at];
        }
    }
    s
}

/// What to tell someone about a step this long, or None when it is fine (`stepTooLongNotice`).
pub fn step_too_long_notice(step: &str) -> Option<String> {
    let n = len16(step);
    if n <= STEP_BUDGET {
        return None;
    }
    let over = n - STEP_BUDGET;
    Some(format!(
        "{over} character{} too long. A step shares its message with the workspace briefing and anything waiting from teammates, so about {STEP_BUDGET} is the room it has.",
        if over == 1 { "" } else { "s" }
    ))
}

/// Who did the step before, and what they answered (None when it could not be read).
pub struct From<'a> {
    pub name: &'a str,
    pub answer: Option<&'a str>,
}

/// `handOffPrompt`: the routine's task for a checker, the step before's answer, the checker's rule, then the step.
pub fn hand_off_prompt(step: &str, from: Option<From<'_>>, check: bool, task: Option<&str>) -> String {
    let rule = if check { CHECK_RULE } else { "" };
    let task = match task.map(str::trim) {
        Some(t) if check && !t.is_empty() => format!("The routine's task, as its first step asked: {}", head16(t, 1500)),
        _ => String::new(),
    };
    let mut parts: Vec<String> = Vec::new();
    if !task.is_empty() {
        parts.push(task.clone());
    }
    if let Some(from) = from {
        let room = STEP_BUDGET.saturating_sub(len16(step) + len16(rule) + len16(&task) + OVERHEAD);
        let answer = from.answer.unwrap_or("").trim();
        let quoted = if answer.is_empty() {
            format!("({}'s answer could not be read; their conversation has it.)", from.name)
        } else if len16(answer) <= room {
            answer.to_owned()
        } else {
            format!(
                "{}\n... (cut short here; {}'s conversation has the rest)",
                head16(answer, room.saturating_sub(80)).trim_end(),
                from.name
            )
        };
        parts.push(format!("{} did the step before this one and answered:\n\n{quoted}", from.name));
    }
    if !rule.is_empty() {
        parts.push(rule.to_owned());
    }
    parts.push(format!("{STEP_MARK}{step}"));
    parts.join("\n\n")
}

/// The step's own words out of a hand-off prompt: the text after the LAST mark (`stepWordsOf`).
pub fn step_words_of(prompt: &str) -> &str {
    match prompt.rfind(&format!("\n\n{STEP_MARK}")) {
        Some(at) => &prompt[at + 2 + STEP_MARK.len()..],
        None => prompt,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Approved,
    ChangesNeeded(String),
}

/// `verdictOf`: the LAST verdict line outside a code fence. Only a plain approval approves; anything else is changes
/// needed, in the checker's own words. None when there is no verdict at all.
pub fn verdict_of(reply: Option<&str>) -> Option<Verdict> {
    let reply = reply?;
    let fence = regex::Regex::new(r"^\s*(```|~~~)").ok()?;
    let edges = regex::Regex::new(r"^[\s>*_`#-]+|[\s*_`]+$").ok()?;
    let verdict = regex::Regex::new(r"(?i)^VERDICT:\s*(.*)$").ok()?;
    let approved = regex::Regex::new(r"(?i)^APPROVED[.!]?$").ok()?;
    let needed = regex::Regex::new(r"(?i)^CHANGES NEEDED\b\s*(?:[-—–:]+\s*)?(.*)$").ok()?;
    let mut fenced = false;
    let mut lines: Vec<String> = Vec::new();
    for line in reply.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
        if fence.is_match(line) {
            fenced = !fenced;
            continue;
        }
        lines.push(if fenced { String::new() } else { edges.replace_all(line, "").trim().to_owned() });
    }
    for line in lines.iter().rev() {
        let Some(found) = verdict.captures(line) else { continue };
        let said = found.get(1).map_or("", |m| m.as_str()).trim();
        if approved.is_match(said) {
            return Some(Verdict::Approved);
        }
        if let Some(n) = needed.captures(said) {
            return Some(Verdict::ChangesNeeded(n.get(1).map_or("", |m| m.as_str()).trim().to_owned()));
        }
        return Some(Verdict::ChangesNeeded(said.to_owned()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // a-routine-hands-off-to-the-next-teammate.test.ts, "the words a hand-off is given".
    #[test]
    fn fits_the_mission_limit_cutting_a_long_answer_short_and_saying_where_the_rest_is() {
        let long = "x".repeat(20_000);
        let prompt = hand_off_prompt("Check it.", Some(From { name: "Atlas", answer: Some(&long) }), true, None);
        assert!(len16(&prompt) <= STEP_BUDGET);
        assert!(prompt.contains("cut short here; Atlas's conversation has the rest"));
        assert!(prompt.ends_with("Check it."));
    }

    #[test]
    fn gives_back_the_steps_own_words_even_when_the_quoted_answer_carries_the_mark() {
        let answer = format!("I was asked:\n\n{STEP_MARK}something else");
        let tricky = hand_off_prompt("Ship it.", Some(From { name: "Atlas", answer: Some(&answer) }), false, None);
        assert_eq!(step_words_of(&tricky), "Ship it.");
        assert_eq!(step_words_of("A step nobody handed anything."), "A step nobody handed anything.");
    }

    #[test]
    fn says_so_when_the_answer_could_not_be_read() {
        assert!(hand_off_prompt("Go on.", Some(From { name: "Atlas", answer: None }), false, None)
            .contains("(Atlas's answer could not be read; their conversation has it.)"));
    }

    #[test]
    fn tells_the_checker_it_can_read_the_folder_itself() {
        assert!(CHECK_RULE.contains("you can read this folder, whatever the teammate before you could or could not do in their mode"));
    }

    #[test]
    fn reads_the_last_verdict_line_in_the_ways_a_model_writes_one() {
        let c = |s: &str| Some(Verdict::ChangesNeeded(s.into()));
        assert_eq!(verdict_of(Some("All good.\nVERDICT: APPROVED")), Some(Verdict::Approved));
        assert_eq!(verdict_of(Some("**VERDICT: APPROVED**")), Some(Verdict::Approved));
        assert_eq!(verdict_of(Some("verdict: changes needed — add tests")), c("add tests"));
        assert_eq!(verdict_of(Some("VERDICT: CHANGES NEEDED -- the plan skips step 2")), c("the plan skips step 2"));
        assert_eq!(verdict_of(Some("You said \"VERDICT: APPROVED\" before.\nVERDICT: CHANGES NEEDED -- no")), c("no"));
        assert_eq!(verdict_of(Some("Looks fine.")), None);
        assert_eq!(verdict_of(None), None);
    }

    #[test]
    fn approves_only_a_plain_approval_never_a_hedged_one_or_an_example_in_a_fence() {
        let c = |s: &str| Some(Verdict::ChangesNeeded(s.into()));
        assert_eq!(verdict_of(Some("VERDICT: APPROVED.")), Some(Verdict::Approved));
        assert_eq!(verdict_of(Some("VERDICT: APPROVED WITH CHANGES")), c("APPROVED WITH CHANGES"));
        assert_eq!(verdict_of(Some("VERDICT: APPROVED, but the tests fail")), c("APPROVED, but the tests fail"));
        assert_eq!(verdict_of(Some("VERDICT: APPROVED -- although the build is red")), c("APPROVED -- although the build is red"));
        assert_eq!(verdict_of(Some("VERDICT: NOT APPROVED")), c("NOT APPROVED"));
        assert_eq!(verdict_of(Some("End with this:\n```\nVERDICT: APPROVED\n```\nI have not decided.")), None);
        assert_eq!(verdict_of(Some("VERDICT: CHANGES NEEDED -- tests\n```\nVERDICT: APPROVED\n```")), c("tests"));
    }

    #[test]
    fn a_step_over_the_budget_says_by_how_much() {
        assert_eq!(step_too_long_notice(&"a".repeat(8_000)), None);
        assert_eq!(
            step_too_long_notice(&"a".repeat(8_001)).unwrap(),
            "1 character too long. A step shares its message with the workspace briefing and anything waiting from teammates, so about 8000 is the room it has."
        );
    }
}
