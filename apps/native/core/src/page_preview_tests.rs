//! Carries `apps/desktop/src/main/a-page-runs-in-its-own-frame.test.ts` (T158: the page server).
//! Expected values come from Electron's `page-preview.ts` and `a-page-runs-in-its-own-frame.test.ts`.
use super::*;
use std::fs;
use std::path::PathBuf;

struct Site {
    outside: PathBuf,
    root: PathBuf,
    pages: Pages,
    url: String,
}

impl Drop for Site {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.outside);
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn site() -> Site {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let base = std::env::temp_dir();
    let outside = base.join(format!("locust-page-outside-{}-{n}", std::process::id()));
    let root = base.join(format!("locust-page-root-{}-{n}", std::process::id()));
    fs::create_dir_all(&outside).unwrap();
    fs::create_dir_all(root.join("site/css")).unwrap();
    fs::write(outside.join("secret.txt"), "not for the page").unwrap();
    fs::write(root.join("site/index.html"), "<h1>Corner Shop</h1>").unwrap();
    fs::write(root.join("site/css/style.css"), "h1 { color: rgb(1, 2, 3) }").unwrap();
    let mut pages = Pages::default();
    let url = pages.url_for(&root.join("site/index.html"), Some(&root)).unwrap();
    Site { outside, root, pages, url }
}

// case: opens a page named relative to the folder, as a comparison column names the one in its copy (0.446)
#[test]
fn opens_a_page_named_relative_to_the_folder_as_a_comparison_column_names_the_one_in_its_copy() {
    let s = site();
    fs::create_dir_all(s.root.join(".locust/compare/cmp_1-a")).unwrap();
    fs::write(s.root.join(".locust/compare/cmp_1-a/index.html"), "<h1>Column A</h1>").unwrap();
    let mut server = Pages::default();
    let opened = server.url_for(Path::new(".locust/compare/cmp_1-a/index.html"), Some(&s.root)).unwrap();
    assert_eq!(server.read(&opened, Some(&s.root)).body, b"<h1>Column A</h1>");
    assert!(server.url_for(Path::new("site/index.html"), Some(&s.root)).is_ok());
    // Kept: the copy is removed, and the page is the folder's own at the same place.
    fs::write(s.root.join("index.html"), "<h1>Kept</h1>").unwrap();
    fs::remove_dir_all(s.root.join(".locust/compare/cmp_1-a")).unwrap();
    let after = server.url_for(Path::new(".locust/compare/cmp_1-a/index.html"), Some(&s.root)).unwrap();
    assert_eq!(server.read(&after, Some(&s.root)).body, b"<h1>Kept</h1>");
    // Relative never means outside: `..` out of the folder is still refused.
    fs::write(s.outside.join("elsewhere.html"), "<h1>Out</h1>").unwrap();
    assert!(server.url_for(Path::new("../elsewhere.html"), Some(&s.root)).is_err());
    let outside_name = s.outside.file_name().unwrap().to_string_lossy();
    assert!(server.url_for(Path::new(&format!("../{outside_name}/elsewhere.html")), Some(&s.root)).is_err());
}

// case: gives a page inside the folder its own address, on its own scheme, with a random host
#[test]
fn gives_a_page_inside_the_folder_its_own_address_on_its_own_scheme_with_a_random_host() {
    let s = site();
    let re = regex::Regex::new(&format!(r"^{PAGE_SCHEME}://[0-9a-f]{{24}}/index\.html$")).unwrap();
    assert!(re.is_match(&s.url), "unexpected url: {}", s.url);
}

// case: serves the page and what it links to, typed
#[test]
fn serves_the_page_and_what_it_links_to_typed() {
    let s = site();
    let page = s.pages.read(&s.url, Some(&s.root));
    assert_eq!(page.status, 200);
    assert!(page.mime.contains("text/html"), "{}", page.mime);
    assert_eq!(page.body, b"<h1>Corner Shop</h1>");
    let style = s.pages.read(&s.url.replace("index.html", "css/style.css"), Some(&s.root));
    assert_eq!(style.status, 200);
    assert!(style.mime.contains("text/css"), "{}", style.mime);
}

