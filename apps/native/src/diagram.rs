//! Mermaid diagrams in replies — port of `shared/diagram.ts` and the ReplyDiagram chrome words.
//!
//! Drawing uses Rune's `rune-mermaid` via `rune-kit`'s `mermaid` feature (no WebView, no network).
//! Types `rune-mermaid` does not know (kanban, packet, architecture, radar, treemap, …), empty
//! sources, or sources past its size bounds fall back to the source fence; this module adds
//! Electron's failure caption when that happens.

use std::collections::HashMap;

/// Why a diagram is text when Locust itself is at fault (`shared/diagram.ts`).
pub const DIAGRAM_UNDRAWN: &str = "Locust could not draw it";

/// Fence languages that mean Mermaid (`shared/diagram.ts`).
const LANGUAGES: &[&str] = &["mermaid", "mmd"];

/// Longest diagram Locust draws (`shared/diagram.ts` MAX_DIAGRAM_CHARS).
pub const MAX_DIAGRAM_CHARS: usize = 40_000;

/// Electron ReplyDiagram tab / wait / failure words.
pub const TAB_DIAGRAM: &str = "Diagram";
pub const TAB_CODE: &str = "Code";
pub const WAITING: &str = "Drawing the diagram…";
pub const TABS_ARIA: &str = "Show the diagram or its text";

/// Figcaption when a diagram stays text (`ReplyDiagram.tsx`).
pub fn not_drawn_caption(message: &str) -> String {
    format!("Not drawn as a diagram ({message}). Its text is above, as written.")
}

/// Whether a code block is a diagram to draw (`shared/diagram.ts` `isDiagram`).
pub fn is_diagram(code: &str, language: Option<&str>) -> bool {
    let Some(language) = language else {
        return false;
    };
    let lang = language.trim().to_ascii_lowercase();
    LANGUAGES.iter().any(|l| *l == lang)
        && !code.trim().is_empty()
        && code.len() <= MAX_DIAGRAM_CHARS
}

/// Kind label from Mermaid's diagram name (`shared/diagram.ts` `diagramKindWords`).
pub fn diagram_kind_words(kind: Option<&str>) -> &'static str {
    match kind {
        Some("flowchart" | "flowchart-v2" | "flowchart-elk" | "graph") => "Flowchart",
        Some("sequence" | "sequenceDiagram") => "Sequence diagram",
        Some("class" | "classDiagram") => "Class diagram",
        Some("state" | "stateDiagram" | "stateDiagram-v2") => "State diagram",
        Some("er" | "erDiagram") => "Entity relationships",
        Some("gantt") => "Gantt chart",
        Some("pie") => "Pie chart",
        Some("journey") => "User journey",
        Some("gitGraph") => "Git graph",
        Some("mindmap") => "Mind map",
        Some("timeline") => "Timeline",
        Some("quadrantChart") => "Quadrant chart",
        Some("xychart" | "xychart-beta") => "Chart",
        Some("sankey" | "sankey-beta") => "Sankey diagram",
        Some("block" | "block-beta") => "Block diagram",
        Some("architecture") => "Architecture",
        Some("packet") => "Packet diagram",
        Some("kanban") => "Kanban board",
        Some("radar") => "Radar chart",
        Some("treemap") => "Treemap",
        Some("requirement" | "requirementDiagram") => "Requirements",
        Some("c4" | "C4Context" | "C4Container" | "C4Component" | "C4Dynamic" | "C4Deployment") => {
            "C4 diagram"
        }
        Some("info") => "Diagram",
        _ => "Diagram",
    }
}

/// First diagram-type token in a Mermaid source (skips `%%` comments).
pub fn diagram_kind_of(source: &str) -> Option<&str> {
    source
        .split_whitespace()
        .find(|w| !w.starts_with("%%"))
        .map(|w| w.trim_end_matches(';'))
}

/// Whether `rune-mermaid` can draw this source (closed fence body only).
pub fn can_draw(source: &str) -> bool {
    is_diagram(source, Some("mermaid")) && rune_kit::mermaid::supported(source)
}

