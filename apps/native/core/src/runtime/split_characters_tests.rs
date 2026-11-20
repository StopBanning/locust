//! Electron's `app-server-output-keeps-its-characters.test.ts`: a UTF-8 character split across two pipe reads became
//! two replacement characters. Rust reads a runtime's output as whole lines of bytes (`read_line`); its stderr tail now
//! holds a split character's first bytes until the rest arrives.
use super::*;

/// A pipe that hands over these pieces, one per read.
struct Pieces(std::collections::VecDeque<Vec<u8>>);
impl Read for Pieces {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let Some(mut piece) = self.0.pop_front() else { return Ok(0) };
        let n = piece.len().min(buf.len());
        buf[..n].copy_from_slice(&piece[..n]);
        if n < piece.len() {
            self.0.push_front(piece.split_off(n));
        }
        Ok(n)
    }
}

/// `{"text":"café 🦗"}` split inside "é" and inside the emoji, as Electron's case splits it.
fn split_line() -> Pieces {
    let line = "{\"text\":\"café 🦗\"}\n".as_bytes().to_vec();
    let e = line.iter().position(|b| *b == 0xc3).unwrap();
    let emoji = line.iter().position(|b| *b == 0xf0).unwrap();
    Pieces([line[..=e].to_vec(), line[e + 1..emoji + 2].to_vec(), line[emoji + 2..].to_vec()].into())
}

// case: keeps a character split across two reads whole
#[test]
fn keeps_a_character_split_across_two_reads_whole() {
    let mut reader = std::io::BufReader::with_capacity(4, split_line());
    let mut line = Vec::new();
    read_line(&mut reader, &mut line).unwrap();
    let text = String::from_utf8(line).unwrap();
    assert_eq!(text.trim_end(), "{\"text\":\"café 🦗\"}");
}

#[test]
fn the_stderr_tail_keeps_a_split_character_whole_too() {
    let tail = stderr_tail(split_line());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !crate::invariants::lock(&tail).contains('}') {
        assert!(std::time::Instant::now() < deadline, "the tail never filled");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let said = crate::invariants::lock(&tail).clone();
    assert!(said.contains("café 🦗") && !said.contains('\u{FFFD}'), "{said:?}");
    let mut pending = vec![b'a', 0xff, b'b'];
    assert_eq!(take_whole_characters(&mut pending), "a\u{FFFD}b", "bytes that are never a character still read as one");
}
