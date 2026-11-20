//! SKILLS FROM A GITHUB REPOSITORY (main/skill-library.ts, 0.710; shared/popular-skills.ts).
//!
//! A person names a public repository; this lists the skill folders in it (a folder holding a SKILL.md) with every
//! file each carries, the files a run could execute marked, and keeps the ones they pick at the one commit the list was
//! read from. Every file is downloaded at that commit and checked against the git blob id GitHub's own tree gives it,
//! so what lands is byte for byte what the list described; links and submodules are never carried; a redirect is
//! followed only to GitHub's own two hosts. Public repositories only: no sign-in is asked for or sent.
//!
//! Kept skills live in `<root>/skills/<name>/` (the native profile's `skill-library`, which `skills::sources_for_store`
//! hands to every run), and `<root>/library.json` records which repository and commit each came from, in Electron's
//! own shape.
//!
//! Done differently from Electron: files are fetched one at a time, not six at once. Electron's lanes let a refused
//! install's cleanup race a file still being written, which once left a `.staging-*` folder behind (0.711); with one
//! request at a time nothing can still be writing when the staging folder goes. A skill is a handful of small files.
//!
//! The HTTP client is passed in (`chat_http::HttpClient`), so no test reaches the network.

use crate::chat_http::{HttpClient, HttpRequest, HttpResponse};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Mutex;

const API: &str = "https://api.github.com";
const RAW: &str = "https://raw.githubusercontent.com";
const SKILL_FILE: &str = "SKILL.md";
/// The same bounds a folder's own skills have (skills.rs). Bundled canvas-design fonts pass 5 MB; 10 MB admits them.
const MOST_BYTES_A_SKILL: u64 = 10 * 1024 * 1024;
const MOST_BYTES_ALL: u64 = 40 * 1024 * 1024;
const MOST_SKILLS: usize = 200;
const MOST_FILES_A_SKILL: usize = 500;
const MOST_REDIRECTS: usize = 3;
const TIMEOUT_SECS: u64 = 20;

