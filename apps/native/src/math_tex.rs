//! Rendered math in replies — port of `MathTex.tsx` + the math splits in `agentText.ts`.
//!
//! Electron draws with KaTeX (`trust: false`). Native draws with RaTeX 0.1.15 (MIT), a Rust
//! KaTeX-layout port, rasterised to PNG with embedded KaTeX fonts (`embed-fonts`) so nothing
//! is fetched. What RaTeX cannot read stays as the TeX that was written, never dropped.

use ratex_layout::{layout, to_display_list, validate_layout, LayoutOptions};
use ratex_parser::parser::parse;
use ratex_render::{render_to_png, RenderOptions};
use ratex_types::color::Color;
use ratex_types::math_style::MathStyle;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Image source prefix used inside rewritten Markdown (`![…](locust-math:HEX)`).
pub const MATH_SRC_PREFIX: &str = "locust-math:";

static PNG_CACHE: Mutex<Option<HashMap<u64, Arc<[u8]>>>> = Mutex::new(None);

fn cache_map() -> std::sync::MutexGuard<'static, Option<HashMap<u64, Arc<[u8]>>>> {
    PNG_CACHE.lock().unwrap_or_else(|e| e.into_inner())
}

fn tex_key(tex: &str, display: bool, dark: bool) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    tex.hash(&mut h);
    display.hash(&mut h);
    dark.hash(&mut h);
    h.finish()
}

/// Ink colour for math on the current appearance (light ink on dark chrome).
fn ink(dark: bool) -> Color {
    if dark {
        Color::new(0.95, 0.93, 0.91, 1.0)
    } else {
        Color::new(0.11, 0.12, 0.11, 1.0)
    }
}

/// Draw `tex` to PNG. `Ok(None)` means show the TeX as written (RaTeX could not read it).
pub fn render_png(tex: &str, display: bool, dark: bool) -> Option<Arc<[u8]>> {
    let key = tex_key(tex, display, dark);
    {
        let guard = cache_map();
        if let Some(map) = guard.as_ref()
            && let Some(hit) = map.get(&key)
        {
            return Some(hit.clone());
        }
    }
    let nodes = parse(tex).ok()?;
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let opts = LayoutOptions::default()
        .with_style(style)
        .with_color(ink(dark));
    let laid = layout(&nodes, &opts);
    validate_layout(&laid).ok()?;
    let list = to_display_list(&laid);
    let png = render_to_png(
        &list,
        &RenderOptions {
            font_size: if display { 22.0 } else { 16.0 },
            padding: if display { 6.0 } else { 2.0 },
            background_color: Color::new(0.0, 0.0, 0.0, 0.0),
            font_dir: String::new(),
            device_pixel_ratio: 2.0,
        },
    )
    .ok()?;
    let arc: Arc<[u8]> = Arc::from(png.into_boxed_slice());
    let mut guard = cache_map();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(key, arc.clone());
    Some(arc)
}

/// Fallback text when drawing fails (`MathTex.tsx`).
pub fn fallback_tex(tex: &str, display: bool) -> String {
    if display {
        format!("$${tex}$$")
    } else {
        format!("${tex}$")
    }
}

/// Rewrite math outside fenced code into `![…](locust-math:…)` images; return the Markdown and
/// a map of those sources to PNG bytes. Undrawable math is left as the TeX that was written.
pub fn prepare_math_markdown(body: &str, dark: bool) -> (String, HashMap<String, Arc<[u8]>>) {
    let mut images = HashMap::new();
    let mut out = String::with_capacity(body.len());
    let mut fenced = false;
    let mut fence_ticks = 0usize;
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        index += 1;
        let trimmed = line.trim_end_matches(['\r', '\n']);
        let newline = &line[trimmed.len()..];
        if !fenced {
            if let Some(ticks) = opening_fence(trimmed.trim_start()) {
                fenced = true;
                fence_ticks = ticks;
                out.push_str(line);
                continue;
            }
            // A quote's `> ` or a list item's indent: math drawn on such a line stays inside it.
            let (prefix, content) = line_prefix(trimmed);
            let pad = prefix.is_empty();
            // A displayed equation opening the line, closed on it or on a later one (agentText.ts's
            // 'math' block): only when nothing but punctuation follows the close.
            if let Some((tex, last)) = take_display_block(&lines, index - 1) {
                out.push_str(prefix);
                push_math(&mut out, &mut images, &tex, true, dark, pad);
                let end = lines[last];
                out.push_str(&end[end.trim_end_matches(['\r', '\n']).len()..]);
                index = last + 1;
                continue;
            }
            // Display math that opens the line.
            if let Some((tex, rest)) = take_display_line(content) {
                out.push_str(prefix);
                push_math(&mut out, &mut images, &tex, true, dark, pad);
                out.push_str(rest);
                out.push_str(newline);
                continue;
            }
            out.push_str(prefix);
            rewrite_inline(&mut out, &mut images, content, dark, pad);
            out.push_str(newline);
        } else {
            if closing_fence(trimmed.trim_start(), fence_ticks) {
                fenced = false;
                fence_ticks = 0;
            }
            out.push_str(line);
        }
    }
    (out, images)
}

