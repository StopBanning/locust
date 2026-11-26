//! antigravity-usage-does-not-hold-the-models.test.ts, as the window shows it: the model catalog's Antigravity reading
//! (kept by the catalog, a late one too, in `antigravity_usage::usage_cache`) is drawn in the usage dialog's Limits
//! beside the receipts' readings, as App.tsx's `adoptCatalogUsage` hands it to UsageDialog.tsx -- though no turn of
//! Antigravity's ever reported it.
use super::*;
use rune::{Env, Runtime};

struct Dirs(std::path::PathBuf);

impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn opened() -> (Dirs, Vec<String>) {
    let root = std::env::temp_dir().join(format!("locust-catalog-usage-{}", std::process::id()));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    super::usage_panel::open(&mut s);
    let mut rt = Runtime::new(s, |s, _| super::usage_panel::view(s, &Palette::new(true)), (900.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let said = rt.elements().into_iter().filter_map(|e| e.text).collect();
    (Dirs(root), said)
}

#[test]
fn the_usage_dialog_shows_the_catalogs_antigravity_reading() {
    // Electron's reading, as the catalog keeps it once `agy /usage` answered (model-catalog.ts: `· as of` the read).
    let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    locust_core::antigravity_usage::store_cached_usage_text(format!("Gemini: weekly window 67% left \u{b7} as of {at}"));
    let (_dirs, said) = opened();
    assert!(said.iter().any(|t| t == "antigravity"), "{said:?}");
    assert!(
        said.iter().any(|t| t.starts_with("Gemini: weekly window") && t.ends_with("67% left")),
        "the catalog's reading is not in the Limits: {said:?}"
    );
    assert!(!said.iter().any(|t| t == locust_core::usage::USAGE_LIMITS_EMPTY), "{said:?}");
}
