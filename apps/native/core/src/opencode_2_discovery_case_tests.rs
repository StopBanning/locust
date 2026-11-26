//! Electron's `packages/runtime-adapters/test/opencode-2-discovery-uses-its-acp-command.test.ts`, case by case, through
//! `discover_one("opencode", ..)` with the same answers Electron's runner gives: `--version` says the version, `acp`
//! says its ACP help, `run --help` the old run help and anything else the model list. Each call's arguments are kept,
//! as Electron's `calls` are.
//!
//! A native `Found` carries no `supportedFeatures`, `requiredFeatures`, `diagnostics` or `modelHints`: Electron's
//! "ready" is `Probe::Ready`, its "unsupported" on a capability it could not see is `Probe::ProbeFailed` (as for every
//! agent whose help lacks a required feature) and on a refused version `Probe::Unsupported` with the note; a model
//! list is read apart from discovery (models.rs), so "no model hints" is "no model command asked". The native launch
//! is the found path alone, with no prefix or environment, so Electron's launch-prefix case has nothing to carry.
use super::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const LEGACY_HELP: &str = "opencode run [message..]  --format <format>  text | json  -m, --model  -s, --session";
const ACP_HELP_TEXT: &str = "opencode acp: start an AgentClientProtocol server over stdin and stdout";
const MODELS: &str = "opencode/big-pickle\nopencode/deepseek-v4.1-flash-free\n";
const INSTALLED: &str = "/tools/opencode";

fn ok(stdout: &str) -> Answer {
    Answer { exit: Some(0), stdout: stdout.into(), stderr: String::new(), timed_out: false }
}

/// Electron's runner: what each check answers, and every call it was asked (path, arguments).
struct Runner {
    version: Mutex<String>,
    help: Option<Answer>,
    calls: Mutex<Vec<(PathBuf, Vec<String>)>>,
}

impl Runner {
    fn new(version: &str, help: Option<Answer>) -> Self {
        Self { version: Mutex::new(version.into()), help, calls: Mutex::new(Vec::new()) }
    }
    fn run(&self, path: &Path, args: &[&str]) -> Option<Answer> {
        crate::invariants::lock(&self.calls).push((path.to_path_buf(), args.iter().map(|a| a.to_string()).collect()));
        match args {
            ["--version"] => Some(ok(&crate::invariants::lock(&self.version))),
            ["acp", ..] => self.help.clone(),
            ["run", "--help"] => Some(ok(LEGACY_HELP)),
            _ => Some(ok(MODELS)),
        }
    }
    fn args(&self) -> Vec<Vec<String>> {
        crate::invariants::lock(&self.calls).iter().map(|(_, args)| args.clone()).collect()
    }
    fn forget(&self) {
        crate::invariants::lock(&self.calls).clear();
    }
}

fn args(list: &[&[&str]]) -> Vec<Vec<String>> {
    list.iter().map(|a| a.iter().map(|s| s.to_string()).collect()).collect()
}

/// Electron's `discover(version, help)`.
fn discover(runner: &Runner, recall: Option<Recall>) -> Found {
    let find = |c: &str| (c == "opencode").then(|| PathBuf::from(INSTALLED));
    let run = |p: &Path, a: &[&str]| runner.run(p, a);
    let exists = |_: &Path| true;
    let seams = Seams { find: &find, run: &run, exists: &exists, home: Some("/home/someone".into()), xdg_config_home: None, recall };
    discover_one("opencode", &seams).unwrap()
}

fn at(version: &str, help: Option<Answer>) -> (Found, Runner) {
    let runner = Runner::new(version, help);
    let found = discover(&runner, None);
    (found, runner)
}

/// Electron's `RuntimeFactsCache`: every write kept, and `get` answering `fixed` when given, else what was written.
#[derive(Default)]
struct Facts {
    fixed: Option<BinaryFacts>,
    kept: Mutex<HashMap<String, BinaryFacts>>,
    writes: Mutex<Vec<BinaryFacts>>,
}

impl FactsCache for Facts {
    fn get(&self, fingerprint: &str) -> Option<BinaryFacts> {
        self.fixed.clone().or_else(|| crate::invariants::lock(&self.kept).get(fingerprint).cloned())
    }
    fn set(&self, fingerprint: &str, facts: BinaryFacts) {
        crate::invariants::lock(&self.writes).push(facts.clone());
        crate::invariants::lock(&self.kept).insert(fingerprint.to_owned(), facts);
    }
}

