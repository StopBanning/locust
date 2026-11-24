use super::*;
use std::path::PathBuf;

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-voice-asset-{}",
            crate::invariants::unique_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn path(&self) -> PathBuf {
        self.0.join("model")
    }
    fn partial(&self) -> PathBuf {
        self.0.join("model.part")
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// case: Accurate uses the pinned base model and refuses a wrong cached/downloaded file
#[test]
fn the_accurate_pin_and_sha256_are_enforced() {
    let folder = Folder::new();
    fs::write(folder.path(), b"wrong cached model").unwrap();
    assert_eq!(ACCURATE.bytes, 59_721_011);
    assert!(
        ACCURATE
            .url
            .contains("/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en-q5_1.bin")
    );
    let download_zeros =
        |_: &str| -> io::Result<Box<dyn Read>> { Ok(Box::new(io::repeat(0).take(ACCURATE.bytes))) };
    let err = download(
        ACCURATE,
        &folder.path(),
        &AtomicBool::new(false),
        download_zeros,
        |_| {},
    )
    .unwrap_err();
    assert!(err.to_string().contains("SHA-256"));
    assert!(!folder.path().exists());
    assert!(!folder.partial().exists());
}
