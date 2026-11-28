//! Carries `apps/desktop/src/main/a-teammate-browses-and-asks-before-it-acts.test.ts` (T250)
//! and `apps/desktop/src/main/a-teammate-signs-in-apart-in-the-browser.test.ts` (T256).
use super::*;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp(name: &str) -> Temp {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "locust-browser-parity-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    Temp(root)
}

// --- T250: a-teammate-browses-and-asks-before-it-acts.test.ts ---

// case: uses an isolated persistent Chromium partition apart from the person’s Chrome
#[test]
fn uses_an_isolated_persistent_partition_apart_from_the_persons_browser() {
    // Native uses WKWebView WKWebsiteDataStore keyed by a namespaced per-profile UUID (macOS 14+)
    // or nonPersistentDataStore, apart from Safari/Chrome's default store.
    let mac = include_str!("../../src/browser_mac.rs");
    assert!(mac.contains("locust-native-browser-v1:"));
    assert!(mac.contains("WKWebsiteDataStore::dataStoreForIdentifier"));
    assert!(mac.contains("WKWebsiteDataStore::nonPersistentDataStore"));
    assert!(!mac.contains("defaultDataStore"));
}

// case: allows http, https, localhost, and file URLs inside the conversation’s folder, and refuses paths or symlinks outside it
#[test]
fn allows_http_https_localhost_and_workspace_file_urls_and_refuses_outside_paths_or_symlinks() {
    let root = temp("url-root");
    let outside = temp("url-outside");
    let workspace = root.0.join("workspace");
    fs::create_dir_all(&workspace).unwrap();
    let inside_file = workspace.join("index.html");
    fs::write(&inside_file, "<h1>Inside</h1>").unwrap();
    let outside_file = outside.0.join("secret.html");
    fs::write(&outside_file, "<h1>Outside</h1>").unwrap();
    let real_inside = inside_file.canonicalize().unwrap();

    let u1 = resolve("https://example.com/docs", Some(&workspace)).unwrap();
    assert_eq!(u1.as_str(), "https://example.com/docs");
    assert_eq!(origin(&u1), "https://example.com");

    let u2 = resolve("localhost:3000/app", Some(&workspace)).unwrap();
    assert_eq!(u2.as_str(), "http://localhost:3000/app");
    assert_eq!(origin(&u2), "http://localhost:3000");

    let u3 = resolve("./index.html", Some(&workspace)).unwrap();
    assert_eq!(u3.to_file_path().unwrap(), real_inside);
    assert_eq!(origin(&u3), "file://workspace");

    let inside_url = Url::from_file_path(&inside_file).unwrap();
    let u4 = resolve(inside_url.as_str(), Some(&workspace)).unwrap();
    assert_eq!(u4.to_file_path().unwrap(), real_inside);
    assert_eq!(origin(&u4), "file://workspace");

    assert!(resolve("../outside.html", Some(&workspace)).is_err());
    let outside_url = Url::from_file_path(&outside_file).unwrap();
    assert!(resolve(outside_url.as_str(), Some(&workspace)).is_err());
    assert!(resolve("javascript:alert(1)", Some(&workspace)).is_err());
    assert!(resolve("data:text/html,hi", Some(&workspace)).is_err());

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside_file, workspace.join("escape-link.html")).unwrap();
        assert!(resolve("./escape-link.html", Some(&workspace)).is_err());
        std::os::unix::fs::symlink(&outside.0, workspace.join("escape-dir")).unwrap();
        assert!(resolve("./escape-dir/secret.html", Some(&workspace)).is_err());
    }
}

