//! Which attached files are images, and what to call them (Electron's `shared/image-files.ts`).
//!
//! By EXTENSION, never by sniffing the bytes: the question is "should this be drawn as a picture", and a file called
//! `notes.txt` that happens to begin with PNG bytes should be drawn as neither. The list is deliberately short, every
//! entry a format a window draws without help, because a thumbnail that silently fails to paint is worse than a file
//! row that never claimed to be a picture.
//!
//! SVG is deliberately absent. It is a document that can carry script and fetch remote content, and this window would
//! be drawing one chosen from anywhere on the machine. It still attaches and still reaches the runtime as a file; it
//! just is not painted.

const IMAGE_TYPES: &[(&str, &str)] = &[
    (".png", "image/png"),
    (".jpg", "image/jpeg"),
    (".jpeg", "image/jpeg"),
    (".gif", "image/gif"),
    (".webp", "image/webp"),
    (".bmp", "image/bmp"),
    (".avif", "image/avif"),
    (".ico", "image/x-icon"),
];

/// The media type for a path, or None when it is not an image drawn here. Case-insensitive, because `SHOT.PNG` off a
/// camera or a Windows share is the same file as `shot.png`. The extension is whatever follows the LAST dot anywhere
/// in the path (a dot in a folder name counts, as in Electron: `dir.png/file` has the extension `.png/file`).
pub fn image_media_type(path: &str) -> Option<&'static str> {
    let dot = path.rfind('.')?;
    let extension = path[dot..].to_lowercase();
    IMAGE_TYPES.iter().find(|(ext, _)| *ext == extension).map(|(_, media)| *media)
}

/// Whether a path should be drawn as a picture rather than named as a file.
pub fn is_image_path(path: &str) -> bool {
    image_media_type(path).is_some()
}

/// The largest image handed to the window, in bytes. A file past it still attaches and still reaches the runtime; it is
/// simply named rather than drawn.
pub const MAX_PREVIEW_BYTES: u64 = 8 * 1024 * 1024;

#[cfg(test)]
#[path = "image_files_tests.rs"]
mod tests;