// case: serves a folder by its index.html
#[test]
fn serves_a_folder_by_its_index_html() {
    let s = site();
    let res = s.pages.read(&s.url.replace("index.html", ""), Some(&s.root));
    assert_eq!(res.status, 200);
    assert_eq!(res.body, b"<h1>Corner Shop</h1>");
}

// case: never serves a file outside the folder: not by ../, not by an encoded ../
#[test]
fn never_serves_a_file_outside_the_folder_not_by_dotdot_not_by_encoded_dotdot() {
    let s = site();
    let name = s.outside.file_name().unwrap().to_string_lossy();
    for escape in [
        format!("../../{name}/secret.txt"),
        format!("..%2F..%2F{name}%2Fsecret.txt"),
        format!("%2e%2e/%2e%2e/{name}/secret.txt"),
    ] {
        let answer = s.pages.read(&s.url.replace("index.html", &escape), Some(&s.root));
        assert_ne!(answer.status, 200, "{escape}");
    }
}

// case: never serves the project around the page, only the page's own folder
#[test]
fn never_serves_the_project_around_the_page_only_the_pages_own_folder() {
    let s = site();
    fs::write(s.root.join("README.md"), "the project").unwrap();
    fs::write(s.root.join(".env"), "API_KEY=sk-live").unwrap();
    for reach in ["../README.md", "../.env", "..%2F.env", "%2e%2e/.env"] {
        let answer = s.pages.read(&s.url.replace("index.html", reach), Some(&s.root));
        assert_ne!(answer.status, 200, "{reach}");
    }
}

// case: never serves a hidden file or a key, even beside the page
#[test]
fn never_serves_a_hidden_file_or_a_key_even_beside_the_page() {
    let s = site();
    let beside = s.root.join("site");
    fs::create_dir_all(beside.join(".git")).unwrap();
    fs::create_dir_all(beside.join("secrets")).unwrap();
    fs::create_dir_all(beside.join("data")).unwrap();
    let refused = [
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
    ];
    for name in refused {
        fs::write(beside.join(name), "secret").unwrap();
    }
    let served = ["data/prices.json", "tokens.css", "keyboard.js", "monkey.png"];
    for name in served {
        fs::write(beside.join(name), "fine").unwrap();
    }
    for name in refused {
        assert_eq!(s.pages.read(&s.url.replace("index.html", name), Some(&s.root)).status, 403, "{name}");
    }
    for name in served {
        assert_eq!(s.pages.read(&s.url.replace("index.html", name), Some(&s.root)).status, 200, "{name}");
    }
}

// case: never serves through a link inside the folder that points outside it
#[test]
fn never_serves_through_a_link_inside_the_folder_that_points_outside_it() {
    let s = site();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&s.outside, s.root.join("site/escape")).unwrap();
        let answer = s.pages.read(&s.url.replace("index.html", "escape/secret.txt"), Some(&s.root));
        assert_ne!(answer.status, 200);
    }
}

// case: judges the file a link resolves to, not only the name asked for
#[test]
fn judges_the_file_a_link_resolves_to_not_only_the_name_asked_for() {
    let s = site();
    fs::create_dir_all(s.root.join("site/.git")).unwrap();
    fs::write(s.root.join("site/.git/config"), "[remote] url = secret").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(s.root.join("site/.git"), s.root.join("site/gitstuff")).unwrap();
        let answer = s.pages.read(&s.url.replace("index.html", "gitstuff/config"), Some(&s.root));
        assert_eq!(answer.status, 403);
    }
}

// case: answers nothing for a host it did not hand out
#[test]
fn answers_nothing_for_a_host_it_did_not_hand_out() {
    let s = site();
    let answer = s.pages.read(&format!("{PAGE_SCHEME}://000000000000000000000000/site/index.html"), Some(&s.root));
    assert_eq!(answer.status, 404);
}

// case: gives no address to a page outside the folder, or to a file that is not a page
#[test]
fn gives_no_address_to_a_page_outside_the_folder_or_to_a_file_that_is_not_a_page() {
    let mut s = site();
    fs::write(s.outside.join("page.html"), "<p>x</p>").unwrap();
    assert!(s.pages.url_for(&s.outside.join("page.html"), Some(&s.root)).is_err());
    assert!(s.pages.url_for(&s.root.join("site/css/style.css"), Some(&s.root)).is_err());
    assert!(is_page_path(Path::new("a/b/INDEX.HTM")));
}
