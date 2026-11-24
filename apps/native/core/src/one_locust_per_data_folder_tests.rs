//! LIVE2 #15: a second Locust on the same data folder took the engine and deleted the door file. Electron keeps one
//! instance per data folder (a second launch focuses the first), and quitting never removes another process's door
//! file.
use super::tests::{echo_door, temp};
use super::*;
use crate::engine_process::{Client, LOCK, SOCKET, Server};

fn file_text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

/// The door the file names still listens (the real door's tools are not asked: only that it is there).
fn listens(connection: &Path) -> bool {
    let (url, _) = read_connection(connection).expect("a connection file");
    let port = url::Url::parse(&url).unwrap().port().unwrap();
    TcpStream::connect_timeout(&std::net::SocketAddr::from(([127, 0, 0, 1], port)), Duration::from_secs(2)).is_ok()
}

fn answers(connection: &Path) -> bool {
    bridge_call(connection, "list_teammates", &json!({}))["content"][0]["text"] != NOT_RUNNING
}

#[test]
fn a_second_launch_on_one_data_folder_finds_the_doors_engine_and_never_starts_its_own() {
    // Short: `engine.sock`'s whole path must fit a Unix socket's name.
    let dir = std::env::temp_dir().join(format!("l15-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    // `--serve-door`: its door is open and it is this folder's engine.
    let (engine, door) = serve_door_in(store.clone(), "/Applications/Locust Native.app/x").unwrap();
    let file = door.connection_path();
    let written = file_text(&file);
    assert!(listens(&file));
    assert!(store.root().join(LOCK).exists());
    // A window launched now finds an engine answering, so `join_or_start` joins it and starts nothing.
    assert!(Client::new(store.root().join(SOCKET)).answers());
    // The engine a second launch would start refuses, and so does a second `--serve-door`.
    assert_eq!(Server::start(store.root()).err().as_deref(), Some("Another Locust engine is already running."));
    assert!(serve_door_in(store.clone(), "/Applications/Locust Native.app/x").is_err());
    // The first one's door is untouched and still answers.
    assert_eq!(file_text(&file), written);
    assert!(listens(&file));
    door.dispose();
    // The engine lets go while its socket is still there to wake it.
    drop(engine);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn quitting_never_deletes_or_replaces_a_door_file_another_live_door_owns() {
    let dir = temp("two-doors");
    let first = echo_door(&dir);
    first.load();
    assert!(first.set_enabled(true).settings.enabled);
    let file = first.connection_path();
    let written = file_text(&file);

    // A second Locust on the same folder, its switch saved on: it opens nothing and says why.
    let second = echo_door(&dir);
    let state = second.load();
    assert_eq!(state.message.as_deref(), Some(ANOTHER_DOOR));
    assert!(!state.settings.enabled);
    assert_eq!(file_text(&file), written, "the live door's file is not removed at load");
    let state = second.set_enabled(true);
    assert_eq!((state.settings.enabled, state.message.as_deref()), (false, Some(ANOTHER_DOOR)));
    assert_eq!(file_text(&file), written, "nor replaced by turning the second one on");
    // The second one quits: the first one's file stays, and its door still answers.
    second.dispose();
    drop(second);
    assert_eq!(file_text(&file), written, "quitting never removes another process's door file");
    assert!(answers(&file));

    // The owner's own quit removes it, and a file left by a door that no longer answers is a dead one's: it goes.
    first.dispose();
    assert!(!file.exists(), "the owner's quit removes its own file");
    drop(first);
    std::fs::write(&file, &written).unwrap();
    let third = echo_door(&dir);
    third.load();
    assert_ne!(file_text(&file), written, "a dead door's token is never reused");
}
