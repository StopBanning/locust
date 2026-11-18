//! Transport traits for chat apps (`HttpClient` and `WsClient`) so tests never touch the network,
//! plus the macOS `NSURLSession` / `NSURLSessionWebSocketTask` implementation via `objc2-foundation`.

use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    pub timeout_secs: u64,
}

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

pub trait HttpClient: Send + Sync {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, String>;
}

pub trait WsConnection: Send + Sync {
    fn send_text(&self, text: &str) -> Result<(), String>;
    fn close(&self, code: u16, reason: &str);
}

pub trait WsCallbacks: Send + Sync {
    fn on_message(&self, text: String);
    fn on_close(&self, code: u16);
}

pub trait WsClient: Send + Sync {
    fn connect(&self, url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String>;
}

#[cfg(target_os = "macos")]
pub mod mac {
    use super::*;
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_foundation::{
        NSData, NSError, NSHTTPURLResponse, NSMutableURLRequest, NSString, NSURL, NSURLResponse, NSURLSession,
        NSURLSessionConfiguration, NSURLSessionWebSocketCloseCode,
        NSURLSessionWebSocketMessage, NSURLSessionWebSocketMessageType, NSURLSessionWebSocketTask,
    };
    use std::sync::{Condvar, Mutex};

    pub struct MacUrlSessionClient {
        session: Retained<NSURLSession>,
    }

    // NSURLSession is thread-safe for creating and running tasks.
    unsafe impl Send for MacUrlSessionClient {}
    unsafe impl Sync for MacUrlSessionClient {}

    impl MacUrlSessionClient {
        pub fn new() -> Self {
            let config = NSURLSessionConfiguration::ephemeralSessionConfiguration();
            let session = NSURLSession::sessionWithConfiguration(&config);
            Self { session }
        }
    }

    impl Default for MacUrlSessionClient {
        fn default() -> Self {
            Self::new()
        }
    }

    impl HttpClient for MacUrlSessionClient {
        fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
            let url_str = NSString::from_str(&req.url);
            let url = NSURL::URLWithString(&url_str).ok_or_else(|| "invalid URL".to_owned())?;
            let mut_req = NSMutableURLRequest::requestWithURL(&url);
            mut_req.setHTTPMethod(&NSString::from_str(&req.method));
            mut_req.setTimeoutInterval(req.timeout_secs as f64);
            for (k, v) in &req.headers {
                mut_req.setValue_forHTTPHeaderField(Some(&NSString::from_str(v)), &NSString::from_str(k));
            }
            if let Some(ref body) = req.body {
                let data = NSData::with_bytes(body);
                mut_req.setHTTPBody(Some(&data));
            }

            let pair = Arc::new((Mutex::new(None::<Result<HttpResponse, String>>), Condvar::new()));
            let pair_cb = pair.clone();

            let block = RcBlock::new(move |data: *mut NSData, resp: *mut NSURLResponse, err: *mut NSError| {
                let result = if !err.is_null() {
                    let e = unsafe { &*err };
                    Err(e.localizedDescription().to_string())
                } else if resp.is_null() {
                    Err("no response".to_owned())
                } else {
                    let r = unsafe { &*resp };
                    let status = r
                        .downcast_ref::<NSHTTPURLResponse>()
                        .map(|h| h.statusCode() as u16)
                        .unwrap_or(0);
                    let mut headers = Vec::new();
                    if let Some(h) = r.downcast_ref::<NSHTTPURLResponse>() {
                        let fields = h.allHeaderFields();
                        let keys = fields.allKeys();
                        for i in 0..keys.count() {
                            let k = keys.objectAtIndex(i);
                            if let Some(k_str) = k.downcast_ref::<NSString>() {
                                if let Some(v) = fields.objectForKey(&*k) {
                                    if let Some(v_str) = v.downcast_ref::<NSString>() {
                                        headers.push((k_str.to_string().to_lowercase(), v_str.to_string()));
                                    }
                                }
                            }
                        }
                    }
                    let body = if !data.is_null() {
                        unsafe { &*data }.to_vec()
                    } else {
                        Vec::new()
                    };
                    Ok(HttpResponse { status, headers, body })
                };
                let (lock, cvar) = &*pair_cb;
                *lock.lock().unwrap() = Some(result);
                cvar.notify_one();
            });

