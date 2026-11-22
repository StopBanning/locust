//! How a sweep asks each agent (Electron `runtime-adapters/src/discovery.ts`): one agent's checks at once
//! (`one-runtimes-probes-run-at-once.test.ts`), each agent's start told before it happens and a beat apart
//! (`probes-report-as-they-run.test.ts`), a slow one shown as being checked (`a-slow-runtime-is-shown-as-checking.test.ts`),
//! a late answer kept (`a-late-check-is-never-lost.test.ts`, the host half) and an unchanged binary not asked its
//! version and help again (`a-binary-that-did-not-change-is-not-asked-twice.test.ts`).
use super::*;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn ok(text: &str) -> Option<Answer> {
    Some(Answer { exit: Some(0), stdout: text.into(), stderr: String::new(), timed_out: false })
}

fn failed(code: i32, stderr: &str) -> Option<Answer> {
    Some(Answer { exit: Some(code), stdout: String::new(), stderr: stderr.into(), timed_out: false })
}

/// A test's machine: these lookups and runs, and no cache.
fn machine(
    find: impl Fn(&str) -> Option<PathBuf> + Send + Sync + 'static,
    run: impl Fn(&Path, &[&str]) -> Option<Answer> + Send + Sync + 'static,
) -> Machine {
    Machine {
        find: Arc::new(find),
        run: Arc::new(run),
        exists: Arc::new(|_| true),
        home: Some("/home/someone".into()),
        xdg_config_home: None,
        facts: None,
        stat: Arc::new(|_| None),
        canonical: Arc::new(|_| None),
    }
}

fn everything(c: &str) -> Option<PathBuf> {
    Some(PathBuf::from(format!("/fake/bin/{c}")))
}

fn nothing(_: &str) -> Option<PathBuf> {
    None
}

/// Every command asked, as one line each.
#[derive(Clone, Default)]
struct Asked(Arc<Mutex<Vec<String>>>);

impl Asked {
    fn push(&self, args: &[&str]) {
        crate::invariants::lock(&self.0).push(args.join(" "));
    }
    fn all(&self) -> Vec<String> {
        crate::invariants::lock(&self.0).clone()
    }
    fn count(&self, line: &str) -> usize {
        self.all().iter().filter(|l| *l == line).count()
    }
}

/// What a watcher was told, in order.
#[derive(Default)]
struct Told {
    said: Mutex<Vec<(String, String, Instant)>>,
    named: Mutex<Vec<(String, String, String)>>,
}

impl Watch for Told {
    fn started(&self, id: &str, display_name: &str, bin: &str) {
        crate::invariants::lock(&self.said).push(("started".into(), id.into(), Instant::now()));
        crate::invariants::lock(&self.named).push((id.into(), display_name.into(), bin.into()));
    }
    fn finished(&self, id: &str, _found: &Found) {
        crate::invariants::lock(&self.said).push(("finished".into(), id.into(), Instant::now()));
    }
}

impl Told {
    fn starts(&self) -> Vec<(String, Instant)> {
        crate::invariants::lock(&self.said).iter().filter(|(p, _, _)| p == "started").map(|(_, id, at)| (id.clone(), *at)).collect()
    }
}

fn watched(told: &Arc<Told>, stagger_ms: u64) -> EachOptions {
    EachOptions { stagger: Duration::from_millis(stagger_ms), watch: Some(told.clone() as Arc<dyn Watch>), ..Default::default() }
}

fn ids() -> Vec<String> {
    known().map(|(id, _)| id.to_owned()).collect()
}

// ---- one-runtimes-probes-run-at-once.test.ts ----

// case: overlaps the probes of a single runtime rather than queueing them
#[test]
fn one_agents_checks_are_asked_together_not_one_after_another() {
    let (now, peak) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicUsize::new(0)));
    let (n, p) = (now.clone(), peak.clone());
    let run = move |_: &Path, _: &[&str]| {
        let at = n.fetch_add(1, Ordering::SeqCst) + 1;
        p.fetch_max(at, Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(200));
        n.fetch_sub(1, Ordering::SeqCst);
        ok("")
    };
    let exists = |_: &Path| true;
    let find = |c: &str| everything(c);
    let seams = Seams { find: &find, run: &run, exists: &exists, home: None, xdg_config_home: None, recall: None };
    // Codex has exactly three: version, help and readiness, all in flight together.
    discover_one("codex", &seams).unwrap();
    assert!(peak.load(Ordering::SeqCst) >= 3, "peak {}", peak.load(Ordering::SeqCst));
}

