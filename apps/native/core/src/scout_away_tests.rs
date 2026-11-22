use crate::scout::store::ScoutFiles;
use crate::scout::types::{ScoutQuietHours, ScoutUrgency};
use crate::scout::{AnswerResult, AskResult, Scout, ScoutDeps, ScoutRoute, ScoutTeammate};
use crate::scout_away::{
    AWAY_ASK_KEY, AWAY_SCRIPT_PATH, AWAY_WORKFLOW_PATH, AwayComment, AwayConfig, AwayFetch,
    AwayFinding, AwayHttpResponse, AwayLetter, AwayLetterFrom, AwayLetterKind, ScoutAway,
    ScoutAwayDeps, ScoutAwayGh, ScoutAwayRelay, ScoutAwaySet, ScoutAwayStatus, SealedAwayBox,
    ask_of, away_ask, away_comment_of, away_workflow, new_away_key, open_away_letter,
    seal_away_letter,
};
use chrono::{DateTime, FixedOffset, Utc};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const ROOM: &str = "RRRRRRRRRRRRRRRRRRRRRR";
const REPO: &str = "acme/app";

fn dt(iso: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(iso).unwrap().with_timezone(&Utc)
}

fn config_of(key: &str, quiet: Option<ScoutQuietHours>) -> AwayConfig {
    AwayConfig {
        v: 1,
        relay: "https://app.locust.lol".into(),
        room: ROOM.into(),
        token: "T".repeat(43),
        key: key.into(),
        login: "Person".into(),
        tz: "America/New_York".into(),
        quiet,
        brief_at: None,
    }
}

fn yours() -> AwayComment {
    AwayComment {
        id: 12,
        login: "person".into(),
        body: "/locust ada fix the flaky Mac test\nand say what broke".into(),
        url: format!("https://github.com/{REPO}/pull/212#issuecomment-12"),
        number: 212,
        title: "Talk starts from any app".into(),
    }
}

struct FakeAwayRelay {
    pc: bool,
    latest: Option<Value>,
    calls: AtomicUsize,
    posted: Mutex<Vec<Value>>,
}

impl FakeAwayRelay {
    fn new(pc: bool, latest: Option<Value>) -> Self {
        Self {
            pc,
            latest,
            calls: AtomicUsize::new(0),
            posted: Mutex::new(Vec::new()),
        }
    }
}

impl AwayFetch for FakeAwayRelay {
    fn fetch(&self, _url: &str, method: &str, body: Option<&str>) -> Result<AwayHttpResponse, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if method == "GET" {
            let mut resp = json!({ "pc": self.pc });
            if let Some(l) = &self.latest {
                resp["latest"] = l.clone();
            }
            return Ok(AwayHttpResponse {
                status: 200,
                body: resp.to_string(),
            });
        }
        let parsed: Value = serde_json::from_str(body.unwrap_or("{}")).unwrap();
        self.posted.lock().unwrap().push(parsed);
        Ok(AwayHttpResponse {
            status: 200,
            body: json!({ "rung": 1 }).to_string(),
        })
    }
}