/// Rewrite reply Markdown so `mmd` fences become `mermaid`, and undrawable closed Mermaid
/// fences keep their text with Electron's failure caption beneath (as a plain paragraph).
pub fn prepare_reply_markdown(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 64);
    let mut rest = body;
    while let Some(start) = find_fence_open(rest) {
        out.push_str(&rest[..start.byte]);
        let after_open = &rest[start.byte + start.open_len..];
        let lang = start.lang.as_str();
        let is_diag = LANGUAGES.iter().any(|l| lang.eq_ignore_ascii_case(l));
        if !is_diag {
            out.push_str(&rest[start.byte..start.byte + start.open_len]);
            rest = after_open;
            continue;
        }
        match find_fence_close(after_open, start.ticks) {
            None => {
                // Still streaming: normalise language to mermaid so rune-kit can stream-draw.
                out.push_str(&rest[start.byte..start.byte + start.ticks]);
                out.push_str("mermaid");
                if let Some(tail) = start.lang_tail.as_deref() {
                    out.push_str(tail);
                }
                out.push('\n');
                out.push_str(after_open);
                return out;
            }
            Some(close_at) => {
                let code = &after_open[..close_at];
                let after = &after_open[close_at..];
                let close_line_end = after.find('\n').map(|i| i + 1).unwrap_or(after.len());
                let after_fence = &after[close_line_end..];
                if can_draw(code) {
                    out.push_str(&rest[start.byte..start.byte + start.ticks]);
                    out.push_str("mermaid");
                    if let Some(tail) = start.lang_tail.as_deref() {
                        out.push_str(tail);
                    }
                    out.push('\n');
                    out.push_str(code);
                    out.push_str(&after[..close_line_end]);
                } else {
                    // Capital-M `Mermaid` keeps the language label Electron shows, but rune-kit
                    // only routes lowercase `mermaid` to rune-mermaid — so this stays a code fence.
                    out.push_str(&rest[start.byte..start.byte + start.ticks]);
                    out.push_str("Mermaid");
                    out.push('\n');
                    out.push_str(code);
                    out.push_str(&after[..close_line_end]);
                    let message = if code.trim().is_empty() {
                        "it is not a diagram Locust draws"
                    } else {
                        // reason_of keeps Mermaid-shaped errors as one short line.
                        &reason_of(DIAGRAM_UNDRAWN)
                    };
                    // Marker the reply renderer turns into ReplyDiagram's figcaption chrome.
                    out.push('\n');
                    out.push_str("<!--locust-diagram-undrawn:");
                    out.push_str(message);
                    out.push_str("-->\n");
                }
                rest = after_fence;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Pull undrawn markers out of prepared Markdown so Markdown itself never shows them; the
/// reply view draws the caption under the preceding code fence.
pub fn take_undrawn_markers(body: &str) -> (String, HashMap<usize, String>) {
    let mut markers = HashMap::new();
    let mut clean = String::with_capacity(body.len());
    let mut fence_index = 0usize;
    let mut rest = body;
    let marker_prefix = "<!--locust-diagram-undrawn:";
    while let Some(at) = rest.find(marker_prefix) {
        let before = &rest[..at];
        fence_index += count_closed_fences(before);
        clean.push_str(before);
        let after_prefix = &rest[at + marker_prefix.len()..];
        let Some(end) = after_prefix.find("-->") else {
            clean.push_str(&rest[at..]);
            return (clean, markers);
        };
        let message = after_prefix[..end].trim();
        // The undrawn fence is the one just before this marker.
        let idx = fence_index.saturating_sub(1);
        markers.insert(idx, message.to_owned());
        let mut next = &after_prefix[end + 3..];
        if next.starts_with('\n') {
            next = &next[1..];
        }
        rest = next;
    }
    clean.push_str(rest);
    (clean, markers)
}

fn count_closed_fences(text: &str) -> usize {
    let mut n = 0;
    let mut rest = text;
    while let Some(start) = find_fence_open(rest) {
        let after = &rest[start.byte + start.open_len..];
        match find_fence_close(after, start.ticks) {
            Some(close_at) => {
                n += 1;
                let after_close = &after[close_at..];
                let line_end = after_close.find('\n').map(|i| i + 1).unwrap_or(after_close.len());
                rest = &after_close[line_end..];
            }
            None => break,
        }
    }
    n
}

struct FenceOpen {
    byte: usize,
    ticks: usize,
    open_len: usize,
    lang: String,
    lang_tail: Option<String>,
}

fn find_fence_open(text: &str) -> Option<FenceOpen> {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i == 0 || bytes[i - 1] == b'\n' {
            let mut ticks = 0usize;
            while i + ticks < bytes.len() && bytes[i + ticks] == b'`' {
                ticks += 1;
            }
            if ticks >= 3 {
                let after_ticks = i + ticks;
                let line_end = text[after_ticks..]
                    .find('\n')
                    .map(|n| after_ticks + n)
                    .unwrap_or(text.len());
                let info = text[after_ticks..line_end].trim();
                let mut parts = info.splitn(2, char::is_whitespace);
                let lang = parts.next().unwrap_or("").to_owned();
                let lang_tail = parts.next().map(|s| {
                    let mut t = String::from(" ");
                    t.push_str(s);
                    t
                });
                let open_len = if line_end < text.len() {
                    line_end + 1 - i
                } else {
                    line_end - i
                };
                return Some(FenceOpen {
                    byte: i,
                    ticks,
                    open_len,
                    lang,
                    lang_tail,
                });
            }
        }
        i += 1;
    }
    None
}

fn find_fence_close(text: &str, ticks: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if i == 0 || bytes[i - 1] == b'\n' {
            let mut n = 0usize;
            while i + n < bytes.len() && bytes[i + n] == b'`' {
                n += 1;
            }
            if n >= ticks {
                let after = i + n;
                let rest_line = text[after..]
                    .split('\n')
                    .next()
                    .unwrap_or("");
                if rest_line.trim().is_empty() {
                    return Some(i);
                }
            }
        }
        i += 1;
    }
    None
}

/// Mermaid's reason as one plain phrase (`shared/diagram.ts` `reasonOf`).
pub fn reason_of(error: &str) -> String {
    let line = error
        .split(['\r', '\n'])
        .map(str::trim)
        .find(|p| !p.is_empty())
        .unwrap_or("");
    let plain = line.replace(['\t'], " ");
    let plain = {
        let mut s = String::new();
        for word in plain.split_whitespace() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(word);
        }
        s.trim_end_matches([':', '.']).to_owned()
    };
    if plain.is_empty() {
        return "Mermaid could not read it".into();
    }
    if plain.len() > 160 {
        format!("{}...", &plain[..157])
    } else {
        plain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_diagram_matches_electron() {
        assert!(is_diagram("graph TD\n  A --> B", Some("mermaid")));
        assert!(is_diagram("graph TD\n  A --> B", Some("Mermaid ")));
        assert!(is_diagram("graph TD\n  A --> B", Some("mmd")));
        assert!(!is_diagram("graph TD\n  A --> B", Some("js")));
        assert!(!is_diagram("graph TD\n  A --> B", None));
        assert!(!is_diagram("   \n", Some("mermaid")));
        let huge = format!("graph TD\n{}", "  A --> B\n".repeat(MAX_DIAGRAM_CHARS / 10));
        assert!(!is_diagram(&huge, Some("mermaid")));
    }

    #[test]
    fn kind_words_match_electron() {
        assert_eq!(diagram_kind_words(Some("flowchart-v2")), "Flowchart");
        assert_eq!(diagram_kind_words(Some("sequence")), "Sequence diagram");
        assert_eq!(diagram_kind_words(Some("somethingNew")), "Diagram");
        assert_eq!(diagram_kind_words(None), "Diagram");
    }

    #[test]
    fn reason_of_matches_electron() {
        assert_eq!(
            reason_of("Parse error on line 3:\n...A --> \n-------^\nExpecting 'NODE_STRING', got 'EOF'"),
            "Parse error on line 3"
        );
        assert_eq!(reason_of(""), "Mermaid could not read it");
        let long = reason_of(&"x".repeat(400));
        assert_eq!(long.len(), 160);
    }

    #[test]
    fn prepare_rewrites_mmd_and_captions_undrawable() {
        let body = "Before\n\n```mmd\ngraph LR\n  A --> B\n```\n\nAfter";
        let prepared = prepare_reply_markdown(body);
        assert!(prepared.contains("```mermaid\n"));
        assert!(can_draw("graph LR\n  A --> B\n"));

        let bad = "```mermaid\nkanban\n  Todo\n```\n";
        let prepared = prepare_reply_markdown(bad);
        assert!(prepared.contains("<!--locust-diagram-undrawn:"));
        let (clean, markers) = take_undrawn_markers(&prepared);
        assert!(!clean.contains("<!--locust-diagram-undrawn:"));
        assert_eq!(
            markers.get(&0).map(String::as_str),
            Some(DIAGRAM_UNDRAWN)
        );
        assert_eq!(
            not_drawn_caption(DIAGRAM_UNDRAWN),
            "Not drawn as a diagram (Locust could not draw it). Its text is above, as written."
        );
    }

    #[test]
    fn flowchart_is_supported_by_rune_mermaid() {
        assert!(can_draw("graph LR\n  A[Push] --> B[Tests]\n"));
        assert!(!can_draw("kanban\n  Todo\n"));
    }

    #[test]
    fn reply_diagram_words_match_electron() {
        assert_eq!(TAB_DIAGRAM, "Diagram");
        assert_eq!(TAB_CODE, "Code");
        assert_eq!(WAITING, "Drawing the diagram…");
        assert_eq!(TABS_ARIA, "Show the diagram or its text");
        assert_eq!(DIAGRAM_UNDRAWN, "Locust could not draw it");
    }
}
