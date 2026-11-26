//! Settings › Teammates › "Skills from GitHub" (Electron `SkillsFromGitHub.tsx`, 0.710 to 0.849): the popular shelf
//! and the new-this-week row, a Look that lists every skill in a pack with the files that run before anything is
//! kept, Keep as its own decision, and the kept packs with Look for changes and Remove. Every word about a pack and
//! every request is `locust_core::skill_library`; this file draws it and runs the requests off the window's thread.
//!
//! GitHub is asked through `chat_http::PlainHttp`, which hands every redirect back, so the library's rule (only
//! GitHub's own two hosts) decides each hop on the real network as it does against the tests' fake.

use super::*;
use locust_core::chat_http::HttpClient;
use locust_core::skill_library::{
    KeptSource, POPULAR_SKILLS, POPULAR_SKILLS_AS_OF, Pack, Preview, PreviewSkill, SkillLibrary, TRENDING_SKILLS, TRENDING_SKILLS_AS_OF, files_that_run, preview_picks, runs_text,
    since_text, stars_text,
};
use std::sync::{Arc, Mutex};

pub(super) const HEADING: &str = "Skills from GitHub";
pub(super) const LOOKING: &str = "Looking…";

#[derive(Default)]
pub(super) struct Shelf {
    pub kept: Vec<KeptSource>,
    pub preview: Option<Preview>,
    pub picked: Vec<String>,
    /// "Looking…" or "keeping" while a request is out.
    pub busy: Option<&'static str>,
    pub said: Option<(String, bool)>,
    /// The kept pack whose Remove was pressed once and now asks to be sure.
    pub arming: Option<String>,
    /// Pages a tile opened, kept instead of opened in tests.
    pub opened: Vec<String>,
    /// The box under the shelf: a repository of the person's own (`owner/repo` or its github.com link).
    pub link: TextEdit,
}

pub(super) enum Answer {
    Looked(Result<Preview, String>, bool),
    Kept(Result<Vec<KeptSource>, String>, usize, String),
}

static CLIENT: Mutex<Option<Arc<dyn HttpClient>>> = Mutex::new(None);

/// Tests answer for GitHub themselves; without a fake of their own, nothing is asked.
#[cfg(test)]
pub(super) fn use_client(client: Arc<dyn HttpClient>) {
    *CLIENT.lock().unwrap_or_else(|e| e.into_inner()) = Some(client);
}

fn client() -> Option<Arc<dyn HttpClient>> {
    if let Some(c) = CLIENT.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Some(c);
    }
    if cfg!(test) {
        return None;
    }
    Some(real_client())
}

/// The client the shelf asks GitHub with, on every platform: one that never follows a redirect itself.
pub(super) fn real_client() -> Arc<dyn HttpClient> {
    Arc::new(locust_core::chat_http::PlainHttp::new())
}

struct Shared(Arc<dyn HttpClient>);
impl HttpClient for Shared {
    fn send(&self, req: locust_core::chat_http::HttpRequest) -> Result<locust_core::chat_http::HttpResponse, String> {
        self.0.send(req)
    }
}

fn library(s: &State) -> Option<SkillLibrary> {
    let data = s.data.as_ref()?;
    Some(SkillLibrary::for_store(data, Box::new(Shared(client()?))))
}

/// What is kept, read from the profile when Settings opens: no request.
pub(super) fn reload(s: &mut State) {
    if let Some(data) = s.data.as_ref() {
        s.skills_from_github.kept = SkillLibrary::for_store(data, Box::new(Shared(Arc::new(Offline)))).list();
    }
}

struct Offline;
impl HttpClient for Offline {
    fn send(&self, _: locust_core::chat_http::HttpRequest) -> Result<locust_core::chat_http::HttpResponse, String> {
        Err("offline".into())
    }
}

