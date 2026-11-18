//! The phone's plain-HTTP listener on the home network (Electron `main/phone-host.ts`'s `createServer`, with Node's
//! `requestTimeout` 30 s, `headersTimeout` 10 s). `std::net` only, no new crate.
//!
//! The framing is written over any `Read + Write`, so tests drive it with an in-memory stream and never bind a port;
//! [`TcpPhoneListener`] is the one place a socket is opened. The order is Electron's: the head is read and checked
//! (Host, the page, the wrong-key count, the key) BEFORE a byte of the body is read, and a body is never read past
//! [`MAX_BODY`] + 1 bytes.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::phone_host::{
    MAX_BODY, PhoneBound, PhoneHttpRequest, PhoneHttpResponse, PhoneListenerFactory, PhoneRequestHandler,
};

/// Node's default `maxHeaderSize`: a head larger than this is refused with 431.
pub const MAX_HEAD: usize = 16 * 1024;
pub const HEADERS_TIMEOUT: Duration = Duration::from_secs(10);
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// At most this many phones' requests are answered at once; the rest are closed unanswered.
pub const MAX_OPEN: usize = 32;

#[derive(Debug, PartialEq, Eq)]
pub enum HeadError {
    /// The connection closed or timed out before a whole head: nothing is answered.
    Gone,
    TooLarge,
    Malformed,
}

/// The request line and headers, the bytes after them (the body's start), and the declared body length.
#[derive(Debug)]
pub struct Head {
    pub request: PhoneHttpRequest,
    pub content_length: Option<usize>,
    pub chunked: bool,
    pub rest: Vec<u8>,
}

fn find_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|b| *b != b' ' && *b != b'\t').unwrap_or(bytes.len());
    let end = bytes.iter().rposition(|b| *b != b' ' && *b != b'\t').map_or(start, |at| at + 1);
    &bytes[start..end.max(start)]
}

/// Parses a whole head (up to, not including, the blank line).
pub fn parse_head(head: &[u8], remote_addr: &str) -> Result<(PhoneHttpRequest, Option<usize>, bool), HeadError> {
    let mut lines = head.split(|b| *b == b'\n').map(|line| line.strip_suffix(b"\r").unwrap_or(line));
    let first = std::str::from_utf8(lines.next().ok_or(HeadError::Malformed)?).map_err(|_| HeadError::Malformed)?;
    let mut parts = first.split(' ');
    let (Some(method), Some(path), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(HeadError::Malformed);
    };
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) || !version.starts_with("HTTP/1.") {
        return Err(HeadError::Malformed);
    }
    let mut headers = Vec::new();
    let (mut content_length, mut chunked) = (None, false);
    for line in lines {
        let colon = line.iter().position(|b| *b == b':').ok_or(HeadError::Malformed)?;
        let name = std::str::from_utf8(&line[..colon]).map_err(|_| HeadError::Malformed)?;
        if name.is_empty() || name.bytes().any(|b| b.is_ascii_whitespace()) {
            return Err(HeadError::Malformed);
        }
        let name = name.to_ascii_lowercase();
        let value = trim(&line[colon + 1..]).to_vec();
        if name == "content-length" {
            let n = std::str::from_utf8(&value).ok().and_then(|v| v.parse::<usize>().ok()).ok_or(HeadError::Malformed)?;
            if content_length.is_some_and(|was| was != n) {
                return Err(HeadError::Malformed);
            }
            content_length = Some(n);
        }
        if name == "transfer-encoding" {
            chunked = true;
        }
        headers.push((name, value));
    }
    let request = PhoneHttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body: Vec::new(),
        remote_addr: remote_addr.to_owned(),
    };
    Ok((request, content_length, chunked))
}