// case: spawns a command that is both readiness and model list ONCE
#[test]
fn opencodes_plain_list_is_started_once_and_as_its_readiness() {
    let asked = Asked::default();
    let a = asked.clone();
    discover_installed(&machine(everything, move |_, args| {
        a.push(args);
        ok("")
    }), &EachOptions::default());
    // `models` is OpenCode's readiness and its list: one start of it, asked because the richer list did not answer.
    assert_eq!(asked.count("models"), 1, "{:?}", asked.all());
    assert_eq!(asked.count("models --verbose"), 1, "{:?}", asked.all());
}

// case: still asks a runtime with a separate list command for its list, once
#[test]
fn cursors_separate_list_is_asked_once() {
    let asked = Asked::default();
    let a = asked.clone();
    discover_installed(&machine(everything, move |_, args| {
        a.push(args);
        ok("")
    }), &EachOptions::default());
    assert_eq!(asked.count("--list-models"), 1, "{:?}", asked.all());
}

// ---- probes-report-as-they-run.test.ts ----

// case: never reports a probe finished before it reported it started
#[test]
fn no_check_is_told_finished_before_it_was_told_started() {
    let told = Arc::new(Told::default());
    discover_installed(&machine(nothing, |_, _| ok("")), &watched(&told, 0));
    let said = crate::invariants::lock(&told.said).clone();
    assert!(!said.is_empty());
    let mut started = HashMap::new();
    for (index, (phase, id, _)) in said.iter().enumerate() {
        if phase == "started" {
            started.insert(id.clone(), index);
        } else {
            let at = started.get(id).unwrap_or_else(|| panic!("{id} finished with no start"));
            assert!(index > *at);
        }
    }
    assert_eq!(said.iter().filter(|(p, _, _)| p == "finished").count(), ids().len(), "every one finishes");
}

// case: starts them one at a time, in order, however slow they are
#[test]
fn checks_start_one_at_a_time_in_order_however_slow() {
    let told = Arc::new(Told::default());
    discover_installed(
        &machine(everything, |_, _| {
            std::thread::sleep(Duration::from_millis(200));
            ok("")
        }),
        &watched(&told, 5),
    );
    let starts = told.starts();
    assert_eq!(starts.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(), ids(), "in order");
    for pair in starts.windows(2) {
        assert!(pair[1].1 >= pair[0].1);
    }
    // The last start lands long before the sum of the check times: a slow agent delays nobody else's start.
    let spread = starts.last().unwrap().1 - starts[0].1;
    assert!(spread < Duration::from_millis(200 * starts.len() as u64), "{spread:?}");
}

// case: does not make the total the sum of every probe
#[test]
fn a_sweep_costs_the_slowest_agent_not_the_sum() {
    let told = Arc::new(Told::default());
    let began = Instant::now();
    discover_installed(
        &machine(everything, |_, _| {
            std::thread::sleep(Duration::from_millis(150));
            ok("")
        }),
        &watched(&told, 2),
    );
    let took = began.elapsed();
    // One after another, eight agents would be 1.2 s at the very least.
    assert!(took < Duration::from_millis(150 * ids().len() as u64), "{took:?}");
}

// case: names the binary it is about to run, and its id, which are not the same
#[test]
fn each_start_names_the_agent_and_the_command_it_runs() {
    let told = Arc::new(Told::default());
    discover_installed(&machine(nothing, |_, _| ok("")), &watched(&told, 0));
    let named = crate::invariants::lock(&told.named).clone();
    assert_eq!(named.len(), ids().len());
    for (id, name, bin) in &named {
        assert!(!id.is_empty() && !name.is_empty() && !bin.is_empty(), "{id} {name} {bin}");
    }
    let cursor = named.iter().find(|(id, _, _)| id == "cursor").unwrap();
    assert_eq!((cursor.1.as_str(), cursor.2.as_str()), ("Cursor Agent", "cursor-agent"));
}

// case: still answers with every runtime when nobody is watching
#[test]
fn a_sweep_nobody_watches_still_answers_every_agent() {
    let found = discover_installed(&machine(nothing, |_, _| ok("")), &EachOptions::default());
    assert_eq!(found.iter().map(|f| f.id.clone()).collect::<Vec<_>>(), ids());
}

