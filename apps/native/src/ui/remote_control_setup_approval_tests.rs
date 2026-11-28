use super::*;
use locust_core::remote_control::Launcher;

// case: An unsupported setup terminal gives manual guidance without starting a process.
#[test]
fn approval_remote_setup_on_an_unsupported_terminal_shows_manual_guidance() {
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let temp = Temp(std::env::temp_dir().join(format!("remote-setup-{}", locust_core::invariants::unique_nanos())));
    let store = crate::store::Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let folder = temp.0.join("workspace");
    std::fs::create_dir_all(&folder).unwrap();
    let launcher = RealLauncher { store: Some(store.clone()) };
    let refusal = launcher.open_setup(&folder.join("not-a-real-agent"), &folder).unwrap_err();
    let expected = "Open Claude Code in a terminal in this folder to complete setup, then enable this switch again.";
    assert_eq!(refusal, expected);
    assert!(!store.attachments_dir().unwrap().exists(), "no setup script directory was created");

    let mut s = State::default();
    s.remote_control = Some(RemoteControlState { phase: Phase::Error, error: Some(refusal), ..Default::default() });
    let mut rt = rune::Runtime::new(s, |s, _| col().children(section(s, &Palette::new(true))), (900., 900.), rune::Env::new(true, 1.));
    rt.frame(0.0);
    let alert = rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("remote-control-problem")).unwrap();
    assert_eq!(alert.role, "alert");
    assert_eq!(alert.text.as_deref(), Some(expected));
    assert!(!rt.state.remote_control.as_ref().unwrap().enabled);
}