/// Look at a pack: only a look, never a keep. A shelf pack's offered skills are all picked, ready for Keep.
pub(super) fn look(s: &mut State, target: &str, all: bool) {
    if s.skills_from_github.busy.is_some() || target.trim().is_empty() {
        return;
    }
    let Some(lib) = library(s) else {
        s.skills_from_github.said = Some(("Locust could not reach GitHub. Check the connection and try again.".into(), false));
        return;
    };
    let shelf = &mut s.skills_from_github;
    shelf.busy = Some(LOOKING);
    shelf.said = None;
    shelf.preview = None;
    let target = target.trim().to_owned();
    s.skills_job.request((), move |()| Answer::Looked(lib.preview(&target), all));
}

fn keep(s: &mut State) {
    let Some(preview) = s.skills_from_github.preview.clone() else { return };
    let chosen = chosen(&s.skills_from_github);
    if chosen.is_empty() || s.skills_from_github.busy.is_some() {
        return;
    }
    let Some(lib) = library(s) else { return };
    s.skills_from_github.busy = Some("keeping");
    s.skills_from_github.said = None;
    let request = preview.keep(&chosen.iter().map(String::as_str).collect::<Vec<_>>());
    let (count, source) = (chosen.len(), preview.source.clone());
    s.skills_job.request((), move |()| Answer::Kept(lib.install(&request), count, source));
}

fn remove(s: &mut State, source: &str) {
    let Some(data) = s.data.as_ref() else { return };
    let lib = SkillLibrary::for_store(data, Box::new(Shared(Arc::new(Offline))));
    let shelf = &mut s.skills_from_github;
    shelf.arming = None;
    match lib.remove(source) {
        Ok(kept) => {
            shelf.kept = kept;
            if shelf.preview.as_ref().is_some_and(|p| p.source == source) {
                shelf.preview = None;
            }
            shelf.said = Some((format!("Removed the skills kept from {source}. Runs that start now do not get them."), true));
        }
        Err(why) => shelf.said = Some((why, false)),
    }
}

pub(super) fn poll(s: &mut State) -> bool {
    let Some(answer) = s.skills_job.take() else { return false };
    let shelf = &mut s.skills_from_github;
    shelf.busy = None;
    match answer {
        Answer::Looked(Ok(preview), all) => {
            shelf.picked = preview_picks(&preview, all);
            shelf.preview = Some(preview);
        }
        Answer::Looked(Err(why), _) | Answer::Kept(Err(why), _, _) => shelf.said = Some((why, false)),
        Answer::Kept(Ok(kept), count, source) => {
            shelf.kept = kept;
            shelf.preview = None;
            shelf.link = TextEdit::default();
            let them = if count == 1 { "it" } else { "them" };
            shelf.said = Some((format!("Kept {} from {source}. {} teammates get {them} from their next run.", plural(count, "skill"), locust_core::skill_runtimes::SKILL_RUNTIME_NAMES), true));
        }
    }
    true
}

fn plural(n: usize, one: &str) -> String {
    format!("{n} {one}{}", if n == 1 { "" } else { "s" })
}
fn short(sha: &str) -> &str {
    &sha[..sha.len().min(7)]
}
fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", (bytes as f64 / 1024.0).round() as u64)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
fn chosen(shelf: &Shelf) -> Vec<String> {
    let Some(preview) = &shelf.preview else { return Vec::new() };
    preview.skills.iter().filter(|k| k.held.is_none() && shelf.picked.contains(&k.name)).map(|k| k.name.clone()).collect()
}

/// A tile's meta line on the popular row: "307,555 stars · MIT".
pub(super) fn popular_meta(pack: &Pack) -> String {
    format!("{} stars · {}", stars_text(pack.stars), pack.license)
}
/// On the new-this-week row: "2,549 stars since Oct 24 · MIT".
pub(super) fn trending_meta(pack: &Pack) -> String {
    format!("{} stars since {} · {}", stars_text(pack.stars), since_text(pack.since.unwrap_or_default()), pack.license)
}
/// A kept pack's row: where it is kept, when, how many skills, and how many of its files run when any do.
pub(super) fn kept_meta(kept: &KeptSource) -> String {
    let runs: usize = kept.skills.iter().map(|k| k.runs.len()).sum();
    let at = if kept.reference == kept.sha { short(&kept.sha).to_owned() } else { format!("{} @ {}", kept.reference, short(&kept.sha)) };
    let day = chrono::DateTime::parse_from_rfc3339(&kept.kept_at).map(|d| d.with_timezone(&chrono::Local).format("%b %-d").to_string()).unwrap_or_default();
    format!("{at} · kept {day} · {}{}", plural(kept.skills.len(), "skill"), if runs > 0 { format!(" · {}", files_that_run(runs)) } else { String::new() })
}