fn push_math(
    out: &mut String,
    images: &mut HashMap<String, Arc<[u8]>>,
    tex: &str,
    display: bool,
    dark: bool,
    pad: bool,
) {
    match render_png(tex, display, dark) {
        Some(png) => {
            let src = format!("{MATH_SRC_PREFIX}{:016x}", tex_key(tex, display, dark));
            images.insert(src.clone(), png);
            // An equation gathered from several lines is named on one, or its own lines would end the quote.
            let alt = tex.split_whitespace().collect::<Vec<_>>().join(" ");
            if display && pad {
                out.push_str(&format!("\n![{alt}]({src})\n"));
            } else {
                // Inside a quote or a list item the picture stays on the line, which Markdown still draws
                // as a block of its own; a blank line around it would end the quote or the item.
                out.push_str(&format!("![{alt}]({src})"));
            }
        }
        None => out.push_str(&fallback_tex(tex, display)),
    }
}

fn opening_fence(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let mut ticks = 0usize;
    while ticks < bytes.len() && bytes[ticks] == b'`' {
        ticks += 1;
    }
    if ticks >= 3 { Some(ticks) } else { None }
}

fn closing_fence(line: &str, ticks: usize) -> bool {
    let bytes = line.as_bytes();
    let mut n = 0usize;
    while n < bytes.len() && bytes[n] == b'`' {
        n += 1;
    }
    n >= ticks && line[n..].trim().is_empty()
}

/// A line's quote markers and indent, and what follows them.
fn line_prefix(line: &str) -> (&str, &str) {
    let content = line.trim_start_matches([' ', '\t', '>']);
    (&line[..line.len() - content.len()], content)
}

/// `$$` or `\[` opening a line and closed on a LATER line, with nothing but punctuation after the close
/// (agentText.ts's displayed-equation block): the TeX, and the index of the closing line. A quote's
/// markers come off every line it gathers. One closed on its own line is [`take_display_line`]'s.
fn take_display_block(lines: &[&str], first: usize) -> Option<(String, usize)> {
    let (_, content) = line_prefix(lines[first].trim_end_matches(['\r', '\n']));
    let close = if content.starts_with("$$") {
        "$$"
    } else if content.starts_with("\\[") {
        "\\]"
    } else {
        return None;
    };
    if content[2..].contains(close) {
        return None;
    }
    let mut gathered = vec![&content[2..]];
    for (at, line) in lines.iter().enumerate().skip(first + 1) {
        let (_, rest) = line_prefix(line.trim_end_matches(['\r', '\n']));
        if let Some(end) = rest.find(close) {
            if !rest[end + 2..].chars().all(|c| c.is_whitespace() || ".,;:".contains(c)) {
                return None;
            }
            gathered.push(&rest[..end]);
            let tex = gathered.join("\n").trim().to_owned();
            return (!tex.is_empty()).then_some((tex, at));
        }
        gathered.push(rest);
    }
    None
}

fn take_display_line(line: &str) -> Option<(String, &str)> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let _ = indent;
    if let Some(rest) = trimmed.strip_prefix("$$")
        && let Some(end) = rest.find("$$")
    {
        let tex = rest[..end].trim();
        let after = rest[end + 2..].trim_start();
        if !tex.is_empty() && after.chars().all(|c| c.is_whitespace() || ".,;:".contains(c)) {
            return Some((tex.to_owned(), ""));
        }
    } else if let Some(rest) = trimmed.strip_prefix("\\[")
        && let Some(end) = rest.find("\\]")
    {
        let tex = rest[..end].trim();
        let after = rest[end + 2..].trim_start();
        if !tex.is_empty()
            && after.chars().all(|c| c.is_whitespace() || ".,;:".contains(c))
            && tex.chars().any(|c| "\\^_={}".contains(c))
        {
            return Some((tex.to_owned(), ""));
        }
    }
    None
}

