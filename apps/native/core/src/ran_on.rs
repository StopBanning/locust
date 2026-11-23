//! The conditions a run happened under: scope, stated positively (`renderer/src/ranOn.ts`).
//!
//! State scope, not absence. A Locust run happens on one machine, in one environment.

/// `win32` -> `Windows`. The platform as a person would name it (`platformName`).
pub fn platform_name(platform: &str) -> String {
    match platform {
        "win32" => "Windows".to_owned(),
        "darwin" => "macOS".to_owned(),
        "linux" => "Linux".to_owned(),
        _ if platform.trim().is_empty() => "this machine".to_owned(),
        other => other.to_owned(),
    }
}

/// The receipt's `Ran on` line (`ranOnLine`): the platform, and the folder only when given one.
pub fn ran_on_line(platform: &str, folder: Option<&str>) -> String {
    let where_ = platform_name(platform);
    match folder.map(str::trim).filter(|f| !f.is_empty()) {
        Some(f) => format!("{where_} \u{b7} {f}"),
        None => where_,
    }
}

/// The last segment of a path, as a folder is spoken about (`folderName`).
pub fn folder_name(path: Option<&str>) -> Option<String> {
    let raw = path?;
    let trimmed = raw.trim_end_matches(['/', '\\']);
    let last = trimmed.rsplit(['/', '\\']).next()?;
    (!last.is_empty()).then(|| last.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- ran-on.test.ts: the conditions a run happened under

    // case: names a platform the way a person does
    #[test]
    fn names_a_platform_the_way_a_person_does() {
        assert_eq!(platform_name("win32"), "Windows");
        assert_eq!(platform_name("darwin"), "macOS");
        assert_eq!(platform_name("linux"), "Linux");
    }

    // case: passes through a platform it does not know, rather than guessing
    #[test]
    fn passes_through_a_platform_it_does_not_know_rather_than_guessing() {
        assert_eq!(platform_name("freebsd"), "freebsd");
    }

    // case: says something true even knowing nothing
    #[test]
    fn says_something_true_even_knowing_nothing() {
        assert_eq!(platform_name(""), "this machine");
        assert_eq!(ran_on_line("", None), "this machine");
    }

    // case: names the folder only when it was given one
    #[test]
    fn names_the_folder_only_when_it_was_given_one() {
        assert_eq!(ran_on_line("win32", None), "Windows");
        assert_eq!(ran_on_line("win32", Some("")), "Windows");
        assert_eq!(ran_on_line("win32", Some("   ")), "Windows");
        assert_eq!(ran_on_line("win32", Some("locust-astra")), "Windows · locust-astra");
    }

    // case: says nothing about what was NOT covered
    #[test]
    fn says_nothing_about_what_was_not_covered() {
        let said = ran_on_line("win32", Some("locust-astra")).to_lowercase();
        for word in ["not ", "never", "untested", "missing", "no ", "without"] {
            assert!(!said.contains(word), "{word}");
        }
    }

    // case: reads a folder off either separator, and off a trailing slash
    #[test]
    fn reads_a_folder_off_either_separator_and_off_a_trailing_slash() {
        assert_eq!(
            folder_name(Some(r"C:\Users\<home>\locust-astra")).as_deref(),
            Some("locust-astra")
        );
        assert_eq!(folder_name(Some("/home/you/proj/")).as_deref(), Some("proj"));
        assert_eq!(folder_name(None), None);
        assert_eq!(folder_name(Some("")), None);
    }
}
