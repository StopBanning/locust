//! renderer/src/popular-skills-wait-for-keep.test.tsx, drawn headless on Settings › Teammates. Electron's cases also
//! grep its .tsx source for a string (the tile calls `look(pack.source, true)`, never `installSkillLibrary`); a Rust
//! source has no such lines to read, so the same behaviour is pressed instead: a tile looks, and nothing is kept until
//! Keep. A fake GitHub answers; nothing reaches the network.
use super::*;
use locust_core::chat_http::{HttpRequest, HttpResponse};
use locust_core::skill_library::{KeptSkill, git_blob_id};
use crate::settings_pages::SettingsPageId;
use rune::{Env, Runtime};
use serde_json::json;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const SHA: &str = "cccccccccccccccccccccccccccccccccccccccc";

/// A pack with one skill that runs a script and one GitHub would not let Claude Code take.
struct Pack;
impl HttpClient for Pack {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
        let files = [("skills/review/SKILL.md", "---\ndescription: Review it.\n---\n"), ("skills/review/scripts/check.py", "print(1)"), ("skills/bad name/SKILL.md", "held")];
        let ok = |body: String| Ok(HttpResponse { status: 200, headers: vec![], body: body.into_bytes() });
        let url = req.url;
        if url.ends_with("/repos/mattpocock/skills") {
            return ok(json!({"full_name": "mattpocock/skills", "default_branch": "main"}).to_string());
        }
        if url.ends_with("/commits/main") {
            return ok(json!({"sha": SHA}).to_string());
        }
        if url.contains("/git/trees/") {
            let tree: Vec<_> = files.iter().map(|(p, t)| json!({"path": p, "mode": "100644", "type": "blob", "sha": git_blob_id(t.as_bytes()), "size": t.len()})).collect();
            return ok(json!({"tree": tree}).to_string());
        }
        match files.iter().find(|(p, _)| url.ends_with(&p.replace(' ', "%20"))) {
            Some((_, t)) => ok(t.to_string()),
            None => Ok(HttpResponse { status: 404, headers: vec![], body: vec![] }),
        }
    }
}