            let task = unsafe { self.session.dataTaskWithRequest_completionHandler(&mut_req, &block) };
            task.resume();

            let (lock, cvar) = &*pair;
            let mut guard = lock.lock().unwrap();
            while guard.is_none() {
                guard = cvar.wait(guard).unwrap();
            }
            guard.take().unwrap()
        }
    }

    struct MacWsConnection {
        task: Retained<NSURLSessionWebSocketTask>,
    }

    unsafe impl Send for MacWsConnection {}
    unsafe impl Sync for MacWsConnection {}

    impl WsConnection for MacWsConnection {
        fn send_text(&self, text: &str) -> Result<(), String> {
            let msg = NSURLSessionWebSocketMessage::initWithString(
                NSURLSessionWebSocketMessage::alloc(),
                &NSString::from_str(text),
            );
            let pair = Arc::new((Mutex::new(None::<Result<(), String>>), Condvar::new()));
            let pair_cb = pair.clone();
            let block = RcBlock::new(move |err: *mut NSError| {
                let res = if err.is_null() {
                    Ok(())
                } else {
                    Err(unsafe { &*err }.localizedDescription().to_string())
                };
                let (lock, cvar) = &*pair_cb;
                *lock.lock().unwrap() = Some(res);
                cvar.notify_one();
            });
            unsafe { self.task.sendMessage_completionHandler(&msg, &block) };
            let (lock, cvar) = &*pair;
            let mut guard = lock.lock().unwrap();
            while guard.is_none() {
                guard = cvar.wait(guard).unwrap();
            }
            guard.take().unwrap()
        }

        fn close(&self, code: u16, reason: &str) {
            let data = if reason.is_empty() {
                None
            } else {
                Some(NSData::with_bytes(reason.as_bytes()))
            };
            let close_code = NSURLSessionWebSocketCloseCode(code as isize);
            self.task.cancelWithCloseCode_reason(close_code, data.as_deref());
        }
    }

    fn receive_loop(task: Retained<NSURLSessionWebSocketTask>, callbacks: Arc<dyn WsCallbacks>) {
        loop {
            let pair = Arc::new((Mutex::new(None::<Result<Option<String>, u16>>), Condvar::new()));
            let pair_cb = pair.clone();
            let task_ref = task.clone();
            let block = RcBlock::new(move |msg: *mut NSURLSessionWebSocketMessage, err: *mut NSError| {
                let res = if !err.is_null() {
                    let code = task_ref.closeCode().0 as u16;
                    Err(if code == 0 { 1006 } else { code })
                } else if msg.is_null() {
                    Err(1006)
                } else {
                    let m = unsafe { &*msg };
                    match m.r#type() {
                        NSURLSessionWebSocketMessageType::String => {
                            Ok(m.string().map(|s| s.to_string()))
                        }
                        NSURLSessionWebSocketMessageType::Data => {
                            Ok(m.data().and_then(|d| String::from_utf8(d.to_vec()).ok()))
                        }
                        _ => Ok(None),
                    }
                };
                let (lock, cvar) = &*pair_cb;
                *lock.lock().unwrap() = Some(res);
                cvar.notify_one();
            });
            unsafe { task.receiveMessageWithCompletionHandler(&block) };
            let (lock, cvar) = &*pair;
            let mut guard = lock.lock().unwrap();
            while guard.is_none() {
                guard = cvar.wait(guard).unwrap();
            }
            match guard.take().unwrap() {
                Ok(Some(text)) => callbacks.on_message(text),
                Ok(None) => {}
                Err(code) => {
                    callbacks.on_close(code);
                    return;
                }
            }
        }
    }

    impl WsClient for MacUrlSessionClient {
        fn connect(&self, url: &str, callbacks: Arc<dyn WsCallbacks>) -> Result<Arc<dyn WsConnection>, String> {
            let ns_url = NSURL::URLWithString(&NSString::from_str(url)).ok_or_else(|| "invalid WebSocket URL".to_owned())?;
            let task = self.session.webSocketTaskWithURL(&ns_url);
            task.resume();
            let conn = Arc::new(MacWsConnection { task: task.clone() });
            std::thread::spawn(move || {
                receive_loop(task, callbacks);
            });
            Ok(conn)
        }
    }
}
