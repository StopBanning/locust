//! This computer's side of keep-awake and start-at-login (Electron's `powerSaveBlocker` and `app.setLoginItemSettings`).
//!
//! Keep awake is Apple's `caffeinate -i -w <this process>`: it holds idle sleep off, and it ends with this process
//! whatever happens, so a crash can never leave the Mac kept awake. Start at login is a launch agent in the person's
//! own LaunchAgents folder that opens the app with `--background`; only a copy of the app from its bundle registers,
//! never a development build. Tests use the core's stand-ins and touch neither.

use locust_core::keep_awake::PowerBlocker;
use locust_core::login_item::{HostLoginItemSettings, LoginItemHost, LoginItemQuery};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct Caffeinate {
    next: u64,
    held: HashMap<u64, std::process::Child>,
}

impl PowerBlocker for Caffeinate {
    fn start(&mut self) -> Result<u64, String> {
        let child = std::process::Command::new("/usr/bin/caffeinate")
            .args(["-i", "-w", &std::process::id().to_string()])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        self.next += 1;
        self.held.insert(self.next, child);
        Ok(self.next)
    }
    fn stop(&mut self, id: u64) {
        if let Some(mut child) = self.held.remove(&id) {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

type Awake = locust_core::keep_awake::KeepAwake<Caffeinate>;
static AWAKE: Mutex<Option<Awake>> = Mutex::new(None);

/// Hold the Mac awake while a turn runs, or always when the person asked to stay awake.
pub fn keep_awake(live_runs: usize, stay_awake: bool) {
    if cfg!(test) {
        return;
    }
    let mut held = AWAKE.lock().unwrap_or_else(|e| e.into_inner());
    held.get_or_insert_with(|| Awake::new(Caffeinate::default())).update(live_runs, stay_awake);
}

/// Locust is quitting: the hold goes with it (caffeinate would end with the process anyway).
pub fn release_awake() {
    if let Some(awake) = AWAKE.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        awake.dispose();
    }
}

pub const LAUNCH_AGENT: &str = "lol.locust.native";

/// A launch agent in the person's own LaunchAgents folder.
pub struct LaunchAgent {
    pub file: PathBuf,
}

impl LaunchAgent {
    pub fn for_user() -> Option<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from)?;
        Some(Self { file: home.join("Library").join("LaunchAgents").join(format!("{LAUNCH_AGENT}.plist")) })
    }
}

fn escape(v: &str) -> String {
    v.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl LoginItemHost for LaunchAgent {
    fn set_login_item_settings(&mut self, open_at_login: bool, query: Option<&LoginItemQuery>) {
        if !open_at_login {
            let _ = std::fs::remove_file(&self.file);
            return;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        let mut args = vec![exe.display().to_string()];
        args.extend(query.map(|q| q.args.clone()).unwrap_or_else(|| vec![locust_core::login_item::BACKGROUND_ARG.to_owned()]));
        let program: String = args.iter().map(|a| format!("<string>{}</string>", escape(a))).collect();
        let plist = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>{LAUNCH_AGENT}</string><key>ProgramArguments</key><array>{program}</array><key>RunAtLoad</key><true/></dict></plist>\n"
        );
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.file, plist);
    }
    fn get_login_item_settings(&self, _query: Option<&LoginItemQuery>) -> HostLoginItemSettings {
        HostLoginItemSettings { open_at_login: self.file.exists(), status: None }
    }
}

/// Whether this is the app from its bundle, the only copy that registers at login.
pub fn packaged() -> bool {
    std::env::current_exe().is_ok_and(|p| p.to_string_lossy().contains(".app/Contents/MacOS/"))
}

/// The login item this window offers: the real one in the app, the stand-in in tests.
pub fn login_item() -> Box<dyn LoginItemHost> {
    if cfg!(test) {
        return Box::new(locust_core::login_item::UnwiredLoginItemHost::default());
    }
    match LaunchAgent::for_user() {
        Some(agent) => Box::new(agent),
        None => Box::new(locust_core::login_item::UnwiredLoginItemHost::default()),
    }
}

/// Node's name for this computer (`process.platform`), which the tray's picture is chosen by.
pub fn node_platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// The icon's pixels for Rune's tray, at the picture's own size: never resized (`trayIcon`).
pub fn tray_icon(image: &crate::menu_bar::TrayImage) -> Result<rune::TrayIcon, String> {
    let rgba = image::load_from_memory_with_format(image.png, image::ImageFormat::Png).map_err(|e| e.to_string())?.to_rgba8();
    Ok(rune::TrayIcon::rgba(rgba.width(), rgba.height(), rgba.into_raw()))
}

/// The icon Rune's tray shows on this platform (`new Tray(trayIcon())` off a Mac): `trayIcon`'s picture, unresized.
pub fn menu_bar_icon(platform: &str) -> Result<rune::TrayIcon, String> {
    tray_icon(&crate::menu_bar::tray_image(platform))
}