fn page(s: State) -> Runtime<State> {
    let mut s = s;
    s.settings_panel.open = true;
    s.settings_panel.page = Some(SettingsPageId::Teammates);
    let mut rt = Runtime::new(s, crate::view, (1400.0, 6000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}
fn words(rt: &Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect::<Vec<_>>().join(" | ")
}
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
}
fn settle(rt: &mut Runtime<State>) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let mut answered = false;
    while !answered {
        assert!(std::time::Instant::now() < deadline, "the look never answered");
        std::thread::sleep(std::time::Duration::from_millis(10));
        rt.update(|s, _| answered = poll(s));
        rt.frame(0.016);
    }
}
fn with_store() -> (Temp, State) {
    let root = std::env::temp_dir().join(format!("locust-skills-shelf-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    (Temp(root), s)
}

#[test]
fn a_kept_pack_says_how_many_of_its_files_run_in_a_sentence_that_agrees() {
    assert_eq!(files_that_run(1), "1 file that runs");
    assert_eq!(files_that_run(3), "3 files that run");
    let kept = |runs: Vec<&str>| KeptSource {
        source: "t3dotgg/tstack".into(),
        reference: "main".into(),
        sha: SHA.into(),
        kept_at: "2026-11-01T10:00:00.000Z".into(),
        skills: vec![KeptSkill { name: "review".into(), files: 3, bytes: 10, runs: runs.into_iter().map(Into::into).collect() }],
    };
    // The trending drive's kept tstack row (2026-11-01) read "9 skills · 1 file that run".
    assert!(kept_meta(&kept(vec!["a.sh"])).ends_with("1 skill · 1 file that runs"), "{}", kept_meta(&kept(vec!["a.sh"])));
    assert!(kept_meta(&kept(vec!["a.sh", "b.py", "c.js"])).ends_with(" · 3 files that run"));
    assert!(!kept_meta(&kept(vec![])).contains("that run"));
    // And the row on the page says it.
    let mut s = State::default();
    s.skills_from_github.kept = vec![kept(vec!["a.sh"])];
    assert!(words(&page(s)).contains("1 skill · 1 file that runs"));
}

#[test]
fn the_popular_shelf_shows_five_packs_and_opens_the_existing_look_before_any_keep() {
    let (_t, s) = with_store();
    let mut rt = page(s);
    let shown = words(&rt);
    assert!(shown.contains("Popular skills"));
    for pack in POPULAR_SKILLS {
        assert!(shown.contains(pack.source) && shown.contains(pack.license) && shown.contains(&stars_text(pack.stars)), "{}", pack.source);
    }
    assert!(shown.contains("Needs its own install to work."));
    assert!(shown.contains("Stars as of 2026-10-30"));
    // The repository opens on GitHub, from its own link under the tile.
    click(&mut rt, "skills-repo-mattpocock/skills");
    assert_eq!(rt.state.skills_from_github.opened, ["https://github.com/mattpocock/skills"]);
    // The tile looks, and only looks: every offered skill is picked, ready for Keep, and nothing is kept.
    use_client(Arc::new(Pack));
    click(&mut rt, "skills-pack-mattpocock/skills");
    settle(&mut rt);
    let preview = rt.state.skills_from_github.preview.clone().expect("a look");
    assert_eq!(preview.source, "mattpocock/skills");
    assert_eq!(rt.state.skills_from_github.picked, ["review"]);
    let store = rt.state.data.clone().unwrap();
    assert!(SkillLibrary::for_store(&store, Box::new(Offline)).list().is_empty(), "a look keeps nothing");
    assert!(!store.root().join("skill-library/skills").exists());
    let shown = words(&rt);
    assert!(shown.contains("Runs: scripts/check.py"), "{shown}");
    assert!(shown.contains("Keep 1 skill"));
    // Keep is its own press.
    click(&mut rt, "skills-keep");
    settle(&mut rt);
    assert!(store.root().join("skill-library/skills/review/scripts/check.py").is_file());
    assert_eq!(rt.state.skills_from_github.kept.len(), 1);
    assert!(words(&rt).contains("1 skill · 1 file that runs"));
}

#[test]
fn the_new_this_week_row_uses_the_same_tile_and_says_how_long_each_pack_has_been_climbing() {
    let shown = words(&page(State::default()));
    assert!(shown.contains("New this week"));
    assert!(shown.contains("Stars as of 2026-11-01"));
    for pack in TRENDING_SKILLS {
        assert!(shown.contains(pack.source));
        assert!(shown.contains(&format!("{} stars since {} · {}", stars_text(pack.stars), since_text(pack.since.unwrap()), pack.license)), "{}", pack.source);
    }
    assert_eq!(since_text("2026-10-24"), "Oct 24");
    // The same tile: each new pack is pressed the same way a popular one is.
    let rt = page(State::default());
    for pack in TRENDING_SKILLS {
        assert!(rt.find(&format!("skills-pack-{}", pack.source)).is_some(), "{}", pack.source);
    }
}

#[test]
fn every_file_that_runs_is_named_before_keeping_even_when_a_skill_has_more_than_four_scripts() {
    assert_eq!(runs_text(&["a.sh", "b.py", "c.js", "d.exe", "e.ps1"]).as_deref(), Some("Runs: a.sh, b.py, c.js, d.exe, e.ps1"));
    assert_eq!(runs_text(&[]), None);
}

// skills-kept-from-github.test.ts's cases are core's (skill_library.rs); these two are where the window holds them:
// the box a repository of the person's own is pasted in (SkillsFromGitHub.tsx `lc-skilllib__look`), and the client
// that lets "follows a redirect only to GitHub's own hosts" hold on the real network.

#[test]
fn a_repository_of_your_own_is_pasted_in_the_box_and_only_looked_at() {
    let (_t, s) = with_store();
    let mut rt = page(s);
    assert!(rt.find("skills-link").is_some(), "the box is drawn");
    assert!(words(&rt).contains("| Look |"), "and its Look");
    // Look does nothing with the box empty.
    click(&mut rt, "skills-look");
    assert_eq!(rt.state.skills_from_github.busy, None);
    use_client(Arc::new(Pack));
    click(&mut rt, "skills-link");
    rt.paste("https://github.com/mattpocock/skills");
    rt.frame(0.0);
    click(&mut rt, "skills-look");
    assert_eq!(rt.state.skills_from_github.busy, Some(LOOKING), "Look asks GitHub");
    settle(&mut rt);
    let preview = rt.state.skills_from_github.preview.clone().expect("a look");
    assert_eq!(preview.source, "mattpocock/skills");
    // A pasted repository is looked at, nothing picked for it and nothing kept.
    assert!(rt.state.skills_from_github.picked.is_empty());
    let store = rt.state.data.clone().unwrap();
    assert!(SkillLibrary::for_store(&store, Box::new(Offline)).list().is_empty(), "a look keeps nothing");
    // Return in the box looks too.
    rt.update(|s, _| s.skills_from_github.preview = None);
    click(&mut rt, "skills-link");
    rt.key(rune::Key::Enter.into());
    rt.frame(0.0);
    settle(&mut rt);
    assert!(rt.state.skills_from_github.preview.is_some(), "Return looked again");
}

#[test]
fn the_shelfs_own_client_hands_a_redirect_back_so_only_githubs_hosts_are_followed() {
    use std::io::{Read as _, Write as _};
    let elsewhere = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let target = format!("http://{}/payload", elsewhere.local_addr().unwrap());
    elsewhere.set_nonblocking(true).unwrap();
    let first = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/repos/acme/skills", first.local_addr().unwrap());
    let answer = format!("HTTP/1.1 302 Found\r\nLocation: {target}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = first.accept() {
            let _ = stream.read(&mut [0u8; 4096]);
            let _ = stream.write_all(answer.as_bytes());
        }
    });
    let got = real_client()
        .send(HttpRequest { method: "GET".into(), url, headers: vec![("User-Agent".into(), "Locust".into())], body: None, timeout_secs: 5 })
        .expect("an answer");
    assert_eq!(got.status, 302, "the redirect comes back to the library, which allows only GitHub's own hosts");
    assert!(elsewhere.accept().is_err(), "where it pointed was never asked");
}

