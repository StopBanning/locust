//! Electron's `a-credential-file-is-never-shown.test.ts` (4 cases). Colin, 2026-09-30: a teammate working in his
//! `.claude` folder, and the thread drew `.credentials.json` (Claude Code's access and refresh tokens, in full) as a
//! change "seen on disk"; the ledger kept them. The watch read a changed file to draw its change and never asked what
//! the file was.
use super::*;
use std::cell::RefCell;

fn token() -> String {
    // Put together here so no token-shaped text sits in the public source.
    ["s", "k-ant-oat01-", &"Q".repeat(40)].concat()
}

// case: refuses the ones from that run, and their kind
#[test]
fn refuses_the_ones_from_that_run_and_their_kind() {
    for path in [
        ".credentials.json",
        "mcp-needs-auth-cache.json",
        "sessions/35372.782cf699e1d.key",
        ".config/app/settings.json",
        "secrets/prod.yml",
        "server.pem",
        "id_ed25519",
        "oauth-state.json",
        "cookies.sqlite",
    ] {
        assert!(holds_secrets(path), "{path}");
    }
}

// case: lets an ordinary project file through
#[test]
fn lets_an_ordinary_project_file_through() {
    for path in ["src/app.ts", "README.md", "data/prices.csv", "file-history/abc/def@v2"] {
        assert!(!holds_secrets(path), "{path}");
    }
}

/// Reads only through `read`, and remembers what it was asked to open.
struct Reader {
    opened: RefCell<Vec<String>>,
}

impl DiskObservationHost for Reader {
    fn run_git(&self, _args: &[&str], _cwd: &Path) -> Result<String, GitOutputError> {
        Ok(String::new())
    }
    fn stat_of(&self, _path: &Path) -> Option<FileStat> {
        None
    }
    fn read_text(&self, path: &Path) -> Option<String> {
        let path = path.to_string_lossy().into_owned();
        self.opened.borrow_mut().push(path.clone());
        Some(if path.ends_with(".credentials.json") {
            format!("{{\"accessToken\":\"{}\"}}", token())
        } else {
            format!("{{\"note\": \"copy of {}\", \"accessToken\": \"abc123secretvalue\"}}", token())
        })
    }
    fn read_text_up_to(&self, path: &Path, _max: usize) -> Option<String> {
        self.read_text(path)
    }
    fn list_directory(&self, _dir: &Path) -> io::Result<Vec<FolderEntry>> {
        Ok(Vec::new())
    }
    fn now_ms(&self) -> u64 {
        0
    }
}

// case: never opens a credential file, and scrubs a token anywhere else
#[test]
fn never_opens_a_credential_file_and_scrubs_a_token_anywhere_else() {
    let after = WorkspaceSnapshot {
        entries: BTreeMap::from([(".credentials.json".into(), "??".into()), ("notes.json".into(), "??".into())]),
        ..Default::default()
    };
    let host = Reader { opened: RefCell::new(Vec::new()) };
    let patches = observed_patches(Path::new("C:/work"), &after, &[".credentials.json".into(), "notes.json".into()], &host, None);
    assert!(!host.opened.borrow().iter().any(|p| p.contains(".credentials.json")), "{:?}", host.opened.borrow());
    assert!(!patches.contains_key(".credentials.json"));
    // The other file was read and drawn, with nothing secret left in it.
    assert!(host.opened.borrow().iter().any(|p| p.ends_with("notes.json")));
    let drawn = patches.get("notes.json").expect("notes.json is drawn").to_string();
    assert!(!drawn.contains(&token()), "{drawn}");
    assert!(!drawn.contains("abc123secretvalue"), "{drawn}");
}

// case: scrubs a key in JSON's own shape, quote before the colon
#[test]
fn scrubs_a_key_in_jsons_own_shape_quote_before_the_colon() {
    assert!(!crate::secrets::scrub_secrets("{\"refreshToken\":\"abcdef123456\"}").0.contains("abcdef123456"));
    assert!(!crate::secrets::scrub_secrets(&format!("token {} here", token())).0.contains(&token()));
}
