//! Capability URLs for previews. Only the page's directory is exposed; never the project above it.
use std::{
    collections::HashMap,
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};
use url::Url;
const MAX_BYTES: u64 = 64 * 1024 * 1024;
struct Root {
    path: PathBuf,
    directory: File,
}
#[derive(Default)]
pub struct Pages {
    roots: HashMap<String, Root>,
}
pub struct Response {
    pub status: u16,
    pub body: Vec<u8>,
    pub mime: String,
}
fn refused(status: u16, words: &str) -> Response {
    Response {
        status,
        body: words.as_bytes().to_vec(),
        mime: "text/plain; charset=utf-8".into(),
    }
}
pub fn never_served(path: &Path) -> bool {
    let parts: Vec<_> = path
        .components()
        .filter_map(|c| {
            if let Component::Normal(p) = c {
                Some(p.to_string_lossy().to_lowercase())
            } else {
                None
            }
        })
        .collect();
    parts.iter().enumerate().any(|(i, p)| {
        p.starts_with('.')
            || if i + 1 < parts.len() {
                ["secret", "secrets", "credentials", "private", "keys"].contains(&p.as_str())
            } else {
                p.starts_with("id_rsa")
                    || p.starts_with("id_dsa")
                    || p.starts_with("id_ecdsa")
                    || p.starts_with("id_ed25519")
                    || p.rsplit_once('.').is_some_and(|(stem, ext)| {
                        [
                            "pem", "key", "p12", "pfx", "jks", "keystore", "kdbx", "ppk", "asc",
                            "gpg",
                        ]
                        .contains(&ext)
                            || (["json", "yaml", "yml", "toml", "txt"].contains(&ext)
                                && (["credentials", "secret", "secrets"].contains(&stem)
                                    || stem.starts_with("service-account")))
                    })
            }
    })
}
#[cfg(unix)]
fn open_beneath(dir: &File, path: &Path) -> std::io::Result<File> {
    use std::os::fd::{AsRawFd, FromRawFd};
    let mut held = dir.try_clone()?;
    let parts: Vec<_> = path.components().collect();
    for (index, c) in parts.iter().enumerate() {
        let Component::Normal(name) = c else {
            return Err(std::io::Error::other("Outside the page's folder."));
        };
        use std::os::unix::ffi::OsStrExt;
        let name = std::ffi::CString::new(name.as_bytes())?;
        // Each component is anchored to the previously opened directory; no path-following race.
        let flags = libc::O_RDONLY
            | libc::O_NOFOLLOW
            | libc::O_CLOEXEC
            | libc::O_NONBLOCK
            | if index + 1 < parts.len() {
                libc::O_DIRECTORY
            } else {
                0
            };
        let fd = unsafe { libc::openat(held.as_raw_fd(), name.as_ptr(), flags) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        held = unsafe { File::from_raw_fd(fd) };
    }
    Ok(held)
}
#[cfg(not(unix))]
fn open_beneath(_dir: &File, _path: &Path) -> std::io::Result<File> {
    Err(std::io::Error::other(
        "Page previews are unavailable on this platform.",
    ))
}
impl Pages {
    pub fn url_for(&mut self, path: &Path, workspace: Option<&Path>) -> Result<String, String> {
        if !path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"))
        {
            return Err("Only a web page (.html) opens as a page.".into());
        }
        let real = path
            .canonicalize()
            .map_err(|_| "That page is not there. The teammate named it but did not write it.")?;
        let workspace = workspace
            .and_then(|p| p.canonicalize().ok())
            .ok_or("No conversation folder.")?;
        if !real.starts_with(workspace) {
            return Err("That page is outside the folder your teammates work in, so Locust will not open it.".into());
        }
        let parent = real
            .parent()
            .ok_or("That page is not there. The teammate named it but did not write it.")?;
        let token = if let Some((token, _)) = self.roots.iter().find(|(_, r)| r.path == parent) {
            token.clone()
        } else {
            let token = crate::native::uuid_v4()
                .map_err(|e| e.to_string())?
                .replace('-', "");
            let directory = open_beneath(
                &File::open("/").map_err(|e| e.to_string())?,
                parent.strip_prefix("/").map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            self.roots.insert(
                token.clone(),
                Root {
                    path: parent.to_path_buf(),
                    directory,
                },
            );
            token
        };
        let mut u = Url::parse(&format!("locust-page://{token}/")).map_err(|e| e.to_string())?;
        u.path_segments_mut()
            .map_err(|_| "Not an address.")?
            .push(&real.file_name().ok_or("Not an address.")?.to_string_lossy());
        Ok(u.to_string())
    }
    pub fn read(&self, address: &str, workspace: Option<&Path>) -> Response {
        let Ok(u) = Url::parse(address) else {
            return refused(400, "Not an address.");
        };
        let Some(root) = u.host_str().and_then(|h| self.roots.get(h)) else {
            return refused(404, "Nothing is served here.");
        };
        if u.scheme() != "locust-page"
            || !workspace
                .and_then(|p| p.canonicalize().ok())
                .is_some_and(|p| root.path.starts_with(p))
        {
            return refused(403, "Outside the page's folder.");
        }
        let decoded = match decode(u.path()) {
            Ok(s) => s,
            Err(()) => return refused(400, "Not an address."),
        };
        let inside = Path::new(decoded.trim_start_matches('/'));
        if inside
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return refused(403, "Outside the page's folder.");
        }
        if never_served(inside) {
            return refused(403, "Hidden files and keys are never served to a page.");
        }
        let Ok(mut real) = root.path.join(inside).canonicalize() else {
            return refused(404, "Not found.");
        };
        if !real.starts_with(&root.path) {
            return refused(404, "Not found.");
        }
        if real.is_dir() {
            real = match real.join("index.html").canonicalize() {
                Ok(p) => p,
                Err(_) => return refused(404, "Not found."),
            };
        }
        let Ok(relative) = real.strip_prefix(&root.path) else {
            return refused(404, "Not found.");
        };
        if never_served(relative) {
            return refused(403, "Hidden files and keys are never served to a page.");
        }
        let Ok(file) = open_beneath(&root.directory, relative) else {
            return refused(404, "Not found.");
        };
        let Ok(meta) = file.metadata() else {
            return refused(404, "Not found.");
        };
        if !meta.is_file() {
            return refused(404, "Not found.");
        }
        if meta.len() > MAX_BYTES {
            return refused(413, "Too large to serve here.");
        }
        let mut body = vec![];
        if file.take(MAX_BYTES + 1).read_to_end(&mut body).is_err() {
            return refused(404, "Not found.");
        }
        if body.len() as u64 > MAX_BYTES {
            return refused(413, "Too large to serve here.");
        }
        Response {
            status: 200,
            body,
            mime: mime(&real).into(),
        }
    }
}
fn decode(s: &str) -> Result<String, ()> {
    let mut bytes = vec![];
    let mut source = s.bytes();
    while let Some(b) = source.next() {
        if b == b'%' {
            let a = source
                .next()
                .and_then(|c| (c as char).to_digit(16))
                .ok_or(())?;
            let b = source
                .next()
                .and_then(|c| (c as char).to_digit(16))
                .ok_or(())?;
            bytes.push((16 * a + b) as u8);
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8(bytes).map_err(|_| ())
}
fn mime(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
        .to_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "txt" | "md" | "csv" => "text/plain; charset=utf-8",
        "xml" => "application/xml",
        "wasm" => "application/wasm",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
