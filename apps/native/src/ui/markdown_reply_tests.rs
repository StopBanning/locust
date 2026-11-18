//! A reply's Markdown as the thread draws it: Electron's `a-blank-line-does-not-end-a-list.test.tsx`,
//! `a-wrapped-bullet-stays-one-bullet.test.ts`, `nested-lists.test.ts`, `links-go-somewhere.test.ts` and
//! `a-quoted-equation-is-drawn.test.tsx`, with their values and words.
//!
//! Electron checks its own parser's blocks (`parseAgentText`). Here the same text goes through what the window
//! draws a reply with: `markdown_prep::prepare_reply` (the steps `reply_markdown` runs), then rune-kit's parser,
//! and for what a person sees, `reply_markdown` itself drawn headless.
use super::*;
use rune_kit::markdown::{Block, parse};

fn lines(rows: &[&str]) -> String {
    rows.join("\n")
}

/// The blocks the reply is drawn from.
fn blocks(text: &str) -> Vec<Block> {
    parse(&crate::markdown_prep::prepare_reply(text, false).markdown)
}

/// The top-level lists, each as (start, items).
fn lists(blocks: &[Block]) -> Vec<(Option<u64>, &Vec<Vec<Block>>)> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::List { start, items } => Some((*start, items)),
            _ => None,
        })
        .collect()
}

fn para_text(b: &Block) -> Option<&str> {
    match b {
        Block::Para(i) | Block::Heading(_, i) => Some(&i.text),
        _ => None,
    }
}

/// An item's own line (its first paragraph) and the paragraphs after it.
fn item_words(item: &[Block]) -> (String, Vec<String>) {
    let mut paras = item.iter().filter_map(para_text).map(str::to_owned);
    (paras.next().unwrap_or_default(), paras.collect())
}

