use crate::store::Store;
use std::process::Command;
pub(super) fn reader_check(store: &Store, id: &str, phase: &str) {
    if let Ok(reader) = std::env::var("LOCUST_TEST_TS_READER") {
        assert!(
            Command::new("node")
                .arg(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/scripts/check-m1-ledger.mjs"
                ))
                .arg(reader)
                .arg(store.ledger_dir().unwrap())
                .arg(id)
                .arg(phase)
                .status()
                .unwrap()
                .success()
        );
    }
}
