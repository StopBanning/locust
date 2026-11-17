//! Which picture the window may draw (Electron's `main/workspace-image.ts` `readWorkspaceImage`): only one inside the
//! conversation's own folder (a comparison column's copy is its own folder), checked on disk so a link cannot lead
//! out; never a scheme, a data address or a network path; never SVG; never a folder; never past 8 MB. The window reads
//! the file itself (Rune's image cache), so this answers with the path it may read, not the bytes.

use std::path::{Path, PathBuf};

use locust_core::image_files::{MAX_PREVIEW_BYTES, image_media_type};

/// The file `requested` names, when the window may draw it for a conversation in `folder`; `roots` are the folders the
/// app knows conversations work in, and `folder` must be one of them or inside one. Refusals in Electron's words.
pub fn workspace_image(requested: &str, folder: &Path, roots: &[PathBuf]) -> Result<PathBuf, &'static str> {
    if requested.is_empty() {
        return Err("No path.");
    }
    if !folder.is_absolute() || !roots.iter().any(|root| crate::file_menu::contains(root, folder)) {
        return Err("No workspace.");
    }
    if image_media_type(requested).is_none() {
        return Err("Not an image this app draws.");
    }
    // Schemes (including data:) and network paths never become disk requests; a Windows drive letter is a path.
    if locust_core::invariants::regex(r"(?i)^(?:[a-z][a-z\d+.-]*:)|^[\\/]{2}").is_match(requested)
        && !locust_core::invariants::regex(r"(?i)^[a-z]:[\\/]").is_match(requested)
    {
        return Err("Not a local image.");
    }
    let wanted = folder.join(requested);
    if !crate::file_menu::contains(folder, &wanted) {
        return Err("Outside the workspace.");
    }
    let (Ok(real_folder), Ok(real)) = (folder.canonicalize(), wanted.canonicalize()) else {
        return Err("Could not be read.");
    };
    let real_roots: Vec<PathBuf> = roots.iter().filter_map(|root| root.canonicalize().ok()).collect();
    if !real_roots.iter().any(|root| crate::file_menu::contains(root, &real_folder)) || !crate::file_menu::contains(&real_folder, &real) {
        return Err("Outside the workspace.");
    }
    let Ok(measured) = std::fs::metadata(&real) else { return Err("Could not be read.") };
    if !measured.is_file() {
        return Err("Not a file.");
    }
    if measured.len() > MAX_PREVIEW_BYTES {
        return Err("Too large to preview.");
    }
    Ok(real)
}

#[cfg(test)]
#[path = "workspace_image_tests.rs"]
mod tests;
