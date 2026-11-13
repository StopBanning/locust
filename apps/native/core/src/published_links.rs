//! What a teammate published, as a card (Electron `renderer/src/publishedLinks.ts`, 0.727). A reply that names
//! something made elsewhere (a Claude artifact or doc, a Google Doc, Sheet or Slides, a Notion page, a Figma file, a
//! gist) ends with a card for each: what it is, its name, where it lives, and Open. No preview: each needs the person's
//! own sign-in. Only https and only these hosts, so a card never vouches for an address it does not recognise.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Published {
    pub url: String,
    /// What it is, in its own product's words: "Claude artifact", "Google Doc".
    pub kind: &'static str,
    /// The reply's own words for it when it gave some, else what its address says.
    pub title: String,
    pub host: String,
}

fn kind_of(host: &str, path: &str) -> Option<&'static str> {
    let starts = |p: &str| path.starts_with(p);
    let after = |prefix: &str| path.strip_prefix(prefix).unwrap_or("");
    if host == "claude.ai" && (starts("/artifact/") || starts("/code/artifact/") || starts("/public/artifacts/")) {
        return Some("Claude artifact");
    }
    if host == "docs.google.com" {
        if starts("/document/") {
            return Some("Google Doc");
        }
        if starts("/spreadsheets/") {
            return Some("Google Sheet");
        }
        if starts("/presentation/") {
            return Some("Google Slides");
        }
    }
    if (host == "notion.so" || host == "www.notion.so" || host.ends_with(".notion.site")) && path.len() > 1 {
        return Some("Notion page");
    }
    if (host == "figma.com" || host == "www.figma.com") && ["/file/", "/design/", "/board/", "/proto/", "/slides/"].iter().any(|p| starts(p)) {
        return Some("Figma file");
    }
    if host == "gist.github.com" {
        let mut parts = after("/").splitn(2, '/');
        let (user, id) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        if !user.is_empty() && id.chars().next().is_some_and(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
            return Some("Gist");
        }
    }
    None
}

/// A path segment as a person reads it: one holding a % that is not an escape stays as written (Electron 0.753).
fn decoded(segment: &str) -> String {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = |b: u8| (b as char).to_digit(16);
            match (bytes.get(i + 1).copied().and_then(hex), bytes.get(i + 2).copied().and_then(hex)) {
                (Some(h), Some(l)) => {
                    out.push((h * 16 + l) as u8);
                    i += 3;
                    continue;
                }
                _ => return segment.to_owned(),
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| segment.to_owned())
}

fn title_from_address(path: &str, kind: &str) -> String {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let last = decoded(segments.last().copied().unwrap_or(""));
    if kind == "Claude artifact" {
        let uuid = crate::invariants::regex(r"(?i)-?[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$");
        let short = crate::invariants::regex(r"-?[A-Za-z0-9]{22}$");
        let words = short.replace(&uuid.replace(&last, ""), "").into_owned();
        if !words.is_empty() && words != last {
            let spaced = words.replace('-', " ");
            let mut chars = spaced.chars();
            return chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default();
        }
    }
    if kind == "Notion page" {
        let words = crate::invariants::regex(r"(?i)-?[0-9a-f]{32}$").replace(&last, "").into_owned();
        if !words.is_empty() {
            return words.replace('-', " ");
        }
    }
    if kind == "Figma file"
        && let Some(name) = segments.get(2)
    {
        return decoded(name).replace('-', " ");
    }
    kind.to_owned()
}

/// What this address is, or None when it is not one Locust knows.
pub fn published_of(address: &str, words: Option<&str>) -> Option<Published> {
    let rest = address.strip_prefix("https://")?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    if authority.is_empty() || authority.contains('@') || authority.contains(char::is_whitespace) {
        return None;
    }
    let host = authority.split(':').next().unwrap_or("").to_ascii_lowercase();
    let after = &rest[end..];
    let path_end = after.find(['?', '#']).unwrap_or(after.len());
    let path = if after[..path_end].is_empty() { "/" } else { &after[..path_end] };
    let kind = kind_of(&host, path)?;
    let said = words.map(str::trim).filter(|w| !w.is_empty() && *w != address && !w.to_ascii_lowercase().starts_with("http://") && !w.to_ascii_lowercase().starts_with("https://"));
    let title = said.map(str::to_owned).unwrap_or_else(|| title_from_address(path, kind));
    // The URL as Electron's `url.href` spells it: the host lowercased, a bare host given its slash.
    let url = format!("https://{}{}{}", authority.to_ascii_lowercase(), if after.starts_with('/') { "" } else { "/" }, after);
    Some(Published { url, kind, title: title.chars().take(120).collect(), host })
}

/// Every known thing the reply names, in the order it names them, each once; none inside a code block.
pub fn published_in(reply: &str) -> Vec<Published> {
    // A code block is code, not something published: blanked, keeping every offset.
    let fence = crate::invariants::regex(r"(?s)```.*?(?:```|\z)");
    let text = fence.replace_all(reply, |c: &regex::Captures| " ".repeat(c[0].len())).into_owned();
    let mut found: Vec<(usize, Published)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut take = |at: usize, address: &str, words: Option<&str>| {
        let address = address.trim_end_matches(['.', ',', ';', ':', '!', '?']);
        if let Some(item) = published_of(address, words)
            && seen.insert(item.url.clone())
        {
            found.push((at, item));
        }
    };
    for c in crate::invariants::regex(r"\[([^\]\n]{1,200})\]\((https://[^\s)]+)\)").captures_iter(&text) {
        take(c.get(0).map_or(0, |m| m.start()), &c[2], Some(&c[1]));
    }
    // A bare address, not one already inside a link's parentheses or a word.
    for m in crate::invariants::regex(r#"https://[^\s<>"'`)\]]+"#).find_iter(&text) {
        let before = text[..m.start()].chars().next_back();
        if before.is_some_and(|c| c == '(' || c == '/' || c.is_alphanumeric() || c == '_') {
            continue;
        }
        take(m.start(), m.as_str(), None);
    }
    found.sort_by_key(|(at, _)| *at);
    found.into_iter().map(|(_, item)| item).collect()
}

