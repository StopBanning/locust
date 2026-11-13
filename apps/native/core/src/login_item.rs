//! Start Locust when you sign in to Windows, or log in to a Mac (`main/login-item.ts`).
//!
//! Packaged builds only: in development it would register the dev binary.
//! On a Mac, a login item is the app itself (`SMAppService`), with no path or arguments;
//! macOS may hold it until the person allows it in System Settings › Login Items (`requires-approval`).

pub const BACKGROUND_ARG: &str = "--background";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginItemQuery {
    pub path: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostLoginItemSettings {
    pub open_at_login: bool,
    pub status: Option<String>,
}

/// The slice of the OS login-item API this module needs. Injected so tests never touch real login items.
pub trait LoginItemHost {
    fn set_login_item_settings(&mut self, open_at_login: bool, query: Option<&LoginItemQuery>);
    fn get_login_item_settings(&self, query: Option<&LoginItemQuery>) -> HostLoginItemSettings;
}

impl LoginItemHost for Box<dyn LoginItemHost> {
    fn set_login_item_settings(&mut self, open_at_login: bool, query: Option<&LoginItemQuery>) {
        (**self).set_login_item_settings(open_at_login, query);
    }
    fn get_login_item_settings(&self, query: Option<&LoginItemQuery>) -> HostLoginItemSettings {
        (**self).get_login_item_settings(query)
    }
}

/// Runtime stub when OS login-item integration (`SMAppService` / Windows registry) is not wired yet.
#[derive(Default)]
pub struct UnwiredLoginItemHost {
    open_at_login: bool,
}

impl LoginItemHost for UnwiredLoginItemHost {
    fn set_login_item_settings(&mut self, open_at_login: bool, _query: Option<&LoginItemQuery>) {
        self.open_at_login = open_at_login;
    }

    fn get_login_item_settings(&self, _query: Option<&LoginItemQuery>) -> HostLoginItemSettings {
        HostLoginItemSettings {
            open_at_login: self.open_at_login,
            status: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoginItemWhy {
    Development,
    Platform,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoginItemState {
    pub open_at_login: bool,
    pub available: bool,
    pub why: Option<LoginItemWhy>,
    pub approval: bool,
}

impl LoginItemState {
    pub fn note(&self) -> Option<&'static str> {
        if self.approval {
            return Some("Waiting for approval in System Settings › General › Login Items.");
        }
        match self.why {
            Some(LoginItemWhy::Development) => {
                Some("Available only in a packaged build.")
            }
            Some(LoginItemWhy::Platform) => {
                Some("Not available on this platform yet.")
            }
            None => None,
        }
    }
}

pub struct LoginItem<H: LoginItemHost> {
    host: H,
    packaged: bool,
    exec_path: String,
    platform: String,
}

fn registers(platform: &str) -> bool {
    platform == "win32" || platform == "darwin"
}

fn unavailable_for(platform: &str) -> LoginItemState {
    LoginItemState {
        open_at_login: false,
        available: false,
        why: Some(if registers(platform) {
            LoginItemWhy::Development
        } else {
            LoginItemWhy::Platform
        }),
        approval: false,
    }
}

impl<H: LoginItemHost> LoginItem<H> {
    pub fn new(host: H, packaged: bool, exec_path: impl Into<String>, platform: impl Into<String>) -> Self {
        Self {
            host,
            packaged,
            exec_path: exec_path.into(),
            platform: platform.into(),
        }
    }

    pub fn host(&self) -> &H {
        &self.host
    }

    fn is_mac(&self) -> bool {
        self.platform == "darwin"
    }

    fn available(&self) -> bool {
        self.packaged && registers(&self.platform)
    }

    fn query(&self) -> LoginItemQuery {
        LoginItemQuery {
            path: self.exec_path.clone(),
            args: vec![BACKGROUND_ARG.into()],
        }
    }

    pub fn read(&self) -> LoginItemState {
        if !self.available() {
            return unavailable_for(&self.platform);
        }
        if !self.is_mac() {
            let query = self.query();
            let held = self.host.get_login_item_settings(Some(&query));
            return LoginItemState {
                open_at_login: held.open_at_login,
                available: true,
                why: None,
                approval: false,
            };
        }
        let held = self.host.get_login_item_settings(None);
        let status = held.status.as_deref();
        LoginItemState {
            open_at_login: held.open_at_login || status == Some("enabled"),
            available: true,
            why: None,
            approval: status == Some("requires-approval"),
        }
    }

    pub fn set(&mut self, open_at_login: bool) -> LoginItemState {
        if !self.available() {
            return unavailable_for(&self.platform);
        }
        if self.is_mac() {
            self.host.set_login_item_settings(open_at_login, None);
        } else {
            let query = self.query();
            self.host.set_login_item_settings(open_at_login, Some(&query));
        }
        self.read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeHost {
        sets: Vec<(bool, Option<LoginItemQuery>)>,
        held: HostLoginItemSettings,
        expected_query: Option<LoginItemQuery>,
        require_none_query: bool,
    }

    impl LoginItemHost for FakeHost {
        fn set_login_item_settings(&mut self, open_at_login: bool, query: Option<&LoginItemQuery>) {
            self.sets.push((open_at_login, query.cloned()));
            self.held.open_at_login = open_at_login;
            if self.require_none_query {
                self.held.status = Some(if open_at_login {
                    "requires-approval".into()
                } else {
                    "not-registered".into()
                });
            }
        }

        fn get_login_item_settings(&self, query: Option<&LoginItemQuery>) -> HostLoginItemSettings {
            if self.require_none_query {
                assert_eq!(query, None);
            } else if let Some(expected) = &self.expected_query {
                assert_eq!(query, Some(expected));
            }
            self.held.clone()
        }
    }

    #[test]
    fn sets_background_on_packaged_windows_executable_and_reads_back() {
        let fake = FakeHost {
            expected_query: Some(LoginItemQuery {
                path: r"C:\Locust\Locust.exe".into(),
                args: vec![BACKGROUND_ARG.into()],
            }),
            ..Default::default()
        };
        let mut item = LoginItem::new(fake, true, r"C:\Locust\Locust.exe", "win32");
        assert_eq!(
            item.read(),
            LoginItemState {
                open_at_login: false,
                available: true,
                why: None,
                approval: false,
            }
        );
        assert_eq!(
            item.set(true),
            LoginItemState {
                open_at_login: true,
                available: true,
                why: None,
                approval: false,
            }
        );
        assert_eq!(
            item.host().sets,
            vec![(
                true,
                Some(LoginItemQuery {
                    path: r"C:\Locust\Locust.exe".into(),
                    args: vec!["--background".into()],
                })
            )]
        );
        assert!(!item.set(false).open_at_login);
    }

    #[test]
    fn does_not_register_development_build_or_unsupported_platform() {
        let mut win_dev = LoginItem::new(
            FakeHost::default(),
            false,
            r"C:\electron\electron.exe",
            "win32",
        );
        assert_eq!(
            win_dev.set(true),
            LoginItemState {
                open_at_login: false,
                available: false,
                why: Some(LoginItemWhy::Development),
                approval: false,
            }
        );
        assert!(win_dev.host().sets.is_empty());

        let mut linux = LoginItem::new(
            FakeHost::default(),
            true,
            "/opt/Locust/locust",
            "linux",
        );
        assert_eq!(
            linux.set(true),
            LoginItemState {
                open_at_login: false,
                available: false,
                why: Some(LoginItemWhy::Platform),
                approval: false,
            }
        );
        assert!(linux.host().sets.is_empty());

        let mut mac_dev = LoginItem::new(FakeHost::default(), false, "/electron", "darwin");
        assert_eq!(
            mac_dev.set(true),
            LoginItemState {
                open_at_login: false,
                available: false,
                why: Some(LoginItemWhy::Development),
                approval: false,
            }
        );
        assert!(mac_dev.host().sets.is_empty());
    }

    #[test]
    fn registers_app_itself_on_mac_and_reports_requires_approval() {
        let fake = FakeHost {
            require_none_query: true,
            held: HostLoginItemSettings {
                open_at_login: false,
                status: Some("not-registered".into()),
            },
            ..Default::default()
        };
        let mut item = LoginItem::new(
            fake,
            true,
            "/Applications/Locust.app/Contents/MacOS/Locust",
            "darwin",
        );
        assert_eq!(
            item.read(),
            LoginItemState {
                open_at_login: false,
                available: true,
                why: None,
                approval: false,
            }
        );
        assert_eq!(
            item.set(true),
            LoginItemState {
                open_at_login: true,
                available: true,
                why: None,
                approval: true,
            }
        );
        assert_eq!(item.host().sets, vec![(true, None)]);

        // Enabled via status even if open_at_login bool is false.
        let mut item2 = LoginItem::new(
            FakeHost {
                require_none_query: true,
                held: HostLoginItemSettings {
                    open_at_login: false,
                    status: Some("enabled".into()),
                },
                ..Default::default()
            },
            true,
            "/Applications/Locust.app/Contents/MacOS/Locust",
            "darwin",
        );
        assert_eq!(
            item2.read(),
            LoginItemState {
                open_at_login: true,
                available: true,
                why: None,
                approval: false,
            }
        );
        assert!(!item2.set(false).open_at_login);
    }
}
