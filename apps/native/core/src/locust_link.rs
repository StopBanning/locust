//! A link that opens Locust with a message ready (Electron `shared/locust-link.ts` and `main/locust-links.ts`, 0.770):
//! `locust://new?prompt=...` opens a new conversation with that message in the box.
//!
//! It is never sent. A link is text from somewhere else, so the person reads it and presses Send, or does not, and the
//! box says where it came from. What they read is all there is: characters a person cannot see but a model reads
//! (Unicode tag characters, direction overrides, zero-width marks, control characters) are taken out before it reaches
//! the box. Only a new conversation's text: no folder, teammate, model or file, which would act on the machine before
//! anyone read anything.

pub const LOCUST_LINK_SCHEME: &str = "locust";

/// The longest message a link may bring: a long, careful prompt is a few thousand characters.
pub const MAX_LINK_PROMPT_CHARS: usize = 8_000;

/// What the box says over a message a link brought.
pub const FROM_A_LINK: &str = "From a link \u{2014} read it before you send it.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocustLink {
    /// The message for the box, as the person will read it and as it would be sent.
    pub prompt: String,
}

/// What a person cannot see in a message box: C0 and C1 controls but tab and newline, the soft hyphen, zero-width marks
/// and joiners, direction marks, embeddings, overrides and isolates, the byte-order mark, the invisible math operators,
/// variation selectors, and the tag characters (Electron's UNSEEN).
fn unseen(c: char) -> bool {
    matches!(c as u32,
        0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f..=0x9f
        | 0xad | 0x061c | 0x180e
        | 0x200b..=0x200f | 0x202a..=0x202e | 0x2060..=0x206f
        | 0xfe00..=0xfe0f | 0xfeff
        | 0xe0000..=0xe007f | 0xe0100..=0xe01ef)
}

/// The message as the person will see it, and nothing they will not.
pub fn seen_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").chars().filter(|c| !unseen(*c)).collect::<String>().trim().to_owned()
}

/// `%XX` and `+` as a URL's query decodes them (URLSearchParams); None for bytes that are not UTF-8.
fn query_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() && bytes[i + 1].is_ascii_hexdigit() && bytes[i + 2].is_ascii_hexdigit() => {
                let hex = |b: u8| (b as char).to_digit(16).unwrap_or(0) as u8;
                out.push(hex(bytes[i + 1]) * 16 + hex(bytes[i + 2]));
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    // URLSearchParams puts U+FFFD for bad bytes; a link that carries them is not one a person wrote to read.
    String::from_utf8(out).ok()
}

/// Reads a link, or nothing for anything that is not one Locust opens.
pub fn parse_locust_link(raw: &str) -> Option<LocustLink> {
    if raw.len() > MAX_LINK_PROMPT_CHARS * 12 {
        return None;
    }
    let raw = raw.trim();
    let (scheme, rest) = raw.split_once(':')?;
    if !scheme.eq_ignore_ascii_case(LOCUST_LINK_SCHEME) {
        return None;
    }
    let (place, query) = match rest.split_once('?') {
        Some((place, query)) => (place, query.split('#').next().unwrap_or("")),
        None => (rest.split('#').next().unwrap_or(""), ""),
    };
    // locust://new?..., and the locust:new?... and locust:///new?... spellings a system may hand over.
    let action = place.trim_start_matches('/').split('/').next().unwrap_or("").to_ascii_lowercase();
    if action != "new" {
        return None;
    }
    let given = query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (query_decode(key).as_deref() == Some("prompt")).then(|| query_decode(value))
    })??;
    let prompt = seen_text(&given);
    // Electron counts UTF-16 units.
    let length = prompt.encode_utf16().count();
    if length == 0 || length > MAX_LINK_PROMPT_CHARS {
        return None;
    }
    Some(LocustLink { prompt })
}

/// The link that opens Locust with `prompt` in the box (encodeURIComponent's spelling).
pub fn locust_link_for(prompt: &str) -> String {
    let mut out = format!("{LOCUST_LINK_SCHEME}://new?prompt=");
    for b in prompt.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// A link among a launch's arguments: Windows and Linux hand the link to the app that way.
pub fn link_in_arguments<S: AsRef<str>>(argv: &[S]) -> Option<LocustLink> {
    argv.iter().map(AsRef::as_ref).filter(|a| a.to_ascii_lowercase().starts_with(&format!("{LOCUST_LINK_SCHEME}:"))).find_map(parse_locust_link)
}

#[cfg(test)]
#[path = "locust_link_tests.rs"]
mod tests;
