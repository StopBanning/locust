use super::*;

// Electron's index.ts syncHooks: the same callback is used after connecting or getting a called-link event.
#[test]
fn a_failed_live_hook_refresh_writes_one_safe_notice_and_a_good_refresh_stays_quiet() {
    let root = crate::routine_copy::tests::temp("locust-hook-refresh-");
    let store = Store::new(root.0.join("native"), root.0.join("electron")).unwrap();
    let routines = store.root().join("routines.json");
    std::fs::create_dir_all(store.root()).unwrap();
    // The routines read fails before the host asks the keychain or relay.
    let broken = b"not JSON: private-fixture-text";
    std::fs::write(&routines, broken).unwrap();
    let host = LiveHost::with_hook_relay(store.clone(), Utc::now(), Arc::new(StubHookRelay));
    let log = crate::diagnostic_log::log_path(store.root());

    assert!(host.hooks().sync().is_err());
    let said = std::fs::read_to_string(&log).expect("the failed refresh wrote a diagnostic");
    assert_eq!(said.lines().count(), 1);
    assert!(said.ends_with(" routine: The called links could not be collected. They will be tried again when the relay connects.\n"), "{said}");
    assert!(!said.contains("private-fixture-text"));
    assert!(host.take_updates().is_empty(), "diagnostics are not a window event");
    assert_eq!(std::fs::read(&routines).unwrap().as_slice(), broken);

    // A successful retry does not copy the old failure into the log again.
    std::fs::write(&routines, br#"{"schemaVersion":1,"routines":[]}"#).unwrap();
    assert!(host.hooks().sync().is_ok());
    assert_eq!(std::fs::read_to_string(&log).unwrap(), said);
}
