use super::*;
use std::sync::mpsc::{Receiver, Sender, channel};

struct Stalled(Sender<usize>, Receiver<()>);

impl Write for Stalled {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.send(bytes.len()).unwrap();
        self.1.recv().unwrap();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}

#[test]
fn the_terminal_limit_includes_bytes_still_in_a_physical_write() {
    let (entered, writing) = channel();
    let (release, held) = channel();
    let mut out = TerminalOutput::new(Stalled(entered, held)).unwrap();
    let first = vec![b'x'; TERMINAL_PENDING_LIMIT / 2];
    out.write_all(&first).unwrap();
    assert_eq!(writing.recv_timeout(Duration::from_secs(5)).unwrap(), first.len());
    out.write_all(&first).unwrap();
    let overflow = out.write_all(b"x");
    out.flush().ok();
    release.send(()).unwrap();
    assert_eq!(overflow.unwrap_err().to_string(), TERMINAL_SLOW);
}
