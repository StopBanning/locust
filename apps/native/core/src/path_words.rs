//! How a path is written in a row (Electron `missionView.ts` `relativePath`, `displayPath`, `cursorMirrorRelative`;
//! `shared/cloud-computer.ts` `cloudRelative`). A row is one line and the file name is the point: inside the folder a
//! path reads relative to it; Locust's own copies (a teammate's tree, a comparison column, a routine, a cloud turn, a
//! skill's run copy) read as the file they are a copy of; outside the folder the place is the information, so it stays
//! whole, or behind an ellipsis when it would not fit.

fn regex(p: &'static str) -> regex::Regex {
    crate::invariants::regex(p)
}

fn normalise(value: &str) -> String {
    regex(r"[\\/]+").replace_all(value, "/").trim_end_matches('/').to_owned()
}

/// A cloud turn's copy, `/workspaces/.locust/t_<id>/...`, read as this folder (`cloudRelative`).
pub fn cloud_relative(path: &str) -> Option<String> {
    let path = path.replace('\\', "/");
    let c = regex(r"^/workspaces/\.locust/t_[a-z0-9]{6,32}(?:/(.*))?$").captures(&path)?;
    Some(c.get(1).map(|m| m.as_str()).filter(|s| !s.is_empty()).unwrap_or(".").to_owned())
}

/// Cursor works in a mirror under `~/.cursor/projects/<flattened folder>/`; a path there is the folder's own.
fn cursor_mirror_relative(full: &str, root: &str) -> Option<String> {
    let c = regex(r"(?i)^(.*)/\.cursor/projects/([^/]+)/(.+)$").captures(full)?;
    let flattened = regex(r"[:\\/]+").replace_all(root, "-").trim_matches('-').to_owned();
    (c[2].to_lowercase() == flattened.to_lowercase()).then(|| c[3].to_owned())
}

fn strip_worktree(path: &str) -> String {
    regex(r"^\.locust/(?:worktrees|compare)/[^/]+/").replace(path, "").into_owned()
}

/// A path relative to the conversation's folder, as a row writes it (`relativePath`).
pub fn relative_path(path: &str, workspace: Option<&str>) -> String {
    let Some(workspace) = workspace.filter(|w| !w.is_empty()) else { return path.to_owned() };
    let root = normalise(workspace);
    let full = normalise(path);
    if root.is_empty() {
        return path.to_owned();
    }
    if let Some(c) = regex(r"(?:^|/)\.locust/compare/cmp_[A-Za-z0-9]+-[abc]/(.+)$").captures(&full) {
        return c[1].to_owned();
    }
    if let Some(c) = regex(r"(?:^|/)\.locust/routines/rt_[A-Za-z0-9]+/(.+)$").captures(&full) {
        return c[1].to_owned();
    }
    if let Some(inside) = cloud_relative(&full) {
        return inside;
    }
    if let Some(c) = regex(r"(?i)(?:^|/)claude-skills/[0-9a-f-]{36}/(project|personal)/skills/(.+)$").captures(&full) {
        return format!("{}.claude/skills/{}", if c[1].eq_ignore_ascii_case("personal") { "~/" } else { "" }, &c[2]);
    }
    if regex(r"(?i)(?:^|/)\.cursor/projects/[^/]+/agent-tools/[^/]+$").is_match(&full) {
        return "Cursor's saved tool result".into();
    }
    if let Some(mirrored) = cursor_mirror_relative(&full, &root) {
        return mirrored;
    }
    // Windows paths are case-insensitive; a correct prefix must match whatever the case.
    if full.to_lowercase() == root.to_lowercase() {
        return root.rsplit('/').next().unwrap_or(path).to_owned();
    }
    let prefix = format!("{}/", root.to_lowercase());
    if !full.to_lowercase().starts_with(&prefix) {
        // Already relative: the tree prefix is still noise. An absolute path elsewhere stays whole.
        let relative = if regex(r"(?i)^([a-z]:)?/").is_match(&full) { full.clone() } else { strip_worktree(&full) };
        return if relative.is_empty() || relative == full { path.to_owned() } else { relative };
    }
    let inside = &full[root.len() + 1..];
    if inside.is_empty() {
        return path.to_owned();
    }
    let stripped = strip_worktree(inside);
    if stripped.is_empty() { inside.to_owned() } else { stripped }
}

