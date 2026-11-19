//! Carries `apps/desktop/src/main/a-phone-shows-an-answer-as-the-window-does.test.ts` (all 6 cases)
//! and the page-level checks of `scouts-cards-are-on-your-phone.test.ts` and `a-phone-is-paired-by-its-code.test.ts`.

use super::*;
use serde_json::Value;
use std::process::Command;

fn run_phone_answer_js(expr: &str) -> Option<Value> {
    let script = format!(
        r#"
function el(tag, attrs, children) {{
  const made = {{
    tag,
    ...(attrs && typeof attrs.class === 'string' ? {{ className: attrs.class }} : {{}}),
    ...(attrs && typeof attrs.text === 'string' ? {{ text: attrs.text }} : {{}}),
    children: [],
    append(child) {{ this.children.push(child); }},
    get lastChild() {{ return this.children[this.children.length - 1]; }}
  }};
  for (const child of children || []) if (child) made.children.push(child);
  return made;
}}
const txt = (text) => ({{ tag: '#text', text, children: [] }});
{PHONE_ANSWER}
const shape = (made) =>
  made.tag === '#text' ? (made.text || '') : `<${{made.tag}}${{made.className === undefined ? '' : '.' + made.className}}>${{made.text || ''}}${{made.children.map(shape).join('')}}</${{made.tag}}>`;
const tags = (made) => [made.tag, ...made.children.flatMap(tags)];
const out = (() => {{ {expr} }})();
process.stdout.write(JSON.stringify(out));
"#
    );
    let output = match Command::new("node").arg("-e").arg(&script).output() {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => panic!("node failed: {e}"),
    };
    assert!(
        output.status.success(),
        "node exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    Some(serde_json::from_slice(&output.stdout).expect("valid JSON from node"))
}

#[test]
fn draws_the_bullets_kit_sent_the_phone_as_a_list_with_bold_names() {
    let kit = "- **Plan header**: The weekend name and proposed dates set by the host.\n- **Availability grid**: A day-by-day time grid.\n- **Share prompt**: An \"Invite friends\" button.";
    let Some(res) = run_phone_answer_js(&format!(
        "return shape(answerOf({}));",
        serde_json::to_string(kit).unwrap()
    )) else {
        return;
    };
    assert_eq!(
        res,
        "<div.answer><ul><li><strong>Plan header</strong>: The weekend name and proposed dates set by the host.</li><li><strong>Availability grid</strong>: A day-by-day time grid.</li><li><strong>Share prompt</strong>: An \"Invite friends\" button.</li></ul></div>"
    );
}

#[test]
fn draws_paragraphs_a_heading_a_numbered_list_code_and_emphasis() {
    let source = "## Steps\nFirst, run `npm test`\nand read it.\n\n1. Open *it*\n2. Close __it__\n\n```js\nnpm run build\n  --watch\n```\nDone.";
    let Some(res) = run_phone_answer_js(&format!(
        "return shape(answerOf({}));",
        serde_json::to_string(source).unwrap()
    )) else {
        return;
    };
    assert_eq!(
        res,
        "<div.answer><p.h>Steps</p><p>First, run <code>npm test</code>\nand read it.</p><ol><li>Open <em>it</em></li><li>Close <strong>it</strong></li></ol><pre><code>npm run build\n  --watch</code></pre><p>Done.</p></div>"
    );
}

#[test]
fn carries_an_item_on_over_a_line_indented_under_it_and_shows_an_unclosed_fence_as_code() {
    let Some(res) = run_phone_answer_js(
        r#"return [shape(answerOf("- one\n  more of one\n- two")), shape(answerOf("Run this:\n```\nls -la"))];"#,
    ) else {
        return;
    };
    assert_eq!(res[0], "<div.answer><ul><li>one more of one</li><li>two</li></ul></div>");
    assert_eq!(res[1], "<div.answer><p>Run this:</p><pre><code>ls -la</code></pre></div>");
}

#[test]
fn keeps_every_word_from_a_run_as_text_never_as_html_and_a_link_as_its_words() {
    let source = "<img src=x onerror=alert(1)> **<b>bold</b>** [click](javascript:alert(1)) 2 * 3 * 4";
    let Some(res) = run_phone_answer_js(&format!(
        "const made = answerOf({}); return {{ tags: tags(made), shape: shape(made) }};",
        serde_json::to_string(source).unwrap()
    )) else {
        return;
    };
    assert_eq!(
        res["tags"],
        serde_json::json!(["div", "p", "#text", "strong", "#text", "#text", "#text"])
    );
    assert_eq!(
        res["shape"],
        "<div.answer><p><img src=x onerror=alert(1)> <strong><b>bold</b></strong> click 2 * 3 * 4</p></div>"
    );
}

#[test]
fn gives_the_list_of_conversations_the_words_without_the_marks() {
    let source = "- **Plan header**: The `weekend` name\n## Next\n```\ncode\n```\n[docs](https://example.com) __done__";
    let Some(res) = run_phone_answer_js(&format!(
        "return plainOf({});",
        serde_json::to_string(source).unwrap()
    )) else {
        return;
    };
    assert_eq!(res, "Plan header: The weekend name Next code docs done");
}

#[test]
fn is_in_the_page_both_ways_drawing_the_thread_and_the_snippets() {
    for via in [PhonePageVia::Lan, PhonePageVia::Relay] {
        let page = phone_page(via, None);
        assert!(page.contains(PHONE_ANSWER));
        assert!(page.contains("said.append(answerOf(turn.answer))"));
        assert!(page.contains("text: plainOf(c.snippet)"));
        assert!(!page.contains("el('div', { class: 'answer', text: turn.answer })"));
        assert!(page.contains("Pair this phone with Locust"));
        assert!(page.contains("el('h2', { text: 'Scout found' })"));
    }
    assert!(phone_page(PhonePageVia::Relay, None).contains("el('h2', { text: 'While your computer is off' })"));
    assert!(!phone_page(PhonePageVia::Lan, None).contains("manifest.webmanifest"));
    assert!(phone_page(PhonePageVia::Relay, None).contains("manifest.webmanifest"));
}

// THE PAGES A BROWSER RUNS ARE WHOLE (`main/the-pages-a-browser-runs-are-whole.test.ts`, 0.830). Electron's pages are
// scripts inside template strings, where `\/` came out as `/`; Rust's raw strings keep every character, but the page
// must still parse as the browser receives it. Run with node, as the tests above are; skipped where node is missing.

fn script_of(html: &str) -> &str {
    let start = html.find("<script>").expect("the page has a script");
    let end = html.rfind("</script>").expect("the script is closed");
    assert!(start > 0);
    &html[start + "<script>".len()..end]
}

/// Whether node parses `script` as a function body, as `new Function` does; `None` without node.
fn parses(script: &str) -> Option<bool> {
    let probe = format!("try {{ new Function({}); process.stdout.write('true') }} catch (e) {{ process.stdout.write('false') }}", serde_json::to_string(script).unwrap());
    let out = match Command::new("node").arg("-e").arg(&probe).output() {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => panic!("node failed: {e}"),
    };
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    Some(out.stdout == b"true")
}

#[test]
fn parses_as_the_browser_receives_it_the_phone_page_both_ways_its_service_worker_and_the_way_in() {
    // The way in (Electron's deskPage) is not built in this app: there is no desk page to parse (tests.tsv says so).
    for (name, script) in [
        ("the home Wi-Fi page", script_of(&phone_page(PhonePageVia::Lan, None)).to_owned()),
        ("the relay page", script_of(&phone_page(PhonePageVia::Relay, Some(&"k".repeat(87)))).to_owned()),
        ("the service worker", PHONE_SERVICE_WORKER.to_owned()),
    ] {
        let Some(ok) = parses(&script) else { return };
        assert!(ok, "{name} does not parse as the browser receives it");
    }
    // And a page that does not parse is caught, so the check is not always yes.
    assert_eq!(parses("var a = /Edg//; }"), Some(false));
}

#[test]
fn names_a_computers_browser_by_its_name_and_system_as_settings_lists_it() {
    let page = phone_page(PhonePageVia::Relay, None);
    let script = script_of(&page);
    let named = &script[script.find("function deviceName()").unwrap()..script.find("function readCode()").unwrap()];
    let agents = [
        ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36", "Chrome on Windows"),
        ("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36 Edg/141.0.0.0", "Edge on Windows"),
        ("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Safari/605.1.15", "Safari on Mac"),
        ("Mozilla/5.0 (X11; Linux x86_64; rv:131.0) Gecko/20100101 Firefox/131.0", "Firefox on Linux"),
        ("Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1", "iPhone"),
        ("Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36", "Android phone"),
    ];
    let uas = serde_json::to_string(&agents.iter().map(|(ua, _)| *ua).collect::<Vec<_>>()).unwrap();
    let probe = format!(
        "const named = new Function('navigator', {} + '; return deviceName();'); process.stdout.write(JSON.stringify({uas}.map((userAgent) => named({{ userAgent }}))));",
        serde_json::to_string(named).unwrap()
    );
    let out = match Command::new("node").arg("-e").arg(&probe).output() {
        Ok(out) => out,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
        Err(e) => panic!("node failed: {e}"),
    };
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    let names: Vec<String> = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(names, agents.iter().map(|(_, n)| n.to_string()).collect::<Vec<_>>());
}
