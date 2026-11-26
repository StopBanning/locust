//! Electron `FirstLaunch.tsx` `teamAndFolder`, seen in live hour 1 (fault 12): with a folder open, Home still drew
//! the folder strip ("Pick it before the first conversation", Choose folder). Electron keeps the strip for the two cases
//! that need it, no folder chosen and a folder Locust made itself; an open folder shows in the composer's foot.
use super::*;

const LEDE: &str = "Every teammate works inside one project folder. Pick it before the first conversation.";

fn home(folder: Option<&str>, invented: bool) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.home.folder_path = folder.map(Into::into);
    s.home.folder_invented = invented;
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(true)), (1215.0, 4000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn said(all: &[rune::ElementInfo]) -> Vec<String> {
    all.iter().filter_map(|e| e.text.clone().or(e.label.clone())).collect()
}

fn strip_drawn(all: &[rune::ElementInfo]) -> bool {
    all.iter().any(|e| e.key.as_deref() == Some("home-folder-strip"))
}

#[test]
fn with_a_folder_open_home_draws_no_folder_strip() {
    let all = home(Some("/Users/locust/work/locust"), false);
    let words = said(&all);
    assert!(!strip_drawn(&all), "{words:?}");
    assert!(!words.iter().any(|w| w == LEDE || w == "No folder chosen" || w == "Choose folder"), "{words:?}");
    // The folder is not said on Home either: the composer's foot says it.
    assert!(!words.iter().any(|w| w.contains("/Users/locust/work/locust")), "{words:?}");
}

#[test]
fn with_no_folder_chosen_home_asks_for_one_before_the_first_conversation() {
    let all = home(None, false);
    let words = said(&all);
    assert!(strip_drawn(&all));
    assert!(words.iter().any(|w| w == "No folder chosen"), "{words:?}");
    assert!(words.iter().any(|w| w == LEDE), "{words:?}");
    assert!(words.iter().any(|w| w == "Choose folder"), "{words:?}");
}

#[test]
fn a_folder_locust_made_is_said_with_its_path_and_can_be_changed() {
    let all = home(Some("/Users/someone/Documents/Locust"), true);
    let words = said(&all);
    assert!(strip_drawn(&all));
    assert!(words.iter().any(|w| w == "Locust made a folder to work in"), "{words:?}");
    assert!(
        words.iter().any(|w| w == "Teammates will work in /Users/someone/Documents/Locust — it is new and empty. Point them at your own project instead."),
        "{words:?}"
    );
    assert!(words.iter().any(|w| w == "Choose folder"), "{words:?}");
    assert!(!words.iter().any(|w| w == LEDE), "{words:?}");
}