fn status_ready(found: &Found) -> (Probe, bool) {
    (found.status, found.ready)
}

#[test]
fn checks_v2_acp_help_without_launching_v1_probes_or_spending_a_turn() {
    // case: checks V2 ACP help without launching V1 probes or spending a turn
    let (found, runner) = at("opencode v2.0.32", Some(ok(ACP_HELP_TEXT)));
    assert_eq!(status_ready(&found), (Probe::Ready, true), "{found:?}");
    assert!(found.installed);
    assert_eq!(found.version.as_deref(), Some("2.0.32"));
    assert_eq!(found.unsupported_note, None);
    assert_eq!(runner.args(), args(&[&["--version"], &["acp", "--help"]]));
    // Both at the binary that was found, each on the app's one probe clock (PROBE_TIMEOUT_MS, 10 s).
    assert!(crate::invariants::lock(&runner.calls).iter().all(|(path, _)| path == Path::new(INSTALLED)));
    assert_eq!(PROBE_TIMEOUT, std::time::Duration::from_millis(10_000));
}

#[test]
fn leaves_v2_model_choices_to_the_acp_session_instead_of_inventing_cli_hints() {
    // case: leaves V2 model choices to the ACP session instead of inventing CLI hints
    let (found, runner) = at("2.1.0", Some(ok(&format!("{ACP_HELP_TEXT}\n{MODELS}\n--effort low|high"))));
    assert_eq!(status_ready(&found), (Probe::Ready, true));
    let asked = runner.args();
    assert!(!asked.iter().any(|a| a.first().map(String::as_str) == Some("models")), "{asked:?}");
}

#[test]
fn keeps_the_v1_help_model_list_readiness_and_free_model_parsing() {
    // case: keeps the V1 help, model-list readiness and free-model parsing
    let (found, runner) = at("1.19.3", Some(ok(ACP_HELP_TEXT)));
    assert_eq!(status_ready(&found), (Probe::Ready, true), "{found:?}");
    assert_eq!(found.version.as_deref(), Some("1.19.3"));
    let asked = runner.args();
    // The version first (it picks the protocol), then the run help and the model list together.
    assert_eq!(asked[0], ["--version"]);
    let mut rest = asked[1..].to_vec();
    rest.sort();
    assert_eq!(rest, args(&[&["models", "--verbose"], &["run", "--help"]]));
    // What the list says, as the model picker reads it.
    let listed = crate::models::parse_opencode_model_list(MODELS).unwrap();
    assert_eq!(
        listed["models"],
        serde_json::json!([
            {"id": "opencode/big-pickle", "displayName": "big-pickle"},
            {"id": "opencode/deepseek-v4.1-flash-free", "displayName": "deepseek-v4.1-flash-free", "description": "free"},
        ])
    );
}

#[test]
fn refuses_future_major_versions_without_guessing_either_protocol() {
    // case: refuses future major versions without guessing either protocol
    let (found, runner) = at("3.0.0", Some(ok(ACP_HELP_TEXT)));
    assert_eq!(status_ready(&found), (Probe::Unsupported, false));
    assert!(found.unsupported_note.as_deref().is_some_and(|n| n.contains("3.0.0")), "{found:?}");
    assert_eq!(runner.args(), args(&[&["--version"]]));
}

#[test]
fn does_not_promote_an_unrecognized_version_into_acp() {
    // case: does not promote an unrecognized version into ACP
    let (found, runner) = at("opencode nightly", Some(ok(ACP_HELP_TEXT)));
    assert_eq!(found.version, None);
    assert!(!runner.args().contains(&vec!["acp".to_owned(), "--help".to_owned()]), "{:?}", runner.args());
}

#[test]
fn refuses_v2_without_safe_acp_capability_evidence() {
    // case: refuses V2 without safe ACP capability evidence: %s
    let cases = [
        ("old run help", ok(LEGACY_HELP)),
        ("nonzero exit despite an ACP label", Answer { exit: Some(1), stdout: ACP_HELP_TEXT.into(), stderr: "unknown command".into(), timed_out: false }),
        ("timed out help", Answer { timed_out: true, ..ok(ACP_HELP_TEXT) }),
    ];
    for (reason, help) in cases {
        let (found, _) = at("2.0.32", Some(help));
        assert_eq!(status_ready(&found), (Probe::ProbeFailed, false), "{reason}");
    }
}

