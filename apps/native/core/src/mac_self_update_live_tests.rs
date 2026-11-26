//! The Rust app keeps updating itself (Cutover 2 row 4): `this_mac` with its real seams, the download, the commands
//! that open the disk image and copy its app, the swap started apart from the app, and the quit that lets it run.
//! Electron's `main/mac-self-update.ts` is the model.
//!
//! Nothing here fetches from the network or goes near /Applications. The releases list and the disk image are served
//! by a file server on 127.0.0.1 in this test process; the release's address is Locust's own downloads address, which
//! only this crate's test build sends to that server (`fetch_address`). The app being replaced is a copy of this test
//! binary inside a `Locust.app` in a temp folder, run as the app would run: `this_mac` finds that bundle from its own
//! path. `open`, which the swap script calls to reopen Locust, is a stand-in that writes down what it was asked to open.

use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Set only on the copy of this binary the end-to-end test runs as the app: the releases list's address.
const CHILD_FEED: &str = "LOCUST_TEST_SELF_UPDATE_FEED";
const NEW_VERSION: &str = "999.0.0";

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("self-update-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root.canonicalize().unwrap()
}

fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" }
}

/// A file server on this machine only, for as long as the test runs: each path answers with its bytes, anything else
/// with 404. Returns its address (`http://127.0.0.1:<port>`).
fn serve(files: HashMap<String, Vec<u8>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&chunk[..n]),
                }
            }
            let line = String::from_utf8_lossy(&request).lines().next().unwrap_or_default().to_owned();
            let path = line.split(' ').nth(1).unwrap_or_default().to_owned();
            let (status, body) = match files.get(&path) {
                Some(body) => ("200 OK", body.clone()),
                None => ("404 Not Found", b"not here".to_vec()),
            };
            let head = format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(head.as_bytes());
            let _ = stream.write_all(&body);
        }
    });
    base
}

