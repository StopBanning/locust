//! Electron's `a-path-only-message-is-titled-by-its-file.test.ts`, with Electron's values.
use super::path_only_title;

#[test]
fn is_titled_by_the_file_it_names() {
    assert_eq!(path_only_title("/Users/locust/work/locust-move/handoffs/HANDOFF-menu-bar-icon.md").as_deref(), Some("HANDOFF-menu-bar-icon.md"));
    assert_eq!(path_only_title("  ~/work/now-coding/handoff-mac-tools.md ").as_deref(), Some("handoff-mac-tools.md"));
    assert_eq!(path_only_title("C:\\work\\locust\\docs\\PLAN.md").as_deref(), Some("PLAN.md"));
}

#[test]
fn leaves_any_other_first_line_as_it_was_typed() {
    assert_eq!(path_only_title("Fix /Users/a/b.ts please"), None);
    assert_eq!(path_only_title("continue"), None);
    assert_eq!(path_only_title("/"), None);
    assert_eq!(path_only_title("/README.md"), None);
}
