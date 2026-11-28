use super::*;

#[test]
fn a_pdf_card_shows_a_kept_first_page_but_falls_back_when_it_cannot_be_read() {
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let folder = Temp(std::env::temp_dir().join(format!("pdf-card-fallback-{}", locust_core::invariants::unique_nanos())));
    std::fs::create_dir_all(&folder.0).unwrap();
    let mut s = State::default();
    crate::scenes::composer_base_scene(&mut s, false);
    s.teammates[s.selected].folder = Some(folder.0.to_string_lossy().into_owned());
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    s.attached = vec!["Homework 5.pdf".into(), "Unreadable.pdf".into()];
    let homework = folder.0.join("Homework 5.pdf");
    let unreadable = folder.0.join("Unreadable.pdf");
    std::fs::write(&homework, b"original source").unwrap();
    std::fs::write(&unreadable, b"source with no picture").unwrap();
    let png = "page-01.png";
    image::RgbaImage::from_pixel(22, 28, image::Rgba([32, 123, 200, 255])).save(folder.0.join(png)).unwrap();
    keep(&folder.0, "Homework 5.pdf", crate::pdf::ReadablePdf {
        pdf: "Homework 5.pdf".into(), text: None, pictures: vec![png.into()], pages: 2,
    });
    keep(&folder.0, "Unreadable.pdf", crate::pdf::ReadablePdf {
        pdf: "Unreadable.pdf".into(), text: None, pictures: vec!["bad.png".into()], pages: 1,
    });
    let mut rt = rune::Runtime::new(
        s, |s, _| crate::thread_view::thread_view(s, &Palette::new(false)),
        (1200.0, 1000.0), rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    let words: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect();
    assert!(rt.find("pdf-card-thumbnail").is_some(), "the cached first page replaces the badge");
    assert!(words.iter().any(|t| t == "PDF · 2 pages"));
    assert!(words.iter().any(|t| t == "PDF · 1 page"));
    assert_eq!(words.iter().filter(|t| t.as_str() == "PDF").count(), 1, "only the missing picture keeps its badge");
    assert!(!words.iter().any(|t| t.contains("page-01.png") || t.contains("bad.png")));

    // Different size even on file systems with coarse timestamps; a newly
    // replaced file may no longer be a readable PDF at all.
    std::fs::write(&homework, b"replacement is not a readable PDF").unwrap();
    std::fs::remove_file(&unreadable).unwrap();
    rt.update(|_, _| {});
    rt.frame(0.016);
    let words: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect();
    assert!(rt.find("pdf-card-thumbnail").is_none(), "replaced source must not show its former first page");
    assert!(!words.iter().any(|t| t.starts_with("PDF ·")), "{words:?}");
    assert_eq!(words.iter().filter(|t| t.as_str() == "PDF").count(), 4, "two PDF marks and two generic PDF meta labels");
}
