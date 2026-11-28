use super::*;

#[test]
fn word_and_powerpoint_open_with_structured_headings_and_tables_in_the_real_viewer() {
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let dir = Temp(std::env::temp_dir().join(format!("locust-office-view-{}", locust_core::invariants::unique_nanos())));
    std::fs::create_dir_all(&dir.0).unwrap();
    std::fs::write(dir.0.join("report.docx"), include_bytes!("../../../desktop/test/documents/report-python-docx.docx")).unwrap();
    std::fs::write(dir.0.join("deck.pptx"), include_bytes!("../../../desktop/test/documents/deck-python-pptx.pptx")).unwrap();
    let mut s = State::default();
    crate::scenes::composer_base_scene(&mut s, false);
    s.teammates[s.selected].folder = Some(dir.0.to_string_lossy().into_owned());
    open_file(&mut s, FileTarget { path: "report.docx".into(), start: 0, end: 0 });
    let mut rt = rune::Runtime::new(
        s, |s, _| viewer_on(s, &Palette::new(false), "darwin", &[]).unwrap(),
        (1200.0, 1800.0), rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    let looks = |rt: &rune::Runtime<State>, role: &str, words: &str| rt.elements().iter().any(|e| {
        !e.exiting && e.role == role && e.text.as_deref() == Some(words)
    });
    assert!(looks(&rt, "heading", "Quarterly Report"));
    assert!(rt.find("file-reader/code").is_none());
    rt.update(|s, _| open_file(s, FileTarget { path: "deck.pptx".into(), start: 0, end: 0 }));
    rt.frame(0.05);
    assert!(looks(&rt, "heading", "Slide 1: Launch Plan"));
    assert!(looks(&rt, "cell", "1,200"));
    assert!(rt.find("file-reader/code").is_none());
}
