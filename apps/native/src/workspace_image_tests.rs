//! `main/an-image-preview-stays-in-its-folder.test.ts`.

use super::workspace_image;
use std::path::PathBuf;

struct Folders {
    root: PathBuf,
    folder: PathBuf,
    copy: PathBuf,
}

impl Drop for Folders {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn folders() -> Folders {
    let root = std::env::temp_dir().join(format!("locust-image-test-{}", crate::composer::new_queue_id())).canonicalize_or_self();
    let folder = root.join("conversation");
    let copy = root.join("compare").join("slot-a");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(&copy).unwrap();
    std::fs::write(folder.join("chart.png"), "folder-image").unwrap();
    std::fs::write(copy.join("chart.png"), "copy-image").unwrap();
    std::fs::write(root.join("private.png"), "outside").unwrap();
    Folders { root, folder, copy }
}

trait CanonicalOrSelf {
    fn canonicalize_or_self(self) -> PathBuf;
}
impl CanonicalOrSelf for PathBuf {
    fn canonicalize_or_self(self) -> PathBuf {
        std::fs::create_dir_all(&self).unwrap();
        self.canonicalize().unwrap_or(self)
    }
}

#[test]
fn reads_relative_and_absolute_images_from_the_conversation() {
    let f = folders();
    let absolute = f.folder.join("chart.png");
    for path in ["chart.png".to_owned(), absolute.to_string_lossy().into_owned()] {
        let found = workspace_image(&path, &f.folder, std::slice::from_ref(&f.folder)).unwrap();
        assert_eq!(found, absolute);
        assert_eq!(std::fs::read(&found).unwrap(), b"folder-image");
    }
}

#[test]
fn refuses_a_path_outside_the_conversation_even_when_another_allowed_root_holds_it() {
    let f = folders();
    let roots = std::slice::from_ref(&f.root);
    assert!(workspace_image("../private.png", &f.folder, roots).is_err());
    assert!(workspace_image(&f.root.join("private.png").to_string_lossy(), &f.folder, roots).is_err());
    assert_eq!(workspace_image("chart.png", &f.root, std::slice::from_ref(&f.folder)), Err("No workspace."));
}

#[test]
fn reads_a_comparison_column_from_its_own_copy_and_never_substitutes_the_workspace() {
    let f = folders();
    let roots = [f.folder.clone(), f.root.join("compare")];
    let found = workspace_image("chart.png", &f.copy, &roots).unwrap();
    assert_eq!(found, f.copy.join("chart.png"));
    assert_eq!(std::fs::read(&found).unwrap(), b"copy-image");
    std::fs::remove_dir_all(&f.copy).unwrap();
    assert!(workspace_image("chart.png", &f.copy, &roots).is_err());
}

#[cfg(unix)]
#[test]
fn refuses_a_link_that_leads_outside_the_conversation() {
    let f = folders();
    std::fs::create_dir_all(f.root.join("outside")).unwrap();
    std::fs::write(f.root.join("outside").join("secret.png"), "secret").unwrap();
    std::os::unix::fs::symlink(f.root.join("outside"), f.folder.join("linked")).unwrap();
    assert_eq!(workspace_image("linked/secret.png", &f.folder, std::slice::from_ref(&f.root)), Err("Outside the workspace."));
}

#[test]
fn draws_no_svg_network_address_missing_file_folder_or_oversized_image() {
    let f = folders();
    std::fs::create_dir_all(f.folder.join("directory.png")).unwrap();
    std::fs::write(f.folder.join("big.png"), vec![0u8; 8 * 1024 * 1024 + 1]).unwrap();
    let roots = std::slice::from_ref(&f.folder);
    for path in ["x.svg", "https://example.com/chart.png", "//server/share/chart.png", "data:image/png", "missing.png", "directory.png", "big.png"] {
        assert!(workspace_image(path, &f.folder, roots).is_err(), "{path}");
    }
    assert_eq!(workspace_image("big.png", &f.folder, roots), Err("Too large to preview."));
    assert_eq!(workspace_image("directory.png", &f.folder, roots), Err("Not a file."));
    assert_eq!(workspace_image("https://example.com/chart.png", &f.folder, roots), Err("Not a local image."));
}