// case: chains straight through when every probe is instant
#[test]
fn a_bare_machine_waits_for_no_beat() {
    let began = Instant::now();
    let told = Arc::new(Told::default());
    discover_installed(&machine(nothing, |_, _| ok("")), &watched(&told, 240));
    // Eight agents at a fixed 240 ms beat would be 1.7 s before the last one even started.
    assert!(began.elapsed() < Duration::from_millis(1_000), "{:?}", began.elapsed());
}

// case: still spaces them out when the probes are slow
#[test]
fn slow_checks_still_start_a_beat_apart() {
    let told = Arc::new(Told::default());
    discover_installed(
        &machine(everything, |_, _| {
            std::thread::sleep(Duration::from_millis(600));
            ok("")
        }),
        &watched(&told, 40),
    );
    let starts = told.starts();
    let gaps: Vec<Duration> = starts.windows(2).map(|p| p[1].1 - p[0].1).collect();
    // Every gap is the beat, not the check: the next one does not wait for a slow answer.
    for gap in &gaps {
        assert!(*gap < Duration::from_millis(600), "{gaps:?}");
    }
    assert!(gaps.iter().max().unwrap() > &Duration::from_millis(10), "{gaps:?}");
}

// ---- a-slow-runtime-is-shown-as-checking.test.ts ----

const HELP: &str = "opencode run [message..]  --format <format>  text | json  -m, --model  -s, --session";
const MODELS: &str = "opencode/big-pickle\nopencode/muse-spark-1.3-contributor-free\n";

fn only(ids: &[&str]) -> Option<HashSet<String>> {
    Some(ids.iter().map(|s| (*s).to_owned()).collect())
}

fn found_in(names: &'static [&'static str]) -> impl Fn(&str) -> Option<PathBuf> + Send + Sync + 'static {
    move |c| names.contains(&c).then(|| PathBuf::from(format!("/tools/{c}")))
}

// case: is shown as installed and being checked -- never as signed out or missing -- while its check goes on
#[test]
fn a_slow_agent_is_shown_installed_and_being_checked_while_its_check_goes_on() {
    let (release, wait) = std::sync::mpsc::channel::<()>();
    let wait = Mutex::new(wait);
    let m = machine(found_in(&["claude", "opencode"]), move |p, args| {
        if p.ends_with("opencode") {
            // Every OpenCode command waits: a CLI that is slow to start.
            let _ = crate::invariants::lock(&wait).recv_timeout(Duration::from_secs(10));
            return ok(MODELS);
        }
        ok(if args == ["--version"] { "2.1.200" } else { "" })
    });
    let settled = settle_within(discover_each(&m, &EachOptions::default()), Duration::from_millis(300), |_, _| {});
    // The other agent's answer does not wait for OpenCode's.
    let claude = settled.answers.iter().find(|f| f.id == "claude").unwrap();
    assert!(claude.installed && !claude.checking, "{claude:?}");
    let shown = settled.answers.iter().find(|f| f.id == "opencode").unwrap();
    assert!(shown.installed && shown.checking, "{shown:?}");
    assert!(!matches!(shown.status, Probe::Missing | Probe::AuthRequired) && shown.auth != Auth::Unauthenticated, "{shown:?}");
    let words = row_status(shown, integration_of("opencode"), false, None).detail;
    assert!(!crate::invariants::regex(r"(?i)sign|not found|missing").is_match(&words), "{words}");
    // And the check was only ever waiting: it answers when its CLI does.
    let handle = settled.left.get("opencode").expect("held").clone();
    for _ in 0..4 {
        let _ = release.send(());
    }
    let answer = handle.wait().unwrap();
    assert!(answer.installed && !answer.checking, "{answer:?}");
}

// case: says a runtime that is not installed is not installed, at once
#[test]
fn an_agent_not_installed_says_so_at_once() {
    let m = machine(nothing, |_, _| ok("1.0.0"));
    let mut checks = discover_each(&m, &EachOptions { only: only(&["opencode"]), ..Default::default() });
    assert_eq!(checks.len(), 1);
    let check = checks.remove(0);
    let answer = (check.run)(Box::new(|| {})).unwrap();
    assert_eq!((answer.installed, answer.status), (false, Probe::Missing));
    let pending = (check.pending)();
    assert_eq!((pending.installed, pending.status, pending.checking), (false, Probe::Missing, false));
}

