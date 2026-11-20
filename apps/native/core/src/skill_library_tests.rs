//! main/skills-kept-from-github.test.ts and main/popular-skill-packs-fit-and-agent-copies-are-chosen.test.ts, case for
//! case. A fake GitHub answers every request, the way its API and raw host do: nothing reaches the network, and every
//! folder is a temp one.
use super::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[derive(Clone)]
struct File {
    text: Vec<u8>,
    mode: &'static str,
}
fn f(text: &str) -> File {
    File { text: text.as_bytes().to_vec(), mode: "100644" }
}
fn fm(text: &str, mode: &'static str) -> File {
    File { text: text.as_bytes().to_vec(), mode }
}
type Commit = Vec<(String, File)>;

#[derive(Clone, Default)]
struct Repo {
    branch: Option<String>,
    private: bool,
    /// In order: the last is the branch's head.
    commits: Vec<(String, Commit)>,
}

#[derive(Default)]
struct Fake {
    repos: Mutex<Vec<(String, Repo)>>,
    asked: Mutex<Vec<String>>,
    tamper: Option<String>,
    limited: bool,
    unreachable: bool,
    /// url -> location: answered 301 before anything else.
    redirects: Vec<(String, String)>,
    /// A raw file answers 302 to here.
    raw_elsewhere: Option<String>,
}

fn answer(status: u16, body: impl Into<Vec<u8>>, headers: &[(&str, &str)]) -> HttpResponse {
    HttpResponse { status, headers: headers.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), body: body.into() }
}

