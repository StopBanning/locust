use super::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

#[derive(Default)]
struct Platform {
    held: BTreeMap<String, Box<dyn Fn() + Send + Sync>>,
    taken: BTreeSet<String>,
}
impl Registration for Arc<Mutex<Platform>> {
    fn register(&mut self, keys: &str, f: Box<dyn Fn() + Send + Sync>) -> bool {
        let mut platform = crate::invariants::lock(self);
        if platform.taken.contains(keys) {
            return false;
        }
        platform.held.insert(keys.into(), f);
        true
    }
    fn unregister(&mut self, keys: &str) {
        crate::invariants::lock(self).held.remove(keys);
    }
}
#[derive(Default, Clone)]
struct Memory(Arc<Mutex<Value>>);
impl Preferences for Memory {
    fn read(&self) -> Value {
        crate::invariants::lock(&self.0).clone()
    }
    fn write(&mut self, keys: Option<&str>) -> Result<(), String> {
        *crate::invariants::lock(&self.0) = keys.map_or(json!(false), |keys| json!(keys));
        Ok(())
    }
}

type Fake = Shortcut<Arc<Mutex<Platform>>, Memory>;
fn shortcut() -> (Fake, Arc<Mutex<Platform>>, Memory, Arc<AtomicUsize>) {
    let platform = Arc::new(Mutex::new(Platform::default()));
    let memory = Memory::default();
    let count = Arc::new(AtomicUsize::new(0));
    let presses = count.clone();
    let shortcut = Shortcut::new(platform.clone(), memory.clone(), move || {
        presses.fetch_add(1, Ordering::Relaxed);
    });
    (shortcut, platform, memory, count)
}
fn held(platform: &Mutex<Platform>) -> Vec<String> {
    crate::invariants::lock(platform)
        .held
        .keys()
        .cloned()
        .collect()
}

// case: are Ctrl or Alt, Shift if wanted, then a letter, digit, F-key or Space
#[test]
fn permissible_keys() {
    for keys in [
        "Ctrl+Alt+Space",
        "Ctrl+T",
        "Alt+Shift+F9",
        "Ctrl+Alt+Shift+7",
        "Alt+F24",
    ] {
        assert!(valid(keys), "{keys}");
    }
    for keys in [
        "Space",
        "Shift+A",
        "Super+Space",
        "Ctrl+Alt",
        "Alt+Ctrl+Space",
        "Ctrl+Ctrl+A",
        "Ctrl+Enter",
        "Ctrl+F25",
        "Ctrl+a",
        "",
        "Ctrl+Alt+Space+X",
    ] {
        assert!(!valid(keys), "{keys}");
    }
}

// case: come from a key press in Ctrl, Alt, Shift order, and not while only modifiers are down
#[test]
fn modifier_order_and_supported_keys() {
    for (code, ctrl, alt, shift, meta, want) in [
        ("Space", true, true, false, false, Some(DEFAULT)),
        ("KeyT", false, true, true, false, Some("Alt+Shift+T")),
        ("Digit5", true, false, false, false, Some("Ctrl+5")),
        ("F9", false, true, false, false, Some("Alt+F9")),
        ("ControlLeft", true, false, false, false, None),
        ("KeyT", false, false, false, false, None),
        ("KeyT", true, false, false, true, None),
        ("Enter", true, false, false, false, None),
    ] {
        assert_eq!(from_press(code, ctrl, alt, shift, meta).as_deref(), want);
    }
}

// case: are Ctrl+Alt+Space until the person chooses others, taken once Talk could start
#[test]
fn default_waits_for_readiness() {
    let (mut shortcut, platform, _, _) = shortcut();
    shortcut.ensure(false);
    assert!(held(&platform).is_empty());
    assert!(
        shortcut
            .status()
            .why
            .unwrap()
            .contains("once Talk is ready")
    );
    shortcut.ensure(true);
    assert_eq!(held(&platform), [DEFAULT]);
    assert!(shortcut.status().working);
}

// case: say so when another app holds them, and are tried again at the next check
#[test]
fn collision_is_visible_and_retried() {
    let (mut shortcut, platform, _, _) = shortcut();
    platform.lock().unwrap().taken.insert(DEFAULT.into());
    shortcut.ensure(true);
    assert!(
        shortcut
            .status()
            .why
            .unwrap()
            .contains("Another app is using")
    );
    platform.lock().unwrap().taken.clear();
    shortcut.ensure(true);
    assert_eq!(held(&platform), [DEFAULT]);
}

// case: change to the person’s own at once, the old ones given back, and stay off once turned off
#[test]
fn changing_the_shortcut_releases_the_previous_keys() {
    let (mut shortcut, platform, memory, _) = shortcut();
    shortcut.ensure(true);
    assert!(shortcut.set(&json!("Alt+Shift+T")).unwrap().working);
    assert_eq!(memory.read(), "Alt+Shift+T");
    assert_eq!(held(&platform), ["Alt+Shift+T"]);
    assert!(!shortcut.set(&json!(false)).unwrap().working);
    shortcut.ensure(true);
    assert!(held(&platform).is_empty());
    assert_eq!(shortcut.set(&json!("Space")).unwrap().keys, None);
    assert_eq!(memory.read(), false);
}

// case: read the kept keys, and the default for anything a person could not have chosen
#[test]
fn reads_a_saved_or_default_shortcut() {
    let (shortcut, _, mut memory, _) = shortcut();
    memory.write(Some("Ctrl+Alt+Shift+F2")).unwrap();
    assert_eq!(shortcut.status().keys.as_deref(), Some("Ctrl+Alt+Shift+F2"));
    memory.write(Some("invalid")).unwrap();
    assert_eq!(shortcut.status().keys.as_deref(), Some(DEFAULT));
}

// case: press Talk through the window when Windows hears them
#[test]
fn dispatches_the_os_callback() {
    let (mut shortcut, platform, _, count) = shortcut();
    shortcut.ensure(true);
    platform.lock().unwrap().held[DEFAULT]();
    assert_eq!(count.load(Ordering::Relaxed), 1);
}

// case: are given back when Locust quits
#[test]
fn drops_the_registration_on_quit() {
    let (mut shortcut, platform, _, _) = shortcut();
    shortcut.ensure(true);
    drop(shortcut);
    assert!(held(&platform).is_empty());
}

// case: are none of the person’s in a test drive’s window, which takes only keys it names
#[test]
fn a_drive_does_not_touch_the_persons_settings_or_registration() {
    let (shortcut, platform, memory, _) = shortcut();
    let reason = "A window a test drive opened leaves your keys alone.";
    let mut drive = shortcut.in_drive(reason.into(), None);
    drive.ensure(true);
    assert_eq!(
        drive.set(&json!(DEFAULT)).unwrap().why.as_deref(),
        Some(reason)
    );
    assert!(held(&platform).is_empty());
    assert_eq!(memory.read(), Value::Null);

    let (shortcut, platform, _, _) = self::shortcut();
    let other = "Ctrl+Alt+Shift+F11";
    let mut drive = shortcut.in_drive(reason.into(), Some(other.into()));
    drive.ensure(true);
    assert_eq!(held(&platform), [other]);
    assert_eq!(
        drive.set(&json!(DEFAULT)).unwrap().keys.as_deref(),
        Some(other)
    );
    assert_eq!(held(&platform), [other]);
}