// case: answers exactly as the all-at-once call does
#[test]
fn each_check_answers_as_the_all_at_once_sweep_does() {
    let m = machine(found_in(&["opencode"]), |_, args| match args.join(" ").as_str() {
        "--version" => ok("1.18.27"),
        "run --help" => ok(HELP),
        _ => ok(MODELS),
    });
    let options = EachOptions { only: only(&["opencode"]), ..Default::default() };
    let together = discover_installed(&m, &options);
    let each = settle_within(discover_each(&m, &options), Duration::from_secs(20), |_, _| {}).answers;
    assert_eq!(each, together);
    assert_eq!(together[0].status, Probe::Ready);
}

// case: is not asked a second time when the caller says a check for it is already going
#[test]
fn an_agent_whose_check_is_going_is_not_asked_again() {
    let m = machine(found_in(&["claude", "opencode"]), |_, _| ok("1.0.0"));
    let checks = discover_each(
        &m,
        &EachOptions { only: only(&["claude", "opencode"]), skip: ["opencode".to_owned()].into_iter().collect(), ..Default::default() },
    );
    assert_eq!(checks.iter().map(|c| c.id.clone()).collect::<Vec<_>>(), ["claude"]);
}

/// The sweep the app runs: a check still going when the next sweep starts is joined, never started twice, and its
/// answer comes to `late` when it lands.
#[test]
fn a_second_sweep_joins_a_check_still_going_and_its_late_answer_arrives() {
    let (release, wait) = std::sync::mpsc::channel::<()>();
    let wait = Mutex::new(wait);
    let opencode_versions = Arc::new(AtomicUsize::new(0));
    let counted = opencode_versions.clone();
    let m = machine(found_in(&["opencode"]), move |p, args| {
        if p.ends_with("opencode") && args == ["--version"] {
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = crate::invariants::lock(&wait).recv_timeout(Duration::from_secs(10));
            return ok("1.18.27");
        }
        ok(if args == ["run", "--help"] { HELP } else { MODELS })
    });
    let late: Arc<Mutex<Vec<(String, Option<Found>)>>> = Arc::default();
    let heard = late.clone();
    let plan = SweepPlan {
        deadline: Duration::from_millis(200),
        each: EachOptions::default(),
        outstanding: Arc::default(),
        late: Arc::new(move |id, found| crate::invariants::lock(&heard).push((id.to_owned(), found.cloned()))),
    };
    let first = sweep(&m, Vec::new(), &plan);
    assert!(first.answers.iter().any(|f| f.id == "opencode" && f.checking));
    assert!(crate::invariants::lock(&plan.outstanding).contains_key("opencode"));
    let second = sweep(&m, Vec::new(), &plan);
    assert!(second.answers.iter().any(|f| f.id == "opencode" && f.checking), "still being checked, by the first check");
    assert_eq!(opencode_versions.load(Ordering::SeqCst), 1, "OpenCode was started once");
    // Electron's order, whatever order the joined check was asked in.
    let order: Vec<String> = second.answers.iter().map(|f| f.id.clone()).collect();
    assert_eq!(order, ids());
    let _ = release.send(());
    let until = Instant::now() + Duration::from_secs(10);
    while crate::invariants::lock(&plan.outstanding).contains_key("opencode") && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!crate::invariants::lock(&plan.outstanding).contains_key("opencode"), "its check is no longer outstanding");
    let heard = crate::invariants::lock(&late).clone();
    assert!(heard.iter().all(|(id, f)| id == "opencode" && f.as_ref().is_some_and(|f| f.status == Probe::Ready)), "{heard:?}");
    assert!(!heard.is_empty());
}

fn opencode_lists(verbose: Option<Answer>, plain: Option<Answer>) -> (Vec<String>, Found) {
    let asked = Asked::default();
    let a = asked.clone();
    let m = machine(found_in(&["opencode"]), move |_, args| match args.join(" ").as_str() {
        "--version" => ok("1.18.27"),
        "run --help" => ok(HELP),
        line => {
            a.push(args);
            if line.contains("--verbose") { verbose.clone() } else { plain.clone() }
        }
    });
    let found = discover_installed(&m, &EachOptions { only: only(&["opencode"]), ..Default::default() }).remove(0);
    let mut asked = asked.all();
    asked.sort();
    (asked, found)
}