/// The text a row shows for a path (`displayPath`): the relative path inside the folder; outside it, an absolute path
/// as its last two parts behind an ellipsis (the row keeps the whole path on hover).
pub fn display_path(path: &str, workspace: Option<&str>) -> String {
    let relative = relative_path(path, workspace);
    if relative != path || !regex(r"(?i)^(?:[a-z]:)?[\\/]").is_match(path) {
        return relative;
    }
    let parts: Vec<&str> = regex(r"[\\/]+").split(path).filter(|p| !p.is_empty()).collect();
    if parts.len() <= 3 { path.to_owned() } else { format!("\u{2026}/{}", parts[parts.len() - 2..].join("/")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Electron's missionView.test.ts, "how a path is written in a row".
    const WS: &str = r"C:\Users\x\projects\streaks";
    fn rel(path: &str) -> String {
        relative_path(path, Some(WS))
    }

    #[test]
    fn drops_the_workspace_because_the_row_is_one_line_and_the_filename_is_the_point() {
        assert_eq!(rel(r"C:\Users\x\projects\streaks\src\streak.js"), "src/streak.js");
    }

    #[test]
    fn matches_case_insensitively_the_way_windows_does() {
        assert_eq!(rel(r"c:\users\x\projects\streaks\src\cli.js"), "src/cli.js");
    }

    #[test]
    fn draws_the_workspace_root_as_the_folder_name_not_its_whole_path() {
        assert_eq!(rel(WS), "streaks");
        assert_eq!(rel(&format!("{WS}\\")), "streaks");
    }

    #[test]
    fn keeps_a_path_outside_the_workspace_whole_because_there_the_location_is_the_information() {
        let outside = r"C:\Users\x\other\thing.js";
        assert_eq!(rel(outside), outside);
    }

    #[test]
    fn changes_nothing_when_the_workspace_is_unknown() {
        let path = r"C:\Users\x\projects\streaks\src\streak.js";
        assert_eq!(relative_path(path, None), path);
        assert_eq!(relative_path(path, Some("")), path);
    }

    #[test]
    fn leaves_a_path_that_is_already_relative_alone() {
        assert_eq!(rel("src/streak.js"), "src/streak.js");
    }

    #[test]
    fn drops_a_teammates_own_branch_tree_too_because_the_tree_is_the_same_project() {
        assert_eq!(rel(r"C:\Users\x\projects\streaks\.locust\worktrees\tm_abc123\src\streak.js"), "src/streak.js");
        assert_eq!(rel(".locust/worktrees/tm_abc123/README.md"), "README.md");
    }

    #[test]
    fn drops_a_comparison_columns_copy_the_same_way() {
        assert_eq!(rel(".locust/compare/cmp_f7da0ffad7e54dd8-a/cart.py"), "cart.py");
        assert_eq!(rel(r"C:\Users\x\projects\streaks\.locust\compare\cmp_1-b\src\streak.js"), "src/streak.js");
    }

    #[test]
    fn drops_a_routines_copy_the_same_way() {
        assert_eq!(rel(r"C:\Users\x\.locust\routines\rt_goal0000\answer.txt"), "answer.txt");
        assert_eq!(rel("/home/x/.locust/routines/rt_abc/docs/plan.md"), "docs/plan.md");
    }

    #[test]
    fn drops_a_comparison_columns_plain_copy_outside_the_folder_too() {
        assert_eq!(rel(r"C:\Users\x\.locust\compare\cmp_ab12-b\site\index.html"), "site/index.html");
        assert_eq!(rel(r"C:\Users\x\.locust\other\index.html"), r"C:\Users\x\.locust\other\index.html");
    }

    #[test]
    fn keeps_the_tree_root_itself_since_there_is_nothing_shorter_that_is_true() {
        assert_eq!(rel(r"C:\Users\x\projects\streaks\.locust\worktrees\tm_abc123"), ".locust/worktrees/tm_abc123");
    }

    // "the text a file row shows for a path (0.596)", and the copies Electron reads as the folder's.
    #[test]
    fn a_path_outside_the_folder_reads_as_its_last_two_parts_and_locusts_copies_as_their_files() {
        assert_eq!(display_path("/Users/x/elsewhere/checkout/src/app.js", Some("/Users/x/projects/streaks")), "\u{2026}/src/app.js");
        assert_eq!(display_path("/Users/x/projects/streaks/src/app.js", Some("/Users/x/projects/streaks")), "src/app.js");
        assert_eq!(display_path("/tmp/a.js", Some("/Users/x/p")), "/tmp/a.js");
        assert_eq!(cloud_relative("/workspaces/.locust/t_abc123/src/a.js").as_deref(), Some("src/a.js"));
        assert_eq!(rel("/Users/x/Library/claude-skills/0123abcd-0123-4567-89ab-0123456789ab/personal/skills/pdf/SKILL.md"), "~/.claude/skills/pdf/SKILL.md");
        assert_eq!(relative_path("/Users/x/.cursor/projects/Users-x-projects-streaks/src/a.js", Some("/Users/x/projects/streaks")), "src/a.js");
    }
}