/// A pattern compiled once (a tree can list thousands of files), from source literals only.
fn re(pattern: &'static str) -> regex::Regex {
    static SEEN: Mutex<Vec<(&'static str, regex::Regex)>> = Mutex::new(Vec::new());
    let mut seen = crate::invariants::lock(&SEEN);
    if let Some((_, r)) = seen.iter().find(|(p, _)| *p == pattern) {
        return r.clone();
    }
    let r = crate::invariants::regex(pattern);
    seen.push((pattern, r.clone()));
    r
}
fn skill_name_ok(name: &str) -> bool {
    re(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$").is_match(name)
}
fn owner_ok(owner: &str) -> bool {
    re(r"^[A-Za-z0-9][A-Za-z0-9-]{0,38}$").is_match(owner)
}
fn repo_ok(repo: &str) -> bool {
    re(r"^[A-Za-z0-9._-]{1,100}$").is_match(repo) && repo != "." && repo != ".."
}
fn sha_ok(sha: &str) -> bool {
    re(r"^[0-9a-f]{40}$").is_match(sha)
}
/// A file a run could execute: by its extension, or because git records it executable (mode 100755).
fn runs(path: &str, mode: &str) -> bool {
    mode == "100755"
        || re(r"(?i)\.(?:sh|bash|zsh|fish|ps1|psm1|bat|cmd|py|pyw|js|mjs|cjs|ts|mts|cts|rb|pl|php|lua|exe|dll|so|dylib|jar|vbs|wsf|applescript|scpt|command)$")
            .is_match(path)
}
/// A path part Windows cannot hold; a repository made on Linux can carry one.
fn windows_bad(part: &str) -> bool {
    re(r#"[<>:"\\|?*\x00-\x1f]|[. ]$"#).is_match(part) || re(r"(?i)^(?:con|prn|aux|nul|com[0-9]|lpt[0-9])(?:\..*)?$").is_match(part)
}

/// Git's own id for a file's bytes, what a tree entry's `sha` is: SHA-1 of `blob <len>\0` and the bytes.
pub fn git_blob_id(bytes: &[u8]) -> String {
    let mut data = format!("blob {}\0", bytes.len()).into_bytes();
    data.extend_from_slice(bytes);
    sha1_hex(&data)
}

/// SHA-1 (FIPS 180-4), for git's object ids only: no new crate for one hash.
fn sha1_hex(data: &[u8]) -> String {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut message = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 80];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (slot, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *slot = slot.wrapping_add(v);
        }
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}

/// A repository the person named: `owner/repo`, `github.com/owner/repo`, or GitHub's own
/// `https://github.com/owner/repo/tree/<branch>/<folder>` (a branch with a slash reads as branch + folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryLink {
    pub owner: String,
    pub repo: String,
    pub reference: Option<String>,
    pub path: Option<String>,
}

fn decode(part: &str) -> String {
    url::form_urlencoded::parse(format!("x={}", part.replace('+', "%2B")).as_bytes()).next().map(|(_, v)| v.into_owned()).unwrap_or_default()
}

/// parseRepositoryLink.
pub fn parse_repository_link(text: &str) -> Option<RepositoryLink> {
    let rest = text.trim();
    let rest = rest.split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    let rest = re(r"(?i)^(?:https?://)?(?:www\.)?github\.com/").replace(rest, "").into_owned();
    if re(r"(?i)^[a-z][a-z0-9+.-]*:").is_match(&rest) {
        return None;
    }
    let parts: Vec<&str> = rest.split('/').collect();
    let owner = *parts.first()?;
    let repo = re(r"(?i)\.git$").replace(parts.get(1)?, "").into_owned();
    if !owner_ok(owner) || !repo_ok(&repo) {
        return None;
    }
    if parts.len() == 2 {
        return Some(RepositoryLink { owner: owner.into(), repo, reference: None, path: None });
    }
    if parts[2] != "tree" || parts.get(3).is_none_or(|p| p.is_empty()) {
        return None;
    }
    let folder: Vec<String> = parts[4..].iter().map(|p| decode(p)).collect();
    if folder.iter().any(|p| p.is_empty() || p == "." || p == "..") {
        return None;
    }
    Some(RepositoryLink { owner: owner.into(), repo, reference: Some(decode(parts[3])), path: (!folder.is_empty()).then(|| folder.join("/")) })
}

/// skillDescription: a SKILL.md front matter's `description:`, one line, quotes taken off, at most 300 characters.
pub fn skill_description(text: &str) -> Option<String> {
    let front = re(r"(?s)^\x{feff}?---\r?\n(.*?)\r?\n---").captures(text)?;
    let line = re(r"(?m)^description:[ \t]*(.+)$").captures(&front[1])?[1].trim().to_owned();
    let value = re(r#"^(["'])(.*)["']$"#).captures(&line).filter(|c| line.starts_with(&c[1]) && line.ends_with(&c[1])).map_or(line.clone(), |c| c[2].to_owned());
    let value = value.trim();
    if value.is_empty() || value == "|" || value == ">" {
        return None;
    }
    Some(if value.chars().count() > 300 { format!("{}…", value.chars().take(299).collect::<String>()) } else { value.to_owned() })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TreeEntry {
    path: String,
    mode: String,
    kind: String,
    sha: String,
    size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFile {
    pub path: String,
    pub bytes: u64,
    pub runs: bool,
}

/// A skill a look found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreviewSkill {
    pub name: String,
    pub folder: String,
    pub bytes: u64,
    pub files: Vec<SkillFile>,
    pub description: Option<String>,
    /// Why it cannot be kept, said to the person; None when it can.
    pub held: Option<String>,
    /// The commit it is kept at, when it is kept from this repository.
    pub kept_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Preview {
    pub source: String,
    pub reference: String,
    pub sha: String,
    pub skills: Vec<PreviewSkill>,
    pub partial: bool,
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptSkill {
    pub name: String,
    pub files: usize,
    pub bytes: u64,
    pub runs: Vec<String>,
}

/// One repository's kept skills, as `library.json` records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptSource {
    pub source: String,
    pub reference: String,
    pub sha: String,
    pub kept_at: String,
    pub skills: Vec<KeptSkill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRequest {
    pub source: String,
    pub reference: String,
    pub sha: String,
    pub names: Vec<String>,
    pub path: Option<String>,
}

impl Preview {
    /// What Keep asks for: this look's commit and folder, with `names`.
    pub fn keep(&self, names: &[&str]) -> InstallRequest {
        InstallRequest { source: self.source.clone(), reference: self.reference.clone(), sha: self.sha.clone(), names: names.iter().map(|n| n.to_string()).collect(), path: self.path.clone() }
    }
}

struct Found {
    skill: PreviewSkill,
    entries: Vec<TreeEntry>,
}

fn megabytes(bytes: u64) -> String {
    format!("{} MB", (bytes as f64 / (1024.0 * 1024.0)).round() as u64)
}

/// Only agent skill roots collapse; other same-named folders stay real clashes.
const AGENT_SKILLS: &str = r"(^|/)(?:\.agent|\.agents|\.claude|\.codex|\.cursor|\.opencode|\.github|\.devin|\.dsh|\.gemini|\.grok|\.hermes|\.kiro|\.omp|\.pi|\.qoder|\.rovodev|\.trae-cn|\.trae|\.veto|\.vibe)/skills/";
fn copy_rank(folder: &str) -> u8 {
    if !re(AGENT_SKILLS).is_match(folder) {
        0
    } else if re(r"(^|/)\.claude/skills/").is_match(folder) {
        1
    } else if re(r"(^|/)\.agents/skills/").is_match(folder) {
        2
    } else {
        3
    }
}
/// localeCompare, near enough for folder paths: case and punctuation fold first, bytes break ties.
fn locale_key(s: &str) -> (String, String) {
    (s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase(), s.to_owned())
}

/// skillsInTree: the skill folders a commit's tree holds, each with its files, bounded and checked.
fn skills_in_tree(tree: &[TreeEntry], repo: &str, under: Option<&str>) -> Vec<Found> {
    let inside = |path: &str, folder: &str| folder.is_empty() || path.starts_with(&format!("{folder}/"));
    let mut folders: Vec<String> = tree
        .iter()
        .filter(|e| e.kind == "blob" && (e.path == SKILL_FILE || e.path.ends_with(&format!("/{SKILL_FILE}"))))
        .map(|e| e.path[..e.path.len().saturating_sub(SKILL_FILE.len() + 1)].to_owned())
        .filter(|f| under.is_none_or(|u| f == u || f.starts_with(&format!("{u}/"))))
        .filter(|f| !f.split('/').any(|p| p == ".git"))
        .collect();
    folders.sort_by_key(|f| locale_key(f));
    // A SKILL.md inside another skill's folder is part of that skill, not one of its own.
    let outermost: Vec<String> = folders.iter().filter(|f| !folders.iter().any(|o| o != *f && inside(f, o) && o.len() < f.len())).cloned().collect();
    let mut copies: Vec<(String, String)> = Vec::new();
    for folder in outermost {
        let identity = re(AGENT_SKILLS).replace(&folder, "${1}skills/").into_owned();
        match copies.iter_mut().find(|(id, _)| *id == identity) {
            Some(slot) if copy_rank(&folder) < copy_rank(&slot.1) => slot.1 = folder,
            Some(_) => {}
            None => copies.push((identity, folder)),
        }
    }
    let mut unique: Vec<String> = copies.into_iter().map(|(_, f)| f).collect();
    unique.sort_by_key(|f| locale_key(f));
    let mut found = Vec::new();
    let mut named: Vec<(String, String)> = Vec::new();
    for folder in unique.into_iter().take(MOST_SKILLS) {
        let name = if folder.is_empty() { repo.to_owned() } else { folder.rsplit('/').next().unwrap_or(&folder).to_owned() };
        // Links and submodules are never carried: a kept skill holds only real files.
        let entries: Vec<TreeEntry> = tree.iter().filter(|e| e.kind == "blob" && e.mode != "120000" && inside(&e.path, &folder)).cloned().collect();
        let files: Vec<SkillFile> = entries
            .iter()
            .map(|e| {
                let path = if folder.is_empty() { e.path.clone() } else { e.path[folder.len() + 1..].to_owned() };
                SkillFile { runs: runs(&path, &e.mode), path, bytes: e.size }
            })
            .collect();
        let bytes: u64 = files.iter().map(|f| f.bytes).sum();
        let bad_path = files.iter().find(|f| f.path.split('/').any(|p| p.is_empty() || p == "." || p == ".." || windows_bad(p)));
        let twin = named.iter().find(|(n, _)| *n == name.to_lowercase()).map(|(_, f)| f.clone());
        let held = if !skill_name_ok(&name) {
            Some(format!("Its folder's name, \"{name}\", is not one Claude Code takes for a skill."))
        } else if let Some(twin) = &twin {
            Some(format!("Another folder here has the same name ({}); only the first is offered.", if twin.is_empty() { "(top)" } else { twin }))
        } else if bytes > MOST_BYTES_A_SKILL {
            Some(format!("Larger than {} ({}): a skill is instructions, not a payload.", megabytes(MOST_BYTES_A_SKILL), megabytes(bytes)))
        } else if files.len() > MOST_FILES_A_SKILL {
            Some(format!("More than {MOST_FILES_A_SKILL} files."))
        } else {
            bad_path.map(|f| format!("It holds a file Windows cannot keep: {}.", f.path))
        };
        if twin.is_none() {
            named.push((name.to_lowercase(), folder.clone()));
        }
        found.push(Found { skill: PreviewSkill { name, folder, bytes, files, description: None, held, kept_at: None }, entries });
    }
    found
}

fn raw_url(owner: &str, repo: &str, sha: &str, path: &str) -> String {
    let encoded: Vec<String> = path.split('/').map(|p| url::form_urlencoded::byte_serialize(p.as_bytes()).collect::<String>().replace('+', "%20")).collect();
    format!("{RAW}/{owner}/{repo}/{sha}/{}", encoded.join("/"))
}

fn header<'a>(answer: &'a HttpResponse, name: &str) -> Option<&'a str> {
    answer.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

const UNREADABLE: &str = "GitHub sent something Locust could not read. Try again in a minute.";
const UNREACHABLE: &str = "Locust could not reach GitHub. Check the connection and try again.";

/// The skills kept from GitHub, in one profile folder.
pub struct SkillLibrary {
    root: PathBuf,
    client: Box<dyn HttpClient>,
    now: Box<dyn Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync>,
    /// One change at a time: two keeps at once must not interleave their swaps.
    changing: Mutex<()>,
}

impl SkillLibrary {
    pub fn new(root: PathBuf, client: Box<dyn HttpClient>) -> Self {
        Self { root, client, now: Box::new(chrono::Utc::now), changing: Mutex::new(()) }
    }
    pub fn with_clock(mut self, now: impl Fn() -> chrono::DateTime<chrono::Utc> + Send + Sync + 'static) -> Self {
        self.now = Box::new(now);
        self
    }
    /// The native profile's library: what `skills::sources_for_store` hands every run.
    pub fn for_store(store: &crate::store::Store, client: Box<dyn HttpClient>) -> Self {
        Self::new(store.root().join("skill-library"), client)
    }
    /// `<root>/skills`: the one folder of kept skills a run is handed.
    pub fn skills_folder(&self) -> PathBuf {
        self.root.join("skills")
    }
    fn record_file(&self) -> PathBuf {
        self.root.join("library.json")
    }

    /// One answer, following a redirect only to GitHub's own two hosts; a 404 says `missing`.
    fn request(&self, first: &str, missing: &str) -> Result<HttpResponse, String> {
        let mut url = first.to_owned();
        let mut answer = None;
        for hop in 0..=MOST_REDIRECTS {
            let accept = if url.starts_with(API) { "application/vnd.github+json" } else { "*/*" };
            let got = self
                .client
                .send(HttpRequest { method: "GET".into(), url: url.clone(), headers: vec![("User-Agent".into(), "Locust".into()), ("Accept".into(), accept.into())], body: None, timeout_secs: TIMEOUT_SECS })
                .map_err(|_| UNREACHABLE.to_owned())?;
            if !(300..400).contains(&got.status) {
                answer = Some(got);
                break;
            }
            let next = header(&got, "location").and_then(|l| url::Url::parse(&url).ok()?.join(l).ok());
            match next {
                Some(n) if n.scheme() == "https" && matches!(n.host_str(), Some("api.github.com" | "raw.githubusercontent.com")) && n.port().is_none() => url = n.to_string(),
                _ => return Err("GitHub sent Locust somewhere other than GitHub. Nothing was read from there.".into()),
            }
            if hop == MOST_REDIRECTS {
                return Err("GitHub kept redirecting. Try again in a minute.".into());
            }
        }
        let answer = answer.ok_or_else(|| UNREACHABLE.to_owned())?;
        if (200..300).contains(&answer.status) {
            return Ok(answer);
        }
        if matches!(answer.status, 403 | 429) && header(&answer, "x-ratelimit-remaining") == Some("0") {
            let until = header(&answer, "x-ratelimit-reset")
                .and_then(|r| r.trim().parse::<i64>().ok())
                .filter(|r| *r > 0)
                .and_then(|r| chrono::DateTime::from_timestamp(r, 0))
                .map_or(" for now".to_owned(), |t| format!(" until {}", t.with_timezone(&chrono::Local).format("%-I:%M %p")));
            return Err(format!("GitHub's limit for looking without signing in is used up{until}. Try again then."));
        }
        if answer.status == 404 {
            return Err(missing.to_owned());
        }
        Err(format!("GitHub answered {} when Locust asked for {}. Try again in a minute.", answer.status, if url.starts_with(RAW) { "a file" } else { "the repository" }))
    }
    fn json(&self, url: &str, missing: &str) -> Result<Value, String> {
        serde_json::from_slice(&self.request(url, missing)?.body).map_err(|_| UNREADABLE.to_owned())
    }
    fn tree_at(&self, owner: &str, repo: &str, sha: &str) -> Result<(Vec<TreeEntry>, bool), String> {
        let body = self.json(&format!("{API}/repos/{owner}/{repo}/git/trees/{sha}?recursive=1"), &format!("GitHub has no commit {} in {owner}/{repo}.", &sha[..7.min(sha.len())]))?;
        let tree = body["tree"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                Some(TreeEntry { path: e["path"].as_str()?.into(), mode: e["mode"].as_str()?.into(), kind: e["type"].as_str()?.into(), sha: e["sha"].as_str()?.into(), size: e["size"].as_u64().unwrap_or(0) })
            })
            .collect();
        Ok((tree, body["truncated"] == true))
    }

    /// What is kept, by repository.
    pub fn list(&self) -> Vec<KeptSource> {
        let Ok(text) = std::fs::read_to_string(self.record_file()) else { return Vec::new() };
        let Ok(record) = serde_json::from_str::<Value>(&text) else { return Vec::new() };
        record["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                Some(KeptSource {
                    source: s["source"].as_str()?.into(),
                    reference: s["ref"].as_str().unwrap_or_default().into(),
                    sha: s["sha"].as_str()?.into(),
                    kept_at: s["keptAt"].as_str().unwrap_or_default().into(),
                    skills: s["skills"]
                        .as_array()?
                        .iter()
                        .filter_map(|k| {
                            Some(KeptSkill {
                                name: k["name"].as_str()?.into(),
                                files: k["files"].as_u64().unwrap_or(0) as usize,
                                bytes: k["bytes"].as_u64().unwrap_or(0),
                                runs: k["runs"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(str::to_owned)).collect(),
                            })
                        })
                        .collect(),
                })
            })
            .collect()
    }
    fn write_record(&self, sources: &[KeptSource]) -> Result<(), String> {
        let record = json!({"version": 1, "sources": sources.iter().map(|s| json!({
            "source": s.source, "ref": s.reference, "sha": s.sha, "keptAt": s.kept_at,
            "skills": s.skills.iter().map(|k| json!({"name": k.name, "files": k.files, "bytes": k.bytes, "runs": k.runs})).collect::<Vec<_>>(),
        })).collect::<Vec<_>>()});
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        let temporary = self.root.join(format!("library.json.{}.tmp", crate::invariants::unique_nanos()));
        std::fs::write(&temporary, format!("{}\n", serde_json::to_string_pretty(&record).unwrap_or_default())).map_err(|e| e.to_string())?;
        std::fs::rename(&temporary, self.record_file()).map_err(|e| e.to_string())
    }

    /// Look: the skills a repository holds, kept nothing. Reads the repository, its commit, its tree, and each
    /// SKILL.md (at most 256 KB) for its description.
    pub fn preview(&self, link: &str) -> Result<Preview, String> {
        let parsed = parse_repository_link(link).ok_or("Paste a GitHub repository: owner/repo, or its github.com link.")?;
        let missing = format!("No public repository at {}/{}. Locust reads public repositories only.", parsed.owner, parsed.repo);
        let about = self.json(&format!("{API}/repos/{}/{}", parsed.owner, parsed.repo), &missing)?;
        if about["private"] == true {
            return Err(missing);
        }
        let source = about["full_name"].as_str().filter(|f| re(r"^[^/]+/[^/]+$").is_match(f)).map_or(format!("{}/{}", parsed.owner, parsed.repo), str::to_owned);
        let (owner, repo) = split_source(&source)?;
        let reference = parsed.reference.clone().or_else(|| about["default_branch"].as_str().map(str::to_owned)).unwrap_or_else(|| "HEAD".into());
        let encoded: String = url::form_urlencoded::byte_serialize(reference.as_bytes()).collect::<String>().replace('+', "%20");
        let commit = self.json(&format!("{API}/repos/{owner}/{repo}/commits/{encoded}"), &format!("{source} has no branch, tag or commit called \"{reference}\"."))?;
        let sha = commit["sha"].as_str().filter(|s| sha_ok(s)).ok_or(UNREADABLE)?.to_owned();
        let (tree, partial) = self.tree_at(&owner, &repo, &sha)?;
        let found = skills_in_tree(&tree, &repo, parsed.path.as_deref());
        if found.is_empty() {
            return Err(match &parsed.path {
                None => format!("{source} has no skills: no folder in it holds a SKILL.md.{}", if partial { " GitHub listed only part of it; link a folder inside it instead." } else { "" }),
                Some(p) => format!("{source} has no skills under {p}: no folder there holds a SKILL.md."),
            });
        }
        let record = self.list();
        let skills = found
            .into_iter()
            .map(|Found { mut skill, entries }| {
                let at = if skill.folder.is_empty() { SKILL_FILE.to_owned() } else { format!("{}/{SKILL_FILE}", skill.folder) };
                skill.description = entries
                    .iter()
                    .find(|e| e.path == at && e.size <= 256 * 1024)
                    .and_then(|e| self.request(&raw_url(&owner, &repo, &sha, &e.path), "").ok())
                    .and_then(|a| String::from_utf8(a.body).ok())
                    .and_then(|t| skill_description(&t));
                let keeper = record.iter().find(|k| k.skills.iter().any(|s| s.name.eq_ignore_ascii_case(&skill.name)));
                let kept_here = keeper.is_some_and(|k| k.source.eq_ignore_ascii_case(&source));
                if skill.held.is_none()
                    && let Some(k) = keeper.filter(|_| !kept_here)
                {
                    skill.held = Some(format!("A skill called {} is kept from {}. Remove that one to keep this.", skill.name, k.source));
                }
                skill.kept_at = keeper.filter(|_| kept_here).map(|k| k.sha.clone());
                skill
            })
            .collect();
        Ok(Preview { source, reference, sha, skills, partial, path: parsed.path })
    }

    /// Keep: the picked skills at the commit that was looked at, every file checked against its blob id before
    /// anything kept changes. Keeping a repository again replaces what was kept from it.
    pub fn install(&self, asked: &InstallRequest) -> Result<Vec<KeptSource>, String> {
        let _one = crate::invariants::lock(&self.changing);
        let (owner, repo) = split_source(&asked.source)?;
        let source = format!("{owner}/{repo}");
        let sha = asked.sha.clone();
        if !sha_ok(&sha) {
            return Err("Look at the repository again, then keep.".into());
        }
        let reference = if asked.reference.len() <= 255 { asked.reference.clone() } else { sha.clone() };
        let mut names: Vec<String> = Vec::new();
        for n in &asked.names {
            if !names.contains(n) {
                names.push(n.clone());
            }
        }
        if names.is_empty() || names.len() > MOST_SKILLS || names.iter().any(|n| !skill_name_ok(n)) {
            return Err("Pick at least one skill to keep.".into());
        }
        // The tree again, at the same commit: a commit never changes, so this is the list the person saw, under the
        // folder the link named.
        let (tree, _) = self.tree_at(&owner, &repo, &sha)?;
        let under = asked.path.as_deref().filter(|p| !p.is_empty() && p.len() <= 1000 && !p.split('/').any(|x| x == ".."));
        let found = skills_in_tree(&tree, &repo, under);
        let record = self.list();
        let others: Vec<KeptSource> = record.iter().filter(|k| !k.source.eq_ignore_ascii_case(&source)).cloned().collect();
        let mut chosen: Vec<&Found> = Vec::new();
        for name in &names {
            let skill = found.iter().find(|f| f.skill.name == *name && f.skill.held.is_none()).ok_or_else(|| format!("{name} is not a skill Locust can keep from that commit."))?;
            if let Some(k) = others.iter().find(|k| k.skills.iter().any(|s| s.name.eq_ignore_ascii_case(name))) {
                return Err(format!("A skill called {name} is kept from {}. Remove that one to keep this.", k.source));
            }
            chosen.push(skill);
        }
        let others_bytes: u64 = others.iter().flat_map(|k| &k.skills).map(|s| s.bytes).sum();
        let bytes: u64 = chosen.iter().map(|f| f.skill.bytes).sum();
        if others_bytes + bytes > MOST_BYTES_ALL {
            return Err(format!("That would keep more than {} of skills in all. Keep fewer, or remove a repository first.", megabytes(MOST_BYTES_ALL)));
        }
        let staging = self.root.join(format!(".staging-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        let result = (|| -> Result<Vec<KeptSource>, String> {
            for f in &chosen {
                for entry in &f.entries {
                    let in_skill = if f.skill.folder.is_empty() { entry.path.clone() } else { entry.path[f.skill.folder.len() + 1..].to_owned() };
                    let answer = self.request(&raw_url(&owner, &repo, &sha, &entry.path), &format!("{} is missing from {source} at {}.", entry.path, &sha[..7]))?;
                    if git_blob_id(&answer.body) != entry.sha {
                        return Err(format!("{}/{in_skill} did not arrive as GitHub lists it. Nothing was kept; try again.", f.skill.name));
                    }
                    let target = in_skill.split('/').fold(staging.join(&f.skill.name), |p, part| p.join(part));
                    std::fs::create_dir_all(target.parent().unwrap_or(&staging)).map_err(|e| e.to_string())?;
                    std::fs::write(&target, &answer.body).map_err(|e| e.to_string())?;
                }
            }
            // Every file arrived and checked: only now does anything kept change.
            let folder = self.skills_folder();
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            if let Some(before) = record.iter().find(|k| k.source.eq_ignore_ascii_case(&source)) {
                for s in before.skills.iter().filter(|s| skill_name_ok(&s.name)) {
                    let _ = std::fs::remove_dir_all(folder.join(&s.name));
                }
            }
            for f in &chosen {
                let _ = std::fs::remove_dir_all(folder.join(&f.skill.name));
                std::fs::rename(staging.join(&f.skill.name), folder.join(&f.skill.name)).map_err(|e| e.to_string())?;
            }
            let kept = KeptSource {
                source: source.clone(),
                reference,
                sha: sha.clone(),
                kept_at: (self.now)().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                skills: chosen
                    .iter()
                    .map(|f| KeptSkill { name: f.skill.name.clone(), files: f.skill.files.len(), bytes: f.skill.bytes, runs: f.skill.files.iter().filter(|x| x.runs).map(|x| x.path.clone()).collect() })
                    .collect(),
            };
            let mut sources = others.clone();
            sources.push(kept);
            sources.sort_by_key(|s| locale_key(&s.source));
            self.write_record(&sources)?;
            Ok(sources)
        })();
        let _ = std::fs::remove_dir_all(&staging);
        result
    }

    /// Remove a repository's kept skills (their folders, never anything else) and its record.
    pub fn remove(&self, source: &str) -> Result<Vec<KeptSource>, String> {
        let _one = crate::invariants::lock(&self.changing);
        let record = self.list();
        let Some(gone) = record.iter().find(|k| k.source.eq_ignore_ascii_case(source)) else { return Ok(record) };
        for s in gone.skills.iter().filter(|s| skill_name_ok(&s.name)) {
            let _ = std::fs::remove_dir_all(self.skills_folder().join(&s.name));
        }
        let sources: Vec<KeptSource> = record.iter().filter(|k| !k.source.eq_ignore_ascii_case(source)).cloned().collect();
        self.write_record(&sources)?;
        Ok(sources)
    }
}

