use super::*;
use crate::{
    browser::{Host, bounded_log, resolve},
    page_preview::Pages,
};
use std::{fs, path::PathBuf, sync::atomic::AtomicUsize};
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-p13-test-{}",
            crate::native::uuid_v4().unwrap()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn host() -> Shared {
    let mut h = Host::default();
    h.profiles = vec![
        ("codec".into(), "Codec".into()),
        ("argon".into(), "Argon".into()),
    ];
    Arc::new(Mutex::new(h))
}
#[test]
fn addresses_match_electron_web_and_loopback_rules() {
    for (raw, url) in [
        (" https://example.com/docs ", "https://example.com/docs"),
        ("localhost:3000/app", "http://localhost:3000/app"),
        ("127.0.0.1:8000/", "http://127.0.0.1:8000/"),
        ("[::1]:8080/", "http://[::1]:8080/"),
        ("example.com", "https://example.com/"),
        ("example.com:8080/a", "https://example.com:8080/a"),
    ] {
        assert_eq!(resolve(raw, None).unwrap().as_str(), url, "{raw}");
    }
    for scheme in [
        "javascript",
        "data",
        "vbscript",
        "blob",
        "chrome",
        "chrome-extension",
        "devtools",
        "about",
        "view-source",
        "ws",
        "wss",
        "ftp",
        "mailto",
        "tel",
        "locust-page",
    ] {
        let err = resolve(&format!("{scheme}:test"), None).unwrap_err();
        assert_eq!(
            err,
            format!(
                "The {scheme}: scheme is not allowed in the Locust browser. The current page stayed open."
            )
        );
    }
    assert!(
        resolve("", None)
            .unwrap_err()
            .starts_with("Enter a web address")
    );
    assert!(resolve("http://[bad", None).is_err());
    assert!(
        resolve("file:///tmp/x.html", None)
            .unwrap_err()
            .starts_with("Local file URLs require")
    );
}
#[test]
fn workspace_paths_and_symlinks_cannot_escape() {
    let t = Temp::new();
    let workspace = t.0.join("workspace");
    fs::create_dir(&workspace).unwrap();
    let inside = workspace.join("index.html");
    fs::write(&inside, "inside").unwrap();
    let outside = t.0.join("outside.html");
    fs::write(&outside, "outside").unwrap();
    assert_eq!(
        resolve("./index.html", Some(&workspace))
            .unwrap()
            .to_file_path()
            .unwrap(),
        inside.canonicalize().unwrap()
    );
    assert_eq!(
        resolve("index.html?q=1#part", Some(&workspace))
            .unwrap()
            .query(),
        Some("q=1")
    );
    assert!(resolve("../outside.html", Some(&workspace)).is_err());
    std::os::unix::fs::symlink(&outside, workspace.join("escape.html")).unwrap();
    assert!(resolve("./escape.html", Some(&workspace)).is_err());
    std::os::unix::fs::symlink(&t.0, workspace.join("escape-dir")).unwrap();
    assert!(resolve("./escape-dir/not-yet.html", Some(&workspace)).is_err());
}
#[test]
fn tabs_close_select_and_keep_page_on_refusal() {
    let mut h = Host::default();
    let a = h.add(Profile(None)).unwrap();
    let b = h.add(Profile(None)).unwrap();
    assert_eq!(h.active, Some(b));
    h.navigate(a, "https://example.com", true).unwrap();
    let before = h.tabs[0].commands.len();
    assert!(h.navigate(a, "javascript:alert(1)", true).is_err());
    assert_eq!(h.tabs[0].commands.len(), before);
    h.close(b);
    assert_eq!(h.active, Some(a));
    h.close(a);
    assert_eq!(h.active, None);
}
#[test]
fn every_run_can_see_and_use_only_its_own_profile() {
    let host = host();
    let a = crate::invariants::lock(&host)
        .add(Profile(Some("codec".into())))
        .unwrap();
    let b = crate::invariants::lock(&host)
        .add(Profile(Some("argon".into())))
        .unwrap();
    let shared = crate::invariants::lock(&host).add(Profile(None)).unwrap();
    let profile = Profile(Some("codec".into()));
    assert_eq!(
        execute(
            &host,
            &profile,
            "browser_tabs",
            &json!({}),
            &|_, _| panic!("read must not ask"),
            &|| false
        )
        .unwrap()
        .as_array()
        .unwrap()
        .len(),
        1
    );
    for id in [b, shared] {
        for name in [
            "browser_open",
            "browser_console",
            "browser_network",
            "browser_back",
            "browser_forward",
        ] {
            assert!(
                execute(
                    &host,
                    &profile,
                    name,
                    &json!({"tab_id":format!("tab_{id}"),"url":"https://example.com"}),
                    &|_, _| panic!("foreign profile must be refused before approval"),
                    &|| false
                )
                .is_err()
            );
        }
    }
    assert!(crate::invariants::lock(&host).scoped(a, &profile).is_ok());
    assert!(
        crate::invariants::lock(&host)
            .add(Profile(Some("missing".into())))
            .is_err()
    );
}
#[test]
fn browser_permissions_deny_before_effect_and_recheck_closed_tabs() {
    let host = host();
    let profile = Profile(Some("codec".into()));
    let id = crate::invariants::lock(&host).add(profile.clone()).unwrap();
    let args = json!({"url":"https://example.com/","tab_id":format!("tab_{id}")});
    let asks = AtomicUsize::new(0);
    assert!(
        execute(
            &host,
            &profile,
            "browser_open",
            &args,
            &|title, detail| {
                assert_eq!(title, "Open https://example.com in the browser?");
                assert!(!detail.is_empty());
                asks.fetch_add(1, Ordering::SeqCst);
                Ok(false)
            },
            &|| false
        )
        .is_err()
    );
    assert_eq!(asks.load(Ordering::SeqCst), 1);
    assert!(crate::invariants::lock(&host).tabs[0].commands.is_empty());
    assert!(
        execute(
            &host,
            &profile,
            "browser_open",
            &args,
            &|_, _| {
                crate::invariants::lock(&host).close(id);
                Ok(true)
            },
            &|| false
        )
        .is_err()
    );
}
#[test]
fn local_tool_navigation_does_not_ask_but_cancel_and_org_refusal_win() {
    let host = host();
    let profile = Profile(Some("codec".into()));
    crate::invariants::lock(&host).add(profile.clone()).unwrap();
    let args = json!({"url":"localhost:3000"});
    assert!(
        execute(
            &host,
            &profile,
            "browser_open",
            &args,
            &|_, _| panic!("loopback is safe"),
            &|| false
        )
        .is_ok()
    );
    assert!(
        execute(
            &host,
            &profile,
            "browser_open",
            &args,
            &|_, _| panic!("cancelled"),
            &|| true
        )
        .is_err()
    );
    crate::invariants::lock(&host).refusal = Some("The browser panel is off.".into());
    assert_eq!(
        execute(
            &host,
            &profile,
            "browser_console",
            &json!({}),
            &|_, _| panic!("policy refusal"),
            &|| false
        )
        .unwrap_err(),
        "The browser panel is off."
    );
}
#[test]
fn logs_are_bounded_and_scoped_for_teammate_readers() {
    let host = host();
    let profile = Profile(Some("codec".into()));
    crate::invariants::lock(&host).add(profile.clone()).unwrap();
    {
        let mut h = crate::invariants::lock(&host);
        for i in 0..105 {
            bounded_log(&mut h.tabs[0].console, json!({"message":i}));
            bounded_log(&mut h.tabs[0].network, json!({"url":i}));
        }
    }
    for name in ["browser_console", "browser_network"] {
        let entries = execute(
            &host,
            &profile,
            name,
            &json!({}),
            &|_, _| panic!("reading logs never asks"),
            &|| false,
        )
        .unwrap();
        assert_eq!(entries.as_array().unwrap().len(), 100);
        assert_eq!(
            entries[0][if name == "browser_console" {
                "message"
            } else {
                "url"
            }],
            5
        );
    }
    assert!(
        execute(
            &host,
            &profile,
            "browser_type",
            &json!({}),
            &|_, _| panic!("unsupported mutation cannot ask or run"),
            &|| false
        )
        .is_err()
    );
}
fn page() -> (Temp, Pages, String, PathBuf) {
    let temp = Temp::new();
    let site = temp.0.join("site");
    fs::create_dir(&site).unwrap();
    let page = site.join("index.html");
    fs::write(&page, "<h1>Page</h1>").unwrap();
    let mut pages = Pages::default();
    let url = pages.url_for(&page, Some(&temp.0)).unwrap();
    (temp, pages, url, site)
}
#[test]
fn previews_have_capability_hosts_and_only_serve_the_page_folder() {
    let (t, pages, url, site) = page();
    assert_eq!(pages.read(&url, Some(&t.0)).body, b"<h1>Page</h1>");
    fs::write(t.0.join("README.md"), "outside").unwrap();
    for path in [
        "../README.md",
        "..%2fREADME.md",
        "%2e%2e/README.md",
        "%00",
        "%GG",
    ] {
        assert_ne!(
            pages
                .read(&url.replace("index.html", path), Some(&t.0))
                .status,
            200,
            "{path}"
        );
    }
    assert_eq!(
        pages
            .read("locust-page://unknown/index.html", Some(&t.0))
            .status,
        404
    );
    assert_eq!(pages.read(&url, None).status, 403);
    assert_eq!(pages.read(&url, Some(&site.join("not-there"))).status, 403);
}
#[test]
fn previews_reject_hidden_keys_secret_folders_and_resolved_aliases() {
    let (t, pages, url, site) = page();
    for dir in [".git", "secrets", "data"] {
        fs::create_dir(site.join(dir)).unwrap();
    }
    for path in [
        ".env",
        ".env.local",
        ".git/config",
        ".npmrc",
        "secrets/token.txt",
        "server.pem",
        "deploy.key",
        "id_rsa",
        "id_ed25519.pub",
        "credentials.json",
        "service-account-prod.json",
    ] {
        fs::write(site.join(path), "secret").unwrap();
        assert_eq!(
            pages
                .read(&url.replace("index.html", path), Some(&t.0))
                .status,
            403,
            "{path}"
        );
    }
    for path in [
        "data/prices.json",
        "tokens.css",
        "keyboard.js",
        "monkey.png",
    ] {
        fs::write(site.join(path), "fine").unwrap();
        assert_eq!(
            pages
                .read(&url.replace("index.html", path), Some(&t.0))
                .status,
            200,
            "{path}"
        );
    }
    std::os::unix::fs::symlink(site.join(".env"), site.join("dotenv.txt")).unwrap();
    assert_eq!(
        pages
            .read(&url.replace("index.html", "dotenv.txt"), Some(&t.0))
            .status,
        403
    );
    fs::write(t.0.join("outside.txt"), "secret").unwrap();
    std::os::unix::fs::symlink(t.0.join("outside.txt"), site.join("escape.txt")).unwrap();
    assert_ne!(
        pages
            .read(&url.replace("index.html", "escape.txt"), Some(&t.0))
            .status,
        200
    );
}
#[test]
fn previews_enforce_extension_size_index_and_mime() {
    let (t, mut pages, url, site) = page();
    let large = fs::File::create(site.join("large.bin")).unwrap();
    large.set_len(64 * 1024 * 1024 + 1).unwrap();
    assert_eq!(
        pages
            .read(&url.replace("index.html", "large.bin"), Some(&t.0))
            .status,
        413
    );
    fs::create_dir(site.join("sub")).unwrap();
    fs::write(site.join("sub/index.html"), "sub").unwrap();
    assert_eq!(
        pages
            .read(&url.replace("index.html", "sub/"), Some(&t.0))
            .body,
        b"sub"
    );
    assert!(pages.read(&url, Some(&t.0)).mime.starts_with("text/html"));
    fs::write(site.join("style.css"), "x").unwrap();
    assert!(
        pages
            .url_for(&site.join("style.css"), Some(&t.0))
            .unwrap_err()
            .starts_with("Only a web page")
    );
    let outside = Temp::new();
    fs::write(outside.0.join("x.html"), "x").unwrap();
    assert!(
        pages
            .url_for(&outside.0.join("x.html"), Some(&t.0))
            .is_err()
    );
}
#[test]
fn native_browser_approval_asks_and_records_without_saved_rule_store_access() {
    let channel = crate::approval::Channel::new();
    let worker = channel.clone();
    let thread = std::thread::spawn(move || {
        worker.ask_browser(
            "browser-test-mission",
            "Open another site?",
            "No field values",
            &|| false,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(3);
    while channel.waiting().is_empty() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    let request = channel.waiting().remove(0);
    assert_eq!(request.runtime, "browser");
    assert!(crate::approval::asks_every_time(&request));
    assert!(!crate::approval::can_save_allow_rule(&request));
    channel
        .decide(crate::approval::Answer::Decision {
            approval_id: request.approval_id,
            decision: crate::approval::Decision::Deny,
            reason: None,
        })
        .unwrap();
    assert!(!thread.join().unwrap().unwrap());
    assert_eq!(channel.take_recorded().len(), 1);
}
#[test]
fn electron_gate_categories_and_safe_search_are_preserved() {
    let trusted = std::collections::HashSet::new();
    for (action, element, key, expected) in [
        (
            "type",
            json!({"name":"Password","type":"password"}),
            "",
            "credential",
        ),
        ("type", json!({"name":"Card number"}), "", "credential"),
        ("select", json!({"name":"Bank account"}), "", "credential"),
        (
            "click",
            json!({"name":"Sign in","submit":true}),
            "",
            "submit",
        ),
        ("click", json!({"name":"Place order"}), "", "purchase"),
        ("click", json!({"name":"Send message"}), "", "send"),
        (
            "click",
            json!({"name":"Report","download":true}),
            "",
            "download",
        ),
        (
            "click",
            json!({"name":"Docs","href":"https://other.example/docs"}),
            "",
            "leave-origin",
        ),
        (
            "press",
            json!({"name":"Password","type":"password"}),
            "Enter",
            "credential",
        ),
        ("press", json!({"name":"Pay now"}), "Enter", "purchase"),
        ("press", json!({"name":"Send reply"}), "Return", "send"),
    ] {
        assert_eq!(
            crate::browser::actions::gate(action, &element, key, "https://example.com", &trusted)
                .unwrap()
                .0,
            expected,
            "{action} {element}"
        );
    }
    for (action, element, key) in [
        (
            "type",
            json!({"name":"Search documentation","type":"search"}),
            "",
        ),
        ("click", json!({"name":"Search","submit":true}), ""),
        (
            "press",
            json!({"name":"Search","role":"searchbox","submit":true}),
            "Enter",
        ),
        ("click", json!({"name":"Docs","href":"/docs"}), ""),
    ] {
        assert!(
            crate::browser::actions::gate(action, &element, key, "https://example.com", &trusted)
                .is_none(),
            "{element}"
        );
    }
}
#[test]
fn policy_browser_false_and_malformed_boolean_close_the_gate() {
    let path = std::path::Path::new("/synthetic/policy.json");
    assert_eq!(
        crate::browser::policy_refusal(&json!({"browser":false}), path).unwrap(),
        "The browser panel is off. Set by your organization (browser in /synthetic/policy.json). Nothing was started."
    );
    for value in [
        json!([]),
        json!({"browser":"true"}),
        json!({"version":2}),
        json!({"accounts":0}),
    ] {
        assert!(crate::browser::policy_refusal(&value, path).is_some());
    }
    assert!(crate::browser::policy_refusal(&json!({"browser":true}), path).is_none());
}
#[test]
fn forgetting_a_profile_closes_only_its_tabs() {
    let mut h = Host::default();
    h.profiles = vec![
        ("codec".into(), "Codec".into()),
        ("argon".into(), "Argon".into()),
    ];
    let codec = h.add(Profile(Some("codec".into()))).unwrap();
    let argon = h.add(Profile(Some("argon".into()))).unwrap();
    let shared = h.add(Profile(None)).unwrap();
    h.forget(Profile(Some("codec".into())));
    assert!(!h.tabs.iter().any(|t| t.id == codec));
    assert!(h.tabs.iter().any(|t| t.id == argon));
    assert!(h.tabs.iter().any(|t| t.id == shared));
    assert_eq!(h.forgotten, vec![Profile(Some("codec".into()))]);
}

#[test]
fn only_the_main_frame_is_held_to_the_address_rules_as_electron_does() {
    use crate::browser::navigation_is_checked;
    assert!(navigation_is_checked(true, "https://example.test/", false));
    assert!(navigation_is_checked(true, "javascript:alert(1)", false));
    // A site's own frames and about:blank are its business (Electron's will-navigate is main-frame only).
    assert!(!navigation_is_checked(false, "about:blank", false));
    assert!(!navigation_is_checked(false, "blob:https://example.test/1", false));
    assert!(!navigation_is_checked(true, "about:blank", false));
    // A preview tab may show only its own pages, frames included.
    assert!(navigation_is_checked(false, "https://elsewhere.test/", true));
    assert!(navigation_is_checked(true, "about:blank", true));
}