// case: asks first before credentials, form submission, purchases, sends, downloads, and leaving for an unopened site
#[test]
fn asks_first_before_credentials_form_submission_purchases_sends_downloads_and_leaving_origin() {
    let trusted = HashSet::from(["https://example.com".to_owned()]);
    // 1. Typing into a password or payment field -> credential
    assert_eq!(
        actions::gate(
            "type",
            &json!({"ref": "@e1", "tag": "input", "role": "textbox", "name": "Password", "type": "password", "sensitive": true}),
            "",
            "https://example.com/login",
            &trusted,
        )
        .map(|g| g.0),
        Some("credential")
    );
    assert_eq!(
        actions::gate(
            "type",
            &json!({"ref": "@e2", "tag": "input", "role": "textbox", "name": "Card number"}),
            "",
            "https://example.com/pay",
            &trusted,
        )
        .map(|g| g.0),
        Some("credential")
    );
    // Ordinary search field does not ask first
    assert!(
        actions::gate(
            "type",
            &json!({"ref": "@e3", "tag": "input", "role": "searchbox", "name": "Search documentation", "type": "search"}),
            "",
            "https://example.com",
            &trusted,
        )
        .is_none()
    );
    // 2. Submitting a form -> submit
    assert_eq!(
        actions::gate(
            "click",
            &json!({"ref": "@e4", "tag": "button", "role": "button", "name": "Sign in", "type": "submit", "submit": true}),
            "",
            "https://example.com/login",
            &trusted,
        )
        .map(|g| g.0),
        Some("submit")
    );
    // 3. Purchase or checkout -> purchase
    assert_eq!(
        actions::gate(
            "click",
            &json!({"ref": "@e5", "tag": "button", "role": "button", "name": "Place order"}),
            "",
            "https://shop.example.com/cart",
            &trusted,
        )
        .map(|g| g.0),
        Some("purchase")
    );
    // 4. Sending or posting -> send
    assert_eq!(
        actions::gate(
            "click",
            &json!({"ref": "@e6", "tag": "button", "role": "button", "name": "Send message"}),
            "",
            "https://mail.example.com/compose",
            &trusted,
        )
        .map(|g| g.0),
        Some("send")
    );
    // 5. Download -> download
    assert_eq!(
        actions::gate(
            "download",
            &json!({"name": "report.csv"}),
            "",
            "https://example.com/releases",
            &trusted,
        )
        .map(|g| g.0),
        Some("download")
    );
    // 6. Leaving for a site the person has not opened in that tab -> leave-origin
    assert_eq!(
        actions::gate(
            "click",
            &json!({"name": "Other site", "href": "https://other-site.org/page"}),
            "",
            "https://example.com/home",
            &trusted,
        )
        .map(|g| g.0),
        Some("leave-origin")
    );
    // Staying on an already-trusted origin does not ask first
    assert!(
        actions::gate(
            "click",
            &json!({"name": "Docs", "href": "https://example.com/docs"}),
            "",
            "https://example.com/home",
            &trusted,
        )
        .is_none()
    );
}

// case: wires per-run browser MCP config into Claude, OpenCode, and Copilot commands and names runtimes that lack per-run MCP
#[test]
fn wires_per_run_browser_mcp_config_into_claude_opencode_and_copilot_and_skips_cursor() {
    let dir = temp("mcp-wire");
    let host = Arc::new(Mutex::new(Host::default()));
    crate::browser_tools::install(host);
    let channel = crate::approval::Channel::new();
    let cancel = Arc::new(AtomicBool::new(false));

    let mut claude_cmd = std::process::Command::new("claude");
    let claude_reg = crate::browser_tools::Registration::prepare(
        &mut claude_cmd,
        "claude",
        "m-claude",
        "tm",
        &dir.0,
        channel.clone(),
        cancel.clone(),
    )
    .unwrap()
    .expect("claude gets browser MCP config");
    assert!(claude_reg.claude_config().is_some());

    let mut opencode_cmd = std::process::Command::new("opencode");
    let _opencode_reg = crate::browser_tools::Registration::prepare(
        &mut opencode_cmd,
        "opencode",
        "m-opencode",
        "tm",
        &dir.0,
        channel.clone(),
        cancel.clone(),
    )
    .unwrap()
    .expect("opencode gets browser MCP config");
    let op_env = opencode_cmd
        .get_envs()
        .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
        .and_then(|(_, v)| v)
        .expect("OPENCODE_CONFIG_CONTENT set");
    let op_json: Value = serde_json::from_str(&op_env.to_string_lossy()).unwrap();
    assert!(op_json["mcp"]["locust_browser"].is_object());

    let mut copilot_cmd = std::process::Command::new("copilot");
    let _copilot_reg = crate::browser_tools::Registration::prepare(
        &mut copilot_cmd,
        "copilot",
        "m-copilot",
        "tm",
        &dir.0,
        channel.clone(),
        cancel.clone(),
    )
    .unwrap()
    .expect("copilot gets browser MCP config");
    let copilot_args: Vec<String> = copilot_cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert!(copilot_args.iter().any(|a| a == "--additional-mcp-config"), "{copilot_args:?}");

    let mut cursor_cmd = std::process::Command::new("cursor");
    let cursor_reg = crate::browser_tools::Registration::prepare(
        &mut cursor_cmd,
        "cursor",
        "m-cursor",
        "tm",
        &dir.0,
        channel,
        cancel,
    )
    .unwrap();
    assert!(cursor_reg.is_none(), "cursor lacks per-run browser MCP");
}