/// Reads until the blank line that ends the head, never more than [`MAX_HEAD`] bytes of head.
pub fn read_head(stream: &mut impl Read, remote_addr: &str) -> Result<Head, HeadError> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = find_end(&buf) {
            let (request, content_length, chunked) = parse_head(&buf[..end], remote_addr)?;
            return Ok(Head { request, content_length, chunked, rest: buf[end + 4..].to_vec() });
        }
        if buf.len() > MAX_HEAD {
            return Err(HeadError::TooLarge);
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return Err(HeadError::Gone),
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        411 => "Length Required",
        415 => "Unsupported Media Type",
        421 => "Misdirected Request",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "",
    }
}

/// Writes one response; every connection is closed after it.
pub fn write_response(stream: &mut impl Write, response: &PhoneHttpResponse) -> std::io::Result<()> {
    let mut out = format!("HTTP/1.1 {} {}\r\n", response.status, reason(response.status)).into_bytes();
    for (name, value) in &response.headers {
        if name.eq_ignore_ascii_case("content-length") || name.eq_ignore_ascii_case("connection") {
            continue;
        }
        out.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    out.extend_from_slice(format!("content-length: {}\r\nconnection: close\r\n\r\n", response.body.len()).as_bytes());
    out.extend_from_slice(&response.body);
    stream.write_all(&out)?;
    stream.flush()
}

fn bare(status: u16) -> PhoneHttpResponse {
    PhoneHttpResponse { status, headers: Vec::new(), body: Vec::new() }
}

/// Answers one request on `stream`: the head is checked before the body is read, as Electron's handler does.
pub fn serve(stream: &mut (impl Read + Write), remote_addr: &str, handler: &PhoneRequestHandler) {
    let head = match read_head(stream, remote_addr) {
        Ok(head) => head,
        Err(HeadError::Gone) => return,
        Err(HeadError::TooLarge) => {
            let _ = write_response(stream, &bare(431));
            return;
        }
        Err(HeadError::Malformed) => {
            let _ = write_response(stream, &bare(400));
            return;
        }
    };
    let Head { mut request, content_length, chunked, rest } = head;
    if let Ok(answered) = handler.check_head(&request) {
        let _ = write_response(stream, &answered);
        return;
    }
    // The key is right: only now is the body read, and never past MAX_BODY + 1 bytes.
    if request.method == "POST" {
        if chunked {
            let _ = write_response(stream, &bare(411));
            return;
        }
        let want = content_length.unwrap_or(0).min(MAX_BODY + 1);
        let mut body = rest;
        body.truncate(want);
        let mut chunk = [0u8; 4096];
        while body.len() < want {
            match stream.read(&mut chunk) {
                Ok(0) | Err(_) => return,
                Ok(n) => body.extend_from_slice(&chunk[..n.min(want - body.len())]),
            }
        }
        request.body = body;
    }
    let _ = write_response(stream, &handler.handle(&request));
}

/// A `TcpStream` that gives up reading at a deadline: the head within [`HEADERS_TIMEOUT`], the whole request within
/// [`REQUEST_TIMEOUT`].
struct Timed {
    stream: TcpStream,
    started: Instant,
}

impl Read for Timed {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let left = REQUEST_TIMEOUT.saturating_sub(self.started.elapsed());
        if left.is_zero() {
            return Err(std::io::ErrorKind::TimedOut.into());
        }
        self.stream.set_read_timeout(Some(left.min(HEADERS_TIMEOUT)))?;
        self.stream.read(buf)
    }
}

impl Write for Timed {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.stream.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.stream.flush()
    }
}

/// The real listener: every network this computer is on (`0.0.0.0`), as Electron listens; the firewall decides which
/// may reach it.
pub struct TcpPhoneListener;

struct TcpBound {
    port: u16,
    stop: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl PhoneBound for TcpBound {
    fn port(&self) -> u16 {
        self.port
    }

    fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = thread.join();
        }
    }
}

impl Drop for TcpBound {
    fn drop(&mut self) {
        self.stop();
    }
}