/// The menu as Rune's tray draws it: each item by its id, the routine line greyed out.
#[allow(dead_code)]
pub fn tray_items(items: &[crate::menu_bar::MenuItem]) -> Vec<rune::TrayItem> {
    items.iter().map(|item| rune::TrayItem::action(item.id, item.label.clone()).enabled(item.enabled)).collect()
}

/// Rune's tray (Linux and Windows): a click on the icon is Open Locust, a menu pick its item's id.
#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
pub struct RuneTray(rune::Tray);

#[cfg(not(target_os = "macos"))]
impl crate::menu_bar::TrayHost for RuneTray {
    fn set_menu(&mut self, items: &[crate::menu_bar::MenuItem]) {
        self.0.set_menu(tray_items(items));
    }
    fn set_tooltip(&mut self, line: &str) {
        self.0.set_tooltip(line);
    }
}

/// This computer's menu bar icon (`new Tray(trayIcon())`), with no menu until the first `say`. Picks come to
/// `on_pick` as a menu item's id on whatever thread the system answers on; the app hands them to `menu_bar::act`.
/// Nothing calls this yet: the window does not stay open in the background while it is closed.
#[allow(dead_code)]
pub fn menu_bar(
    on_pick: impl Fn(&str) + Send + Sync + 'static,
) -> Result<crate::menu_bar::MenuBar<Box<dyn crate::menu_bar::TrayHost>>, String> {
    #[cfg(target_os = "macos")]
    let host: Box<dyn crate::menu_bar::TrayHost> =
        Box::new(mac_menu_bar::StatusItem::new(&crate::menu_bar::tray_image(node_platform()), on_pick)?);
    #[cfg(not(target_os = "macos"))]
    let host: Box<dyn crate::menu_bar::TrayHost> = {
        let spec = rune::TraySpec::new(LAUNCH_AGENT, "Locust", menu_bar_icon(node_platform())?);
        let tray = rune::Tray::new(spec, move |event| match event {
            rune::TrayEvent::Activate { .. } => on_pick(crate::menu_bar::OPEN),
            rune::TrayEvent::Menu(id) => on_pick(&id),
            _ => {}
        })
        .map_err(|e| e.to_string())?;
        Box::new(RuneTray(tray))
    };
    Ok(crate::menu_bar::MenuBar::new(host))
}

