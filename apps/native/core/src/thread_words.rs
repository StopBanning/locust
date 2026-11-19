//! Small word rules of the thread: a thought's headline (missionView.ts `thoughtHeadline`) and a helper's one-line
//! summary as words (ActivityCard.tsx `plainSummary`, `helperResult`).
use regex::Regex;
use std::sync::OnceLock;

fn re(cell: &'static OnceLock<Regex>, pattern: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pattern).expect("a fixed pattern"))
}

/// A thought's headline, when its summary leads with one: Codex writes "**Inspecting the config**" and then the
/// paragraph.
pub fn thought_headline(text: &str) -> Option<String> {
    static R: OnceLock<Regex> = OnceLock::new();
    let found = re(&R, r"^\s*\*\*([^*\n]{2,80})\*\*").captures(text)?;
    let headline = found[1].trim();
    (!headline.is_empty()).then(|| headline.to_owned())
}

/// A helper's one-line summary as words: Markdown marks go, the words and code spans' contents stay.
pub fn plain_summary(text: &str) -> String {
    static HEAD: OnceLock<Regex> = OnceLock::new();
    static BOLD: OnceLock<Regex> = OnceLock::new();
    static UNDER: OnceLock<Regex> = OnceLock::new();
    static CODE: OnceLock<Regex> = OnceLock::new();
    static STAR: OnceLock<Regex> = OnceLock::new();
    static SPACE: OnceLock<Regex> = OnceLock::new();
    let t = re(&HEAD, r"^\s*#{1,6}\s+").replace(text, "");
    let t = re(&BOLD, r"\*\*([^*\n]+)\*\*").replace_all(&t, "$1");
    let t = re(&UNDER, r"__([^_\n]+)__").replace_all(&t, "$1");
    let t = re(&CODE, r"`([^`\n]+)`").replace_all(&t, "$1");
    // Single-star emphasis; the closing star must be followed by a space, a closing mark or the end (no lookahead in
    // `regex`, so the follower is captured and put back).
    let t = re(&STAR, r"(^|[\s(])\*([^*\s][^*\n]*?)\*($|[\s).,!?:;])").replace_all(&t, "$1$2$3");
    re(&SPACE, r"\s+").replace_all(&t, " ").trim().to_owned()
}

/// What a helper row says once it is settled and succeeded, without a background run: "reported back · …".
pub fn helper_reported_back(summary: Option<&str>) -> String {
    match summary {
        None => "reported back".into(),
        Some(s) => format!("reported back \u{b7} {}", plain_summary(s)),
    }
}