fn open(s: &mut State, url: String) {
    if !url.starts_with("https://github.com/") {
        return;
    }
    if cfg!(test) {
        s.skills_from_github.opened.push(url);
        return;
    }
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("/usr/bin/open").arg(&url).spawn();
}

fn button(label: &str, key: String, primary: bool, p: &Palette) -> El {
    let b = text(label.to_owned()).font(type_::LABEL).px(12.0).py(6.0).radius(6.0).key(key).role("button");
    if primary { b.color(p.bg).fill(p.text) } else { b.color(p.secondary).border(1.0, p.hairline) }
}

/// One tile for both rows: a pack opens the same Look, and Keep stays a separate decision.
fn tile(pack: &'static Pack, meta: String, busy: bool, p: &Palette) -> El {
    let mut look = col()
        .gap(2.0)
        .padding(10.0)
        .radius(8.0)
        .border(1.0, p.hairline)
        .key(format!("skills-pack-{}", pack.source))
        .role("button")
        .label(format!("Look at {}", pack.name))
        .child(text(pack.name.to_owned()).font(type_::LABEL).color(p.text))
        .child(text(pack.description.to_owned()).font(type_::BODY).color(p.secondary).wrap())
        .child(text(meta).font(type_::LABEL).color(p.muted));
    if !busy {
        look = look.on_press(move |s: &mut State| look_at(s, pack.source));
    }
    col().gap(4.0).w(260.0).child(look).child(
        text(format!("Repository: {}", pack.source))
            .font(type_::LABEL)
            .color(p.secondary)
            .key(format!("skills-repo-{}", pack.source))
            .role("link")
            .on_press(move |s: &mut State| open(s, format!("https://github.com/{}", pack.source))),
    )
}

/// A tile: the box takes the pack's name, and the pack is looked at with all its skills picked.
fn look_at(s: &mut State, source: &str) {
    s.skills_from_github.link = TextEdit::new(source);
    look(s, source, true)
}

/// The box's Look (and Return in it): only a look at what was pasted, nothing picked yet.
fn look_at_link(s: &mut State) {
    let link = s.skills_from_github.link.text.clone();
    look(s, &link, false);
}

/// SkillsFromGitHub.tsx `lc-skilllib__look`: the box, then Look (Looking… while a look is out).
fn look_box(shelf: &Shelf, p: &Palette) -> El {
    let ready = !shelf.link.text.trim().is_empty() && shelf.busy.is_none();
    let mut go = button(if shelf.busy == Some(LOOKING) { LOOKING } else { "Look" }, "skills-look".into(), false, p);
    if ready {
        go = go.on_press(look_at_link);
    }
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .key("skills-look-box")
        .child(
            text_input(&shelf.link, type_::BODY.mono())
                .key("skills-link")
                .single_line()
                .label("A GitHub repository with skills")
                .placeholder("owner/repo, or its github.com link", p.muted)
                .grow(1.0)
                .h(32.0)
                .padding(6.0)
                .radius(6.0)
                .border(1.0, p.hairline)
                .fill(p.bg)
                .color(p.text)
                .on_edit(|s: &mut State| &mut s.skills_from_github.link)
                .on_submit(|s: &mut State, _: &mut Tx| look_at_link(s)),
        )
        .child(go)
}