#[test]
fn accepts_acp_help_printed_on_stderr_with_the_spaced_protocol_name() {
    // case: accepts ACP help printed on stderr with the spaced protocol name
    let help = Answer { exit: Some(0), stdout: String::new(), stderr: "Agent Client Protocol over stdio".into(), timed_out: false };
    let (found, _) = at("2.0.32", Some(help));
    assert_eq!(status_ready(&found), (Probe::Ready, true));
}

#[test]
fn contains_a_failed_acp_help_runner_without_starting_a_prompt() {
    // case: contains a failed ACP help runner without starting a prompt
    // The seam's None is a check that could not start (Electron's runner throwing).
    let (found, runner) = at("2.0.32", None);
    assert_eq!(status_ready(&found), (Probe::ProbeFailed, false));
    assert_eq!(runner.args().len(), 2);
}

fn stat_of(stamp: &Mutex<u128>) -> impl Fn(&Path) -> Option<Stamp> + Sync + '_ {
    move |_| Some(Stamp { size: 10, modified_ns: *crate::invariants::lock(stamp) })
}

#[test]
fn ignores_legacy_cached_v2_help_and_never_saves_acp_help_under_a_v1_cache_key() {
    // case: ignores legacy cached V2 help and never saves ACP help under a V1 cache key
    let facts = Facts { fixed: Some(BinaryFacts { version_text: "2.0.32".into(), capability_text: LEGACY_HELP.into() }), ..Facts::default() };
    let stamp = Mutex::new(1);
    let stat = stat_of(&stamp);
    let canonical = |_: &Path| None;
    // Electron's runner answers ACP help to everything.
    let everything = Runner::new(ACP_HELP_TEXT, Some(ok(ACP_HELP_TEXT)));
    let found = discover(&everything, Some(Recall { facts: &facts, stat: &stat, canonical: &canonical }));
    assert_eq!(status_ready(&found), (Probe::Ready, true), "{found:?}");
    assert_eq!(everything.args(), args(&[&["acp", "--help"]]));
    assert!(crate::invariants::lock(&facts.writes).is_empty());
}

#[test]
fn recognizes_a_changed_binary_upgraded_from_v1_to_v2_without_reusing_v1_capabilities() {
    // case: recognizes a changed binary upgraded from V1 to V2 without reusing V1 capabilities
    let facts = Facts::default();
    let stamp = Mutex::new(1);
    let stat = stat_of(&stamp);
    let canonical = |_: &Path| None;
    let runner = Runner::new("1.19.3", Some(ok(ACP_HELP_TEXT)));
    let recall = || Some(Recall { facts: &facts, stat: &stat, canonical: &canonical });
    assert_eq!(discover(&runner, recall()).status, Probe::Ready);
    assert_eq!(crate::invariants::lock(&facts.kept).len(), 1);
    runner.forget();
    *crate::invariants::lock(&runner.version) = "2.0.32".into();
    *crate::invariants::lock(&stamp) = 2;
    let found = discover(&runner, recall());
    assert_eq!(status_ready(&found), (Probe::Ready, true));
    assert_eq!(found.version.as_deref(), Some("2.0.32"));
    assert_eq!(runner.args(), args(&[&["--version"], &["acp", "--help"]]));
    assert_eq!(crate::invariants::lock(&facts.kept).len(), 1);
}

#[test]
fn shares_one_exact_major_selector_without_assigning_acp_to_unknown_v1_or_future_versions() {
    // case: shares one exact-major selector without assigning ACP to unknown, V1 or future versions
    use crate::runtime::opencode::version_uses_acp;
    assert!(version_uses_acp("2.0.32"));
    assert!(version_uses_acp("2.1.0-rc.1"));
    // Electron's `undefined`: a version discovery could not read.
    assert!(crate::runtime::version::parse("").is_none());
    assert!(!version_uses_acp("unrecognized"));
    assert!(!version_uses_acp("1.19.3"));
    assert!(!version_uses_acp("3.0.0"));
}