/// A plain web address a reply may open in the person's browser (Electron's `isWebLink`): http or https with a host,
/// at most 2,048 characters. Every other scheme is refused, so a reply never becomes a way to reach this machine.
pub fn is_web_link(url: &str) -> bool {
    if url.len() > 2048 || url.chars().any(char::is_whitespace) {
        return false;
    }
    let lower = url.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("https://").or_else(|| lower.strip_prefix("http://")) else { return false };
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host = authority.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("");
    !host.is_empty()
}

pub const NOT_OPENED: &str = "Locust does not open that address.";
pub const NO_BROWSER: &str = "Your browser could not be opened.";

#[cfg(test)]
mod tests {
    use super::*;

    // Electron's what-a-teammate-published-is-a-card.test.tsx.
    #[test]
    fn knows_each_kind_by_its_address_and_names_it() {
        assert_eq!(
            published_of("https://claude.ai/code/artifact/q3-launch-plan-d01575a2-edd2-452f-b23d-8ca08658f374", None),
            Some(Published {
                url: "https://claude.ai/code/artifact/q3-launch-plan-d01575a2-edd2-452f-b23d-8ca08658f374".into(),
                kind: "Claude artifact",
                title: "Q3 launch plan".into(),
                host: "claude.ai".into()
            })
        );
        let kind = |a: &str| published_of(a, None).map(|p| p.kind);
        assert_eq!(kind("https://docs.google.com/document/d/1abc/edit"), Some("Google Doc"));
        assert_eq!(kind("https://docs.google.com/spreadsheets/d/1abc/edit"), Some("Google Sheet"));
        assert_eq!(kind("https://docs.google.com/presentation/d/1abc/edit"), Some("Google Slides"));
        let notion = published_of("https://www.notion.so/acme/Launch-checklist-0123456789abcdef0123456789abcdef", None).unwrap();
        assert_eq!((notion.kind, notion.title.as_str()), ("Notion page", "Launch checklist"));
        let figma = published_of("https://www.figma.com/design/AbC123/Checkout-flow?node-id=1-2", None).unwrap();
        assert_eq!((figma.kind, figma.title.as_str()), ("Figma file", "Checkout flow"));
        assert_eq!(kind("https://gist.github.com/octo-cat/0123456789abcdef"), Some("Gist"));
    }

    #[test]
    fn takes_the_replys_own_words_for_a_name_and_vouches_for_nothing_it_does_not_know() {
        assert_eq!(published_of("https://docs.google.com/document/d/1abc/edit", Some("The brand guide")).unwrap().title, "The brand guide");
        assert_eq!(published_of("https://example.com/document/d/1abc", None), None);
        assert_eq!(published_of("http://docs.google.com/document/d/1abc", None), None);
        assert_eq!(published_of("https://docs.google.com/forms/d/1abc", None), None);
        assert_eq!(published_of("not an address", None), None);
        assert_eq!(published_of("https://user@docs.google.com/document/d/1abc", None), None);
    }

    #[test]
    fn finds_them_as_links_and_bare_addresses_in_order_each_once_and_none_inside_code() {
        let reply = [
            "Done. The plan is up: [Q3 plan](https://docs.google.com/document/d/1plan/edit),",
            "and the tracker is https://docs.google.com/spreadsheets/d/1track/edit.",
            "Same plan again: https://docs.google.com/document/d/1plan/edit",
            "```",
            "curl https://docs.google.com/document/d/1code/edit",
            "```",
            "Docs on https://example.com/help too.",
        ]
        .join("\n");
        let found: Vec<(&str, String)> = published_in(&reply).into_iter().map(|p| (p.kind, p.title)).collect();
        assert_eq!(found, vec![("Google Doc", "Q3 plan".to_owned()), ("Google Sheet", "Google Sheet".to_owned())]);
        let page = published_in("The page: https://claude.ai/artifact/pricing-page-AbCdEfGhIjKlMnOpQrStUv");
        assert_eq!((page[0].title.as_str(), page[0].kind, page[0].host.as_str()), ("Pricing page", "Claude artifact", "claude.ai"));
        assert!(published_in("Nothing to see at https://example.com").is_empty());
    }

    #[test]
    fn only_a_plain_web_address_opens() {
        assert!(is_web_link("https://example.com/help") && is_web_link("http://localhost:3000/"));
        for no in ["file:///etc/passwd", "javascript:alert(1)", "locust://new?prompt=x", "https://", "ftp://example.com", "https://exa mple.com"] {
            assert!(!is_web_link(no), "{no}");
        }
        assert!(!is_web_link(&format!("https://example.com/{}", "a".repeat(2048))));
    }

    #[test]
    fn a_stray_percent_in_an_address_is_kept_as_written() {
        let notion = published_of("https://www.notion.so/Launch-plan-100%-done", None).unwrap();
        assert_eq!((notion.kind, notion.title.as_str()), ("Notion page", "Launch plan 100% done"));
        assert_eq!(published_of("https://www.figma.com/design/AbC123/Cart-%C3", None).unwrap().title, "Cart %C3");
        assert_eq!(published_in("Done: https://www.notion.so/Launch-plan-100%-done")[0].title, "Launch plan 100% done");
    }
}