fn skill_row(skill: &PreviewSkill, sha: &str, picked: bool, p: &Palette) -> El {
    let held = skill.held.is_some();
    let mark = if held { "–" } else if picked { "☑" } else { "☐" };
    let mut name = row().gap(6.0).child(text(format!("{mark} {}", skill.name)).font(type_::LABEL).color(if held { p.muted } else { p.text }));
    if let Some(at) = &skill.kept_at {
        name = name.child(text(if at == sha { "kept".to_owned() } else { format!("kept at {}", short(at)) }).font(type_::LABEL).color(p.secondary));
    }
    let mut c = col().gap(2.0).w_full().padding(6.0).key(format!("skills-skill-{}", skill.name)).child(name);
    if let Some(d) = &skill.description {
        c = c.child(text(d.clone()).font(type_::BODY).color(p.secondary).wrap());
    }
    let folder = if skill.folder.is_empty() { "(top of the repository)".to_owned() } else { skill.folder.clone() };
    c = c.child(text(format!("{folder} · {} · {}", plural(skill.files.len(), "file"), size_text(skill.bytes))).font(type_::LABEL).color(p.muted));
    let running: Vec<&str> = skill.files.iter().filter(|f| f.runs).map(|f| f.path.as_str()).collect();
    if let Some(runs) = runs_text(&running) {
        c = c.child(text(runs).font(type_::LABEL).color(p.amber).wrap());
    }
    if let Some(why) = &skill.held {
        c = c.child(text(why.clone()).font(type_::BODY).color(p.muted).wrap());
    } else {
        let name = skill.name.clone();
        c = c.role("checkbox").on_press(move |s: &mut State| {
            let picked = &mut s.skills_from_github.picked;
            if let Some(at) = picked.iter().position(|n| *n == name) {
                picked.remove(at);
            } else {
                picked.push(name.clone());
            }
        });
    }
    c
}