// case: takes readiness and the models from the one list that carries both
#[test]
fn opencodes_richer_list_answers_readiness_alone() {
    let (asked, opencode) = opencode_lists(ok(&format!("{MODELS}{{\n \"variants\": {{}}\n}}\n")), ok(MODELS));
    assert_eq!(asked, ["models --verbose"]);
    assert_eq!(opencode.status, Probe::Ready);
}

// case: asks the plain list only when the richer one did not answer, and still reads ready from it
#[test]
fn opencodes_plain_list_is_asked_only_when_the_richer_one_failed() {
    let (asked, opencode) = opencode_lists(failed(2, "unknown option --verbose"), ok(MODELS));
    assert_eq!(asked, ["models", "models --verbose"]);
    assert_eq!(opencode.status, Probe::Ready);
}

// case: is not ready when neither list can be read -- and says unhealthy, not signed out
#[test]
fn opencode_with_no_list_is_not_answering_never_signed_out() {
    let (_, opencode) = opencode_lists(failed(1, "boom"), failed(1, "boom"));
    assert_eq!((opencode.status, opencode.auth, opencode.ready), (Probe::Offline, Auth::NotApplicable, false));
}

// ---- a-late-check-is-never-lost.test.ts (the host's half) ----

fn checking(id: &str, is: bool) -> Found {
    let mut f = pending_found(id, id);
    if !is {
        f = Found { checking: false, ready: true, status: Probe::Ready, ..f };
    }
    f
}

fn set(ids: &[&str]) -> HashSet<String> {
    ids.iter().map(|s| (*s).to_owned()).collect()
}

// case: names an agent still called "being checked" whose check has finished
#[test]
fn an_agent_still_called_checking_whose_check_finished_is_named() {
    assert_eq!(stale_checking(&[checking("opencode", true), checking("codex", false)], &set(&["opencode"])), ["opencode"]);
}

// case: leaves alone one still being checked, and one already ready
#[test]
fn one_still_being_checked_and_one_already_ready_are_left_alone() {
    assert!(stale_checking(&[checking("opencode", true)], &set(&[])).is_empty());
    assert!(stale_checking(&[checking("opencode", false)], &set(&["opencode"])).is_empty());
    // Nor one that answered, not ready: only a row still drawn as being checked is stale.
    let signed_out = Found { checking: false, status: Probe::AuthRequired, ..pending_found("codex", "Codex CLI") };
    assert!(stale_checking(&[signed_out], &set(&["codex"])).is_empty());
    // And one already ready, even while still marked as being checked (status.ts reads both).
    let ready_marked = Found { checking: true, ..checking("opencode", false) };
    assert!(stale_checking(&[ready_marked], &set(&["opencode"])).is_empty());
}

/// answeredLate's patch: the late answer takes its agent's place in what is held, and nothing else moves.
#[test]
fn a_late_answer_takes_its_agents_place_in_the_held_answer() {
    let held = Mutex::new(Some(HeldAnswer { at: 5, value: vec![checking("codex", false), checking("opencode", true)] }));
    patch_held(&held, &checking("opencode", false));
    let now = crate::invariants::lock(&held).clone().unwrap();
    assert_eq!(now, HeldAnswer { at: 5, value: vec![checking("codex", false), checking("opencode", false)] });
    patch_held(&held, &checking("kilo", false));
    assert_eq!(crate::invariants::lock(&held).as_ref().unwrap().value.len(), 3, "one it did not hold is added");
}

// ---- a-binary-that-did-not-change-is-not-asked-twice.test.ts ----

#[derive(Default)]
struct MapFacts(Mutex<HashMap<String, BinaryFacts>>);

impl FactsCache for MapFacts {
    fn get(&self, fingerprint: &str) -> Option<BinaryFacts> {
        crate::invariants::lock(&self.0).get(fingerprint).cloned()
    }
    fn set(&self, fingerprint: &str, facts: BinaryFacts) {
        crate::invariants::lock(&self.0).insert(fingerprint.to_owned(), facts);
    }
}

impl MapFacts {
    fn size(&self) -> usize {
        crate::invariants::lock(&self.0).len()
    }
}

type Files = HashMap<PathBuf, Stamp>;

/// Every agent's binary, all with the same size and time, by COMMAND name (Cursor's is `cursor-agent`).
fn every_binary(modified: u128) -> Files {
    DEFINITIONS.iter().map(|d| (PathBuf::from(format!("/bin/{}", d.command)), Stamp { size: 1000, modified_ns: modified })).collect()
}