fn write_executable(path: &Path, body: &str) {
    std::fs::write(path, body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A disk image holding a `Locust.app` whose executable says `marker`, as a release's would.
fn release_image(root: &Path, marker: &str) -> PathBuf {
    let source = root.join("release-source");
    let app = source.join("Locust.app/Contents");
    std::fs::create_dir_all(app.join("MacOS")).unwrap();
    std::fs::write(
        app.join("Info.plist"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><plist version=\"1.0\"><dict>\
         <key>CFBundleIdentifier</key><string>lol.locust.test.self-update</string>\
         <key>CFBundleExecutable</key><string>locust-native</string></dict></plist>",
    )
    .unwrap();
    write_executable(&app.join("MacOS/locust-native"), marker);
    let image = root.join(format!("Locust-{NEW_VERSION}-mac-{}.dmg", arch()));
    let made = Command::new("/usr/bin/hdiutil")
        .args(["create", "-quiet", "-volname", "Locust", "-format", "UDZO", "-ov", "-srcfolder"])
        .arg(&source)
        .arg(&image)
        .output()
        .unwrap();
    assert!(made.status.success(), "{}", String::from_utf8_lossy(&made.stderr));
    image
}

/// The copy of this test binary that runs as the app: it is `this_mac`'s caller, exactly as the window's update service
/// is (updates.rs `this_copys_update_service`). Ignored unless the end-to-end test below starts it.
#[test]
#[ignore = "run only as the app, by the_app_downloads_a_newer_locust_from_its_feed_swaps_it_in_after_it_quits_and_opens_it"]
fn as_the_app() {
    let Ok(feed) = std::env::var(CHILD_FEED) else { return };
    let mut updater = this_mac(&feed, env!("CARGO_PKG_VERSION")).expect("a Locust.app in a folder this copy can write");
    let percents: Arc<Mutex<Vec<f64>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&percents);
    updater.on(
        "download-progress",
        Arc::new(move |payload: Option<EventPayload>| {
            if let Some(percent) = payload.and_then(|p| p.percent) {
                seen.lock().unwrap().push(percent);
            }
        }),
    );
    let found = updater.check_for_updates().expect("the releases list answers").expect("a result");
    println!("found {}", found.update_info.version);
    updater.download_update().expect("downloaded, checked against the release and staged");
    println!("staged, last percent {:?}", percents.lock().unwrap().last());
    updater.quit_and_install(Some(true), Some(true));
    // The app quits as the swap is handed over. A copy still here has not, and the swap would wait on it.
    println!("still running after quit_and_install");
    std::thread::sleep(Duration::from_secs(90));
    std::process::exit(3);
}

#[test]
fn the_app_downloads_a_newer_locust_from_its_feed_swaps_it_in_after_it_quits_and_opens_it() {
    let root = temp_root("live");
    let marker = format!("#!/bin/sh\n# the new Locust {NEW_VERSION}\n");
    let image = release_image(&root, &marker);
    let image_bytes = std::fs::read(&image).unwrap();
    let image_name = image.file_name().unwrap().to_string_lossy().into_owned();

    // The releases list, the shape `newer_mac_release` reads, with the image at Locust's downloads address.
    let releases = serde_json::json!([{
        "tag_name": NEW_VERSION,
        "assets": [{
            "name": image_name,
            "browser_download_url": format!("{}{image_name}", crate::updates::DOWNLOADS),
            "size": image_bytes.len(),
            "digest": format!("sha256:{:x}", Sha256::digest(&image_bytes)),
        }]
    }]);
    let mut files = HashMap::new();
    files.insert("/releases".to_owned(), serde_json::to_vec(&releases).unwrap());
    files.insert(format!("/{image_name}"), image_bytes.clone());
    let base = serve(files);

    // This copy, as the app: the test binary inside a Locust.app, in a folder it can write.
    let apps = root.join("Apps");
    let macos = apps.join("Locust.app/Contents/MacOS");
    std::fs::create_dir_all(&macos).unwrap();
    let running = macos.join("locust-native");
    std::fs::copy(std::env::current_exe().unwrap(), &running).unwrap();

    // `open`, as the swap script calls it, writes down what it was asked to open.
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let opened = root.join("opened.txt");
    write_executable(&bin.join("open"), &format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n", opened.display()));

    let data = root.join("data");
    let mut child = Command::new(&running)
        .args(["mac_self_update::live_tests::as_the_app", "--exact", "--ignored", "--nocapture", "--test-threads=1"])
        .env(CHILD_FEED, format!("{base}/releases"))
        .env(TEST_DOWNLOADS, &base)
        .env("LOCUST_NATIVE_DATA", &data)
        .env("PATH", format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let mut said = String::new();
    let _ = child.stdout.take().unwrap().read_to_string(&mut said);
    let _ = child.stderr.take().unwrap().read_to_string(&mut said);
    let status = status.unwrap_or_else(|| panic!("the app never quit once the swap was handed over: {said}"));
    assert!(said.contains(&format!("found {NEW_VERSION}")), "{said}");
    assert!(said.contains("staged, last percent Some(100.0)"), "{said}");
    assert!(!said.contains("still running"), "{said}");
    assert!(status.success(), "{status:?}: {said}");

    // The download stayed in the app's own data folder, the bytes the release states.
    assert_eq!(std::fs::read(data.join("mac-updates").join(&image_name)).unwrap(), image_bytes);

    // The swap, once the app had gone: the new app in the old one's place, and opened.
    let deadline = Instant::now() + Duration::from_secs(30);
    while !opened.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    let app = apps.join("Locust.app");
    assert_eq!(std::fs::read_to_string(&opened).unwrap_or_default().trim(), app.display().to_string(), "{said}");
    assert_eq!(std::fs::read_to_string(&running).unwrap(), marker);
    assert!(!apps.join(".Locust-update.app").exists());
    assert!(!apps.join("Locust.app.replaced").exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_real_download_writes_every_byte_and_counts_up_to_100() {
    let body: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let mut files = HashMap::new();
    files.insert("/image.dmg".to_owned(), body.clone());
    let base = serve(files);
    let root = temp_root("download");
    let to = root.join("image.dmg");
    let percents = Mutex::new(Vec::new());
    download_release(&format!("{base}/image.dmg"), &to, &|p| percents.lock().unwrap().push(p)).unwrap();
    assert_eq!(std::fs::read(&to).unwrap(), body);
    let percents = percents.into_inner().unwrap();
    assert_eq!(percents.last(), Some(&100));
    assert!(percents.windows(2).all(|w| w[0] < w[1]), "{percents:?}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_download_that_is_not_there_says_what_answered_and_leaves_no_file() {
    let base = serve(HashMap::new());
    let root = temp_root("missing");
    let to = root.join("image.dmg");
    let err = download_release(&format!("{base}/gone.dmg"), &to, &|_| {}).unwrap_err();
    assert_eq!(err, "The download answered 404.");
    assert!(!to.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_real_run_says_how_a_command_ended_and_what_it_said() {
    let ran = run_command("/bin/sh", &["-c", "echo said; echo complained >&2; exit 3"]);
    assert_eq!(ran.code, Some(3));
    assert!(ran.output.contains("said") && ran.output.contains("complained"), "{}", ran.output);
    let ok = run_command("/bin/sh", &["-c", "exit 0"]);
    assert_eq!(ok.code, Some(0));
    let missing = run_command("/nowhere/no-such-command", &[]);
    assert_eq!(missing.code, None);
    assert!(!missing.output.is_empty());
}

#[test]
fn the_swap_runs_apart_from_the_app_with_its_arguments_so_the_app_quitting_does_not_take_it_down() {
    let root = temp_root("swap");
    let script = root.join("swap.sh");
    let said = root.join("said.txt");
    write_executable(
        &script,
        &format!("#!/bin/sh\nprintf '%s\\n' \"$(ps -o pgid= -p $$ | tr -d ' ')\" \"$@\" > '{}.tmp' && mv '{0}.tmp' '{0}'\n", said.display()),
    );
    start_swap(&script, &["4242".to_owned(), "/x/Locust.app".to_owned(), "/x/.Locust-update.app".to_owned()]);
    let deadline = Instant::now() + Duration::from_secs(20);
    while !said.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(50));
    }
    let lines: Vec<String> = std::fs::read_to_string(&said).unwrap().lines().map(str::to_owned).collect();
    let ours = Command::new("/bin/ps").args(["-o", "pgid=", "-p", &std::process::id().to_string()]).output().unwrap();
    let ours = String::from_utf8_lossy(&ours.stdout).trim().to_owned();
    assert_eq!(&lines[1..], ["4242", "/x/Locust.app", "/x/.Locust-update.app"]);
    assert_ne!(lines[0], ours, "the swap runs in its own process group");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_download_that_stops_half_way_says_so_and_leaves_no_half_file() {
    // A server that promises 1000 bytes, sends 10, and hangs up.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut chunk = [0u8; 1024];
            let _ = stream.read(&mut chunk);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nConnection: close\r\n\r\n0123456789");
        }
    });
    let root = temp_root("half");
    let to = root.join("image.dmg");
    let err = download_release(&format!("{base}/image.dmg"), &to, &|_| {}).unwrap_err();
    assert!(!err.is_empty());
    assert!(!to.exists(), "a half-written image was left behind");
    let _ = std::fs::remove_dir_all(&root);
}