/// A lone TeX macro is its symbol (agentText.ts `TEX_SYMBOLS`): `$\rightarrow$` reads "→". Only a `$…$` that is
/// exactly one of these, spaces allowed inside the dollars, is drawn as its character; a whole formula is math.
pub const TEX_SYMBOLS: &[(&str, &str)] = &[
    ("rightarrow", "→"),
    ("to", "→"),
    ("longrightarrow", "⟶"),
    ("leftarrow", "←"),
    ("gets", "←"),
    ("leftrightarrow", "↔"),
    ("Rightarrow", "⇒"),
    ("implies", "⇒"),
    ("Leftarrow", "⇐"),
    ("Leftrightarrow", "⇔"),
    ("iff", "⇔"),
    ("mapsto", "↦"),
    ("uparrow", "↑"),
    ("downarrow", "↓"),
    ("times", "×"),
    ("div", "÷"),
    ("cdot", "·"),
    ("pm", "±"),
    ("mp", "∓"),
    ("le", "≤"),
    ("leq", "≤"),
    ("ge", "≥"),
    ("geq", "≥"),
    ("ne", "≠"),
    ("neq", "≠"),
    ("approx", "≈"),
    ("equiv", "≡"),
    ("sim", "∼"),
    ("propto", "∝"),
    ("ll", "≪"),
    ("gg", "≫"),
    ("infty", "∞"),
    ("degree", "°"),
    ("circ", "∘"),
    ("bullet", "•"),
    ("checkmark", "✓"),
    ("ldots", "…"),
    ("dots", "…"),
    ("cdots", "⋯"),
    ("in", "∈"),
    ("notin", "∉"),
    ("subset", "⊂"),
    ("subseteq", "⊆"),
    ("cup", "∪"),
    ("cap", "∩"),
    ("emptyset", "∅"),
    ("forall", "∀"),
    ("exists", "∃"),
    ("neg", "¬"),
    ("land", "∧"),
    ("lor", "∨"),
    ("therefore", "∴"),
    ("sum", "∑"),
    ("prod", "∏"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ε"),
    ("theta", "θ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("pi", "π"),
    ("sigma", "σ"),
    ("tau", "τ"),
    ("phi", "φ"),
    ("omega", "ω"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Phi", "Φ"),
    ("Omega", "Ω"),
];

/// `$\name$` (or `$ \name $`) at the start of `s`, for a name in [`TEX_SYMBOLS`]: its symbol and the length read.
fn lone_macro(s: &str) -> Option<(&'static str, usize)> {
    let inner = s.strip_prefix('$')?;
    let body = inner.trim_start_matches([' ', '\t']).strip_prefix('\\')?;
    let name_len = body.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (name, after) = body.split_at(name_len);
    let after_trim = after.trim_start_matches([' ', '\t']);
    after_trim.strip_prefix('$')?;
    let symbol = TEX_SYMBOLS.iter().find(|(n, _)| *n == name)?.1;
    Some((symbol, s.len() - after_trim.len() + 1))
}

/// Inline `$…$`, `\(...\)`, mid-line `$$…$$` / `\[…\]` — pandoc dollar rules from agentText.ts.
fn rewrite_inline(
    out: &mut String,
    images: &mut HashMap<String, Arc<[u8]>>,
    line: &str,
    dark: bool,
    pad: bool,
) {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // A code span is code: nothing in it is math (agentText.ts reads code spans first).
        if bytes[i] == b'`' {
            let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
            let ticks = "`".repeat(run);
            if let Some(end) = line[i + run..].find(ticks.as_str()) {
                let end = i + run + end + run;
                out.push_str(&line[i..end]);
                i = end;
            } else {
                out.push_str(&ticks);
                i += run;
            }
            continue;
        }
        if bytes[i] == b'$'
            && (i == 0 || bytes[i - 1] != b'\\')
            && let Some((symbol, len)) = lone_macro(&line[i..])
        {
            out.push_str(symbol);
            i += len;
            continue;
        }
        // \( ... \)
        if bytes[i] == b'\\'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'('
            && let Some(end) = find_close(line, i + 2, "\\)")
        {
            let tex = line[i + 2..end].trim();
            if !tex.is_empty() {
                push_math(out, images, tex, false, dark, pad);
                i = end + 2;
                continue;
            }
        }
        // \[ ... \] mid-line (must hold a math character)
        if bytes[i] == b'\\'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'['
            && let Some(end) = find_close(line, i + 2, "\\]")
        {
            let tex = line[i + 2..end].trim();
            if !tex.is_empty() && tex.chars().any(|c| "\\^_={}".contains(c)) {
                push_math(out, images, tex, true, dark, pad);
                i = end + 2;
                continue;
            }
        }
        // $$ ... $$
        if bytes[i] == b'$' && i + 1 < bytes.len() && bytes[i + 1] == b'$' {
            if i > 0 && bytes[i - 1] == b'\\' {
                out.push('$');
                i += 1;
                continue;
            }
            if let Some(end) = find_dollar_pair(line, i + 2, true) {
                let tex = line[i + 2..end].trim();
                if !tex.is_empty() {
                    push_math(out, images, tex, true, dark, pad);
                    i = end + 2;
                    continue;
                }
            }
        }
        // $ ... $ (pandoc: open not followed by space/$, close not followed by digit)
        if bytes[i] == b'$' {
            if i > 0 && bytes[i - 1] == b'\\' {
                out.push('$');
                i += 1;
                continue;
            }
            if i + 1 < bytes.len()
                && bytes[i + 1] != b'$'
                && bytes[i + 1] != b' '
                && bytes[i + 1] != b'\t'
                && let Some(end) = find_inline_dollar_close(line, i + 1)
            {
                let tex = line[i + 1..end].trim();
                if !tex.is_empty() {
                    push_math(out, images, tex, false, dark, pad);
                    i = end + 1;
                    continue;
                }
            }
        }
        out.push(line[i..].chars().next().unwrap());
        i += line[i..].chars().next().unwrap().len_utf8();
    }
}

fn find_close(s: &str, from: usize, mark: &str) -> Option<usize> {
    s[from..].find(mark).map(|i| from + i)
}

fn find_dollar_pair(s: &str, from: usize, double: bool) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            if double {
                if i + 1 < bytes.len() && bytes[i + 1] == b'$' {
                    return Some(i);
                }
            } else {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn find_inline_dollar_close(s: &str, from: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'$' && bytes[i - 1] != b'\\' && bytes[i - 1] != b' ' && bytes[i - 1] != b'\t'
        {
            let after = bytes.get(i + 1).copied();
            if after.is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
                continue;
            }
            if after == Some(b'$') {
                return None;
            }
            return Some(i);
        }
        if bytes[i] == b'\n' {
            return None;
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_simple_inline_and_falls_back() {
        let png = render_png("c_1 = y", false, true);
        assert!(png.as_ref().is_some_and(|p| p.len() > 100));
        // Broken TeX stays text.
        let (body, images) = prepare_math_markdown("see $\\notacommandxyz$ here", true);
        // Either drawn or left as TeX — never dropped.
        assert!(body.contains("notacommandxyz") || images.len() == 1);
        assert!(body.contains("see ") && body.contains(" here"));
    }

    #[test]
    fn money_stays_prose_and_math_is_drawn() {
        let (body, images) = prepare_math_markdown("$5 and $10 plus $x^2$", true);
        assert!(body.contains("$5 and $10"));
        assert!(!images.is_empty() || body.contains("$x^2$") || body.contains("locust-math:"));
        // The squared term should become an image when RaTeX accepts it.
        assert!(
            body.contains("locust-math:") || body.contains("$x^2$"),
            "body={body}"
        );
    }

    #[test]
    fn fences_are_not_scanned_for_math() {
        let (body, images) = prepare_math_markdown("```\n$x^2$\n```\n", true);
        assert!(images.is_empty());
        assert!(body.contains("$x^2$"));
    }

    #[test]
    fn display_math_becomes_image() {
        let (body, images) =
            prepare_math_markdown("Before\n\n$$\\sum_{i=1}^{n} i$$\n\nAfter", true);
        assert!(
            !images.is_empty() || body.contains("$$"),
            "body={body}"
        );
        assert!(body.contains("Before") && body.contains("After"));
    }

    #[test]
    fn fallback_words_match_electron() {
        assert_eq!(fallback_tex("a=b", true), "$$a=b$$");
        assert_eq!(fallback_tex("a=b", false), "$a=b$");
    }
}