struct Harness {
    asked: Asked,
    store: Arc<MapFacts>,
    found: Vec<Found>,
}

impl Harness {
    fn sweep(files: Files, store: Arc<MapFacts>, links: HashMap<PathBuf, PathBuf>) -> Self {
        let asked = Asked::default();
        let a = asked.clone();
        let mut m = machine(
            |c| Some(PathBuf::from(format!("/bin/{c}"))),
            move |_, args| {
                a.push(args);
                ok("1.2.3 --resume --output-format stream-json")
            },
        );
        m.facts = Some(store.clone() as Arc<dyn FactsCache>);
        m.stat = Arc::new(move |p| files.get(p).copied());
        m.canonical = Arc::new(move |p| links.get(p).cloned());
        let found = discover_installed(&m, &EachOptions::default());
        Harness { asked, store, found }
    }
    fn asked_version(&self) -> bool {
        self.asked.all().iter().any(|l| l == "--version")
    }
    fn asked_help(&self) -> bool {
        self.asked.all().iter().any(|l| l.ends_with("--help"))
    }
}

// case: asks everything the first time, and remembers what the files said
#[test]
fn the_first_sweep_asks_everything_and_remembers_it() {
    let first = Harness::sweep(every_binary(10), Arc::default(), HashMap::new());
    assert!(first.asked_version() && first.asked_help());
    assert!(first.store.size() > 0);
}

// case: skips version and help on the second sweep, and still asks readiness
#[test]
fn the_second_sweep_skips_version_and_help_and_still_asks_readiness() {
    let store: Arc<MapFacts> = Arc::default();
    Harness::sweep(every_binary(10), store.clone(), HashMap::new());
    let second = Harness::sweep(every_binary(10), store, HashMap::new());
    assert!(!second.asked_version() && !second.asked_help(), "{:?}", second.asked.all());
    // Signing in changes no file, so this one is always asked.
    assert!(second.asked.all().iter().any(|l| l == "login status"), "{:?}", second.asked.all());
}

// case: reports the same version from the remembered text as from the probe
#[test]
fn what_was_remembered_answers_as_the_check_did() {
    let store: Arc<MapFacts> = Arc::default();
    let fresh = Harness::sweep(every_binary(10), store.clone(), HashMap::new());
    let remembered = Harness::sweep(every_binary(10), store, HashMap::new());
    assert!(!remembered.asked_version());
    assert_eq!(remembered.found, fresh.found);
    assert!(fresh.found.iter().any(|f| f.version.as_deref() == Some("1.2.3")), "{:?}", fresh.found);
}

// case: asks again when the file changed, which is what an update looks like
#[test]
fn a_changed_file_is_asked_again() {
    let store: Arc<MapFacts> = Arc::default();
    Harness::sweep(every_binary(10), store.clone(), HashMap::new());
    let updated = Harness::sweep(every_binary(20), store, HashMap::new());
    assert!(updated.asked_version() && updated.asked_help());
}

// case: asks again when the size changed but the time did not
#[test]
fn a_file_of_another_size_is_asked_again() {
    let store: Arc<MapFacts> = Arc::default();
    Harness::sweep(every_binary(10), store.clone(), HashMap::new());
    let resized: Files = every_binary(10).into_iter().map(|(p, s)| (p, Stamp { size: s.size + 1, ..s })).collect();
    assert!(Harness::sweep(resized, store, HashMap::new()).asked_version());
}

// case: fingerprints the script a shim runs, not only the shim
#[test]
fn the_script_a_launcher_points_at_is_fingerprinted_too() {
    // An npm bin on a Mac is a link to the CLI's own script: the script is what an update rewrites.
    let script = PathBuf::from("/npm/lib/node_modules/claude/cli.js");
    let links: HashMap<PathBuf, PathBuf> = [(PathBuf::from("/bin/claude"), script.clone())].into_iter().collect();
    let files = |script_time: u128| {
        let mut f = every_binary(1);
        f.insert(script.clone(), Stamp { size: 5_000, modified_ns: script_time });
        f
    };
    let store: Arc<MapFacts> = Arc::default();
    Harness::sweep(files(1), store.clone(), links.clone());
    let same = Harness::sweep(files(1), store.clone(), links.clone());
    assert!(!same.asked_version(), "the control: nothing changed");
    let updated = Harness::sweep(files(2), store, links);
    assert!(updated.asked.count("--version") >= 1, "{:?}", updated.asked.all());
}

