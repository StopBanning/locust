//! Which routes a launch may start runs on (Electron `main/free-routes.ts`): a window a test drive opened may start runs
//! on free routes only. Which routes are free is `scout::host::is_free_route`; this is whether the launch is held to them.
//!
//! The rule for every drive of the app is "spend nothing you were not given". Every script that drives the app takes
//! the window over with `--remote-debugging-port` (`agent_updates::may_update_agents` reads the same switch), and nobody
//! using Locust launches it that way; the drive harness also sets `LOCUST_FREE_ONLY=1`. `LOCUST_SPEND=1`, set by the
//! person running a drive that is meant to spend, lifts it. It only ever takes something away.
//!
//! The start gate is in `native::send_from` (codex-mission.ts and antigravity-mission.ts in Electron): when
//! `free_routes_only_now` says so, a route `scout::host::is_free_route` does not call free is refused with
//! `FREE_ONLY_REFUSAL` before anything is recorded or spawned. Antigravity has no free route, and starts there too.

/// What the drive harness sets to hold a launch to free routes.
pub const FREE_ONLY_ENVIRONMENT: &str = "LOCUST_FREE_ONLY";

/// What a held launch says when a run on a route that is not free is asked for.
pub const FREE_ONLY_REFUSAL: &str =
    "This window was opened by a test drive that may only use free routes, and this route is not one. Nothing was started.";

/// Whether this launch is held to free routes (Electron's `freeRoutesOnly`): set by the harness, implied by the switch
/// every drive launches with, and lifted only by `LOCUST_SPEND=1`.
pub fn free_routes_only(argv: &[String], environment: &dyn Fn(&str) -> Option<String>) -> bool {
    if environment("LOCUST_SPEND").as_deref() == Some("1") {
        return false;
    }
    if environment(FREE_ONLY_ENVIRONMENT).as_deref() == Some("1") {
        return true;
    }
    argv.iter().any(|argument| argument.starts_with("--remote-debugging-port"))
}

/// Whether THIS launch is held to free routes: `free_routes_only` over its own arguments and environment, which is
/// what a start asks. A test holds or frees its own thread instead (`hold_free_routes_for_test`), never the process's.
pub fn free_routes_only_now() -> bool {
    #[cfg(any(test, feature = "test-hooks"))]
    if let Some(held) = HELD_FOR_TEST.with(std::cell::Cell::get) {
        return held;
    }
    free_routes_only(&std::env::args().collect::<Vec<_>>(), &|key| std::env::var(key).ok())
}

#[cfg(any(test, feature = "test-hooks"))]
thread_local! {
    static HELD_FOR_TEST: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// Until the returned guard is dropped, `free_routes_only_now` on this thread answers `held` (tests only).
#[cfg(any(test, feature = "test-hooks"))]
pub fn hold_free_routes_for_test(held: bool) -> HeldForTest {
    HELD_FOR_TEST.with(|cell| cell.set(Some(held)));
    HeldForTest(())
}

#[cfg(any(test, feature = "test-hooks"))]
pub struct HeldForTest(());

#[cfg(any(test, feature = "test-hooks"))]
impl Drop for HeldForTest {
    fn drop(&mut self) {
        HELD_FOR_TEST.with(|cell| cell.set(None));
    }
}