// case: honors organization policy browser: false
#[test]
fn honors_organization_policy_browser_false() {
    let dir = temp("org-policy");
    let policy_path = dir.0.join("policy.json");
    fs::write(&policy_path, json!({"browser": false}).to_string()).unwrap();
    let parsed: Value = serde_json::from_str(&fs::read_to_string(&policy_path).unwrap()).unwrap();
    let refusal = policy_refusal(&parsed, &policy_path).expect("browser: false refused");
    assert!(refusal.contains("The browser panel is off. Set by your organization"), "{refusal}");
    assert!(policy_refusal(&json!({}), &policy_path).is_none());
}

// --- T256: a-teammate-signs-in-apart-in-the-browser.test.ts ---

// case: opens a tab as the teammate in a partition of its own, colored as it, beside the shared one
#[test]
fn opens_a_tab_as_the_teammate_in_a_partition_of_its_own_beside_the_shared_one() {
    let mut host = Host::default();
    host.profiles = vec![("tm_wren".into(), "Wren".into())];
    host.add(Profile(None)).unwrap();
    host.add(Profile(Some("tm_wren".into()))).unwrap();
    let profiles: Vec<Profile> = host.tabs.iter().map(|t| t.profile.clone()).collect();
    assert_eq!(profiles, vec![Profile(None), Profile(Some("tm_wren".into()))]);
}

// case: opens nothing as a teammate that does not have its own sign-ins
#[test]
fn opens_nothing_as_a_teammate_that_does_not_have_its_own_sign_ins() {
    let mut host = Host::default();
    let err = host.add(Profile(Some("tm_wren".into()))).unwrap_err();
    assert!(host.tabs.is_empty());
    assert_eq!(
        err,
        "That teammate does not have its own browser sign-ins any more, so no tab was opened."
    );
}

// case: lets each run see and use only the tabs in its own sign-ins
#[test]
fn lets_each_run_see_and_use_only_the_tabs_in_its_own_sign_ins() {
    let mut host = Host::default();
    host.profiles = vec![("tm_wren".into(), "Wren".into())];
    let shared_id = host.add(Profile(None)).unwrap();
    let wren_id = host.add(Profile(Some("tm_wren".into()))).unwrap();
    let wren = Profile(Some("tm_wren".into()));
    let sable = Profile(None);

    let wren_tabs = host.state(&wren);
    assert_eq!(wren_tabs.as_array().unwrap().len(), 1);
    assert_eq!(wren_tabs[0]["id"], format!("tab_{wren_id}"));

    let sable_tabs = host.state(&sable);
    assert_eq!(sable_tabs.as_array().unwrap().len(), 1);
    assert_eq!(sable_tabs[0]["id"], format!("tab_{shared_id}"));

    assert!(host.scoped(shared_id, &wren).err().unwrap().contains("signed in as someone else, so it is not yours to use"));
    assert!(host.scoped(wren_id, &sable).err().unwrap().contains("signed in as someone else, so it is not yours to use"));
    assert_eq!(host.tabs.len(), 2);
}

// case: makes a run its own tab in its own sign-ins when it opens one
#[test]
fn makes_a_run_its_own_tab_in_its_own_sign_ins_when_it_opens_one() {
    let mut host = Host::default();
    host.profiles = vec![("tm_wren".into(), "Wren".into())];
    let wren = Profile(Some("tm_wren".into()));
    let id = host.add(wren.clone()).unwrap();
    assert_eq!(host.tabs.len(), 1);
    assert_eq!(host.tabs[0].id, id);
    assert_eq!(host.tabs[0].profile, wren);
}

// case: forgets them: the teammate’s tabs close and its partition is cleared, the shared tab stays
#[test]
fn forgets_them_the_teammates_tabs_close_and_its_partition_is_cleared_the_shared_tab_stays() {
    let mut host = Host::default();
    host.profiles = vec![("tm_wren".into(), "Wren".into())];
    let shared_id = host.add(Profile(None)).unwrap();
    let _wren_id = host.add(Profile(Some("tm_wren".into()))).unwrap();
    let wren = Profile(Some("tm_wren".into()));
    host.forget(wren.clone());
    assert_eq!(host.tabs.iter().map(|t| t.id).collect::<Vec<_>>(), vec![shared_id]);
    assert!(host.forgotten.contains(&wren));
    assert!(host.clearing.contains(&wren));
}