/// The Mac's menu bar icon: an `NSStatusItem` whose image is the template picture with its @2x representation,
/// marked as a template so macOS tints it for a light or dark menu bar. A click opens its menu, as a Mac's do.
#[cfg(target_os = "macos")]
pub(crate) mod mac_menu_bar {
    use crate::menu_bar::{AWAY, DUE, MenuItem, OPEN, QUIT, TrayHost, TrayImage};
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send, sel};
    use objc2_app_kit::NSImage;
    use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSSize, NSString};
    use std::sync::Arc;

    /// A pick's tag is its id's place here.
    const IDS: [&str; 4] = [OPEN, AWAY, DUE, QUIT];

    pub struct Picks(Arc<dyn Fn(&str) + Send + Sync>);

    define_class!(
        #[unsafe(super(NSObject))]
        #[thread_kind = MainThreadOnly]
        #[name = "LocustMenuBarTarget"]
        #[ivars = Picks]
        struct MenuBarTarget;
        unsafe impl NSObjectProtocol for MenuBarTarget {}
        impl MenuBarTarget {
            #[unsafe(method(menuPick:))]
            fn menu_pick(&self, sender: &AnyObject) {
                let tag: isize = unsafe { msg_send![sender, tag] };
                if let Some(id) = usize::try_from(tag).ok().and_then(|i| IDS.get(i)) {
                    (self.ivars().0)(id);
                }
            }
        }
    );

    impl MenuBarTarget {
        fn new(mtm: MainThreadMarker, picks: Picks) -> Retained<Self> {
            let this = Self::alloc(mtm).set_ivars(picks);
            unsafe { msg_send![super(this), init] }
        }
    }

    /// The template picture as one `NSImage`: 16 points, its 1x and @2x representations together, marked as a
    /// template (`image.setTemplateImage(true)`).
    pub fn template_image(image: &TrayImage) -> Option<Retained<NSImage>> {
        let data = NSData::with_bytes(image.png);
        let ns: Option<Retained<NSImage>> = unsafe { msg_send![NSImage::alloc(), initWithData: &*data] };
        let ns = ns?;
        let points = NSSize::new(16.0, 16.0);
        unsafe {
            let _: () = msg_send![&*ns, setSize: points];
            if let Some(retina) = image.retina {
                let data = NSData::with_bytes(retina);
                let rep: Option<Retained<AnyObject>> = msg_send![class!(NSBitmapImageRep), imageRepWithData: &*data];
                if let Some(rep) = rep {
                    let _: () = msg_send![&*rep, setSize: points];
                    let _: () = msg_send![&*ns, addRepresentation: &*rep];
                }
            }
            let _: () = msg_send![&*ns, setTemplate: image.template];
        }
        Some(ns)
    }

    pub struct StatusItem {
        item: Retained<AnyObject>,
        target: Retained<MenuBarTarget>,
    }

    impl StatusItem {
        pub fn new(image: &TrayImage, on_pick: impl Fn(&str) + Send + Sync + 'static) -> Result<Self, String> {
            let mtm = MainThreadMarker::new().ok_or("the menu bar icon is made on the main thread")?;
            let picture = template_image(image).ok_or("the menu bar picture could not be read")?;
            let target = MenuBarTarget::new(mtm, Picks(Arc::new(on_pick)));
            unsafe {
                let bar: Retained<AnyObject> = msg_send![class!(NSStatusBar), systemStatusBar];
                // NSVariableStatusItemLength
                let item: Retained<AnyObject> = msg_send![&*bar, statusItemWithLength: -1.0f64];
                let button: Option<Retained<AnyObject>> = msg_send![&*item, button];
                if let Some(button) = button {
                    let _: () = msg_send![&*button, setImage: &*picture];
                }
                Ok(Self { item, target })
            }
        }
    }

    impl TrayHost for StatusItem {
        fn set_menu(&mut self, items: &[MenuItem]) {
            unsafe {
                let menu: Retained<AnyObject> = msg_send![class!(NSMenu), new];
                let _: () = msg_send![&*menu, setAutoenablesItems: false];
                for item in items {
                    let Some(tag) = IDS.iter().position(|id| *id == item.id) else { continue };
                    let entry: Retained<AnyObject> = msg_send![class!(NSMenuItem), new];
                    let _: () = msg_send![&*entry, setTitle: &*NSString::from_str(&item.label)];
                    let _: () = msg_send![&*entry, setTag: tag as isize];
                    let _: () = msg_send![&*entry, setEnabled: item.enabled];
                    let _: () = msg_send![&*entry, setTarget: &*self.target];
                    let _: () = msg_send![&*entry, setAction: sel!(menuPick:)];
                    let _: () = msg_send![&*menu, addItem: &*entry];
                }
                let _: () = msg_send![&*self.item, setMenu: &*menu];
            }
        }
        fn set_tooltip(&mut self, line: &str) {
            unsafe {
                let button: Option<Retained<AnyObject>> = msg_send![&*self.item, button];
                if let Some(button) = button {
                    let _: () = msg_send![&*button, setToolTip: &*NSString::from_str(line)];
                }
            }
        }
    }

    impl Drop for StatusItem {
        /// `tray.destroy()`: the icon leaves the menu bar.
        fn drop(&mut self) {
            unsafe {
                let bar: Retained<AnyObject> = msg_send![class!(NSStatusBar), systemStatusBar];
                let _: () = msg_send![&*bar, removeStatusItem: &*self.item];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_launch_agent_opens_this_program_in_the_background_and_goes_when_switched_off() {
        let dir = std::env::temp_dir().join(format!("locust-launch-agent-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
        let mut agent = LaunchAgent { file: dir.join(format!("{LAUNCH_AGENT}.plist")) };
        assert!(!agent.get_login_item_settings(None).open_at_login);
        agent.set_login_item_settings(true, None);
        let plist = std::fs::read_to_string(&agent.file).unwrap();
        assert!(plist.contains("<key>RunAtLoad</key><true/>") && plist.contains("<string>--background</string>") && plist.contains(LAUNCH_AGENT));
        assert!(agent.get_login_item_settings(None).open_at_login);
        agent.set_login_item_settings(false, None);
        assert!(!agent.file.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// macOS only: the menu bar's picture is one 16-point NSImage, marked as a template, holding both the 1x and the
    /// @2x representation (Electron's `setTemplateImage(true)` with no resize).
    #[cfg(target_os = "macos")]
    #[test]
    fn on_a_mac_the_menu_bar_image_is_a_16_point_template_with_both_representations() {
        use objc2::msg_send;
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2_foundation::NSSize;
        let image = super::mac_menu_bar::template_image(&crate::menu_bar::tray_image("darwin")).expect("the picture reads");
        let template: bool = unsafe { msg_send![&*image, isTemplate] };
        let size: NSSize = unsafe { msg_send![&*image, size] };
        let reps: Retained<AnyObject> = unsafe { msg_send![&*image, representations] };
        let count: usize = unsafe { msg_send![&*reps, count] };
        assert!(template);
        assert_eq!((size.width, size.height), (16.0, 16.0));
        assert_eq!(count, 2);
    }
}
