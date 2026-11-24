//! math-in-a-reply-is-drawn.test.tsx through the window: the tester's reply, prepared as the thread prepares a reply
//! (`markdown_prep::prepare_reply`, which runs `prepare_math_markdown`), parsed as rune-kit parses it, and drawn headless
//! by `reply_markdown`. Electron draws with KaTeX; the window draws the same TeX with RaTeX, to a picture whose name is the
//! TeX. The reply and the expected spans are Electron's.
use super::*;
use rune_kit::markdown::{parse, Block, Style};

const REPLY: &str = "Write $c_1(3,1,0)+c_2(2,0,3)+c_3(1,0,0) = (3c_1+2c_2+c_3,\\ c_1,\\ 3c_2)$. For a target vector $(x,y,z)$ this gives $c_1=y$.\n\
\n\
$$[\\mathrm{Id}]^\\beta_{\\beta'}=\\begin{pmatrix}2&3&-1\\\\0&\\tfrac13&0\\\\-5&-\\tfrac{29}{3}&2\\end{pmatrix}$$\n\
\n\
Write $x^k=\\big((x+1)-1\\big)^k$ and expand with the binomial theorem:\n\
\n\
$$x^k=\\sum_{j=0}^{k}\\binom{k}{j}(-1)^{k-j}(x+1)^j.$$\n\
\n\
It costs $5 and the other costs $10, so $15 in all.";

fn blocks(text: &str) -> Vec<Block> {
    parse(&crate::markdown_prep::prepare_reply(text, false).markdown)
}

/// The equations drawn as pictures, in order: (name, source).
fn equations(blocks: &[Block]) -> Vec<(String, String)> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::Image { src, alt, .. } if src.starts_with(MATH_SRC_PREFIX) => Some((alt.clone(), src.clone())),
            _ => None,
        })
        .collect()
}

/// The picture a TeX is drawn as, displayed or inline (`push_math`'s source).
fn picture_of(tex: &str, display: bool) -> String {
    format!("{MATH_SRC_PREFIX}{:016x}", tex_key(tex, display, false))
}

