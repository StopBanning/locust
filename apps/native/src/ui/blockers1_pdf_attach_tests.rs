use super::*;

#[test]
fn the_first_pdf_attachment_displays_a_page_without_opening_the_viewer() {
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let dir = Temp(std::env::temp_dir().join(format!("locust-pdf-attach-{}", locust_core::invariants::unique_nanos())));
    std::fs::create_dir_all(&dir.0).unwrap();
    let pdf = dir.0.join("Homework 5.pdf");
    std::fs::write(&pdf, b"fixture whose actual reading was prepared").unwrap();
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(16, 22, image::Rgba([41,55,87,255])))
        .write_to(&mut png, image::ImageFormat::Png).unwrap();
    // Seed the real disk cache, not the window map. This avoids requiring PDFKit in a Linux test.
    locust_core::pdf::readable_pdf_with(&dir.0, "Homework 5.pdf", "Homework 5.pdf", 30, |_| {
        Ok(locust_core::pdf::PdfReading { pages: 2, text: Vec::new(), pictures: vec![png.into_inner()] })
    }).unwrap();
    let mut s = State::default();
    crate::scenes::composer_base_scene(&mut s, false);
    s.teammates[s.selected].folder = Some(dir.0.to_string_lossy().into_owned());
    let (wake, waiting) = std::sync::mpsc::channel();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(move || { let _ = wake.send(()); }))));
    attach_dropped_paths(&mut s, &[pdf]);
    waiting.recv_timeout(Duration::from_secs(10)).expect("attachment should schedule the first page image and wake the window");
    let mut rt = rune::Runtime::new(s, |s, _| crate::thread_view::thread_view(s, &Palette::new(false)), (1200.0, 1000.0), rune::Env::new(false, 1.0));
    rt.frame(0.0);
    assert!(rt.state.file_view.is_none(), "the preview must come from attaching, not opening");
    assert!(rt.find("pdf-card-thumbnail").is_some(), "the attachment should draw its first page");
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("PDF · 2 pages")));
}