impl HttpClient for Arc<Fake> {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
        let url = req.url.clone();
        self.asked.lock().unwrap().push(url.clone());
        if self.unreachable {
            return Err("fetch failed".into());
        }
        if self.limited {
            return Ok(answer(403, r#"{"message":"rate limit"}"#, &[("x-ratelimit-remaining", "0"), ("x-ratelimit-reset", "1791500000")]));
        }
        if let Some((_, to)) = self.redirects.iter().find(|(from, _)| *from == url) {
            return Ok(answer(301, "", &[("location", to)]));
        }
        let repos = self.repos.lock().unwrap().clone();
        let find = |key: &str| repos.iter().find(|(n, _)| n.eq_ignore_ascii_case(key)).cloned();
        let bare = url.split('?').next().unwrap();
        if let Some(c) = re(r"^https://api\.github\.com/repos/([^/]+)/([^/]+)(/.*)?$").captures(bare) {
            let Some((full, repo)) = find(&format!("{}/{}", &c[1], &c[2])).filter(|(_, r)| !r.private) else {
                return Ok(answer(404, r#"{"message":"Not Found"}"#, &[]));
            };
            let branch = repo.branch.clone().unwrap_or_else(|| "main".into());
            let rest = c.get(3).map_or("", |m| m.as_str());
            if rest.is_empty() {
                return Ok(answer(200, json!({"full_name": full, "default_branch": branch, "private": false}).to_string(), &[]));
            }
            if let Some(r) = rest.strip_prefix("/commits/") {
                let r = decode(r);
                if r == branch {
                    return Ok(answer(200, json!({"sha": repo.commits.last().unwrap().0}).to_string(), &[]));
                }
                if repo.commits.iter().any(|(s, _)| *s == r) {
                    return Ok(answer(200, json!({"sha": r}).to_string(), &[]));
                }
                return Ok(answer(422, r#"{"message":"No commit found"}"#, &[]));
            }
            if let Some(sha) = rest.strip_prefix("/git/trees/") {
                let Some((_, files)) = repo.commits.iter().find(|(s, _)| s == sha) else { return Ok(answer(404, "{}", &[])) };
                let tree: Vec<Value> = files.iter().map(|(p, file)| json!({"path": p, "mode": file.mode, "type": "blob", "sha": git_blob_id(&file.text), "size": file.text.len()})).collect();
                return Ok(answer(200, json!({"sha": sha, "truncated": false, "tree": tree}).to_string(), &[]));
            }
            return Ok(answer(404, "{}", &[]));
        }
        if let Some(c) = re(r"^https://raw\.githubusercontent\.com/([^/]+)/([^/]+)/([0-9a-f]{40})/(.+)$").captures(&url) {
            if let Some(to) = &self.raw_elsewhere {
                return Ok(answer(302, "", &[("location", to)]));
            }
            let path: Vec<String> = c[4].split('/').map(decode).collect();
            let path = path.join("/");
            let file = find(&format!("{}/{}", &c[1], &c[2])).and_then(|(_, r)| r.commits.iter().find(|(s, _)| *s == c[3]).cloned()).and_then(|(_, files)| files.into_iter().find(|(p, _)| *p == path));
            return Ok(match file {
                None => answer(404, "404: Not Found", &[]),
                Some((p, file)) if Some(&p) == self.tamper.as_ref() => answer(200, [file.text, b" and a little more".to_vec()].concat(), &[]),
                Some((_, file)) => answer(200, file.text, &[]),
            });
        }
        Ok(answer(404, "nowhere", &[]))
    }
}

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scratch() -> Temp {
    let p = std::env::temp_dir().join(format!("locust-skill-library-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&p).unwrap();
    Temp(p)
}
fn fake(repos: Vec<(&str, Repo)>) -> Arc<Fake> {
    Arc::new(Fake { repos: Mutex::new(repos.into_iter().map(|(n, r)| (n.to_owned(), r)).collect()), ..Default::default() })
}
fn library(root: &Path, github: &Arc<Fake>) -> SkillLibrary {
    SkillLibrary::new(root.to_path_buf(), Box::new(github.clone()))
}
fn repo(commits: Vec<(&str, Commit)>) -> Repo {
    Repo { commits: commits.into_iter().map(|(s, c)| (s.to_owned(), c)).collect(), ..Default::default() }
}
fn names(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default();
    v.sort();
    v
}
fn skill<'a>(preview: &'a Preview, name: &str) -> &'a PreviewSkill {
    preview.skills.iter().find(|s| s.name == name).unwrap_or_else(|| panic!("no {name}"))
}

const PDF_SKILL: &str = "---\nname: pdf\ndescription: \"Fill and read PDF forms.\"\n---\n\nUse scripts/fill.py.\n";

fn repo_at_a() -> Commit {
    vec![
        ("README.md".into(), f("Skills.")),
        ("skills/pdf/SKILL.md".into(), f(PDF_SKILL)),
        ("skills/pdf/scripts/fill.py".into(), f("print(\"filled\")\n")),
        ("skills/pdf/tools/convert".into(), fm("#!/bin/sh\necho hi\n", "100755")),
        ("skills/pdf/reference.md".into(), f("Forms.")),
        ("skills/pdf/link".into(), fm("../../README.md", "120000")),
        ("skills/brand/SKILL.md".into(), f("---\ndescription: The house style.\n---\nBe brief.\n")),
        ("skills/bad name/SKILL.md".into(), f("x")),
        ("other/aux/SKILL.md".into(), f("x")),
        ("tools/build.sh".into(), f("echo build")),
    ]
}
fn repo_at_b() -> Commit {
    let mut c = repo_at_a();
    for (p, file) in c.iter_mut() {
        if p == "skills/brand/SKILL.md" {
            *file = f("---\ndescription: The house style, revised.\n---\nBe briefer.\n");
        }
        if p == "skills/pdf/scripts/fill.py" {
            *file = f("print(\"filled twice\")\n");
        }
    }
    c
}

#[test]
fn git_blob_ids_are_gits_own() {
    // `printf 'hello\n' | git hash-object --stdin` and the empty file's id.
    assert_eq!(git_blob_id(b"hello\n"), "ce013625030ba8dba906f756967f9e9ca394464a");
    assert_eq!(git_blob_id(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
}

// --- a GitHub link ---

#[test]
fn reads_owner_repo_a_github_com_link_and_a_link_to_a_folder_on_a_branch() {
    let link = |o: &str, r: &str, reference: Option<&str>, path: Option<&str>| Some(RepositoryLink { owner: o.into(), repo: r.into(), reference: reference.map(Into::into), path: path.map(Into::into) });
    assert_eq!(parse_repository_link("anthropics/skills"), link("anthropics", "skills", None, None));
    assert_eq!(parse_repository_link(" https://github.com/anthropics/skills.git/ "), link("anthropics", "skills", None, None));
    assert_eq!(parse_repository_link("github.com/acme/kit/tree/v2/skills/pdf"), link("acme", "kit", Some("v2"), Some("skills/pdf")));
    assert_eq!(parse_repository_link("https://github.com/acme/kit?tab=readme"), link("acme", "kit", None, None));
}

#[test]
fn refuses_what_is_not_one() {
    for text in ["", "acme", "https://gitlab.com/acme/kit", "file:///C:/kit", "acme/kit/blob/main/x", "acme/kit/tree/main/../x", "acme/.."] {
        assert_eq!(parse_repository_link(text), None, "{text}");
    }
}

#[test]
fn reads_a_skill_mds_description_quoted_or_not() {
    assert_eq!(skill_description(PDF_SKILL).as_deref(), Some("Fill and read PDF forms."));
    assert_eq!(skill_description("---\r\ndescription: plain\r\n---\r\n").as_deref(), Some("plain"));
    assert_eq!(skill_description("no front matter"), None);
}

// --- skills from a GitHub repository ---

#[test]
fn lists_each_skill_with_its_files_the_ones_that_run_marked_and_keeps_nothing_yet() {
    let t = scratch();
    let github = fake(vec![("Acme/kit", repo(vec![(SHA_A, repo_at_a())]))]);
    let lib = library(&t.0, &github);
    let preview = lib.preview("acme/kit").unwrap();
    assert_eq!((preview.source.as_str(), preview.reference.as_str(), preview.sha.as_str(), preview.partial), ("Acme/kit", "main", SHA_A, false));
    let pdf = skill(&preview, "pdf");
    assert_eq!(pdf.description.as_deref(), Some("Fill and read PDF forms."));
    assert_eq!(pdf.held, None);
    // The link is not a file: never carried.
    let mut files: Vec<&str> = pdf.files.iter().map(|f| f.path.as_str()).collect();
    files.sort();
    assert_eq!(files, ["SKILL.md", "reference.md", "scripts/fill.py", "tools/convert"]);
    let mut running: Vec<&str> = pdf.files.iter().filter(|f| f.runs).map(|f| f.path.as_str()).collect();
    running.sort();
    assert_eq!(running, ["scripts/fill.py", "tools/convert"]);
    assert_eq!(skill(&preview, "brand").description.as_deref(), Some("The house style."));
    assert!(skill(&preview, "bad name").held.as_deref().is_some_and(|h| h.contains("not one Claude Code takes")));
    assert_eq!(skill(&preview, "aux").held, None);
    // Only reads: nothing under the library yet, and only GitHub's two hosts asked.
    assert!(lib.list().is_empty());
    assert!(github.asked.lock().unwrap().iter().all(|u| u.starts_with("https://api.github.com/") || u.starts_with("https://raw.githubusercontent.com/")));
}

#[test]
fn keeps_the_picked_skills_at_the_commit_that_was_looked_at_byte_for_byte() {
    let t = scratch();
    let github = fake(vec![("acme/kit", repo(vec![(SHA_A, repo_at_a())]))]);
    let lib = library(&t.0, &github).with_clock(|| "2026-10-09T02:00:00Z".parse().unwrap());
    let preview = lib.preview("https://github.com/acme/kit").unwrap();
    // The branch moves on after the look: keeping still installs what was seen.
    github.repos.lock().unwrap()[0].1.commits.push((SHA_B.into(), repo_at_b()));
    let sources = lib.install(&preview.keep(&["pdf"])).unwrap();
    assert_eq!(sources.len(), 1);
    let kept = &sources[0];
    assert_eq!((kept.source.as_str(), kept.reference.as_str(), kept.sha.as_str(), kept.kept_at.as_str()), ("acme/kit", "main", SHA_A, "2026-10-09T02:00:00.000Z"));
    assert_eq!(kept.skills.len(), 1);
    assert_eq!((kept.skills[0].name.as_str(), kept.skills[0].files), ("pdf", 4));
    assert_eq!(kept.skills[0].runs, ["scripts/fill.py", "tools/convert"]);
    assert_eq!(std::fs::read_to_string(lib.skills_folder().join("pdf/scripts/fill.py")).unwrap(), "print(\"filled\")\n");
    assert_eq!(std::fs::read_to_string(lib.skills_folder().join("pdf/SKILL.md")).unwrap(), PDF_SKILL);
    assert_eq!(names(&lib.skills_folder()), ["pdf"]);
    assert!(!github.asked.lock().unwrap().iter().any(|u| u.contains(SHA_B)));
    // The record is Electron's shape, so either app reads it.
    let record: Value = serde_json::from_str(&std::fs::read_to_string(t.0.join("library.json")).unwrap()).unwrap();
    assert_eq!(record["version"], 1);
    assert_eq!(record["sources"][0]["keptAt"], "2026-10-09T02:00:00.000Z");
}

#[test]
fn keeps_the_folder_the_link_named_not_a_same_named_one_elsewhere_nor_nothing_for_a_root_skill() {
    let t = scratch();
    let twins: Commit = vec![
        ("SKILL.md".into(), f("---\ndescription: The root one.\n---\nRoot.\n")),
        ("a/foo/SKILL.md".into(), f("---\ndescription: The A foo.\n---\nThe A foo.\n")),
        ("b/foo/SKILL.md".into(), f("---\ndescription: The B foo.\n---\nThe B foo.\n")),
    ];
    let github = fake(vec![("acme/kit", repo(vec![(SHA_A, twins)]))]);
    let lib = library(&t.0, &github);
    let preview = lib.preview("https://github.com/acme/kit/tree/main/b").unwrap();
    assert_eq!(preview.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["foo"]);
    lib.install(&preview.keep(&["foo"])).unwrap();
    assert!(std::fs::read_to_string(lib.skills_folder().join("foo/SKILL.md")).unwrap().contains("The B foo."));
}

#[test]
fn looking_again_shows_what_is_kept_and_keeping_again_replaces_it_with_the_new_commit() {
    let t = scratch();
    let github = fake(vec![("acme/kit", repo(vec![(SHA_A, repo_at_a())]))]);
    let lib = library(&t.0, &github);
    let first = lib.preview("acme/kit").unwrap();
    lib.install(&first.keep(&["pdf", "brand"])).unwrap();
    github.repos.lock().unwrap()[0].1.commits.push((SHA_B.into(), repo_at_b()));
    let lib2 = library(&t.0, &github);
    let again = lib2.preview("acme/kit").unwrap();
    assert_eq!(again.sha, SHA_B);
    assert_eq!(skill(&again, "pdf").kept_at.as_deref(), Some(SHA_A));
    assert_eq!(skill(&again, "brand").description.as_deref(), Some("The house style, revised."));
    // Kept again with brand left unticked: brand goes, pdf moves to the new commit.
    let sources = lib2.install(&again.keep(&["pdf"])).unwrap();
    assert_eq!(sources.iter().map(|s| (s.source.as_str(), s.sha.as_str(), s.skills.iter().map(|k| k.name.as_str()).collect::<Vec<_>>())).collect::<Vec<_>>(), [("acme/kit", SHA_B, vec!["pdf"])]);
    assert_eq!(std::fs::read_to_string(lib2.skills_folder().join("pdf/scripts/fill.py")).unwrap(), "print(\"filled twice\")\n");
    assert_eq!(names(&lib2.skills_folder()), ["pdf"]);
}

#[test]
fn a_file_that_does_not_arrive_as_github_lists_it_keeps_nothing_at_all() {
    let t = scratch();
    let looked = library(&t.0, &fake(vec![("acme/kit", repo(vec![(SHA_A, repo_at_a())]))])).preview("acme/kit").unwrap();
    let tampered = Arc::new(Fake { repos: Mutex::new(vec![("acme/kit".into(), repo(vec![(SHA_A, repo_at_a())]))]), tamper: Some("skills/pdf/scripts/fill.py".into()), ..Default::default() });
    let lib = library(&t.0, &tampered);
    let refused = lib.install(&looked.keep(&["brand", "pdf"])).unwrap_err();
    assert!(re(r"pdf/scripts/fill\.py did not arrive as GitHub lists it\. Nothing was kept").is_match(&refused), "{refused}");
    assert!(lib.list().is_empty());
    assert!(!names(&t.0).contains(&"skills".to_owned()), "{:?}", names(&t.0));
    assert!(names(&t.0).iter().all(|n| !n.starts_with(".staging-")));
}

#[test]
fn and_leaves_no_staging_folder_when_the_other_files_are_still_arriving_as_it_refuses() {
    // Electron's race (0.711): a lane still downloading wrote its folder back after cleanup. Native fetches one file at
    // a time, so when the refusal returns nothing is still writing: the staging folder is already gone, and stays gone.
    let t = scratch();
    let looked = library(&t.0, &fake(vec![("acme/kit", repo(vec![(SHA_A, repo_at_a())]))])).preview("acme/kit").unwrap();
    let tampered = Arc::new(Fake { repos: Mutex::new(vec![("acme/kit".into(), repo(vec![(SHA_A, repo_at_a())]))]), tamper: Some("skills/pdf/scripts/fill.py".into()), ..Default::default() });
    let lib = library(&t.0, &tampered);
    assert!(lib.install(&looked.keep(&["brand", "pdf"])).unwrap_err().contains("did not arrive as GitHub lists it"));
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(names(&t.0).iter().all(|n| !n.starts_with(".staging-")), "{:?}", names(&t.0));
}

#[test]
fn a_name_kept_from_one_repository_is_held_in_another_and_refused_if_asked_anyway() {
    let t = scratch();
    let github = fake(vec![
        ("acme/kit", repo(vec![(SHA_A, repo_at_a())])),
        ("other/tools", repo(vec![(SHA_B, vec![("pdf/SKILL.md".into(), f("---\ndescription: Another pdf.\n---\n"))])])),
    ]);
    let lib = library(&t.0, &github);
    let kit = lib.preview("acme/kit").unwrap();
    lib.install(&kit.keep(&["pdf"])).unwrap();
    let tools = lib.preview("other/tools").unwrap();
    assert_eq!(tools.skills[0].held.as_deref(), Some("A skill called pdf is kept from acme/kit. Remove that one to keep this."));
    assert!(lib.install(&tools.keep(&["pdf"])).unwrap_err().contains("kept from acme/kit"));
    // Removed, the name is free.
    assert!(lib.remove("acme/kit").unwrap().is_empty());
    assert!(names(&lib.skills_folder()).is_empty());
    let kept = lib.install(&tools.keep(&["pdf"])).unwrap();
    assert_eq!(kept.iter().map(|s| s.source.as_str()).collect::<Vec<_>>(), ["other/tools"]);
}

#[test]
fn says_plainly_what_github_would_not_give() {
    let t = scratch();
    let repos = || {
        vec![
            ("acme/secret", Repo { private: true, ..repo(vec![(SHA_A, repo_at_a())]) }),
            ("acme/empty", repo(vec![(SHA_A, vec![("README.md".into(), f("none"))])])),
        ]
    };
    let lib = library(&t.0, &fake(repos()));
    assert_eq!(lib.preview("acme/secret").unwrap_err(), "No public repository at acme/secret. Locust reads public repositories only.");
    assert!(lib.preview("acme/nothing").unwrap_err().starts_with("No public repository at acme/nothing"));
    assert_eq!(lib.preview("acme/empty").unwrap_err(), "acme/empty has no skills: no folder in it holds a SKILL.md.");
    assert!(lib.preview("not a link").unwrap_err().contains("Paste a GitHub repository"));
    let limited = Arc::new(Fake { limited: true, ..Default::default() });
    let said = library(&t.0, &limited).preview("acme/empty").unwrap_err();
    assert!(re(r"^GitHub's limit for looking without signing in is used up until .+\. Try again then\.$").is_match(&said), "{said}");
    let offline = Arc::new(Fake { unreachable: true, ..Default::default() });
    assert_eq!(library(&t.0, &offline).preview("acme/empty").unwrap_err(), "Locust could not reach GitHub. Check the connection and try again.");
}

#[test]
fn follows_a_redirect_only_to_githubs_own_hosts() {
    let t = scratch();
    // A renamed repository: the old name answers 301 to the new one, on api.github.com.
    let renamed = Arc::new(Fake {
        repos: Mutex::new(vec![("acme/kit".into(), repo(vec![(SHA_A, repo_at_a())]))]),
        redirects: vec![("https://api.github.com/repos/acme/old-kit".into(), "https://api.github.com/repos/acme/kit".into())],
        ..Default::default()
    });
    assert_eq!(library(&t.0, &renamed).preview("acme/old-kit").unwrap().source, "acme/kit");
    let elsewhere = Arc::new(Fake {
        repos: Mutex::new(vec![("acme/kit".into(), repo(vec![(SHA_A, repo_at_a())]))]),
        raw_elsewhere: Some("https://files.example.com/fill.py".into()),
        ..Default::default()
    });
    let lib = library(&t.0, &elsewhere);
    let request = InstallRequest { source: "acme/kit".into(), reference: "main".into(), sha: SHA_A.into(), names: vec!["brand".into()], path: None };
    assert_eq!(lib.install(&request).unwrap_err(), "GitHub sent Locust somewhere other than GitHub. Nothing was read from there.");
    assert!(!elsewhere.asked.lock().unwrap().iter().any(|u| u.contains("example.com")));
    assert!(lib.list().is_empty());
}

#[test]
fn refuses_a_keep_it_was_not_shown_a_name_not_in_that_commit_or_no_commit() {
    let t = scratch();
    let lib = library(&t.0, &fake(vec![("acme/kit", repo(vec![(SHA_A, repo_at_a())]))]));
    let ask = |source: &str, sha: &str, name: &str| lib.install(&InstallRequest { source: source.into(), reference: "main".into(), sha: sha.into(), names: vec![name.into()], path: None }).unwrap_err();
    assert!(ask("acme/kit", SHA_A, "nope").contains("nope is not a skill"));
    assert!(ask("acme/kit", SHA_A, "bad name").contains("Pick at least one"));
    assert!(ask("acme/kit", "main", "pdf").contains("Look at the repository again"));
    assert!(ask("../kit", SHA_A, "pdf").contains("not a repository"));
    assert!(lib.list().is_empty());
}

#[test]
fn outside_auto_beside_the_folders_own_in_auto_the_kept_ones_alone() {
    let t = scratch();
    let lib = library(&t.0.join("library"), &fake(vec![("acme/kit", repo(vec![(SHA_A, repo_at_a())]))]));
    let looked = lib.preview("acme/kit").unwrap();
    lib.install(&looked.keep(&["pdf", "brand"])).unwrap();
    let workspace = t.0.join("work");
    std::fs::create_dir_all(workspace.join(".claude/skills/review")).unwrap();
    std::fs::write(workspace.join(".claude/skills/review/SKILL.md"), "Review it.").unwrap();
    let sources = crate::skills::Sources { library: Some(lib.skills_folder()), ..Default::default() };
    let ask = crate::skills::prepare_claude(&t.0.join("copies"), &workspace, false, None, false, &sources);
    assert_eq!(
        ask.iter().map(|p| (p.name, p.skills.clone())).collect::<Vec<_>>(),
        [("project", vec!["project:review".to_owned()]), ("library", vec!["library:brand".to_owned(), "library:pdf".to_owned()])]
    );
    let plugin = ask.iter().find(|p| p.name == "library").unwrap();
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(plugin.dir.join(".claude-plugin/plugin.json")).unwrap()).unwrap();
    assert_eq!(manifest["name"], "library");
    assert_eq!(std::fs::read_to_string(plugin.dir.join("skills/pdf/scripts/fill.py")).unwrap(), "print(\"filled\")\n");
    crate::skills::remove_copies(&crate::skills::copies_of(&ask, None));
    let auto = crate::skills::prepare_claude(&t.0.join("copies"), &workspace, true, None, true, &sources);
    assert_eq!(auto.iter().map(|p| p.name).collect::<Vec<_>>(), ["library"]);
    crate::skills::remove_copies(&crate::skills::copies_of(&auto, None));
    // The run's copy goes; the kept skills stay.
    assert_eq!(names(&lib.skills_folder()), ["brand", "pdf"]);
}

// --- popular-skill-packs-fit-and-agent-copies-are-chosen.test.ts ---

fn pack(files: Vec<(String, File)>) -> Arc<Fake> {
    fake(vec![("acme/pack", repo(vec![(SHA_A, files)]))])
}

#[test]
fn a_font_bearing_skill_above_five_mb_is_kept_and_handed_to_claude_code_but_ten_mb_remains_the_ceiling() {
    let t = scratch();
    let font = "f".repeat(6 * 1024 * 1024);
    let over = "x".repeat(10 * 1024 * 1024 + 1);
    let lib = library(&t.0.join("library"), &pack(vec![("skills/canvas-design/SKILL.md".into(), f("Draw.")), ("skills/canvas-design/font.ttf".into(), f(&font)), ("skills/oversize/SKILL.md".into(), f(&over))]));
    let preview = lib.preview("acme/pack").unwrap();
    assert_eq!(skill(&preview, "canvas-design").held, None);
    assert!(skill(&preview, "oversize").held.as_deref().is_some_and(|h| h.starts_with("Larger than 10 MB")));
    assert!(lib.install(&preview.keep(&["oversize"])).unwrap_err().contains("not a skill"));
    lib.install(&preview.keep(&["canvas-design"])).unwrap();
    let sources = crate::skills::Sources { library: Some(lib.skills_folder()), ..Default::default() };
    let run = crate::skills::prepare_claude(&t.0.join("runs"), &t.0.join("workspace"), false, None, true, &sources);
    assert_eq!(run[0].skills, ["library:canvas-design"]);
    assert_eq!(std::fs::metadata(run[0].dir.join("skills/canvas-design/font.ttf")).unwrap().len(), 6 * 1024 * 1024);
    crate::skills::remove_copies(&crate::skills::copies_of(&run, None));
}

#[test]
fn agent_copies_choose_skills_then_claude_then_agents_then_any_without_hiding_real_name_clashes() {
    let t = scratch();
    let mut files: HashMap<String, String> = HashMap::new();
    for name in ["root-first", "claude-next", "agents-next", "any-agent"] {
        for agent in [".agent", ".agents", ".claude", ".codex", ".cursor", ".opencode", ".github", ".gemini"] {
            files.insert(format!("{agent}/skills/{name}/SKILL.md"), agent.into());
        }
    }
    files.insert("bundle/.agent/skills/nested/SKILL.md".into(), "agent".into());
    files.insert("bundle/.claude/skills/nested/SKILL.md".into(), "claude".into());
    files.insert("skills/root-first/SKILL.md".into(), "root".into());
    for gone in [".claude/skills/agents-next/SKILL.md", ".claude/skills/any-agent/SKILL.md", ".agents/skills/any-agent/SKILL.md"] {
        files.remove(gone);
    }
    files.insert("different/claude-next/SKILL.md".into(), "a different skill with a real clash".into());
    files.insert("tests/.claude/skills/root-first/SKILL.md".into(), "a fixture, not the root copy".into());
    let lib = library(&t.0, &pack(files.into_iter().map(|(p, text)| (p, f(&text))).collect()));
    let preview = lib.preview("acme/pack").unwrap();
    let mut offered: Vec<&str> = preview.skills.iter().filter(|s| s.held.is_none()).map(|s| s.folder.as_str()).collect();
    offered.sort();
    assert_eq!(offered, [".agent/skills/any-agent", ".agents/skills/agents-next", ".claude/skills/claude-next", "bundle/.claude/skills/nested", "skills/root-first"]);
    let mut held: Vec<&str> = preview.skills.iter().filter(|s| s.held.is_some()).map(|s| s.folder.as_str()).collect();
    held.sort();
    assert_eq!(held, ["different/claude-next", "tests/.claude/skills/root-first"]);
    let picks: Vec<&str> = preview.skills.iter().filter(|s| s.held.is_none()).map(|s| s.name.as_str()).collect();
    lib.install(&preview.keep(&picks)).unwrap();
    assert_eq!(std::fs::read_to_string(lib.skills_folder().join("claude-next/SKILL.md")).unwrap(), ".claude");
    assert_eq!(std::fs::read_to_string(lib.skills_folder().join("nested/SKILL.md")).unwrap(), "claude");
    assert_eq!(lib.list()[0].skills.len(), 5);
}

#[test]
fn a_popular_pack_is_only_previewed_on_the_first_click_and_all_offered_skills_are_ready_for_the_second() {
    let t = scratch();
    let github = pack(vec![("skills/review/SKILL.md".into(), f("Review.")), ("skills/review/scripts/check.py".into(), f("print(1)")), ("skills/bad name/SKILL.md".into(), f("held"))]);
    let lib = library(&t.0, &github);
    let preview = lib.preview("acme/pack").unwrap();
    assert!(lib.list().is_empty());
    assert!(github.asked.lock().unwrap().iter().filter(|u| u.starts_with("https://raw.")).all(|u| u.ends_with("/SKILL.md")));
    assert_eq!(skill(&preview, "review").files.iter().filter(|f| f.runs).map(|f| f.path.as_str()).collect::<Vec<_>>(), ["scripts/check.py"]);
    assert_eq!(preview_picks(&preview, true), ["review"]);
    assert!(preview_picks(&preview, false).is_empty());
    let picks = preview_picks(&preview, true);
    lib.install(&preview.keep(&picks.iter().map(String::as_str).collect::<Vec<_>>())).unwrap();
    assert_eq!(lib.list()[0].skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["review"]);
}

#[test]
fn the_five_popular_packs_have_dated_stars_and_licenses_and_claude_mem_names_its_separate_install() {
    assert_eq!(POPULAR_SKILLS_AS_OF, "2026-10-30");
    assert_eq!(POPULAR_SKILLS.map(|p| p.source), ["mattpocock/skills", "obra/superpowers", "anthropics/skills", "pbakaus/impeccable", "thedotmack/claude-mem"]);
    for p in POPULAR_SKILLS {
        assert!(p.stars > 0 && !p.license.is_empty());
        assert!(parse_repository_link(&format!("https://github.com/{}", p.source)).is_some(), "{} opens on GitHub", p.source);
    }
    assert_eq!(POPULAR_SKILLS[2].license, "Apache-2.0 + source-available");
    assert!(POPULAR_SKILLS[4].description.contains("Needs its own install to work."));
}

#[test]
fn the_new_this_week_packs_are_dated_licensed_younger_than_the_snapshot_and_can_be_opened() {
    assert_eq!(TRENDING_SKILLS_AS_OF, "2026-11-01");
    assert_eq!(TRENDING_SKILLS.map(|p| p.source), ["heydamir/unslop", "t3dotgg/tstack", "patch-gg/pixelfx", "enesozturk/dimensional-drawing"]);
    let snapshot = chrono::NaiveDate::parse_from_str(TRENDING_SKILLS_AS_OF, "%Y-%m-%d").unwrap();
    for p in TRENDING_SKILLS {
        assert!(!POPULAR_SKILLS.iter().any(|q| q.source == p.source));
        assert!(p.stars > 0 && !p.license.is_empty());
        let since = chrono::NaiveDate::parse_from_str(p.since.unwrap(), "%Y-%m-%d").unwrap();
        assert!(since <= snapshot && (snapshot - since).num_days() <= 14, "{}", p.source);
        assert!(parse_repository_link(&format!("https://github.com/{}", p.source)).is_some());
    }
}

/// Not one of Electron's cases: its fake GitHub never answers a private repository, so the look's own check that
/// GitHub did not call it private is held here.
#[test]
fn a_repository_github_calls_private_is_refused_even_when_it_answers() {
    struct Private;
    impl HttpClient for Private {
        fn send(&self, _: HttpRequest) -> Result<HttpResponse, String> {
            Ok(HttpResponse { status: 200, headers: vec![], body: br#"{"full_name":"acme/secret","default_branch":"main","private":true}"#.to_vec() })
        }
    }
    let t = scratch();
    let lib = SkillLibrary::new(t.0.clone(), Box::new(Private));
    assert_eq!(lib.preview("acme/secret").unwrap_err(), "No public repository at acme/secret. Locust reads public repositories only.");
}
