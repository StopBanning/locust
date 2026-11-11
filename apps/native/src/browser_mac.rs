//! Main-thread-only WKWebView child host. Headless scenes never call attach or create a view.
use crate::browser::{Command, Profile, Shared};
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained, runtime::ProtocolObject,
};
use objc2_app_kit::NSView;
use objc2_foundation::{
    NSData, NSDictionary, NSHTTPURLResponse, NSObject, NSObjectProtocol, NSPoint, NSProcessInfo,
    NSRect, NSSize, NSString, NSURL, NSURLRequest, NSUUID,
};
use objc2_web_kit::*;
use sha2::{Digest, Sha256};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

pub struct Ivars {
    host: Shared,
    id: u64,
    preview: bool,
}
define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct BrowserDelegate;
    unsafe impl NSObjectProtocol for BrowserDelegate {}
    unsafe impl WKScriptMessageHandler for BrowserDelegate {
        #[unsafe(method(userContentController:didReceiveScriptMessage:))]
        unsafe fn message(&self, _controller: &WKUserContentController, message: &WKScriptMessage) {
            // Messages are page-controlled diagnostic data, never commands or approval evidence.
            let body = unsafe { message.body() };
            if let Some(body) = body.downcast_ref::<NSString>() {
                let body = body.to_string();
                if body.len() > 32768 {
                    return;
                }
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) {
                    let mut host = crate::invariants::lock(&self.ivars().host);
                    if let Some(tab) = host.tabs.iter_mut().find(|t| t.id == self.ivars().id) {
                        if value["kind"] == "console" {
                            crate::browser::bounded_log(&mut tab.console, value);
                        } else if value["kind"] == "network" {
                            crate::browser::bounded_log(&mut tab.network, value);
                        }
                    }
                }
            }
        }
    }
    unsafe impl WKNavigationDelegate for BrowserDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        unsafe fn policy(
            &self,
            _view: &WKWebView,
            action: &WKNavigationAction,
            decision: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            let url = unsafe { action.request().URL() }
                .and_then(|u| u.absoluteString())
                .map(|s| s.to_string())
                .unwrap_or_default();
            // Electron guards the page's own navigation with will-navigate / will-redirect, which fire for the main
            // frame only, and lets about:blank through. Frames inside a page (about:blank, srcdoc, blob: workers,
            // embeds) are the site's own business: blocking them broke ordinary sites.
            let main_frame = unsafe { action.targetFrame() }.is_none_or(|f| unsafe { f.isMainFrame() });
            if !crate::browser::navigation_is_checked(main_frame, &url, self.ivars().preview) {
                decision.call((WKNavigationActionPolicy::Allow,));
                return;
            }
            let parsed = url::Url::parse(&url).ok();
            let mut host = crate::invariants::lock(&self.ivars().host);
            let tool_allowed = parsed.as_ref().is_some_and(|u| {
                host.tabs
                    .iter()
                    .find(|t| t.id == self.ivars().id)
                    .is_some_and(|t| {
                        !t.tool_control
                            || crate::browser::loopback(u)
                            || t.tool_origins.contains(&crate::browser::origin(u))
                    })
            });
            let allowed = tool_allowed
                && host.refusal.is_none()
                && parsed.as_ref().is_some_and(|u| {
                    if self.ivars().preview {
                        u.scheme() == "locust-page"
                            && host
                                .tabs
                                .iter()
                                .find(|t| t.id == self.ivars().id)
                                .and_then(|t| url::Url::parse(&t.url).ok())
                                .is_some_and(|old| old.host_str() == u.host_str())
                    } else {
                        matches!(u.scheme(), "https" | "http")
                    }
                });
            if !allowed {
                host.error = Some(
                    "The browser navigation was refused. Open an allowed address to continue."
                        .into(),
                );
            }
            drop(host);
            decision.call((if allowed {
                WKNavigationActionPolicy::Allow
            } else {
                WKNavigationActionPolicy::Cancel
            },));
        }
    }
    unsafe impl WKUIDelegate for BrowserDelegate {
        /// A link that opens a new window (target=_blank, window.open): Electron's setWindowOpenHandler opens an
        /// allowed address in the same tab and refuses the rest; no second window is ever made.
        #[unsafe(method_id(webView:createWebViewWithConfiguration:forNavigationAction:windowFeatures:))]
        unsafe fn new_window(
            &self,
            _view: &WKWebView,
            _config: &WKWebViewConfiguration,
            action: &WKNavigationAction,
            _features: &WKWindowFeatures,
        ) -> Option<Retained<WKWebView>> {
            let url = unsafe { action.request().URL() }
                .and_then(|u| u.absoluteString())
                .map(|s| s.to_string())
                .unwrap_or_default();
            let mut host = crate::invariants::lock(&self.ivars().host);
            let workspace = host.workspace.clone();
            match crate::browser::resolve(&url, workspace.as_deref()) {
                Ok(allowed) => {
                    if let Some(tab) = host.tabs.iter_mut().find(|t| t.id == self.ivars().id) {
                        tab.commands.push_back(Command::Load(allowed.to_string()));
                    }
                }
                Err(message) => host.error = Some(message),
            }
            None
        }
        #[unsafe(method(webView:requestMediaCapturePermissionForOrigin:initiatedByFrame:type:decisionHandler:))]
        unsafe fn media(
            &self,
            _v: &WKWebView,
            _o: &WKSecurityOrigin,
            _f: &WKFrameInfo,
            _t: WKMediaCaptureType,
            reply: &block2::DynBlock<dyn Fn(WKPermissionDecision)>,
        ) {
            reply.call((WKPermissionDecision::Deny,));
        }
        #[unsafe(method(webView:requestDeviceOrientationAndMotionPermissionForOrigin:initiatedByFrame:decisionHandler:))]
        unsafe fn motion(
            &self,
            _v: &WKWebView,
            _o: &WKSecurityOrigin,
            _f: &WKFrameInfo,
            reply: &block2::DynBlock<dyn Fn(WKPermissionDecision)>,
        ) {
            reply.call((WKPermissionDecision::Deny,));
        }
    }
    unsafe impl WKURLSchemeHandler for BrowserDelegate {
        #[unsafe(method(webView:startURLSchemeTask:))]
        unsafe fn start(&self, _view: &WKWebView, task: &ProtocolObject<dyn WKURLSchemeTask>) {
            // Synchronous task: no callbacks remain after stopURLSchemeTask.
            let Some(url) = (unsafe { task.request().URL() }) else {
                return;
            };
            let response = {
                let h = crate::invariants::lock(&self.ivars().host);
                h.preview.read(
                    &url.absoluteString()
                        .map(|s| s.to_string())
                        .unwrap_or_default(),
                    h.workspace.as_deref(),
                )
            };
            let keys = [
                NSString::from_str("Content-Type"),
                NSString::from_str("Cache-Control"),
                NSString::from_str("Access-Control-Allow-Origin"),
            ];
            let values = [
                NSString::from_str(&response.mime),
                NSString::from_str("no-store"),
                NSString::from_str("*"),
            ];
            let headers = NSDictionary::from_slices(
                &keys.iter().map(|s| &**s).collect::<Vec<_>>(),
                &values.iter().map(|s| &**s).collect::<Vec<_>>(),
            );
            let Some(native) = NSHTTPURLResponse::initWithURL_statusCode_HTTPVersion_headerFields(
                NSHTTPURLResponse::alloc(),
                &url,
                response.status as isize,
                Some(&NSString::from_str("HTTP/1.1")),
                Some(&headers),
            ) else {
                unsafe {
                    task.didFailWithError(
                        &objc2_foundation::NSError::errorWithDomain_code_userInfo(
                            &NSString::from_str("LocustPreview"),
                            1,
                            None,
                        ),
                    );
                }
                return;
            };
            unsafe {
                task.didReceiveResponse(&native);
                task.didReceiveData(&NSData::with_bytes(&response.body));
                task.didFinish();
            }
        }
        #[unsafe(method(webView:stopURLSchemeTask:))]
        unsafe fn stop(&self, _view: &WKWebView, _task: &ProtocolObject<dyn WKURLSchemeTask>) {}
    }
);
impl BrowserDelegate {
    fn new(mtm: MainThreadMarker, host: Shared, id: u64, preview: bool) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { host, id, preview });
        unsafe { msg_send![super(this), init] }
    }
}
struct Page {
    view: Retained<WKWebView>,
    delegate: Retained<BrowserDelegate>,
    preview: bool,
    ready: Rc<Cell<bool>>,
}
#[derive(Default)]
struct Native {
    window: Option<rune::WindowHandle>,
    pages: HashMap<u64, Page>,
    stores: HashMap<Profile, Retained<WKWebsiteDataStore>>,
    pending: Rc<RefCell<HashMap<Profile, Retained<NSUUID>>>>,
}
thread_local! {static NATIVE:RefCell<Native>=RefCell::new(Native::default());}
pub fn is_attached() -> bool {
    NATIVE.with(|n| n.borrow().window.is_some())
}
pub fn attach(window: &rune::WindowHandle) {
    NATIVE.with(|n| {
        let mut n = n.borrow_mut();
        n.pages.clear();
        n.window = Some(window.clone());
    });
}
pub fn dispose() {
    NATIVE.with(|n| {
        let mut n = n.borrow_mut();
        for p in n.pages.values() {
            unsafe {
                p.view.stopLoading();
                p.view.removeFromSuperview();
            }
        }
        n.pages.clear();
        n.stores.clear();
        n.window = None;
    });
}
pub fn persistent_available() -> bool {
    NSProcessInfo::processInfo()
        .operatingSystemVersion()
        .majorVersion
        >= 14
}
fn profile_uuid(profile: &Profile, namespace: &str) -> Result<Retained<NSUUID>, String> {
    let hash = Sha256::digest(format!(
        "locust-native-browser-v1:{namespace}:{}",
        profile.0.as_deref().unwrap_or("shared")
    ));
    let bytes: [u8; 16] = hash[..16]
        .try_into()
        .map_err(|_| "Invalid profile identifier.")?;
    let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    NSUUID::from_string(&NSString::from_str(&format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )))
    .ok_or_else(|| "Invalid profile identifier.".to_owned())
}
fn store(
    n: &mut Native,
    profile: &Profile,
    namespace: &str,
    mtm: MainThreadMarker,
) -> Result<Retained<WKWebsiteDataStore>, String> {
    if let Some(s) = n.stores.get(profile) {
        return Ok(s.clone());
    }
    let s = unsafe {
        if persistent_available() && !namespace.is_empty() {
            let uuid = profile_uuid(profile, namespace)?;
            WKWebsiteDataStore::dataStoreForIdentifier(&uuid, mtm)
        } else {
            WKWebsiteDataStore::nonPersistentDataStore(mtm)
        }
    };
    n.stores.insert(profile.clone(), s.clone());
    Ok(s)
}
fn page(
    n: &mut Native,
    shared: &Shared,
    id: u64,
    profile: &Profile,
    preview: bool,
    mtm: MainThreadMarker,
) -> Result<Page, String> {
    let delegate = BrowserDelegate::new(mtm, shared.clone(), id, preview);
    let config = unsafe { WKWebViewConfiguration::new(mtm) };
    unsafe {
        let data_store = if preview {
            WKWebsiteDataStore::nonPersistentDataStore(mtm)
        } else {
            store(n, profile, &crate::invariants::lock(shared).namespace, mtm)?
        };
        config.setWebsiteDataStore(&data_store);
        let controller = config.userContentController();
        controller.addScriptMessageHandler_name(
            ProtocolObject::from_ref(&*delegate),
            &NSString::from_str("locustLog"),
        );
        let script = WKUserScript::initWithSource_injectionTime_forMainFrameOnly(
            WKUserScript::alloc(mtm),
            &NSString::from_str(include_str!("browser_capture.js")),
            WKUserScriptInjectionTime::AtDocumentStart,
            false,
        );
        controller.addUserScript(&script);
        config.setURLSchemeHandler_forURLScheme(
            Some(ProtocolObject::from_ref(&*delegate)),
            &NSString::from_str("locust-page"),
        );
        let view = WKWebView::initWithFrame_configuration(
            WKWebView::alloc(mtm),
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(1., 1.)),
            &config,
        );
        view.setNavigationDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        view.setUIDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let ready = Rc::new(Cell::new(!preview));
        if preview {
            // Fail closed: block every network resource, including subframes and sockets.
            // CDN exceptions require a request-method-aware transport; do not weaken to host-only rules.
            let controller = controller.clone();
            let ready2 = ready.clone();
            let host = shared.clone();
            let block = block2::RcBlock::new(
                move |rule: *mut WKContentRuleList, error: *mut objc2_foundation::NSError| {
                    if error.is_null() {
                        if let Some(rule) = rule.as_ref() {
                            controller.addContentRuleList(rule);
                            ready2.set(true);
                        }
                    } else {
                        crate::invariants::lock(&host).error=Some("Local preview isolation could not be installed. The page was not loaded.".into());
                    }
                },
            );
            WKContentRuleListStore::defaultStore(mtm).ok_or("WebKit preview isolation is unavailable.")?.compileContentRuleListForIdentifier_encodedContentRuleList_completionHandler(Some(&NSString::from_str("locust-native-preview-v1")),Some(&NSString::from_str(r#"[{"trigger":{"url-filter":"^(https?|wss?|ftp)://"},"action":{"type":"block"}}]"#)),Some(&block));
            view.setHidden(true);
        }
        Ok(Page {
            view,
            delegate,
            preview,
            ready,
        })
    }
}
pub fn sync(shared: &Shared) -> Result<bool, String> {
    let Some(mtm) = MainThreadMarker::new() else {
        return Err("Browser views require the UI thread.".into());
    };
    NATIVE.with(|native|{
        let mut n=native.borrow_mut();let Some(window)=n.window.clone()else{return Ok(false);};
        let forgotten=std::mem::take(&mut crate::invariants::lock(shared).forgotten);
        for profile in forgotten {
            n.pages.retain(|id,p|{let keep=crate::invariants::lock(shared).tabs.iter().any(|t|t.id==*id);if !keep{unsafe{p.view.stopLoading();p.view.removeFromSuperview();}}keep});
            let store=n.stores.remove(&profile);drop(store);
            let namespace=crate::invariants::lock(shared).namespace.clone();
            if persistent_available()&&!namespace.is_empty(){
                let uuid=profile_uuid(&profile,&namespace)?;
                n.pending.borrow_mut().insert(profile.clone(),uuid.clone());
                let host=shared.clone();let pending=n.pending.clone();
                let callback=block2::RcBlock::new(move|err:*mut objc2_foundation::NSError|{
                    let mut h=crate::invariants::lock(&host);
                    if err.is_null(){h.clearing.remove(&profile);pending.borrow_mut().remove(&profile);}else{h.error=Some("Browser sign-ins could not be cleared. Try forgetting them again.".into());}
                });
                unsafe {WKWebsiteDataStore::removeDataStoreForIdentifier_completionHandler(&uuid,&callback,mtm);}
            }else{crate::invariants::lock(shared).clearing.remove(&profile);}

        }
        let ids={let host=crate::invariants::lock(shared);host.tabs.iter().map(|t|t.id).collect::<Vec<_>>()};
        n.pages.retain(|id,page|{if ids.contains(id){true}else{unsafe {page.view.stopLoading();page.view.removeFromSuperview();}false}});
        for id in ids {
            let (profile,commands)={let mut h=crate::invariants::lock(shared);let Some(t)=h.tabs.iter_mut().find(|t|t.id==id)else{continue;};{let mut commands=std::mem::take(&mut t.commands);if commands.is_empty()&&!n.pages.contains_key(&id)&&!t.url.is_empty(){commands.push_back(Command::Load(t.url.clone()));}(t.profile.clone(),commands)}};
            for command in commands {
                let is_preview=match &command {Command::Load(url)=>url.starts_with("locust-page:"),_=>n.pages.get(&id).is_some_and(|p|p.preview)};
                if n.pages.get(&id).is_some_and(|p|p.preview!=is_preview)&& let Some(p)=n.pages.remove(&id){unsafe {p.view.stopLoading();p.view.removeFromSuperview();}}
                if !n.pages.contains_key(&id){let p=page(&mut n,shared,id,&profile,is_preview,mtm)?;n.pages.insert(id,p);}
                let Some(p)=n.pages.get(&id)else{continue;};
                unsafe {
                    match command {
                        Command::Load(url) => {
                            if !p.ready.get() {
                                if let Some(t) = crate::invariants::lock(shared)
                                    .tabs
                                    .iter_mut()
                                    .find(|t| t.id == id)
                                {
                                    t.commands.push_back(Command::Load(url));
                                }
                                continue;
                            }
                            if let Some(t) = crate::invariants::lock(shared)
                                .tabs
                                .iter_mut()
                                .find(|t| t.id == id)
                            {
                                t.url = url.clone();
                            }
                            let u = NSURL::URLWithString(&NSString::from_str(&url))
                                .ok_or("Not an address.")?;
                            p.view
                                .loadRequest(&NSURLRequest::requestWithURL(&u));
                        }
                        Command::Back => {
                            p.view.goBack();
                        }
                        Command::Forward => {
                            p.view.goForward();
                        }
                        Command::Reload => {
                            p.view.reload();
                        }
                        Command::Stop => p.view.stopLoading(),
                        Command::Evaluate {
                            script,
                            expected_url,
                            cancel,
                            reply,
                        } => {
                            let current = p
                                .view
                                .URL()
                                .and_then(|u| u.absoluteString())
                                .map(|s| s.to_string())
                                .unwrap_or_default();
                            if current != expected_url
                                || cancel.load(std::sync::atomic::Ordering::Relaxed)
                            {
                                let _ = reply.send(Err(
                                    "The page changed or the turn ended. Nothing was changed."
                                        .into(),
                                ));
                                continue;
                            }
                            let callback = block2::RcBlock::new(
                                move |result: *mut objc2::runtime::AnyObject,
                                      error: *mut objc2_foundation::NSError| {
                                    let answer = if !error.is_null() {
                                        Err(
                                            "The browser action did not complete. Nothing was retried."
                                                .into(),
                                        )
                                    } else {
                                        result
                                            .as_ref()
                                            .and_then(|v| v.downcast_ref::<NSString>())
                                            .ok_or_else(|| {
                                                "The page did not return a browser result."
                                                    .to_owned()
                                            })
                                            .and_then(|s| {
                                                serde_json::from_str(&s.to_string())
                                                    .map_err(|e| e.to_string())
                                            })
                                    };
                                    let _ = reply.send(answer);
                                },
                            );
                            p.view
                                .evaluateJavaScript_inFrame_inContentWorld_completionHandler(
                                    &NSString::from_str(&script),
                                    None,
                                    &WKContentWorld::worldWithName(
                                        &NSString::from_str("locust-native-tools"),
                                        mtm,
                                    ),
                                    Some(&callback),
                                );
                        }
                    }
                }
            }
        }
        let mut changed=false;
        window.with_raw_handle(|handle|{
            let rune::raw_window_handle::RawWindowHandle::AppKit(handle)=handle.as_raw()else{return;};
            // Borrow parent only inside with_raw_handle, as Rune requires.
            let parent=unsafe {&*handle.ns_view.as_ptr().cast::<NSView>()};
            let mut h=crate::invariants::lock(shared);
            for (id,p) in &n.pages {unsafe {
                let visible=p.ready.get()&&!h.occluded&&h.open&&h.refusal.is_none()&&h.active==Some(*id)&&h.rect[2]>0.&&h.rect[3]>0.;
                if p.view.superview().is_none(){parent.addSubview(&p.view);}
                let [x,y,w,height]=h.rect;
                let y=if parent.isFlipped(){y as f64}else{parent.bounds().size.height-y as f64-height as f64};
                p.view.setFrame(NSRect::new(NSPoint::new(x as f64,y),NSSize::new(w.max(0.)as f64,height.max(0.)as f64)));
                if !visible&& let Some(window)=parent.window()&& window.firstResponder().and_then(|r|r.downcast::<NSView>().ok()).is_some_and(|v|v.isDescendantOf(&p.view)){window.makeFirstResponder(Some(parent));}
                p.view.setHidden(!visible);
                if let Some(t)=h.tabs.iter_mut().find(|t|t.id==*id){
                    let url=p.view.URL().and_then(|u|u.absoluteString()).map(|s|s.to_string()).unwrap_or_default();let title=p.view.title().map(|t|t.to_string()).unwrap_or_else(||"New tab".into());
                    let loading=p.view.isLoading();let back=p.view.canGoBack();let forward=p.view.canGoForward();
                    if !url.is_empty()&&t.url!=url {t.url=url;changed=true;}
                    if (t.title.as_str(),t.loading,t.back,t.forward)!=(title.as_str(),loading,back,forward){t.title=title;t.loading=loading;t.back=back;t.forward=forward;changed=true;}
                }
                let _keep_delegate_alive=&p.delegate;
            }}
        })?;
        Ok(changed)
    })
}