/// The Look button's own words.
fn look_button(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("skills-look")).and_then(|e| e.text)
}

// Lane check: SkillsFromGitHub.tsx -- a tile and Look for changes put the pack's name in the box, Look reads Looking…
// while a look is out, and a keep empties the box.
#[test]
fn a_tile_and_look_for_changes_name_the_pack_in_the_box_and_a_keep_empties_it() {
    let (_t, s) = with_store();
    let mut rt = page(s);
    use_client(Arc::new(Pack));
    assert_eq!(look_button(&rt).as_deref(), Some("Look"));
    click(&mut rt, "skills-pack-mattpocock/skills");
    assert_eq!(rt.state.skills_from_github.link.text, "mattpocock/skills", "the tile names its pack in the box");
    assert_eq!(look_button(&rt).as_deref(), Some("Looking…"), "Look reads Looking… while a look is out");
    settle(&mut rt);
    assert_eq!(look_button(&rt).as_deref(), Some("Look"));
    click(&mut rt, "skills-keep");
    settle(&mut rt);
    assert_eq!(rt.state.skills_from_github.link.text, "", "a keep empties the box");
    for _ in 0..20 {
        rt.frame(0.05);
    }
    click(&mut rt, "skills-again-mattpocock/skills");
    assert_eq!(rt.state.skills_from_github.link.text, "mattpocock/skills", "Look for changes names the pack too");
    settle(&mut rt);
}
