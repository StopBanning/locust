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
/// drawn as its marker, so it is not looked for among the words.
fn missing_word(text: &str, blocks: &[Block]) -> Option<String> {
    let mut words = Vec::new();
    drawn_words(blocks, &mut words);
    let mut have = words.iter();
    let marker = crate::invariants::regex(r"^[\s>]*(?:[-*+]|\d{1,9}[.)])(?:\s|$)");
    text.lines()
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
