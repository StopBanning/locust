//! The phone listener's framing over an in-memory stream: no port is ever bound here.

use super::*;
use crate::phone_host::{PhoneCalls, PhoneSent, PhoneState, PhoneTurn};
use serde_json::{Value, json};

const KEY: &str = "abcdefghijklmnopqrstuvwxyz012345";

/// Hands its input out in the pieces given (one per read), and keeps what is written.
struct Pieces {
    pieces: Vec<Vec<u8>>,
    reads: usize,
    out: Vec<u8>,
}

impl Pieces {
    fn new(pieces: Vec<Vec<u8>>) -> Self {
        Self { pieces, reads: 0, out: Vec::new() }
    }

    fn reply(&self) -> (u16, String, Vec<u8>) {
        let text = String::from_utf8_lossy(&self.out).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").unwrap_or((&text, ""));
        let status = head.split(' ').nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        (status, head.to_ascii_lowercase(), body.as_bytes().to_vec())
    }
}

impl Read for Pieces {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.reads >= self.pieces.len() {
            return Ok(0);
        }
        let piece = &self.pieces[self.reads];
        assert!(piece.len() <= buf.len(), "a test piece fits one read");
        buf[..piece.len()].copy_from_slice(piece);
        self.reads += 1;
        Ok(piece.len())
    }
}

impl Write for Pieces {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.out.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Default)]
struct Calls {
    sent: Mutex<Vec<(String, String)>>,
}

impl PhoneCalls for Calls {
    fn state(&self) -> PhoneState {
        PhoneState { waiting: Vec::new(), conversations: Vec::new(), scout: None }
    }
    fn turns(&self, _mission_id: &str) -> Option<Vec<PhoneTurn>> {
        None
    }
    fn answer_card(&self, _approval_id: &str, _approve: bool) -> bool {
        false
    }
    fn send(&self, mission_id: &str, text: &str) -> PhoneSent {
        self.sent.lock().unwrap().push((mission_id.to_owned(), text.to_owned()));
        PhoneSent::Ok
    }
}

fn handler(calls: Arc<Calls>) -> PhoneRequestHandler {
    PhoneRequestHandler::new(
        Arc::new(|| Some(KEY.to_owned())),
        Arc::new(|| "<html>page</html>".to_owned()),
        calls,
        Arc::new(|| 1_000),
    )
}

fn post_head(key: &str, length: usize) -> Vec<u8> {
    format!(
        "POST /api/send HTTP/1.1\r\nHost: 192.168.1.20:47740\r\nAuthorization: Bearer {key}\r\n\
         Content-Type: application/json\r\nContent-Length: {length}\r\n\r\n"
    )
    .into_bytes()
}

#[test]
fn checks_the_key_before_it_reads_a_byte_of_the_body() {
    let calls = Arc::new(Calls::default());
    let body = br#"{"missionId":"m1","text":"hello"}"#.to_vec();
    let mut stream = Pieces::new(vec![post_head("wrong-key-wrong-key-wrong-key-xx", body.len()), body]);
    serve(&mut stream, "192.168.1.30", &handler(calls.clone()));
    let (status, _, reply) = stream.reply();
    assert_eq!(status, 401);
    assert_eq!(
        serde_json::from_slice::<Value>(&reply).unwrap(),
        json!({ "message": "This phone is not paired. Scan the code in Locust’s Settings again." })
    );
    assert_eq!(stream.reads, 1, "the body was never read");
    assert!(calls.sent.lock().unwrap().is_empty());
}

#[test]
fn sends_a_message_whose_body_comes_in_pieces_and_closes_after_answering() {
    let calls = Arc::new(Calls::default());
    let body = br#"{"missionId":"m1","text":"  hello from the phone  "}"#.to_vec();
    let (a, b) = body.split_at(10);
    let mut stream = Pieces::new(vec![post_head(KEY, body.len()), a.to_vec(), b.to_vec()]);
    serve(&mut stream, "192.168.1.30", &handler(calls.clone()));
    let (status, head, reply) = stream.reply();
    assert_eq!(status, 200);
    assert_eq!(serde_json::from_slice::<Value>(&reply).unwrap(), json!({ "ok": true }));
    assert!(head.contains("connection: close"));
    assert!(head.contains(&format!("content-length: {}", reply.len())));
    assert_eq!(*calls.sent.lock().unwrap(), vec![("m1".to_owned(), "hello from the phone".to_owned())]);
}