/// Every item of the first list at any depth, in order, as (its line, its depth), the way Electron's flat items
/// carry `depth`.
fn items_with_depth(blocks: &[Block]) -> Vec<(String, usize)> {
    fn walk(items: &[Vec<Block>], depth: usize, out: &mut Vec<(String, usize)>) {
        for item in items {
            out.push((item_words(item).0, depth));
            for b in item {
                if let Block::List { items, .. } = b {
                    walk(items, depth + 1, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    if let Some((_, items)) = lists(blocks).first() {
        walk(items, 0, &mut out);
    }
    out
}

fn kinds(blocks: &[Block]) -> Vec<&'static str> {
    blocks
        .iter()
        .map(|b| match b {
            Block::List { .. } => "list",
            Block::Para(_) => "text",
            Block::Quote(_) => "quote",
            Block::Code(_) => "code",
            Block::Image { .. } => "image",
            Block::Heading(..) => "heading",
            _ => "other",
        })
        .collect()
}

/// All the words a person reads in these blocks (links read by their destination too).
fn drawn_words(blocks: &[Block], out: &mut Vec<String>) {
    fn push(out: &mut Vec<String>, text: &str) {
        out.extend(text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned));
    }
    for b in blocks {
        match b {
            Block::Para(i) | Block::Heading(_, i) => {
                // A bold run inside a word (`mcp**github**create_issue`) is still its own words.
                let mut cuts: Vec<usize> = i.spans.iter().flat_map(|s| [s.0, s.1]).collect();
                cuts.extend(i.links.iter().flat_map(|l| [l.0, l.1]));
                cuts.sort_unstable();
                let mut at = 0;
                for cut in cuts.into_iter().chain([i.text.len()]) {
                    if cut >= at && i.text.is_char_boundary(cut) {
                        push(out, &i.text[at..cut]);
                        at = cut;
                    }
                }
                for (_, _, to) in &i.links {
                    push(out, to);
                }
            }
            Block::Code(c) | Block::Diff(c) | Block::Mermaid(c) => push(out, c),
            Block::Image { alt, .. } => push(out, alt),
            Block::Table(t) => {
                for cell in t.head.iter().chain(t.rows.iter().flatten()) {
                    push(out, &cell.text);
                }
            }
            Block::Rule => {}
            Block::Quote(inner) => drawn_words(inner, out),
            Block::List { items, .. } => {
                for item in items {
                    drawn_words(item, out);
                }
            }
        }
    }
}

/// Electron's `segmentsCoverInput`: no word of the reply is lost on the way to the screen. A list item's number is
/// drawn as its marker, so it is not looked for among the words; nor is a fence line.
fn missing_word(text: &str, blocks: &[Block]) -> Option<String> {
    let mut words = Vec::new();
    drawn_words(blocks, &mut words);
    let mut have = words.iter();
    let marker = crate::invariants::regex(r"^[\s>]*(?:[-*+]|\d{1,9}[.)])(?:\s|$)");
    // A fence line (its language too) is the fence's syntax, like a list's number.
    text.lines()
        .filter(|line| !line.trim_start().starts_with("```") && !line.trim_start().starts_with("~~~"))
        .flat_map(|line| {
            let line = marker.find(line).map_or(line, |m| &line[m.end()..]);
            line.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(str::to_owned).collect::<Vec<_>>()
        })
        .find(|w| !have.any(|h| h == w))
}

fn covers(text: &str, blocks: &[Block]) -> bool {
    missing_word(text, blocks).is_none()
}

/// The reply drawn headless: each drawn element's (role, text).
fn drawn(text: &str) -> Vec<(&'static str, String)> {
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    let body = text.to_owned();
    let mut rt = rune::Runtime::new(
        (s, body),
        |(s, body), _| crate::reply_actions::reply_markdown_for_tests(s, body, &Palette::new(false)),
        (700.0, 2000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().map(|e| (e.role, e.text.unwrap_or_default())).collect()
}

fn count_role(drawn: &[(&'static str, String)], role: &str) -> usize {
    drawn.iter().filter(|(r, _)| *r == role).count()
}

fn texts(drawn: &[(&'static str, String)]) -> String {
    drawn.iter().map(|(_, t)| t.as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n")
}

const CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");

/// One entry's body, the way the What changed page is handed it.
fn entry(version: &str) -> String {
    let part = CHANGELOG
        .split("\n## ")
        .find(|c| c.starts_with(&format!("{version} ")))
        .unwrap_or("");
    part.split('\n').skip(1).collect::<Vec<_>>().join("\n").trim().to_owned()
}

// a-blank-line-does-not-end-a-list.test.tsx

#[test]
fn keeps_numbered_steps_with_blank_lines_between_them_as_one_list() {
    let text = lines(&["1. **Install**", "", "2. **Sign in**", "", "3. **Start a mission**"]);
    let b = blocks(&text);
    let l = lists(&b);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].0, Some(1));
    let words: Vec<String> = l[0].1.iter().map(|i| item_words(i).0).collect();
    // Electron keeps the `**`; drawn, they are bold.
    assert_eq!(words, ["Install", "Sign in", "Start a mission"]);
    assert!(covers(&text, &b));
}

#[test]
fn draws_them_1_2_3_not_1_1_1() {
    let d = drawn(&lines(&["1. one", "", "2. two", "", "3. three"]));
    assert_eq!(count_role(&d, "list"), 1, "{d:?}");
    assert_eq!(count_role(&d, "listitem"), 3, "{d:?}");
    let all = texts(&d);
    for marker in ["1.", "2.", "3."] {
        assert_eq!(all.lines().filter(|l| *l == marker).count(), 1, "{marker} in {all}");
    }
}

#[test]
fn keeps_an_indented_second_paragraph_inside_its_item() {
    let text = lines(&[
        "- **Title.** Line one",
        "  wraps here.",
        "",
        "  A second paragraph,",
        "  hard wrapped in the source.",
        "",
        "- Next item",
    ]);
    let b = blocks(&text);
    assert_eq!(b.len(), 1);
    let l = lists(&b);
    let (first, paragraphs) = item_words(&l[0].1[0]);
    assert_eq!(first, "Title. Line one wraps here.");
    // Joined, not broken where the source file happened to wrap.
    assert_eq!(paragraphs, ["A second paragraph, hard wrapped in the source."]);
    assert_eq!(item_words(&l[0].1[1]).0, "Next item");
    assert!(covers(&text, &b));
}

#[test]
fn reads_the_changelog_entry_that_showed_it_with_the_paragraph_inside_its_bullet() {
    let body = entry("0.258.0");
    assert!(!body.is_empty());
    let b = blocks(&body);
    // Two bullets, ONE list, and no stray paragraph between them.
    assert_eq!(kinds(&b), ["list"]);
    let (_, paragraphs) = item_words(&lists(&b)[0].1[0]);
    assert!(paragraphs[0].starts_with("A command-line tool cannot use"), "{paragraphs:?}");
    assert!(covers(&body, &b));
}

#[test]
fn drops_no_words_from_any_entry_in_the_changelog() {
    let versions: Vec<&str> = CHANGELOG
        .lines()
        .filter_map(|l| l.strip_prefix("## "))
        .filter_map(|l| l.split(' ').next())
        .filter(|v| v.split('.').count() == 3 && v.split('.').all(|n| !n.is_empty() && n.bytes().all(|c| c.is_ascii_digit())))
        .collect();
    assert!(versions.len() > 50);
    for version in versions {
        let body = entry(version);
        assert_eq!(missing_word(&body, &blocks(&body)), None, "{version}");
    }
}

/// The changelog's case, in a reply: a line that opens with a tag is drawn as the words written, not dropped.
#[test]
fn a_line_that_opens_like_markup_is_drawn_as_words_not_dropped() {
    for text in ["<!-- big -->\n\n### New", "<div>hello there</div>", "> <span>quoted words</span>"] {
        assert_eq!(missing_word(text, &blocks(text)), None, "{text}");
    }
    assert!(texts(&drawn("<div>hello there</div>")).contains("<div>hello there</div>"));
    // A fenced one is code, as written.
    assert_eq!(blocks("```\n<div>x</div>\n```"), [Block::Code("<div>x</div>".into())]);
}

/// The words-not-markup rule must not reach inside math: `<` in an equation is a sign.
#[test]
fn an_equation_with_a_less_than_sign_is_still_drawn() {
    for text in ["So $0<x<1$ holds.", "So \\(a<b\\) holds.", "\\[ x_0<y \\]"] {
        let prepared = crate::markdown_prep::prepare_reply(text, false);
        assert_eq!(prepared.math.len(), 1, "{text}: {}", prepared.markdown);
        assert!(prepared.markdown.contains(crate::math_tex::MATH_SRC_PREFIX), "{text}: {}", prepared.markdown);
        assert!(!prose_of(&blocks(text)).contains('<'), "{text}");
    }
}

#[test]
fn resumes_the_numbering_after_a_code_block_splits_the_steps() {
    let text = lines(&["1. one", "2. two", "```", "npm test", "```", "3. three"]);
    assert_eq!(lists(&blocks(&text)).iter().map(|l| l.0).collect::<Vec<_>>(), [Some(1), Some(3)]);
    let all = texts(&drawn(&text));
    // Electron's `<ol start="3">`: the third step is drawn "3.", not "1.".
    let markers: Vec<&str> = all.lines().filter(|l| l.ends_with('.') && l[..l.len() - 1].bytes().all(|c| c.is_ascii_digit())).collect();
    assert_eq!(markers, ["1.", "2.", "3."], "{all}");
}

#[test]
fn still_ends_the_list_at_prose_that_is_not_indented_under_it() {
    let text = lines(&["- fast", "- small", "", "That is the whole case for it."]);
    let b = blocks(&text);
    assert_eq!(kinds(&b), ["list", "text"]);
    assert!(lists(&b)[0].1.iter().all(|i| item_words(i).1.is_empty()));
}

#[test]
fn still_starts_a_new_list_when_the_kind_changes_after_the_blank_line() {
    let b = blocks(&lines(&["- a bullet", "", "1. a step"]));
    // Electron's `ordered`: a numbered list has a start.
    assert_eq!(lists(&b).iter().map(|l| l.0.is_some()).collect::<Vec<_>>(), [false, true]);
}

// a-wrapped-bullet-stays-one-bullet.test.ts

#[test]
fn a_bullet_that_wraps_stays_one_bullet_with_the_whole_sentence_in_it() {
    let text = lines(&[
        "- **The decision card says one thing once.** When a teammate stops and asks",
        "  you to choose, the card told you twice that you could answer in your own",
        "  words instead.",
    ]);
    let b = blocks(&text);
    assert_eq!(kinds(&b).iter().filter(|k| **k == "text").count(), 0);
    let l = lists(&b);
    assert_eq!(l[0].1.len(), 1);
    assert_eq!(
        item_words(&l[0].1[0]).0,
        "The decision card says one thing once. When a teammate stops and asks you to choose, the card told you twice that you could answer in your own words instead."
    );
}

#[test]
fn keeps_wrapping_with_the_item_it_belongs_to_not_the_one_before() {
    let b = blocks(&lines(&["- first item", "  wrapped onto a second line", "- second item", "  and its own wrap"]));
    let words: Vec<String> = lists(&b)[0].1.iter().map(|i| item_words(i).0).collect();
    assert_eq!(words, ["first item wrapped onto a second line", "second item and its own wrap"]);
}

#[test]
fn wraps_a_nested_item_into_the_nested_item() {
    let b = blocks(&lines(&["- outer", "  - inner", "    wrapped inner line", "- next outer"]));
    assert_eq!(
        items_with_depth(&b),
        [("outer".into(), 0), ("inner wrapped inner line".into(), 1), ("next outer".into(), 0)]
    );
}

#[test]
fn a_wrapped_bullet_still_ends_the_list_at_a_blank_line() {
    let b = blocks(&lines(&["- a", "", "prose after the list"]));
    assert_eq!(items_with_depth(&b), [("a".into(), 0)]);
    assert!(kinds(&b).contains(&"text"));
}

#[test]
fn still_ends_the_list_at_unindented_prose_which_a_model_usually_means_as_prose() {
    // CommonMark calls this a lazy continuation; Electron, and so the window, reads it as prose.
    let b = blocks(&lines(&["- a", "prose at the margin"]));
    assert_eq!(items_with_depth(&b), [("a".into(), 0)]);
    assert!(kinds(&b).contains(&"text"));
}

#[test]
fn a_wrapped_bullet_drops_nothing() {
    let text = lines(&["intro", "- a", "  wrapped", "- b", "", "after"]);
    assert!(covers(&text, &blocks(&text)));
}

// nested-lists.test.ts

#[test]
fn reads_two_space_indents_as_depth() {
    let b = blocks(&lines(&["- one", "  - one a", "  - one b", "- two"]));
    assert_eq!(
        items_with_depth(&b),
        [("one".into(), 0), ("one a".into(), 1), ("one b".into(), 1), ("two".into(), 0)]
    );
}

#[test]
fn reads_four_space_indents_as_the_same_depth_as_two_space() {
    let depths = |t: &str| items_with_depth(&blocks(t)).into_iter().map(|(_, d)| d).collect::<Vec<_>>();
    assert_eq!(depths(&lines(&["- one", "  - child"])), [0, 1]);
    assert_eq!(depths(&lines(&["- one", "    - child"])), [0, 1]);
    assert_eq!(depths(&lines(&["- one", "\t- child"])), [0, 1]);
}

#[test]
fn comes_back_out_of_a_nesting() {
    let b = blocks(&lines(&["- a", "  - a1", "    - a2", "  - a3", "- b"]));
    assert_eq!(items_with_depth(&b).into_iter().map(|(_, d)| d).collect::<Vec<_>>(), [0, 1, 2, 1, 0]);
}

#[test]
fn treats_a_numbered_list_nested_under_a_bullet_as_a_child_not_a_new_list() {
    let b = blocks(&lines(&["- one", "  1. first", "  2. second", "- two"]));
    assert_eq!(lists(&b).len(), 1);
    assert_eq!(items_with_depth(&b).into_iter().map(|(_, d)| d).collect::<Vec<_>>(), [0, 1, 1, 0]);
}

#[test]
fn still_starts_a_new_list_when_the_marker_changes_at_the_same_depth() {
    assert_eq!(lists(&blocks(&lines(&["- a", "1. b"]))).len(), 2);
}

#[test]
fn a_nested_list_drops_nothing_at_any_depth() {
    let text = lines(&["intro", "- a", "  - b", "    - c", "after"]);
    let b = blocks(&text);
    assert!(covers(&text, &b));
    // "after" is prose after the list, not part of "c".
    assert_eq!(items_with_depth(&b).last(), Some(&("c".into(), 2)));
    assert_eq!(kinds(&b), ["text", "list", "text"]);
}

#[test]
fn a_flat_list_is_still_flat() {
    assert!(items_with_depth(&blocks(&lines(&["- a", "- b"]))).iter().all(|(_, d)| *d == 0));
}

// links-go-somewhere.test.ts

/// The links in a one-paragraph reply: (label, destination).
fn links_of(text: &str) -> Vec<(String, String)> {
    let b = blocks(text);
    let Some(Block::Para(i)) = b.first() else { panic!("{b:?}") };
    i.links.iter().map(|(a, z, to)| (i.text[*a..*z].to_owned(), to.clone())).collect()
}

fn para_of(text: &str) -> String {
    blocks(text).iter().filter_map(para_text).collect::<Vec<_>>().join("\n")
}

#[test]
fn a_link_inside_emphasis_is_a_link_not_four_asterisks_and_a_url() {
    let text = "Amodei published *[We Must Pace the Frontier](https://darioamodei.com/post/we-must-pace-the-frontier)*: slow growth.";
    assert_eq!(
        links_of(text),
        [("We Must Pace the Frontier".into(), "https://darioamodei.com/post/we-must-pace-the-frontier".into())]
    );
    // And nothing anywhere still shows the syntax.
    assert!(!para_of(text).contains("]("));
    assert!(!texts(&drawn(text)).contains("]("));
}

#[test]
fn a_link_works_for_bold_as_well_as_italic() {
    for wrap in ["**", "__", "*", "_"] {
        let links = links_of(&format!("see {wrap}[the essay](https://example.com/x){wrap} for more"));
        assert_eq!(links.iter().map(|l| l.0.as_str()).collect::<Vec<_>>(), ["the essay"], "{wrap}");
    }
}

#[test]
fn leaves_a_plain_link_and_plain_emphasis_exactly_as_they_were() {
    assert_eq!(links_of("[label](https://example.com)"), [("label".into(), "https://example.com".into())]);
    let b = blocks("this is **bold** here");
    let Some(Block::Para(i)) = b.first() else { panic!() };
    let strong: Vec<&str> = i.spans.iter().filter(|s| s.2 == rune_kit::markdown::Style::Strong).map(|s| &i.text[s.0..s.1]).collect();
    assert_eq!(strong, ["bold"]);
    // The case that has to keep working: an underscore inside a name.
    let b = blocks("snake_case_name");
    let Some(Block::Para(i)) = b.first() else { panic!() };
    assert_eq!(i.text, "snake_case_name");
    assert!(i.spans.is_empty() && i.links.is_empty());
}

#[test]
fn keeps_a_code_span_literal_even_when_it_looks_like_a_link() {
    let b = blocks("run `[x](y)` first");
    let Some(Block::Para(i)) = b.first() else { panic!() };
    let code: Vec<&str> = i.spans.iter().filter(|s| s.2 == rune_kit::markdown::Style::Code).map(|s| &i.text[s.0..s.1]).collect();
    assert_eq!(code, ["[x](y)"]);
    assert!(i.links.is_empty());
}

#[test]
fn accepts_an_ordinary_web_address() {
    use locust_core::published_links::is_web_link;
    assert!(is_web_link("https://darioamodei.com/post/we-must-pace-the-frontier"));
    assert!(is_web_link("http://example.com"));
}

#[test]
fn refuses_every_way_of_reaching_this_machine() {
    use locust_core::published_links::is_web_link;
    for url in [
        "file:///C:/Users/<home>/.ssh/id_rsa",
        "vscode://file/C:/secret",
        "javascript:alert(1)",
        "data:text/html,<script>",
        "C:/Users/<home>/streaks/src/streak.test.js",
        "mailto:someone@example.com",
        "https://",
        // Electron's `undefined`: no address at all.
        "",
    ] {
        assert!(!is_web_link(url), "{url}");
    }
    assert!(!is_web_link(&format!("https://example.com/{}", "x".repeat(3000))));
    // And pressing one in a reply opens nothing and says so.
    let mut s = State::default();
    crate::published_cards::open_web_link(&mut s, "file:///C:/Users/<home>/.ssh/id_rsa");
    assert!(s.opened_links.is_empty());
    assert_eq!(s.link_refusal.as_ref().map(|r| r.1.as_str()), Some(locust_core::published_links::NOT_OPENED));
}

#[test]
fn names_where_a_link_actually_goes_so_a_label_cannot_lie_about_it() {
    use crate::markdown_prep::link_host;
    assert_eq!(link_host("https://darioamodei.com/post/x").as_deref(), Some("darioamodei.com"));
    assert_eq!(link_host("https://www.reuters.com/article").as_deref(), Some("reuters.com"));
    // A local path has no host, which is exactly how the renderer knows not to make it clickable.
    assert_eq!(link_host("C:/Users/<home>/notes.md"), None);
}

// a-quoted-equation-is-drawn.test.tsx

const REPLY: &[&str] = &[
    "Each entry of the product is a row of the first matrix times a column of the second.",
    "",
    "**A version you could write down:**",
    "",
    "> Let \\(M=\\begin{bmatrix}A&B\\\\C&D\\end{bmatrix}\\) and \\(N=\\begin{bmatrix}E&F\\\\G&H\\end{bmatrix}\\). For \\(1\\le i,j\\le n\\),",
    "> \\[",
    "> (MN)_{ij}",
    "> =",
    "> \\sum_{k=1}^{n}A_{ik}E_{kj}",
    "> +",
    "> \\sum_{k=1}^{n}B_{ik}G_{kj}",
    "> =",
    "> (AE+BG)_{ij}.",
    "> \\]",
    "> The other three blocks follow the same way, so",
    "> \\[",
    "> MN=",
    "> \\begin{bmatrix}",
    "> AE+BG&AF+BH\\\\",
    "> CE+DG&CF+DH",
    "> \\end{bmatrix}.",
    "> \\]",
];

/// The equations drawn as pictures in these blocks, at any depth: (alt, display).
fn equations(blocks: &[Block], out: &mut Vec<String>) {
    for b in blocks {
        match b {
            Block::Image { src, alt, .. } if src.starts_with(crate::math_tex::MATH_SRC_PREFIX) => out.push(alt.clone()),
            Block::Quote(inner) => equations(inner, out),
            Block::List { items, .. } => items.iter().for_each(|i| equations(i, out)),
            _ => {}
        }
    }
}

/// The words a person reads, without the pictures' names.
fn prose_of(blocks: &[Block]) -> String {
    let mut out = Vec::new();
    for b in blocks {
        match b {
            Block::Para(i) | Block::Heading(_, i) => out.push(i.text.clone()),
            Block::Quote(inner) => out.push(prose_of(inner)),
            Block::List { items, .. } => items.iter().for_each(|i| out.push(prose_of(i))),
            _ => {}
        }
    }
    out.join("\n")
}

#[test]
fn a_quote_is_drawn_as_a_quote_not_as_a_paragraph() {
    let d = drawn("> Check the units before you divide.");
    assert_eq!(count_role(&d, "blockquote"), 1, "{d:?}");
    assert!(texts(&d).contains("Check the units before you divide."));
}

#[test]
fn a_displayed_equation_inside_a_quote_is_drawn_and_none_of_its_tex_is_left_showing() {
    let text = REPLY.join("\n");
    let b = blocks(&text);
    let Some(Block::Quote(quote)) = b.iter().find(|b| matches!(b, Block::Quote(_))) else { panic!("{b:?}") };
    // Both of the quote's displayed equations, inside the quote, with the inline ones before them.
    let mut shown = Vec::new();
    equations(quote, &mut shown);
    let display: Vec<&String> = shown.iter().filter(|a| a.contains("_{ij}") || a.starts_with("MN=")).collect();
    assert_eq!(display.len(), 2, "{shown:?}");
    let words = prose_of(&b);
    for tex in ["\\begin{bmatrix}", "\\sum_{k=1}", "\\[", "\\]", "begin{bmatrix}", "sum_{k=1}"] {
        assert!(!words.contains(tex), "{tex} left in {words}");
    }
    // The words around the equations are still there.
    assert!(words.contains("The other three blocks follow the same way, so"), "{words}");
    let d = drawn(&text);
    assert_eq!(count_role(&d, "blockquote"), 1, "{d:?}");
    let all = texts(&d);
    assert!(!all.contains("\\begin{bmatrix}") && !all.contains("\\sum_{k=1}") && !all.contains("\\["), "{all}");
    assert!(all.contains("The other three blocks follow the same way, so"), "{all}");
}

#[test]
fn a_quote_inside_a_quote_and_a_list_inside_a_quote_keep_their_shape() {
    let d = drawn(&lines(&["> Two things:", "> - the rows", "> - the columns", "> > and a note inside the note"]));
    assert_eq!(count_role(&d, "list"), 1, "{d:?}");
    assert_eq!(count_role(&d, "blockquote"), 2, "{d:?}");
}

#[test]
fn an_equation_written_in_the_middle_of_a_line_is_drawn_too() {
    let b = blocks("So \\[ (AB)^T = B^T A^T \\] holds for any two, and $$x^2$$ too.");
    let mut shown = Vec::new();
    equations(&b, &mut shown);
    assert_eq!(shown, ["(AB)^T = B^T A^T", "x^2"]);
    // Displayed: each its own block, between the words.
    assert_eq!(kinds(&b), ["text", "image", "text", "image", "text"]);
    // A list item's wrapped lines: the equation is drawn inside the item, and the words after it stay there too.
    let text = lines(&["1. The rule is", "   \\[ (AB)^T = B^T A^T \\]", "   for any two matrices."]);
    let b = blocks(&text);
    assert_eq!(kinds(&b), ["list"]);
    let mut shown = Vec::new();
    equations(&b, &mut shown);
    assert_eq!(shown, ["(AB)^T = B^T A^T"]);
    assert!(!prose_of(&b).contains("\\["));
    assert!(prose_of(&b).contains("for any two matrices."));
}

#[test]
fn money_and_an_escaped_bracket_in_prose_are_still_prose() {
    for text in ["It costs $5, or $$ if you upgrade.", "As shown in \\[1\\] and \\[2\\], it holds.", "See [the notes](notes.md) for more."] {
        let mut shown = Vec::new();
        equations(&blocks(text), &mut shown);
        assert!(shown.is_empty(), "{text}: {shown:?}");
    }
    assert_eq!(para_of("It costs $5, or $$ if you upgrade."), "It costs $5, or $$ if you upgrade.");
    assert_eq!(para_of("As shown in \\[1\\] and \\[2\\], it holds."), "As shown in [1] and [2], it holds.");
}

// agentText.test.ts: Electron's parser cases, on what the window draws a reply from.

fn para(text: &str) -> Block {
    Block::Para(rune_kit::markdown::Inline { text: text.into(), ..Default::default() })
}

/// The one paragraph's spans of this style, as text.
fn styled(text: &str, style: rune_kit::markdown::Style) -> Vec<String> {
    let b = blocks(text);
    let Some(Block::Para(i)) = b.first() else { panic!("{b:?}") };
    i.spans.iter().filter(|s| s.2 == style).map(|s| i.text[s.0..s.1].to_owned()).collect()
}

/// The one paragraph's text, without the narrow spaces drawn around inline code.
fn shown(text: &str) -> String {
    para_of(text).replace('\u{202f}', "")
}

fn heading(level: u8, text: &str) -> Block {
    Block::Heading(level, rune_kit::markdown::Inline { text: text.into(), ..Default::default() })
}

fn shape_kinds(blocks: &[Block]) -> Vec<&'static str> {
    blocks
        .iter()
        .map(|b| match b {
            Block::Rule => "rule",
            Block::Table(_) => "table",
            other => kinds(std::slice::from_ref(other))[0],
        })
        .collect()
}

#[test]
fn pulls_a_fenced_block_out_of_the_prose_around_it() {
    let b = blocks(&lines(&["Here is the patch:", "```diff", "-const a = 1", "+const a = 2", "```", "Apply it when ready."]));
    assert_eq!(b, [para("Here is the patch:"), Block::Diff("-const a = 1\n+const a = 2".into()), para("Apply it when ready.")]);
}

#[test]
fn treats_an_unclosed_fence_as_code_still_being_written_not_as_prose() {
    assert_eq!(blocks(&lines(&["Working on it:", "```ts", "const x = 1"])), [para("Working on it:"), Block::Code("const x = 1".into())]);
}

#[test]
fn keeps_a_longer_fence_inside_a_shorter_one_as_code() {
    assert_eq!(blocks(&lines(&["````", "```", "still code", "````"])), [Block::Code("```\nstill code".into())]);
}

#[test]
fn reads_a_fence_with_no_language() {
    assert_eq!(blocks(&lines(&["```", "plain", "```"])), [Block::Code("plain".into())]);
}

#[test]
fn leaves_a_reply_with_no_fences_as_a_single_block_of_prose() {
    assert_eq!(blocks("Just an answer."), [para("Just an answer.")]);
}

#[test]
fn drops_nothing_whatever_the_shape_of_the_reply() {
    for input in [
        String::new(),
        "plain".into(),
        lines(&["a", "```", "b", "```", "c"]),
        lines(&["```", "only code", "```"]),
        lines(&["unclosed", "```js", "x"]),
        lines(&["```", "```"]),
        lines(&["one", "", "two"]),
        lines(&["trailing fence", "```"]),
        lines(&["```", "a", "````", "b", "```"]),
    ] {
        assert_eq!(missing_word(&input, &blocks(&input)), None, "{input:?}");
    }
}

#[test]
fn handles_crlf_the_way_a_windows_runtime_emits_it() {
    assert_eq!(blocks("intro\r\n```\r\ncode\r\n```"), [para("intro"), Block::Code("code".into())]);
}

#[test]
fn lifts_a_run_of_bullets_out_of_the_prose_instead_of_running_them_together() {
    let b = blocks(&lines(&["Implemented the streak fix.", "- currentStreak counts yesterday", "- It resets once missed", "Verification: 3 tests passed."]));
    assert_eq!(kinds(&b), ["text", "list", "text"]);
    assert_eq!(para_text(&b[0]), Some("Implemented the streak fix."));
    assert_eq!(items_with_depth(&b), [("currentStreak counts yesterday".into(), 0), ("It resets once missed".into(), 0)]);
    assert_eq!(para_text(&b[2]), Some("Verification: 3 tests passed."));
}

#[test]
fn reads_a_numbered_list_as_ordered() {
    // `2)` after `1.` is the same list: CommonMark would start a second one.
    let b = blocks(&lines(&["1. first", "2) second"]));
    let l = lists(&b);
    assert_eq!(l.len(), 1, "{b:?}");
    assert_eq!(l[0].0, Some(1));
    assert_eq!(items_with_depth(&b), [("first".into(), 0), ("second".into(), 0)]);
    let all = texts(&drawn(&lines(&["1. first", "2) second"])));
    let markers: Vec<&str> = all.lines().filter(|l| *l == "1." || *l == "2.").collect();
    assert_eq!(markers, ["1.", "2."], "{all}");
}

#[test]
fn starts_a_new_list_when_the_kind_changes() {
    let b = blocks(&lines(&["- a", "1. b"]));
    assert_eq!(lists(&b).iter().map(|l| l.0).collect::<Vec<_>>(), [None, Some(1)]);
}

#[test]
fn does_not_treat_a_sentence_containing_a_dash_as_a_list() {
    let text = "It resets - once yesterday is missing.";
    assert_eq!(blocks(text), [para(text)]);
}

#[test]
fn a_list_still_drops_nothing() {
    for input in [lines(&["intro", "- one", "- two"]), lines(&["- only"]), lines(&["1. a", "", "after"]), lines(&["- a", "```", "code", "```", "- b"])] {
        assert_eq!(missing_word(&input, &blocks(&input)), None, "{input:?}");
    }
}

#[test]
fn shows_the_label_and_keeps_the_target_off_the_sentence() {
    let text = "see [src/streak.test.js](C:/w/src/streak.test.js) for the cases";
    assert_eq!(para_of(text), "see src/streak.test.js for the cases");
    assert_eq!(links_of(text), [("src/streak.test.js".into(), "C:/w/src/streak.test.js".into())]);
}

#[test]
fn leaves_brackets_that_are_not_a_link_alone() {
    assert_eq!(blocks("an array [1, 2] and a note"), [para("an array [1, 2] and a note")]);
}

#[test]
fn marks_a_matched_pair_and_leaves_the_words_around_it_alone() {
    assert_eq!(styled("run `pnpm test` first", rune_kit::markdown::Style::Code), ["pnpm test"]);
    assert_eq!(shown("run `pnpm test` first"), "run pnpm test first");
}

#[test]
fn leaves_a_lone_backtick_visible_rather_than_swallowing_the_rest() {
    assert_eq!(blocks("it's a ` and nothing else"), [para("it's a ` and nothing else")]);
}

#[test]
fn inline_code_never_loses_text() {
    // Electron's spans joined, with code shown without its ticks.
    assert!(blocks("").is_empty());
    for (input, seen) in [("a`b`c", "abc"), ("`x`", "x"), ("no ticks", "no ticks"), ("``", "``"), ("a `b` c `d` e", "a b c d e")] {
        assert_eq!(shown(input), seen, "{input}");
    }
}

#[test]
fn a_lone_tex_macro_is_drawn_as_its_symbol_in_the_words_around_it() {
    assert_eq!(blocks("README.md $\\rightarrow$ docs/README.md"), [para("README.md → docs/README.md")]);
    assert_eq!(blocks("a $\\times$ b $\\le$ c, and $ \\to $ d"), [para("a × b ≤ c, and → d")]);
}

#[test]
fn leaves_money_and_code_as_they_were_written_and_a_whole_formula_is_math() {
    for text in ["$5 and $10", "costs $\\,5"] {
        assert_eq!(para_of(text), text.replace("\\,", ","), "{text}");
    }
    // pulldown reads `\,` as an escaped comma, as Markdown does; the dollar is not math.
    let mut shown_math = Vec::new();
    equations(&blocks("costs $\\,5"), &mut shown_math);
    assert!(shown_math.is_empty());
    // A formula is drawn (RaTeX); one RaTeX cannot read stays as the TeX written, never a symbol.
    let mut drawn_math = Vec::new();
    equations(&blocks("so $x \\to y$ holds"), &mut drawn_math);
    let inline = crate::markdown_prep::prepare_reply("so $x \\to y$ holds", false).markdown;
    assert!(inline.contains(crate::math_tex::MATH_SRC_PREFIX), "{inline}");
    for text in ["use $\\frac$ here", "$\\toolong$"] {
        let out = crate::markdown_prep::prepare_reply(text, false).markdown;
        assert!(out.contains(crate::math_tex::MATH_SRC_PREFIX) || out.contains(&text[text.find('$').unwrap()..text.rfind('$').unwrap() + 1]), "{text}: {out}");
        assert!(!out.contains('→'));
    }
    assert_eq!(styled("write `$\\to$` for an arrow", rune_kit::markdown::Style::Code), ["$\\to$"]);
    assert_eq!(shown("write `$\\to$` for an arrow"), "write $\\to$ for an arrow");
}

#[test]
fn lifts_a_hash_heading_out_of_the_prose_at_its_level() {
    assert_eq!(blocks(&lines(&["### Summary", "Two files changed."])), [heading(3, "Summary"), para("Two files changed.")]);
    assert_eq!(blocks("# Title"), [heading(1, "Title")]);
    assert_eq!(blocks("## Title ##"), [heading(2, "Title")]);
    // A hash that is part of the words stays ("Why C#" read "Why C").
    assert_eq!(blocks("## Why C#"), [heading(2, "Why C#")]);
    assert_eq!(blocks("### F# and C# ##"), [heading(3, "F# and C#")]);
    // Deeper than three is drawn as the third: the thread has three sizes.
    assert_eq!(blocks("##### Deep"), [heading(3, "Deep")]);
}

#[test]
fn does_not_mistake_a_hashtag_or_a_shell_comment_for_a_heading() {
    assert_eq!(blocks("#123 is the issue"), [para("#123 is the issue")]);
    assert_eq!(blocks("#"), [para("#")]);
}

#[test]
fn a_heading_ends_the_list_before_it() {
    let b = blocks(&lines(&["- one", "- two", "## Next", "prose"]));
    assert_eq!(kinds(&b), ["list", "heading", "text"]);
    assert_eq!(items_with_depth(&b), [("one".into(), 0), ("two".into(), 0)]);
    assert_eq!(b[1..], [heading(2, "Next"), para("prose")]);
}

#[test]
fn keeps_every_word_when_headings_are_lifted() {
    let text = lines(&["# A", "para", "## B", "- x", "### C"]);
    assert_eq!(missing_word(&text, &blocks(&text)), None);
}

#[test]
fn draws_bold_and_emphasis_as_spans_and_leaves_the_markers_out() {
    use rune_kit::markdown::Style::{Em, Strong};
    let text = "**Yes, whitespace-only input is already covered.** Then more.";
    assert_eq!(styled(text, Strong), ["Yes, whitespace-only input is already covered."]);
    assert_eq!(para_of(text), "Yes, whitespace-only input is already covered. Then more.");
    assert_eq!(styled("use *care* here", Em), ["care"]);
    assert_eq!(para_of("use *care* here"), "use care here");
    assert_eq!(styled("__also bold__ and _also em_", Strong), ["also bold"]);
    assert_eq!(styled("__also bold__ and _also em_", Em), ["also em"]);
    assert_eq!(para_of("__also bold__ and _also em_"), "also bold and also em");
}

#[test]
fn does_not_turn_identifiers_or_arithmetic_into_emphasis() {
    for text in ["snake_case_name and 2 * 3 * 4", "a lone * star", "**unclosed bold"] {
        assert_eq!(blocks(text), [para(text)], "{text}");
    }
}

#[test]
fn keeps_markers_inside_code_spans_literal() {
    assert_eq!(styled("run `a ** b` now", rune_kit::markdown::Style::Code), ["a ** b"]);
    assert!(styled("run `a ** b` now", rune_kit::markdown::Style::Strong).is_empty());
}

#[test]
fn reads_a_table_as_columns() {
    let b = blocks(&lines(&["| Metric | Q2 2026 | Q2 2025 |", "|---|---|---|", "| Revenue | $30.4M | $60.9M |"]));
    assert_eq!(b.len(), 1);
    let Block::Table(t) = &b[0] else { panic!("{b:?}") };
    assert_eq!(t.head.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(), ["Metric", "Q2 2026", "Q2 2025"]);
    assert_eq!(t.rows.iter().map(|r| r.iter().map(|c| c.text.as_str()).collect::<Vec<_>>()).collect::<Vec<_>>(), [["Revenue", "$30.4M", "$60.9M"]]);
}

#[test]
fn takes_the_alignment_from_the_colons() {
    let b = blocks(&lines(&["| a | b | c |", "|:--|:-:|--:|", "| 1 | 2 | 3 |"]));
    let Block::Table(t) = &b[0] else { panic!("{b:?}") };
    assert_eq!(t.align, [rune::Align::Start, rune::Align::Center, rune::Align::End]);
}

#[test]
fn leaves_a_sentence_with_a_pipe_in_it_as_prose() {
    assert_eq!(kinds(&blocks("Run the command `grep foo | head` and report back.")), ["text"]);
}

#[test]
fn leaves_a_row_of_hyphens_with_nothing_above_it_as_a_rule_not_a_table() {
    assert_eq!(shape_kinds(&blocks(&lines(&["Before.", "", "---", "", "After."]))), ["text", "rule", "text"]);
}

#[test]
fn reads_a_quote_as_a_quote() {
    let b = blocks(&lines(&["Before.", "", "> A caution worth reading.", "> Still the same caution.", "", "After."]));
    assert_eq!(kinds(&b), ["text", "quote", "text"]);
    let quote = prose_of(&b[1..2]);
    assert!(quote.contains("A caution worth reading."));
    assert!(!quote.contains('>'));
}

#[test]
fn keeps_every_word_which_is_what_the_coverage_guard_is_for() {
    let text = lines(&["Here is the split.", "", "| Metric | Q2 2026 |", "|---|---|", "| Revenue | $30.4M |", "", "> Treat that as a range.", "", "---", "", "Done."]);
    assert_eq!(missing_word(&text, &blocks(&text)), None);
}

#[test]
fn double_underscores_inside_a_word_are_not_bold_a_connector_tool_keeps_its_name() {
    assert_eq!(blocks("Called mcp__github__create_issue twice."), [para("Called mcp__github__create_issue twice.")]);
}

#[test]
fn double_underscores_still_make_bold_at_a_word_edge() {
    assert_eq!(styled("This is __important__ now.", rune_kit::markdown::Style::Strong), ["important"]);
    assert_eq!(para_of("This is __important__ now."), "This is important now.");
}

// a-tool-call-written-as-text.test.ts

#[test]
fn a_tool_call_becomes_a_code_block_labelled_tool_call_and_the_prose_around_it_stays_prose() {
    let text = "<tool_call><function=edit><parameter=path>notes.md</parameter></function></tool_call>\nI updated the notes.";
    let prepared = crate::markdown_prep::prepare_reply(text, false).markdown;
    assert!(prepared.contains("```tool call\n"), "{prepared}");
    let b = blocks(text);
    assert_eq!(b[0], Block::Code("<function=edit><parameter=path>notes.md</parameter></function>".into()));
    assert!(b[1..].iter().any(|b| para_text(b) == Some("I updated the notes.")), "{b:?}");
}

#[test]
fn takes_an_unclosed_tool_call_to_the_end_and_leaves_a_reply_without_one_alone() {
    use crate::markdown_prep::fence_tool_calls;
    assert!(fence_tool_calls("Done.\n<tool_call><function=bash>ls").contains("```tool call\n<function=bash>ls\n```"));
    assert_eq!(fence_tool_calls("No calls here."), "No calls here.");
}

// a-tables-status-is-a-chip.test.tsx

/// A table's cells as (text, link destination if the cell is one link).
fn table_of(text: &str) -> Vec<Vec<(String, Option<String>)>> {
    let b = blocks(text);
    let Some(Block::Table(t)) = b.first() else { panic!("{b:?}") };
    t.rows
        .iter()
        .map(|r| r.iter().map(|c| (c.text.clone(), c.links.first().filter(|l| l.0 == 0 && l.1 == c.text.len()).map(|l| l.2.clone()))).collect())
        .collect()
}

#[test]
fn a_status_column_is_found_by_its_header_marks_and_all() {
    use crate::markdown_prep::status_column_of;
    assert_eq!(status_column_of(&["Task", "Owner", "Status"]), Some(2));
    assert_eq!(status_column_of(&["**State**", "Item"]), Some(0));
    assert_eq!(status_column_of(&["Task", "Owner"]), None);
    assert_eq!(status_column_of(&["Status of the build"]), None);
}

#[test]
fn a_status_is_known_by_its_first_words_and_nothing_it_does_not_know() {
    use crate::markdown_prep::{Tone, status_tone_of};
    assert_eq!(status_tone_of("Done"), Some(Tone::Green));
    assert_eq!(status_tone_of("✅ Done"), Some(Tone::Green));
    assert_eq!(status_tone_of("**Merged** (#41)"), Some(Tone::Green));
    assert_eq!(status_tone_of("In progress"), Some(Tone::Blue));
    assert_eq!(status_tone_of("Blocked: waiting on the API key"), Some(Tone::Red));
    assert_eq!(status_tone_of("Waiting"), Some(Tone::Amber));
    assert_eq!(status_tone_of("Not started"), Some(Tone::Quiet));
    // "Done" inside other words is not a status; neither is a sentence.
    assert_eq!(status_tone_of("Doneness check"), None);
    assert_eq!(status_tone_of("The second half of the build is still running on the old machine"), None);
    assert_eq!(status_tone_of(""), None);
}

#[test]
fn statuses_draw_as_chips_in_a_reply_and_other_columns_and_unknown_values_are_left_alone() {
    let text = lines(&["| Task | Status |", "| --- | --- |", "| Login fix | Done |", "| Signup form | In progress |", "| Billing | Blocked |", "| Docs | Halfway, see notes |"]);
    let chip = |tone: &str| Some(format!("{}{tone}", crate::markdown_prep::STATUS_MARK));
    assert_eq!(
        table_of(&text),
        [
            [("Login fix".into(), None), ("Done".into(), chip("green"))],
            [("Signup form".into(), None), ("In progress".into(), chip("blue"))],
            [("Billing".into(), None), ("Blocked".into(), chip("red"))],
            [("Docs".into(), None), ("Halfway, see notes".to_owned(), None)],
        ]
    );
    // Drawn in its tone's tint and text, the way `.lc-status--green` is.
    let l = crate::look::controls::Look::new(false);
    let (span, _) = crate::markdown_prep::reply_link_look(&chip("green").unwrap(), false).unwrap();
    assert_eq!((span.color, span.bg), (Some(l.c("green-text")), Some(l.c("green-tint"))));
    let (span, _) = crate::markdown_prep::reply_link_look(&chip("red").unwrap(), false).unwrap();
    assert_eq!(span.bg, Some(l.c("red-tint")));
    // A chip is never opened as an address.
    assert!(crate::markdown_prep::is_own_mark(&chip("green").unwrap()));
    assert!(texts(&drawn(&text)).contains("Done"));
}

// a-colour-code-shows-its-colour.test.tsx

/// Every swatch the reply draws, as its `#rrggbb`.
fn swatches(text: &str) -> Vec<String> {
    fn inline(i: &rune_kit::markdown::Inline, out: &mut Vec<String>) {
        for (_, _, to) in &i.links {
            if let Some(hex) = to.strip_prefix(crate::markdown_prep::SWATCH_MARK) {
                out.push(format!("#{hex}"));
            }
        }
    }
    fn walk(blocks: &[Block], out: &mut Vec<String>) {
        for b in blocks {
            match b {
                Block::Para(i) | Block::Heading(_, i) => inline(i, out),
                Block::Table(t) => t.head.iter().chain(t.rows.iter().flatten()).for_each(|i| inline(i, out)),
                Block::Quote(inner) => walk(inner, out),
                Block::List { items, .. } => items.iter().for_each(|i| walk(i, out)),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(&blocks(text), &mut out);
    out
}

#[test]
fn a_colour_code_shows_its_colour_beside_it() {
    assert_eq!(swatches("The accent is `#7a9e8e` on paper `#F6F3EE`."), ["#7a9e8e", "#F6F3EE"]);
    let (span, _) = crate::markdown_prep::reply_link_look("locust-swatch:7a9e8e", false).unwrap();
    assert_eq!(span.color, rune::color::hex("#7a9e8e"));
    // The code is still there to read and copy.
    assert!(shown("The accent is `#7a9e8e` on paper `#F6F3EE`.").contains("#7a9e8e on paper"));
}

#[test]
fn a_colour_shows_for_a_table_cell_or_a_list_item_that_is_nothing_but_a_colour() {
    assert_eq!(swatches(&lines(&["| Role | Hex |", "| --- | --- |", "| Crust | #C8A27A |", "| Flour | #F4EFE6 |"])), ["#C8A27A", "#F4EFE6"]);
    assert_eq!(swatches(&lines(&["- #2E4A3F", "- #D98E73"])), ["#2E4A3F", "#D98E73"]);
    // Inside a sentence it is text, as GitHub draws it: "fixed in #123456" is not a colour.
    assert!(swatches("Fixed in #123456 and the crust is #C8A27A now.").is_empty());
}

#[test]
fn a_colour_shows_inside_bold_and_italic_with_no_backticks_left_showing() {
    let text = "- **Crust Brown `#6B4226`** — Primary.\n- *Berry Jam `#A63D40`* — accent.";
    assert_eq!(swatches(text), ["#6B4226", "#A63D40"]);
    let words = texts(&drawn(text));
    assert!(!words.contains('`'), "{words}");
    assert!(words.contains("Crust Brown"), "{words}");
    // And bold with nothing inside it is drawn exactly as it always was.
    assert_eq!(styled("**Plain bold.** After.", rune_kit::markdown::Style::Strong), ["Plain bold."]);
}

#[test]
fn draws_nothing_for_code_that_is_not_exactly_a_colour() {
    assert!(swatches("Run `npm test`, see issue `#12a`, and `#7a9e8e;background:url(x)`.").is_empty());
}

#[test]
fn accepts_rrggbb_only_as_github_does_a_short_form_may_be_an_issue_number() {
    use crate::markdown_prep::hex_colour_of;
    assert_eq!(hex_colour_of(" #a1b2c3 "), Some("#a1b2c3"));
    assert_eq!(hex_colour_of("#333333"), Some("#333333"));
    for no in ["#123", "#abc", "#a1b2c3d4", "#ggg000", "red", "#ffffff) no-repeat, url(x"] {
        assert_eq!(hex_colour_of(no), None, "{no}");
    }
}