/// The reply drawn headless: each element's (role, text, label).
fn drawn(text: &str) -> Vec<(&'static str, String, String)> {
    let mut s = crate::State::default();
    s.image_cache = Some(Arc::new(rune::ImageCache::new(Arc::new(|| {}))));
    let mut rt = rune::Runtime::new(
        (s, text.to_owned()),
        |(s, body), _| crate::reply_actions::reply_markdown_for_tests(s, body, &crate::Palette::new(false)),
        (700.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().map(|e| (e.role, e.text.unwrap_or_default(), e.label.unwrap_or_default())).collect()
}

fn words(drawn: &[(&'static str, String, String)]) -> String {
    drawn.iter().map(|(_, t, _)| t.as_str()).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n")
}

#[test]
fn draws_each_display_equation_as_a_displayed_equation_and_loses_no_line() {
    // case: draws each $$...$$ as a displayed equation, and loses no line
    let matrix = "[\\mathrm{Id}]^\\beta_{\\beta'}=\\begin{pmatrix}2&3&-1\\\\0&\\tfrac13&0\\\\-5&-\\tfrac{29}{3}&2\\end{pmatrix}";
    let binomial = "x^k=\\sum_{j=0}^{k}\\binom{k}{j}(-1)^{k-j}(x+1)^j.";
    let shown: Vec<String> = equations(&blocks(REPLY)).into_iter().map(|(_, src)| src).collect();
    let displayed: Vec<&String> = shown.iter().filter(|src| **src == picture_of(matrix, true) || **src == picture_of(binomial, true)).collect();
    assert_eq!(displayed, [&picture_of(matrix, true), &picture_of(binomial, true)], "{shown:?}");
    assert!(!shown.contains(&picture_of("c_1=y", true)) && shown.contains(&picture_of("c_1=y", false)), "a sentence's math is not displayed");
    // Drawn: each a picture named by its TeX, and every line of prose still there.
    let d = drawn(REPLY);
    let pictures: Vec<&str> = d.iter().filter(|(r, _, _)| *r == "image").map(|(_, _, l)| l.as_str()).collect();
    assert!(pictures.iter().any(|l| l.contains("\\begin{pmatrix}")), "{pictures:?}");
    assert!(pictures.iter().any(|l| l.contains("\\binom{k}{j}")), "{pictures:?}");
    let said = words(&d);
    for line in ["Write", "For a target vector", "this gives", "and expand with the binomial theorem:", "It costs $5 and the other costs $10, so $15 in all."] {
        assert!(said.contains(line), "lost {line:?} from {said}");
    }
}

#[test]
fn draws_inline_math_inside_a_sentence_as_inline_math() {
    // case: draws $...$ inside a sentence as inline math
    let shown = equations(&blocks("For a target vector $(x,y,z)$ this gives $c_1=y$ and \\(x^2\\)."));
    let names: Vec<&str> = shown.iter().map(|(alt, _)| alt.as_str()).collect();
    assert_eq!(names, ["(x,y,z)", "c_1=y", "x^2"]);
    for (alt, src) in &shown {
        assert_eq!(*src, picture_of(alt, false), "{alt} is drawn inline, at a sentence's size");
    }
}

#[test]
fn leaves_money_as_money() {
    // case: leaves money as money
    for text in ["It costs $5 and the other costs $10, so $15 in all.", "Between $5-$10 a month."] {
        assert!(equations(&blocks(text)).is_empty(), "{text}");
        let d = drawn(text);
        assert!(words(&d).contains(text), "{text}: {}", words(&d));
        assert!(!d.iter().any(|(r, _, _)| *r == "image"), "{text}");
    }
}

#[test]
fn is_drawn_by_katex_and_tex_it_cannot_read_is_shown_as_written() {
    // case: is drawn by KaTeX, and TeX it cannot read is shown as written
    // RaTeX (KaTeX's layout, in Rust) stands in for KaTeX: a picture for what it reads, inline and displayed.
    let inline = render_png("c_1=y", false, false).expect("drawn inline");
    let displayed = render_png("\\binom{k}{j}", true, false).expect("drawn displayed");
    assert_eq!(&inline[..4], b"\x89PNG");
    assert_eq!(&displayed[..4], b"\x89PNG");
    // What it cannot read is never thrown away: the TeX as written, in the reply's words.
    assert!(render_png("\\notacommand{x}", false, false).is_none());
    let d = drawn("Then $\\notacommand{x}$ holds.");
    assert!(words(&d).contains("$\\notacommand{x}$"), "{}", words(&d));
    assert!(!d.iter().any(|(r, _, _)| *r == "image"));
}

#[test]
fn never_makes_a_link_out_of_tex() {
    // case: never makes a link out of TeX
    let text = "See $\\href{https://evil.example}{x}$ here.";
    for b in blocks(text) {
        if let Block::Para(i) = b {
            assert!(i.links.is_empty(), "{:?}", i.links);
        }
    }
    let d = drawn(text);
    assert!(!d.iter().any(|(r, _, _)| *r == "link"), "{d:?}");
}

#[test]
fn is_italic_like_a_longer_one() {
    // case: is italic, like a longer one
    let b = blocks("Column *j* of the *j*-th vector");
    let Some(Block::Para(i)) = b.first() else { panic!("{b:?}") };
    assert_eq!(i.text, "Column j of the j-th vector");
    let em: Vec<&str> = i.spans.iter().filter(|(_, _, s)| *s == Style::Em).map(|(a, z, _)| &i.text[*a..*z]).collect();
    assert_eq!(em, ["j", "j"]);
    assert!(words(&drawn("Column *j* of the *j*-th vector")).contains("Column j of the j-th vector"));
}

#[test]
fn still_leaves_arithmetic_alone() {
    // case: still leaves arithmetic alone
    let b = blocks("2 * 3 * 4");
    let Some(Block::Para(i)) = b.first() else { panic!("{b:?}") };
    assert_eq!(i.text, "2 * 3 * 4");
    assert!(i.spans.is_empty(), "{:?}", i.spans);
}
