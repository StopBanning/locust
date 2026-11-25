//! Real HTTPS transport for OpenAI voice transcription (`voice_typing::TranscriptionTransport`).
//! Uses `ureq` with rustls + OS root certificates, no redirects, no cookies, a response size cap, and cancellation.

use crate::voice_typing::{TranscriptionRequest, TranscriptionTransport};
use std::{
    io::Read,
    sync::atomic::Ordering,
    time::Duration,
};

/// Maximum response body size accepted from the transcription endpoint (1 MiB).
pub const MAX_RESPONSE_BYTES: usize = 1_024 * 1_024;

#[derive(Default)]
pub struct UreqTranscriptionTransport;

impl UreqTranscriptionTransport {
    pub fn new() -> Self {
        Self
    }
}

impl TranscriptionTransport for UreqTranscriptionTransport {
    fn send(&self, request: TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()> {
        if request.cancel.load(Ordering::Acquire) {
            return Err(());
        }
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout(Duration::from_secs(request.timeout_seconds))
            .max_idle_connections(0)
            .build();
        let content_type = format!("multipart/form-data; boundary={}", request.multipart_boundary);
        let outcome = agent
            .post(request.url)
            .set("Authorization", &request.authorization)
            .set("Content-Type", &content_type)
            .send_bytes(&request.body);
        let response = match outcome {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(_)) => return Err(()),
        };
        let status = response.status();
        // Refuse all redirects even if the client returns the 3xx status directly.
        if (300..400).contains(&status) {
            return Err(());
        }
        let mut reader = response.into_reader();
        let mut body = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            if request.cancel.load(Ordering::Acquire) {
                return Err(());
            }
            let read = reader.read(&mut chunk).map_err(|_| ())?;
            if read == 0 {
                break;
            }
            if body.len().saturating_add(read) > MAX_RESPONSE_BYTES {
                return Err(());
            }
            body.extend_from_slice(&chunk[..read]);
        }
        Ok((status, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize},
        },
        thread,
    };

    fn spawn_server(
        response: Vec<u8>,
    ) -> (
        &'static str,
        Arc<AtomicUsize>,
        thread::JoinHandle<Option<(String, Vec<(String, String)>, Vec<u8>)>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let url: &'static str = Box::leak(format!("http://{addr}/v1/audio/transcriptions").into_boxed_str());
        let accepted = Arc::new(AtomicUsize::new(0));
        let seen = accepted.clone();
        let handle = thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return None;
            };
            seen.fetch_add(1, Ordering::SeqCst);
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            let mut headers = Vec::new();
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let trimmed = line.trim_end_matches(['\r', '\n']);
                if trimmed.is_empty() {
                    break;
                }
                if let Some((name, value)) = trimmed.split_once(':') {
                    let name_lower = name.trim().to_ascii_lowercase();
                    let value_trim = value.trim().to_owned();
                    if name_lower == "content-length" {
                        content_length = value_trim.parse().unwrap_or(0);
                    }
                    headers.push((name_lower, value_trim));
                }
            }
            let mut body = vec![0u8; content_length];
            reader.read_exact(&mut body).unwrap();
            let _ = stream.write_all(&response);
            let _ = stream.flush();
            Some((request_line, headers, body))
        });
        (url, accepted, handle)
    }

    #[test]
    fn sends_authorization_and_multipart_headers_and_reads_ok_body() {
        let payload = br#"{"text":"hello world"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        )
        .into_bytes();
        let mut full = response;
        full.extend_from_slice(payload);
        let (url, _, handle) = spawn_server(full);
        let cancel = AtomicBool::new(false);
        let transport = UreqTranscriptionTransport::new();
        let (status, body) = transport
            .send(TranscriptionRequest {
                url,
                authorization: "Bearer synthetic-token".into(),
                multipart_boundary: "locust-boundary-1".into(),
                body: b"--locust-boundary-1--\r\n".to_vec(),
                timeout_seconds: 10,
                cancel: &cancel,
            })
            .unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, payload);
        let (line, headers, req_body) = handle.join().unwrap().unwrap();
        assert!(line.starts_with("POST /v1/audio/transcriptions HTTP/1.1"));
        assert!(
            headers
                .iter()
                .any(|(k, v)| k == "authorization" && v == "Bearer synthetic-token")
        );
        assert!(headers.iter().any(|(k, v)| {
            k == "content-type" && v == "multipart/form-data; boundary=locust-boundary-1"
        }));
        assert!(!headers.iter().any(|(k, _)| k == "cookie"));
        assert_eq!(req_body, b"--locust-boundary-1--\r\n");
    }

    #[test]
    fn refuses_302_redirect_without_following() {
        let response = b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/other\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
        let (url, accepted, handle) = spawn_server(response);
        let cancel = AtomicBool::new(false);
        let transport = UreqTranscriptionTransport::new();
        let result = transport.send(TranscriptionRequest {
            url,
            authorization: "Bearer synthetic-token".into(),
            multipart_boundary: "locust-b".into(),
            body: b"wav".to_vec(),
            timeout_seconds: 10,
            cancel: &cancel,
        });
        assert!(result.is_err());
        let _ = handle.join();
        assert_eq!(accepted.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn maps_401_and_500_status_codes_with_body() {
        for (code, reason, body_text) in [
            (401u16, "Unauthorized", r#"{"error":"bad key"}"#),
            (500u16, "Internal Server Error", r#"{"error":"server"}"#),
        ] {
            let response = format!(
                "HTTP/1.1 {code} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body_text}",
                body_text.len()
            )
            .into_bytes();
            let (url, _, handle) = spawn_server(response);
            let cancel = AtomicBool::new(false);
            let transport = UreqTranscriptionTransport::new();
            let (status, body) = transport
                .send(TranscriptionRequest {
                    url,
                    authorization: "Bearer synthetic-token".into(),
                    multipart_boundary: "locust-b".into(),
                    body: b"wav".to_vec(),
                    timeout_seconds: 10,
                    cancel: &cancel,
                })
                .unwrap();
            assert_eq!(status, code);
            assert_eq!(body, body_text.as_bytes());
            let _ = handle.join();
        }
    }

    #[test]
    fn rejects_response_body_exceeding_size_cap() {
        let oversized = vec![b'a'; MAX_RESPONSE_BYTES + 1];
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            oversized.len()
        )
        .into_bytes();
        response.extend_from_slice(&oversized);
        let (url, _, handle) = spawn_server(response);
        let cancel = AtomicBool::new(false);
        let transport = UreqTranscriptionTransport::new();
        let result = transport.send(TranscriptionRequest {
            url,
            authorization: "Bearer synthetic-token".into(),
            multipart_boundary: "locust-b".into(),
            body: b"wav".to_vec(),
            timeout_seconds: 10,
            cancel: &cancel,
        });
        assert!(result.is_err());
        let _ = handle.join();
    }

    #[test]
    fn cancel_short_circuits_before_opening_socket() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let addr = listener.local_addr().unwrap();
        let url: &'static str = Box::leak(format!("http://{addr}/v1/audio/transcriptions").into_boxed_str());
        let cancel = AtomicBool::new(true);
        let transport = UreqTranscriptionTransport::new();
        let result = transport.send(TranscriptionRequest {
            url,
            authorization: "Bearer synthetic-token".into(),
            multipart_boundary: "locust-b".into(),
            body: b"wav".to_vec(),
            timeout_seconds: 10,
            cancel: &cancel,
        });
        assert!(result.is_err());
        assert!(listener.accept().is_err());
    }
}