#[test]
// case: goes to your phone at once, sealed, ringing it
fn goes_to_your_phone_at_once_sealed_ringing_it() {
    let key = new_away_key();
    let before = seal_away_letter(
        &key,
        &AwayLetter {
            v: 1,
            kind: AwayLetterKind::Look,
            at: "2026-10-27T10:00:00Z".into(),
            repo: REPO.into(),
            read: true,
            found: Vec::new(),
            prs: Vec::new(),
            told: vec!["pr:a".into()],
            brief_day: Some("2026-10-27".into()),
        },
        ROOM,
        AwayLetterFrom::Away,
    )
    .unwrap();
    let relay = FakeAwayRelay::new(
        false,
        Some(json!({ "n": before.n, "c": before.c, "from": "away" })),
    );
    let comment = yours();
    let result = away_ask(
        &config_of(&key, None),
        REPO,
        &relay,
        dt("2026-10-27T15:00:00Z"),
        &comment,
    );
    assert!(result.ok);
    assert_eq!(result.said, "Scout passed your ask on. It rang 1 phone.");
    assert!(!result.said.contains("Talk starts"));

    let posted = relay.posted.lock().unwrap();
    assert_eq!(posted.len(), 1);
    assert_eq!(posted[0]["alert"], "found");
    let boxed = SealedAwayBox {
        n: posted[0]["n"].as_str().unwrap().into(),
        c: posted[0]["c"].as_str().unwrap().into(),
    };
    let letter = open_away_letter(&key, &boxed, ROOM, AwayLetterFrom::Away).unwrap();
    assert_eq!(letter.found.len(), 1);
    assert_eq!(letter.found[0].key, format!("{AWAY_ASK_KEY}{REPO}:12"));
    assert_eq!(letter.found[0].urgency, ScoutUrgency::Now);
    assert_eq!(
        letter.found[0].said,
        "You asked on #212 Talk starts from any app: ada fix the flaky Mac test"
    );
    assert_eq!(
        letter.found[0].detail.as_deref(),
        Some("ada fix the flaky Mac test\nand say what broke")
    );
    assert_eq!(letter.found[0].url.as_deref(), Some(comment.url.as_str()));
    assert_eq!(letter.told, vec!["pr:a".to_owned(), format!("{AWAY_ASK_KEY}{REPO}:12")]);
    assert_eq!(letter.brief_day.as_deref(), Some("2026-10-27"));
}