pub(super) fn section(s: &State, p: &Palette) -> El {
    let shelf = &s.skills_from_github;
    let busy = shelf.busy.is_some();
    let mut tiles = row().gap(10.0).wrap();
    for pack in &POPULAR_SKILLS {
        tiles = tiles.child(tile(pack, popular_meta(pack), busy, p));
    }
    let mut trending = row().gap(10.0).wrap();
    for pack in &TRENDING_SKILLS {
        trending = trending.child(tile(pack, trending_meta(pack), busy, p));
    }
    let mut body = col()
        .w_full()
        .gap(8.0)
        .padding(10.0)
        .key("skills-from-github")
        .child(text(HEADING.to_owned()).role("heading").level(2).font(type_::TITLE).color(p.text))
        .child(text("Popular skills".to_owned()).role("heading").level(3).font(type_::LABEL).color(p.text))
        .child(text(format!("Look at a pack to see its skills and files that run, then keep them all. Stars as of {POPULAR_SKILLS_AS_OF}.")).font(type_::BODY).color(p.secondary).wrap())
        .child(tiles)
        .child(text("New this week".to_owned()).role("heading").level(3).font(type_::LABEL).color(p.text))
        .child(text(format!("Packs made in the last two weeks that climbed fastest. Stars as of {TRENDING_SKILLS_AS_OF}.")).font(type_::BODY).color(p.secondary).wrap())
        .child(trending);
    for kept in &shelf.kept {
        let (source, reference, sha) = (kept.source.clone(), kept.reference.clone(), kept.sha.clone());
        let again = if reference == sha { source.clone() } else { format!("https://github.com/{source}/tree/{reference}") };
        let arming = shelf.arming.as_deref() == Some(kept.source.as_str());
        let gone = source.clone();
        let mut actions = row().gap(8.0);
        if !busy {
            let named = source.clone();
            actions = actions.child(button("Look for changes", format!("skills-again-{source}"), false, p).on_press(move |s: &mut State| {
                s.skills_from_github.link = TextEdit::new(&named);
                look(s, &again, false)
            }));
            actions = actions.child(
                button(if arming { "Remove?" } else { "Remove" }, format!("skills-remove-{source}"), false, p)
                    .label(if arming { format!("Remove {}?", plural(kept.skills.len(), "skill")) } else { format!("Remove the skills kept from {source}") })
                    .on_press(move |s: &mut State| {
                        if s.skills_from_github.arming.as_deref() == Some(gone.as_str()) {
                            remove(s, &gone);
                        } else {
                            s.skills_from_github.arming = Some(gone.clone());
                        }
                    }),
            );
        }
        body = body.child(
            row()
                .w_full()
                .gap(10.0)
                .center_y()
                .padding(8.0)
                .key(format!("skills-kept-{}", kept.source))
                .child(
                    col()
                        .grow(1.0)
                        .shrink()
                        .gap(2.0)
                        .child(text(kept.source.clone()).font(type_::LABEL).color(p.text))
                        .child(text(kept_meta(kept)).font(type_::LABEL).color(p.muted))
                        .child(text(kept.skills.iter().map(|k| k.name.as_str()).collect::<Vec<_>>().join(", ")).font(type_::BODY).color(p.secondary).wrap()),
                )
                .child(actions),
        );
    }
    body = body.child(look_box(shelf, p));
    if shelf.busy == Some(LOOKING) {
        body = body.child(text(LOOKING.to_owned()).font(type_::BODY).color(p.secondary).key("skills-looking"));
    }
    if let Some((said, good)) = &shelf.said {
        body = body.child(text(said.clone()).font(type_::BODY).color(if *good { p.secondary } else { p.amber }).wrap().role("status").key("skills-said"));
    }
    if let Some(preview) = &shelf.preview {
        let keepable: Vec<&PreviewSkill> = preview.skills.iter().filter(|k| k.held.is_none()).collect();
        let kept_here: Vec<&PreviewSkill> = preview.skills.iter().filter(|k| k.kept_at.is_some()).collect();
        let behind = kept_here.iter().any(|k| k.kept_at.as_deref() != Some(preview.sha.as_str()));
        let picked = chosen(shelf);
        let runs = keepable.iter().filter(|k| picked.contains(&k.name) && k.files.iter().any(|f| f.runs)).count();
        let at = if preview.reference == preview.sha { short(&preview.sha).to_owned() } else { format!("{} @ {}", preview.reference, short(&preview.sha)) };
        let mut list = col()
            .w_full()
            .gap(4.0)
            .padding(10.0)
            .radius(8.0)
            .border(1.0, p.hairline)
            .key("skills-preview")
            .label(format!("Skills in {}", preview.source))
            .child(
                row()
                    .gap(8.0)
                    .child(text(preview.source.clone()).font(type_::LABEL).color(p.text))
                    .child(text(format!("{at} · {}", plural(preview.skills.len(), "skill"))).font(type_::LABEL).color(p.muted)),
            );
        if !kept_here.is_empty() {
            let it = if kept_here.len() == 1 { "it" } else { "them" };
            list = list.child(
                text(if behind { format!("Changed since you kept {it}. Keep again to move to {}.", short(&preview.sha)) } else { format!("Up to date: nothing has changed since you kept {it}.") })
                    .font(type_::BODY)
                    .color(p.secondary)
                    .wrap(),
            );
        }
        if preview.partial {
            list = list.child(text("GitHub listed only part of this repository. Link a folder inside it to see the rest.").font(type_::BODY).color(p.amber).wrap());
        }
        for skill in &preview.skills {
            list = list.child(skill_row(skill, &preview.sha, picked.contains(&skill.name), p));
        }
        let mut actions = row().gap(8.0).center_y();
        if runs > 0 {
            let who = if runs == 1 { "One of these carries".to_owned() } else { format!("{runs} of these carry") };
            actions = actions.child(text(format!("{who} files that run. In Auto, a teammate can run them without asking.")).font(type_::BODY).color(p.amber).wrap().shrink());
        }
        actions = actions.child(button("Cancel", "skills-cancel".into(), false, p).on_press(|s: &mut State| s.skills_from_github.preview = None));
        let keep_label = if shelf.busy == Some("keeping") {
            "Keeping…".to_owned()
        } else if behind {
            format!("Keep {} at {}", plural(picked.len(), "skill"), short(&preview.sha))
        } else {
            format!("Keep {}", plural(picked.len(), "skill"))
        };
        let mut keep_button = button(&keep_label, "skills-keep".into(), true, p);
        if !busy && !picked.is_empty() {
            keep_button = keep_button.on_press(keep);
        }
        list = list.child(actions.child(keep_button));
        body = body.child(list);
    }
    body
}

#[cfg(test)]
#[path = "skills_from_github_tests.rs"]
mod tests;