fn split_source(source: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = source.split('/').collect();
    match parts.as_slice() {
        [owner, repo] if owner_ok(owner) && repo_ok(repo) => Ok((owner.to_string(), repo.to_string())),
        _ => Err("That is not a repository Locust can read.".into()),
    }
}

/// skillPreviewPicks: a shelf click prepares every offered skill; a pasted link keeps only what was already kept.
pub fn preview_picks(preview: &Preview, all: bool) -> Vec<String> {
    preview.skills.iter().filter(|s| s.held.is_none() && (all || s.kept_at.is_some())).map(|s| s.name.clone()).collect()
}

/// A pack on the shelf: a dated snapshot, refreshed by hand with a release, never fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pack {
    pub source: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub stars: u64,
    pub license: &'static str,
    /// When a new pack began climbing (the trending row); None on the popular row.
    pub since: Option<&'static str>,
}

/// GitHub repository metadata checked 2026-10-30. Stars are a dated snapshot, never a live request.
pub const POPULAR_SKILLS_AS_OF: &str = "2026-10-30";
pub const POPULAR_SKILLS: [Pack; 5] = [
    Pack { source: "mattpocock/skills", name: "Matt Pocock's skills", description: "Practical workflows for building and improving software.", stars: 307555, license: "MIT", since: None },
    Pack { source: "obra/superpowers", name: "Superpowers", description: "Plan, test and debug with a structured development workflow.", stars: 309493, license: "MIT", since: None },
    Pack { source: "anthropics/skills", name: "Anthropic's skills", description: "Create documents, designs and other working artifacts.", stars: 185396, license: "Apache-2.0 + source-available", since: None },
    Pack { source: "pbakaus/impeccable", name: "Impeccable", description: "Design vocabulary for polished interfaces and useful critiques.", stars: 97191, license: "Apache-2.0", since: None },
    Pack { source: "thedotmack/claude-mem", name: "Claude-Mem", description: "Memory workflows. Needs its own install to work.", stars: 106430, license: "Apache-2.0", since: None },
];
/// New this week (2026-11-01): packs made in the last fortnight that climbed fastest.
pub const TRENDING_SKILLS_AS_OF: &str = "2026-11-01";
pub const TRENDING_SKILLS: [Pack; 4] = [
    Pack { source: "heydamir/unslop", name: "Unslop", description: "Turns an AI-looking interface into a designed one.", stars: 3904, license: "CC-BY-4.0", since: Some("2026-10-29") },
    Pack { source: "t3dotgg/tstack", name: "tstack", description: "The skills Theo uses every day: file, babysit and review pull requests.", stars: 2549, license: "MIT", since: Some("2026-10-24") },
    Pack { source: "patch-gg/pixelfx", name: "pixelfx", description: "244 pixel-art effects to play, recolor and export, and a skill that makes new ones.", stars: 1797, license: "MIT", since: Some("2026-10-30") },
    Pack { source: "enesozturk/dimensional-drawing", name: "Dimensional Drawing", description: "Draws your interface parts as Apple-style spec sheets, still or moving.", stars: 1692, license: "MIT", since: Some("2026-10-23") },
];

/// filesThatRun: "1 file that runs", "3 files that run".
pub fn files_that_run(n: usize) -> String {
    if n == 1 { "1 file that runs".into() } else { format!("{n} files that run") }
}
/// runsText: every file that runs, named before keeping, however many; None when none do.
pub fn runs_text(files: &[&str]) -> Option<String> {
    (!files.is_empty()).then(|| format!("Runs: {}", files.join(", ")))
}
/// sinceText: "2026-10-24" reads "Oct 24".
pub fn since_text(date: &str) -> String {
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").map_or(date.to_owned(), |d| d.format("%b %-d").to_string())
}
/// 307555 reads "307,555", as `toLocaleString('en-US')` writes it.
pub fn stars_text(stars: u64) -> String {
    let digits = stars.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
#[path = "skill_library_tests.rs"]
mod tests;