impl PhoneListenerFactory for TcpPhoneListener {
    fn bind(&self, port: u16, handler: Arc<PhoneRequestHandler>) -> Result<Box<dyn PhoneBound>, String> {
        let failed = |_| "The phone listener could not start on this network.".to_owned();
        let listener = TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], port))).map_err(failed)?;
        let port = listener.local_addr().map_err(failed)?.port();
        listener.set_nonblocking(true).map_err(failed)?;
        let stop = Arc::new(AtomicBool::new(false));
        let open = Arc::new(AtomicUsize::new(0));
        let stopping = stop.clone();
        let thread = std::thread::Builder::new()
            .name("locust-phone".into())
            .spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, from)) => {
                            if open.load(Ordering::Relaxed) >= MAX_OPEN || stream.set_nonblocking(false).is_err() {
                                continue;
                            }
                            open.fetch_add(1, Ordering::Relaxed);
                            let (handler, open) = (handler.clone(), open.clone());
                            let spawned = std::thread::Builder::new().name("locust-phone-call".into()).spawn({
                                let open = open.clone();
                                move || {
                                    let _ = stream.set_write_timeout(Some(REQUEST_TIMEOUT));
                                    let mut timed = Timed { stream, started: Instant::now() };
                                    serve(&mut timed, &from.ip().to_string(), &handler);
                                    let _ = timed.stream.shutdown(std::net::Shutdown::Both);
                                    open.fetch_sub(1, Ordering::Relaxed);
                                }
                            });
                            if spawned.is_err() {
                                open.fetch_sub(1, Ordering::Relaxed);
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(25))
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(100)),
                    }
                }
            })
            .map_err(|e| failed(std::io::Error::other(e.to_string())))?;
        Ok(Box::new(TcpBound { port, stop, thread: Mutex::new(Some(thread)) }))
    }
}

/// This computer's IPv4 addresses with their interfaces' names, as Node's `os.networkInterfaces()` gives them to
/// Electron: `phone_addresses` then keeps the home-network ones. Only IPv4 is listed; loopback counts as internal.
pub fn interfaces() -> Vec<crate::phone_host::NetworkInterface> {
    #[cfg(unix)]
    {
        unix_interfaces()
    }
    #[cfg(not(unix))]
    {
        Vec::new()
    }
}

#[cfg(unix)]
fn unix_interfaces() -> Vec<crate::phone_host::NetworkInterface> {
    let mut out = Vec::new();
    let mut list: *mut libc::ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs fills `list` with a linked list that is only read here and freed once, below.
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return out;
    }
    let mut at = list;
    while !at.is_null() {
        // SAFETY: `at` is a node of the list getifaddrs made, not yet freed.
        let ifa = unsafe { &*at };
        at = ifa.ifa_next;
        if ifa.ifa_addr.is_null() || ifa.ifa_name.is_null() {
            continue;
        }
        // SAFETY: ifa_addr is non-null and points at a sockaddr of at least its family's size.
        if i32::from(unsafe { (*ifa.ifa_addr).sa_family }) != libc::AF_INET {
            continue;
        }
        // SAFETY: an AF_INET address is a sockaddr_in.
        let sin = unsafe { &*(ifa.ifa_addr as *const libc::sockaddr_in) };
        let ip = std::net::Ipv4Addr::from(u32::from_be(sin.sin_addr.s_addr));
        // SAFETY: ifa_name is a non-null, NUL-terminated C string owned by the list.
        let name = unsafe { std::ffi::CStr::from_ptr(ifa.ifa_name) }.to_string_lossy().into_owned();
        out.push(crate::phone_host::NetworkInterface {
            name,
            address: ip.to_string(),
            is_ipv4: true,
            internal: ifa.ifa_flags & (libc::IFF_LOOPBACK as libc::c_uint) != 0,
        });
    }
    // SAFETY: `list` came from getifaddrs and is freed exactly once.
    unsafe { libc::freeifaddrs(list) };
    out
}

#[cfg(test)]
#[path = "phone_server_tests.rs"]
mod tests;