#[test]
// case: does nothing for anyone else’s comment, or one that asks Locust nothing
fn does_nothing_for_anyone_elses_comment_or_one_that_asks_locust_nothing() {
    let cases = [
        AwayComment { login: "stranger".into(), ..yours() },
        AwayComment { body: "Please /locust ada fix it".into(), ..yours() },
        AwayComment { body: "/locust   ".into(), ..yours() },
        AwayComment { url: "https://github.com/other/app/pull/212#issuecomment-12".into(), ..yours() },
    ];
    for comment in cases {
        let relay = FakeAwayRelay::new(false, None);
        let result = away_ask(
            &config_of(&new_away_key(), None),
            REPO,
            &relay,
            dt("2026-10-27T15:00:00Z"),
            &comment,
        );
        assert!(result.ok);
        assert_eq!(relay.calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
// case: leaves it to Locust when your computer is on
fn leaves_it_to_locust_when_your_computer_is_on() {
    let relay = FakeAwayRelay::new(true, None);
    let result = away_ask(
        &config_of(&new_away_key(), None),
        REPO,
        &relay,
        dt("2026-10-27T15:00:00Z"),
        &yours(),
    );
    assert!(result.ok);
    assert_eq!(
        result.said,
        "Locust is open on your computer, so it reads your ask from there."
    );
    assert!(relay.posted.lock().unwrap().is_empty());
}

#[test]
// case: is passed on once, and rings no phone in quiet hours
fn is_passed_on_once_and_rings_no_phone_in_quiet_hours() {
    let key = new_away_key();
    let told = seal_away_letter(
        &key,
        &AwayLetter {
            v: 1,
            kind: AwayLetterKind::Look,
            at: "2026-10-27T10:00:00Z".into(),
            repo: REPO.into(),
            read: true,
            found: Vec::new(),
            prs: Vec::new(),
            told: vec![format!("{AWAY_ASK_KEY}{REPO}:12")],
            brief_day: None,
        },
        ROOM,
        AwayLetterFrom::Away,
    )
    .unwrap();
    let again = FakeAwayRelay::new(
        false,
        Some(json!({ "n": told.n, "c": told.c, "from": "away" })),
    );
    let res = away_ask(
        &config_of(&key, None),
        REPO,
        &again,
        dt("2026-10-27T15:00:00Z"),
        &yours(),
    );
    assert_eq!(res.said, "Your phone has this ask already.");
    assert!(again.posted.lock().unwrap().is_empty());

    let night = FakeAwayRelay::new(false, None);
    // 03:00 in New York (EDT = UTC-4 on 27 Oct).
    away_ask(
        &config_of(&key, Some(ScoutQuietHours { from: 22, to: 7 })),
        REPO,
        &night,
        dt("2026-10-27T07:00:00Z"),
        &yours(),
    );
    let posted = night.posted.lock().unwrap();
    assert_eq!(posted.len(), 1);
    assert!(posted[0].get("alert").is_none());
}

#[test]
// case: is read from the event GitHub gives the run, only when the comment was written
fn is_read_from_the_event_github_gives_the_run_only_when_the_comment_was_written() {
    let url = yours().url;
    let event = json!({
        "action": "created",
        "comment": { "id": 12, "user": { "login": "person" }, "body": "/locust go", "html_url": url },
        "issue": { "number": 212, "title": "T" }
    });
    assert_eq!(
        away_comment_of(&event),
        Some(AwayComment {
            id: 12,
            login: "person".into(),
            body: "/locust go".into(),
            url: yours().url,
            number: 212,
            title: "T".into(),
        })
    );
    let mut edited = event.clone();
    edited["action"] = json!("edited");
    assert_eq!(away_comment_of(&edited), None);
    assert_eq!(
        ask_of("Some preface\n/locust fix it\nKeep the test.").as_deref(),
        Some("fix it\nKeep the test.")
    );
}

#[test]
// case: runs the workflow for a comment, one run each, and ends anyone else’s at the job
fn runs_the_workflow_for_a_comment_one_run_each_and_ends_anyone_elses_at_the_job() {
    let workflow = away_workflow(1);
    assert!(workflow.contains("  issue_comment:\n    types: [created]\n"));
    assert!(workflow.contains("if: github.event_name != 'issue_comment' || contains(github.event.comment.body, '/locust')"));
    assert!(workflow.contains("group: locust-scout${{ github.event_name == 'issue_comment' && format('-ask-{0}', github.event.comment.id) || '' }}"));
    assert!(!workflow.contains(": write"));
}

#[test]
// case: makes no card from the phone’s line: the comment itself becomes the ask
fn makes_no_card_from_the_phones_line_the_comment_itself_becomes_the_ask() {
    struct MinDeps {
        dir: PathBuf,
    }
    impl ScoutDeps for MinDeps {
        fn files(&self) -> ScoutFiles {
            ScoutFiles::Paths {
                file: self.dir.join("scout.json"),
                log_file: Some(self.dir.join("scout-looks.json")),
            }
        }
        fn now(&self) -> DateTime<FixedOffset> {
            DateTime::parse_from_rfc3339("2026-10-27T16:00:00Z").unwrap()
        }
        fn github(&self) -> Result<Option<crate::scout::github::ScoutGithub>, crate::scout::github::ScoutGithubUnread> {
            Ok(None)
        }
        fn waiting(&self) -> Result<Vec<crate::scout::types::WaitingCard>, String> {
            Ok(Vec::new())
        }
        fn failed_turn(&self, _mission_id: &str) -> Result<Option<crate::scout::types::FailedTurn>, String> {
            Ok(None)
        }
        fn team(&self) -> Result<Vec<ScoutTeammate>, String> {
            Ok(Vec::new())
        }
        fn ask(&self, _teammate_id: &str, _prompt: &str, _model: Option<&str>) -> AskResult {
            AskResult::Refused("No model".into())
        }
        fn stop(&self, _run_id: &str) {}
        fn next_free_model(&self, _teammate_id: &str, _tried: &[String]) -> Option<String> {
            None
        }
        fn answer_of(&self, _mission_id: &str) -> AnswerResult {
            AnswerResult::Failed("No answer".into())
        }
        fn make_teammate(&self, _route: &ScoutRoute) -> Result<String, String> {
            Ok("scout".into())
        }
        fn teammate_exists(&self, _teammate_id: &str) -> Result<bool, String> {
            Ok(true)
        }
    }

    let dir = std::env::temp_dir().join(format!("locust-away-ask-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let scout = Scout::new(Arc::new(MinDeps { dir: dir.clone() }));
    scout.absorb(&[AwayLetter {
        v: 1,
        kind: AwayLetterKind::Look,
        at: "2026-10-27T15:00:00Z".into(),
        repo: REPO.into(),
        read: true,
        prs: Vec::new(),
        told: Vec::new(),
        brief_day: None,
        found: vec![
            AwayFinding {
                key: format!("{AWAY_ASK_KEY}{REPO}:12"),
                urgency: ScoutUrgency::Now,
                said: "You asked on #212: fix it".into(),
                detail: None,
                lines: None,
                chips: None,
                url: None,
            },
            AwayFinding {
                key: "issue:acme/app:9".into(),
                urgency: ScoutUrgency::Soon,
                said: "A new issue: #9".into(),
                detail: None,
                lines: None,
                chips: None,
                url: None,
            },
        ],
    }]);
    let state = scout.state();
    let keys: Vec<&str> = state.cards.iter().map(|c| c.fact.key.as_str()).collect();
    assert_eq!(keys, vec!["issue:acme/app:9"]);
    assert!(!state.cards.iter().any(|c| c.fact.key.starts_with(AWAY_ASK_KEY)));
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- Repository update helper ---------------------------------------------------------------------------------------

struct DeskState {
    script: String,
    on_main: Option<(String, String)>,
    pr_state: String,
    trees: Vec<Vec<(String, String)>>,
    titles: Vec<String>,
}

struct FakeGh(Arc<Mutex<DeskState>>);

impl ScoutAwayGh for FakeGh {
    fn run(&self, args: &[&str], _cwd: &str, input: Option<&str>) -> Result<String, String> {
        let line = args.join(" ");
        let mut st = self.0.lock().unwrap();
        if line.starts_with("repo view") {
            return Ok(json!({
                "nameWithOwner": REPO,
                "visibility": "PUBLIC",
                "defaultBranchRef": { "name": "main" },
                "viewerPermission": "ADMIN"
            })
            .to_string());
        }
        if line == "api user --jq .login" {
            return Ok("person\n".into());
        }
        if line.starts_with(&format!("api repos/{REPO}/contents/{AWAY_WORKFLOW_PATH}?ref=main")) {
            let Some((wf, _)) = &st.on_main else {
                return Err("HTTP 404".into());
            };
            return Ok(if line.contains("--jq") { "sha\n".into() } else { wf.clone() });
        }
        if line.starts_with(&format!("api repos/{REPO}/contents/{AWAY_SCRIPT_PATH}?ref=main")) {
            let Some((_, sc)) = &st.on_main else {
                return Err("HTTP 404".into());
            };
            return Ok(sc.clone());
        }
        if line == format!("api repos/{REPO}/git/ref/heads/main --jq .object.sha") {
            return Ok("base1\n".into());
        }
        if line == format!("api repos/{REPO}/git/commits/base1") {
            return Ok(json!({ "sha": "base1", "tree": { "sha": "tree0" } }).to_string());
        }
        if line.contains("git/trees") {
            let parsed: Value = serde_json::from_str(input.unwrap()).unwrap();
            let tree: Vec<(String, String)> = parsed["tree"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| {
                    (
                        e["path"].as_str().unwrap().to_owned(),
                        e["content"].as_str().unwrap().to_owned(),
                    )
                })
                .collect();
            st.trees.push(tree);
            return Ok(json!({ "sha": "tree1" }).to_string());
        }
        if line.contains("git/commits") {
            return Ok(json!({ "sha": "commit1" }).to_string());
        }
        if line.contains("git/refs") {
            return Ok("{}".into());
        }
        if line.contains(&format!("repos/{REPO}/pulls --input")) {
            let parsed: Value = serde_json::from_str(input.unwrap()).unwrap();
            st.titles.push(parsed["title"].as_str().unwrap().to_owned());
            let num = 11 + st.titles.len() as u64;
            return Ok(json!({ "url": format!("https://github.com/{REPO}/pull/{num}"), "number": num }).to_string());
        }
        if line.starts_with(&format!("api repos/{REPO}/pulls/")) {
            return Ok(format!("{}\n", st.pr_state));
        }
        Ok(String::new())
    }
}

struct StubRelay;
impl ScoutAwayRelay for StubRelay {
    fn room(&self) -> Option<(String, String)> {
        Some(("https://app.locust.lol".into(), ROOM.into()))
    }
    fn connected(&self) -> bool {
        true
    }
    fn away_open(&self, _hash: &str) -> bool {
        true
    }
    fn away_close(&self) -> bool {
        true
    }
    fn keep_away_alerts(&self) -> bool {
        true
    }
    fn post(&self, _n: &str, _c: &str) -> bool {
        true
    }
}

fn make_desk() -> (PathBuf, Arc<Mutex<DeskState>>, impl Fn() -> ScoutAway) {
    let dir = std::env::temp_dir().join(format!("locust-away-update-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let state = Arc::new(Mutex::new(DeskState {
        script: "// the older away look\n".into(),
        on_main: None,
        pr_state: "open".into(),
        trees: Vec::new(),
        titles: Vec::new(),
    }));
    let dir_c = dir.clone();
    let st_c = state.clone();
    let make = move || {
        let st_script = st_c.clone();
        ScoutAway::new(ScoutAwayDeps {
            file: dir_c.join("scout-away.json"),
            folder: Arc::new(|| Some("/work/app".into())),
            gh: Arc::new(FakeGh(st_c.clone())),
            relay: Arc::new(|| Some(Arc::new(StubRelay) as Arc<dyn ScoutAwayRelay>)),
            script: Arc::new(move || st_script.lock().unwrap().script.clone()),
            tz: Arc::new(|| "America/New_York".into()),
            now: Arc::new(|| dt("2026-10-27T14:00:00Z")),
        })
    };
    (dir, state, make)
}

#[test]
// case: is offered the newer one by a pull request, and is on it once that is merged
fn is_offered_the_newer_one_by_a_pull_request_and_is_on_it_once_that_is_merged() {
    let (dir, state, make) = make_desk();
    make().set(ScoutAwaySet::On);
    {
        let mut st = state.lock().unwrap();
        st.on_main = Some((away_workflow(1), st.script.clone()));
    }
    assert!(matches!(
        make().status(true),
        ScoutAwayStatus::On { ref repo, outdated: false, update: None, .. } if repo == REPO
    ));

    state.lock().unwrap().script = "// the newer away look, that passes on /locust asks\n".into();
    let away = make();
    assert!(matches!(
        away.status(false),
        ScoutAwayStatus::On { outdated: true, .. }
    ));
    let opened = away.set(ScoutAwaySet::Update);
    assert!(matches!(
        opened,
        ScoutAwayStatus::On { outdated: false, update: Some(ref u), .. } if u == &format!("https://github.com/{REPO}/pull/13")
    ));
    {
        let st = state.lock().unwrap();
        assert_eq!(st.titles.last().map(String::as_str), Some("Scout’s away look, updated from Locust"));
        let last_tree = st.trees.last().unwrap();
        assert!(last_tree.contains(&(AWAY_WORKFLOW_PATH.into(), away_workflow(1))));
        assert!(last_tree.contains(&(
            AWAY_SCRIPT_PATH.into(),
            "// the newer away look, that passes on /locust asks\n".into()
        )));
    }
    assert!(matches!(
        away.status(true),
        ScoutAwayStatus::On { update: Some(ref u), .. } if u == &format!("https://github.com/{REPO}/pull/13")
    ));
    {
        let mut st = state.lock().unwrap();
        st.on_main = Some((away_workflow(1), st.script.clone()));
    }
    let after = away.status(true);
    assert!(matches!(
        after,
        ScoutAwayStatus::On { ref repo, outdated: false, update: None, .. } if repo == REPO
    ));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
// case: goes on as it was when the update is closed without merging, and can be offered again
fn goes_on_as_it_was_when_the_update_is_closed_without_merging_and_can_be_offered_again() {
    let (dir, state, make) = make_desk();
    make().set(ScoutAwaySet::On);
    {
        let mut st = state.lock().unwrap();
        st.on_main = Some((away_workflow(1), st.script.clone()));
    }
    make().status(true);
    state.lock().unwrap().script = "// the newer away look, that passes on /locust asks\n".into();
    let away = make();
    away.set(ScoutAwaySet::Update);
    state.lock().unwrap().pr_state = "closed".into();
    let status = away.status(true);
    assert!(matches!(
        status,
        ScoutAwayStatus::On {
            outdated: true,
            update: None,
            message: Some(ref m),
            ..
        } if m == "The update’s pull request was closed without merging, so the away look goes on as it was."
    ));
    let _ = std::fs::remove_dir_all(&dir);
}
