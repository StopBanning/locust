use crate::desk_page::{DeskNoteLink, desk_note_page, desk_page};
use crate::phone_page::{PhonePageVia, phone_page};
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};

fn script_of(html: &str) -> &str {
    let start = html.find("<script>").expect("script open") + "<script>".len();
    let end = html.rfind("</script>").expect("script close");
    &html[start..end]
}

fn run_node(js: &str) -> Option<String> {
    let mut child = Command::new("node")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .ok()?;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(js.as_bytes())
        .unwrap();
    let out = child.wait_with_output().ok()?;
    assert!(
        out.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Some(String::from_utf8(out.stdout).unwrap())
}

fn read_code_of(hash: &str) -> Value {
    let page = phone_page(PhonePageVia::Relay, None);
    let script = script_of(&page);
    let start = script.find("function readCode()").expect("readCode");
    let end = script.find("function failAll(").expect("failAll");
    let body = &script[start..end];
    let runner = format!(
        "const location = {{ hash: {}, pathname: '/' }};\nlet cleared = false;\nconst history = {{ replaceState: () => {{ cleared = true; }} }};\n{body}\nconsole.log(JSON.stringify({{ code: readCode(), cleared }}));",
        json!(hash)
    );
    let stdout = run_node(&runner).expect("node is required for phone_page readCode test");
    serde_json::from_str(stdout.trim()).unwrap()
}

#[test]
// case: reads a GitHub ticket, Open in your browser’s code and a plain code, and keeps none in the address
fn reads_a_github_ticket_open_in_your_browsers_code_and_a_plain_code_and_keeps_none_in_the_address()
{
    let room = "r".repeat(22);
    let key = "k".repeat(43);
    let secret = "s".repeat(22);
    let ticket = "t".repeat(43);
    let token = format!("gho_{}", "g".repeat(36));

    assert_eq!(
        read_code_of(&format!("#g={room}.{key}.{ticket}.{token}")),
        json!({ "code": { "room": room, "pc": key, "ticket": ticket, "github": token, "desk": true }, "cleared": true })
    );
    assert_eq!(
        read_code_of(&format!("#p={room}.{key}.{secret}.d")),
        json!({ "code": { "room": room, "pc": key, "secret": secret, "desk": true }, "cleared": true })
    );
    assert_eq!(
        read_code_of(&format!("#p={room}.{key}.{secret}")),
        json!({ "code": { "room": room, "pc": key, "secret": secret, "desk": false }, "cleared": true })
    );
    // Anything else is no code, and is still cleared away.
    assert_eq!(
        read_code_of(&format!("#p={room}.{key}.{secret}.x")),
        json!({ "code": null, "cleared": true })
    );
    assert_eq!(
        read_code_of(&format!("#g={room}.{key}.{ticket}")),
        json!({ "code": null, "cleared": true })
    );
    assert_eq!(
        read_code_of(&format!("#g={room}.{key}.{ticket}.no<way>")),
        json!({ "code": null, "cleared": true })
    );
    assert_eq!(
        read_code_of("#elsewhere"),
        json!({ "code": null, "cleared": false })
    );
}

#[test]
// case: opens with every way in: GitHub where the relay offers it, a code, and where to get Locust
fn opens_with_every_way_in_github_where_the_relay_offers_it_a_code_and_where_to_get_locust() {
    let offered = desk_page(true);
    let script = script_of(&offered);
    // Parse the emitted script in node without running it.
    let parse_check = format!("new Function({}); console.log('ok');", json!(script));
    let out = run_node(&parse_check).expect("node is required to parse desk_page script");
    assert_eq!(out.trim(), "ok");

    assert!(script.contains("var GITHUB = true;"));
    assert!(script.contains("'Sign in with GitHub'"));
    assert!(script.contains("go.href = '/v1/github/start'"));
    assert!(script.contains("Open in your browser opens it there"));
    assert!(script.contains("get.href = 'https://locust.lol'"));
    assert!(!script.contains("Pair this browser first"));
    // A relay with no GitHub shows no GitHub button.
    assert!(script_of(&desk_page(false)).contains("var GITHUB = false;"));
}

#[test]
// case: says what GitHub’s sign-in ended in on a page with no script, every word escaped
fn says_what_githubs_sign_in_ended_in_on_a_page_with_no_script_every_word_escaped() {
    let room = "r".repeat(22);
    let key = "k".repeat(43);
    let ticket = "t".repeat(43);
    let token = format!("gho_{}", "g".repeat(36));
    let page = desk_note_page(
        "Signed in as <octo>",
        "Close \"this\" tab & go",
        &[
            DeskNoteLink {
                label: "Try again".into(),
                href: "/desk/".into(),
            },
            DeskNoteLink {
                label: "Laptop".into(),
                href: format!("/#g={room}.{key}.{ticket}.{token}"),
            },
        ],
    );
    assert!(!page.contains("<script"));
    assert!(page.contains("<h1>Signed in as &#60;octo&#62;</h1>"));
    assert!(page.contains("Close &#34;this&#34; tab &#38; go"));
    assert!(page.contains("<a class=\"go\" href=\"/desk/\">Try again</a><a class=\"go quiet\" href=\"/#g="));
}