/// A script launcher (a Windows shape) brings its folder: the CLI behind it updates itself while it stays put.
#[test]
fn a_script_launchers_folder_is_part_of_its_fingerprint() {
    let stamps: Files = [
        (PathBuf::from("/tools/muse.cmd"), Stamp { size: 300, modified_ns: 1 }),
        (PathBuf::from("/tools/.muse-version"), Stamp { size: 6, modified_ns: 7 }),
    ]
    .into_iter()
    .collect();
    let facts = MapFacts::default();
    let recall = |files: &Files| {
        let f = files.clone();
        fingerprint_of(Path::new("/tools/muse.cmd"), &Recall { facts: &facts, stat: &move |p: &Path| f.get(p).copied(), canonical: &|_: &Path| None })
    };
    let before = recall(&stamps).unwrap();
    assert!(before.contains("/tools/.muse-version:6:7"), "{before}");
    let mut moved = stamps.clone();
    moved.insert(PathBuf::from("/tools/.muse-version"), Stamp { size: 6, modified_ns: 8 });
    assert_ne!(recall(&moved), Some(before.clone()));
    // A plain binary brings nothing beside it.
    let plain = fingerprint_of(Path::new("/tools/muse"), &Recall { facts: &facts, stat: &|_: &Path| Some(Stamp { size: 1, modified_ns: 1 }), canonical: &|_: &Path| None });
    assert_eq!(plain.as_deref(), Some("/tools/muse:1:1"));
}

// case: probes rather than guesses when nothing about the file can be read
#[test]
fn a_file_that_cannot_be_read_is_asked_every_time() {
    let store: Arc<MapFacts> = Arc::default();
    Harness::sweep(Files::new(), store.clone(), HashMap::new());
    let second = Harness::sweep(Files::new(), store.clone(), HashMap::new());
    assert!(second.asked_version());
    assert_eq!(store.size(), 0);
}

// case: remembers nothing from a probe that failed
#[test]
fn a_check_that_failed_is_not_remembered() {
    let store: Arc<MapFacts> = Arc::default();
    let mut m = machine(|c| Some(PathBuf::from(format!("/bin/{c}"))), |_, _| failed(1, "boom"));
    m.facts = Some(store.clone() as Arc<dyn FactsCache>);
    m.stat = Arc::new(|_| Some(Stamp { size: 1000, modified_ns: 10 }));
    discover_installed(&m, &EachOptions::default());
    assert_eq!(store.size(), 0);
    // Half an answer is not an answer either: a version with no help, or help with no version.
    for failing in ["--version", "--help"] {
        let mut m = machine(|c| Some(PathBuf::from(format!("/bin/{c}"))), move |_, args| {
            if args.last() == Some(&failing) { failed(1, "boom") } else { ok("1.2.3") }
        });
        m.facts = Some(store.clone() as Arc<dyn FactsCache>);
        m.stat = Arc::new(|_| Some(Stamp { size: 1000, modified_ns: 10 }));
        discover_installed(&m, &EachOptions::default());
        assert_eq!(store.size(), 0, "{failing} failed");
    }
}

/// This Mac's stamp follows a link, as Node's `stat` does: an npm bin's stamp is its script's.
#[test]
fn a_links_stamp_is_the_file_it_points_at() {
    let dir = std::env::temp_dir().join(format!("locust-stamp-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("cli.js");
    std::fs::write(&script, "x".repeat(5_000)).unwrap();
    let link = dir.join("claude");
    std::os::unix::fs::symlink(&script, &link).unwrap();
    assert_eq!(stamp_of(&link).map(|s| s.size), Some(5_000));
    assert_eq!(stamp_of(&link), stamp_of(&script));
    assert_eq!(stamp_of(&dir.join("gone")), None);
    let _ = std::fs::remove_dir_all(&dir);
}

// case: asks everything when no cache is given at all
#[test]
fn with_no_cache_everything_is_asked() {
    let asked = Asked::default();
    let a = asked.clone();
    let mut m = machine(|c| Some(PathBuf::from(format!("/bin/{c}"))), move |_, args| {
        a.push(args);
        ok("1.2.3")
    });
    m.stat = Arc::new(|_| Some(Stamp { size: 1000, modified_ns: 10 }));
    discover_installed(&m, &EachOptions::default());
    discover_installed(&m, &EachOptions::default());
    assert!(asked.count("--version") >= 2 * 4, "{:?}", asked.all());
}
