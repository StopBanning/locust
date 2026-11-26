//! The person's own commands as the window reads them (App.tsx's `ownCommandMenu` effect): both folders read in a job,
//! and read again when the folder changes. Words and order are Electron's (own-command-host.ts `readTree`).
use super::*;
use crate::store::Store;
use locust_core::own_commands::Source;
use std::path::Path;

pub(super) struct Dir(pub PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A data folder, an open folder in it, and a window state over them with no teammate on screen.
pub(super) fn state() -> (Dir, State) {
    let root = std::env::temp_dir().join(format!(
        "locust-own-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let mut s = State::default();
    s.data = Some(Store::new(root.join("native"), root.join("electron")).unwrap());
    s.home.folder_path = Some(root.join("work").to_string_lossy().into_owned());
    (Dir(root), s)
}

pub(super) fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The window's own poll (main.rs `poll`, which reads the commands) until the read in flight has landed and been taken:
/// a landed answer stops being pending before a poll takes it.
pub(super) fn read_now(s: &mut State) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        crate::poll(s, 0.0, &mut Tx::default());
        if !s.own_commands_job.is_pending() {
            crate::poll(s, 0.0, &mut Tx::default());
            if !s.own_commands_job.is_pending() {
                return;
            }
        }
        assert!(std::time::Instant::now() < deadline, "the commands were never read");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn listed(s: &State) -> Vec<(String, Source)> {
    s.own_commands.listed.iter().map(|c| (c.name.clone(), c.source)).collect()
}

#[test]
fn the_window_reads_your_commands_and_the_folders_and_again_when_the_folder_changes() {
    let (dir, mut s) = state();
    write(&dir.0.join("electron/commands/team/review.md"), "---\ndescription: Looks at the diff\n---\nReview\n");
    write(&dir.0.join("work/.locust/commands/status.md"), "Status\n");
    read_now(&mut s);
    assert_eq!(listed(&s), vec![("team/review".into(), Source::Global), ("status".into(), Source::Project)]);
    assert_eq!(s.own_commands.listed[0].description.as_deref(), Some("Looks at the diff"));
    // Another folder open: its commands, not the last one's.
    write(&dir.0.join("other/.locust/commands/ship.md"), "Ship\n");
    s.home.folder_path = Some(dir.0.join("other").to_string_lossy().into_owned());
    read_now(&mut s);
    assert_eq!(listed(&s), vec![("team/review".into(), Source::Global), ("ship".into(), Source::Project)]);
}
