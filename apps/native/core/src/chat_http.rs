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

/// The most a [`PlainHttp`] answer is read to: past it the body is cut, and the caller's own checks refuse it.
const PLAIN_HTTP_MOST_BYTES: u64 = 64 * 1024 * 1024;

/// HTTPS on every platform (ureq with the system's certificates) that hands a redirect back to its caller rather than
/// following it: a caller that allows only some hosts (`skill_library`'s GitHub-only rule, Electron's
/// `redirect: 'manual'`) sees every hop. `NSURLSession` follows a redirect itself before the caller is asked.
pub struct PlainHttp {
    agent: ureq::Agent,
}

impl Default for PlainHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl PlainHttp {
    pub fn new() -> Self {
        Self { agent: ureq::AgentBuilder::new().redirects(0).build() }
    }
}

impl HttpClient for PlainHttp {
    fn send(&self, req: HttpRequest) -> Result<HttpResponse, String> {
        use std::io::Read as _;
        let mut call = self.agent.request(&req.method, &req.url).timeout(std::time::Duration::from_secs(req.timeout_secs.max(1)));
        for (name, value) in &req.headers {
            call = call.set(name, value);
        }
        let answer = match req.body {
            Some(body) => call.send_bytes(&body),
            None => call.call(),
        };
        let answer = match answer {
            Ok(answer) | Err(ureq::Error::Status(_, answer)) => answer,
            Err(ureq::Error::Transport(why)) => return Err(why.to_string()),
        };
        let status = answer.status();
        let headers = answer
            .headers_names()
            .into_iter()
            .filter_map(|name| answer.header(&name).map(|value| (name.clone(), value.to_owned())))
            .collect();
        let mut body = Vec::new();
        answer.into_reader().take(PLAIN_HTTP_MOST_BYTES).read_to_end(&mut body).map_err(|why| why.to_string())?;
        Ok(HttpResponse { status, headers, body })
    }
}

#[cfg(test)]
mod plain_http_tests {
    use super::*;
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// One local server that answers every request with `answer`, counting the requests it was sent.
    fn serve(answer: String) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let asked = Arc::new(AtomicUsize::new(0));
        let counted = asked.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { return };
                counted.fetch_add(1, Ordering::SeqCst);
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(answer.as_bytes());
            }
        });
        (address, asked)
    }

    fn get(url: &str) -> Result<HttpResponse, String> {
        PlainHttp::new().send(HttpRequest { method: "GET".into(), url: url.into(), headers: vec![("User-Agent".into(), "Locust".into())], body: None, timeout_secs: 5 })
    }

    #[test]
    fn hands_a_redirect_back_to_its_caller_and_never_goes_where_it_points() {
        let (elsewhere, reached) = serve("HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nno".into());
        let (first, _) = serve(format!("HTTP/1.1 302 Found\r\nLocation: {elsewhere}/payload\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"));
        let answer = get(&format!("{first}/repos/acme/skills")).expect("an answer");
        assert_eq!(answer.status, 302);
        assert!(answer.headers.iter().any(|(k, v)| k.eq_ignore_ascii_case("location") && v == &format!("{elsewhere}/payload")));
        assert_eq!(reached.load(Ordering::SeqCst), 0, "the redirect's target was never asked");
    }

    /// One local server that records the whole request it is sent (head and body) and answers 200.
    fn hear() -> (String, Arc<std::sync::Mutex<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = format!("http://{}", listener.local_addr().unwrap());
        let heard = Arc::new(std::sync::Mutex::new(String::new()));
        let kept = heard.clone();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else { return };
            stream.set_read_timeout(Some(std::time::Duration::from_millis(400))).unwrap();
            let mut all = Vec::new();
            let mut chunk = [0u8; 4096];
            while let Ok(n) = stream.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                all.extend_from_slice(&chunk[..n]);
            }
            *kept.lock().unwrap() = String::from_utf8_lossy(&all).into_owned();
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
        });
        (address, heard)
    }

    #[test]
    fn sends_its_headers_and_a_posts_body_as_given() {
        let (url, heard) = hear();
        let answer = PlainHttp::new()
            .send(HttpRequest {
                method: "POST".into(),
                url: format!("{url}/graphql"),
                headers: vec![("X-Locust-Probe".into(), "yes".into()), ("Content-Type".into(), "application/json".into())],
                body: Some(br#"{"q":"1"}"#.to_vec()),
                timeout_secs: 5,
            })
            .expect("an answer");
        assert_eq!((answer.status, answer.body.as_slice()), (200, b"ok".as_slice()));
        std::thread::sleep(std::time::Duration::from_millis(600));
        let request = heard.lock().unwrap().clone();
        assert!(request.starts_with("POST /graphql HTTP/1.1"), "{request}");
        assert!(request.to_ascii_lowercase().contains("x-locust-probe: yes"), "{request}");
        assert!(request.ends_with(r#"{"q":"1"}"#), "{request}");
    }

    #[test]
    fn reads_an_answer_and_its_failures_alike() {
        let (ok, _) = serve("HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello".into());
        let answer = get(&ok).expect("an answer");
        assert_eq!((answer.status, answer.body.as_slice()), (200, b"hello".as_slice()));
        let (missing, _) = serve("HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
        assert_eq!(get(&missing).expect("a 404 is an answer").status, 404);
    }
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