#[test]
fn reads_no_more_than_one_byte_past_the_limit_of_a_body_too_large() {
    let calls = Arc::new(Calls::default());
    let huge = 10 * MAX_BODY;
    let mut pieces = vec![post_head(KEY, huge)];
    pieces.extend((0..huge / 4096).map(|_| vec![b'x'; 4096]));
    let mut stream = Pieces::new(pieces);
    serve(&mut stream, "192.168.1.30", &handler(calls.clone()));
    let (status, _, reply) = stream.reply();
    assert_eq!(status, 400);
    assert_eq!(
        serde_json::from_slice::<Value>(&reply).unwrap(),
        json!({ "message": "That message could not be read." })
    );
    assert_eq!(stream.reads, 1 + (MAX_BODY + 1).div_ceil(4096), "stopped at MAX_BODY + 1 bytes");
    assert!(calls.sent.lock().unwrap().is_empty());
}

#[test]
fn serves_the_page_and_refuses_a_rebinding_host_over_the_wire() {
    let mut page = Pieces::new(vec![b"GET / HTTP/1.1\r\nHost: 192.168.1.20:47740\r\n\r\n".to_vec()]);
    serve(&mut page, "192.168.1.30", &handler(Arc::default()));
    let (status, _, body) = page.reply();
    assert_eq!((status, body), (200, b"<html>page</html>".to_vec()));

    let mut named = Pieces::new(vec![b"GET / HTTP/1.1\r\nHost: evil.example:47740\r\n\r\n".to_vec()]);
    serve(&mut named, "192.168.1.30", &handler(Arc::default()));
    assert_eq!(named.reply().0, 421);
}

#[test]
fn refuses_a_head_too_large_a_head_it_cannot_read_and_a_chunked_body() {
    let mut big = Pieces::new(
        std::iter::once(b"GET / HTTP/1.1\r\n".to_vec())
            .chain((0..5).map(|_| format!("x-pad: {}\r\n", "a".repeat(4000)).into_bytes()))
            .collect(),
    );
    serve(&mut big, "192.168.1.30", &handler(Arc::default()));
    assert_eq!(big.reply().0, 431);

    let mut garbled = Pieces::new(vec![b"not http at all\r\n\r\n".to_vec()]);
    serve(&mut garbled, "192.168.1.30", &handler(Arc::default()));
    assert_eq!(garbled.reply().0, 400);

    let mut gone = Pieces::new(vec![b"GET / HTTP/1.1\r\nHost: 192.168".to_vec()]);
    serve(&mut gone, "192.168.1.30", &handler(Arc::default()));
    assert!(gone.out.is_empty(), "a request cut short is not answered");

    let calls = Arc::new(Calls::default());
    let chunked = format!(
        "POST /api/send HTTP/1.1\r\nHost: 192.168.1.20\r\nAuthorization: Bearer {KEY}\r\n\
         Content-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n"
    );
    let mut stream = Pieces::new(vec![chunked.into_bytes(), b"5\r\nhello\r\n0\r\n\r\n".to_vec()]);
    serve(&mut stream, "192.168.1.30", &handler(calls.clone()));
    assert_eq!(stream.reply().0, 411);
    assert!(calls.sent.lock().unwrap().is_empty());
}

#[test]
fn keeps_a_header_value_as_its_bytes_and_names_in_lower_case() {
    let (request, length, chunked) =
        parse_head(b"POST /api/x HTTP/1.1\r\nAuthorization:  Bearer \xe9t\xe9 \r\nCONTENT-LENGTH: 3", "10.0.0.2").unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.path, "/api/x");
    assert_eq!(request.remote_addr, "10.0.0.2");
    assert_eq!(request.header_bytes("authorization"), Some(&b"Bearer \xe9t\xe9"[..]));
    assert_eq!((length, chunked), (Some(3), false));
    assert_eq!(
        parse_head(b"POST / HTTP/1.1\r\nContent-Length: 3\r\nContent-Length: 4", "x").unwrap_err(),
        HeadError::Malformed
    );
}
