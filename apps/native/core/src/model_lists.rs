//! Which routes a launch may start runs on (Electron `main/free-routes.ts`): a window a test drive opened may start runs
//! on free routes only. Which routes are free is `scout::host::is_free_route`; this is whether the launch is held to them.
//!
//! The rule for every drive of the app is "spend nothing you were not given". Every script that drives the app takes
//! the window over with `--remote-debugging-port` (`agent_updates::may_update_agents` reads the same switch), and nobody
//! using Locust launches it that way; the drive harness also sets `LOCUST_FREE_ONLY=1`. `LOCUST_SPEND=1`, set by the
//! person running a drive that is meant to spend, lifts it. It only ever takes something away.
//!
//! On the shelf: the start gate that refuses a route that is not free with `FREE_ONLY_REFUSAL` when this says so
//! (codex-mission.ts and antigravity-mission.ts in Electron) belongs in `native::send_from`, which this file does not own.

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
