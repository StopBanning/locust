//! A growing text held with both ends (acp-events.ts `heldText`): each piece is appended as it comes, and once the
//! whole outgrows four times what is kept of each end, it is cut to its start and its end. Holding a piece costs its
//! own size, never the whole held text again, so a long answer streamed a token at a time is read in time that grows
//! with its length, not its square (SOAK-NOTES row 2c). Secrets are scrubbed and the text bounded when it is written
//! (`events::text`), as Electron scrubs at the message's close, not while it grows.

/// What a held message or thought keeps of each end while it is still growing (acp-events.ts `HELD_END`).
pub const HELD_END: usize = 32_768;

/// Said between the two ends of a held text that was cut (acp-events.ts `heldText`).
const CUT: &str = "\n…\n";

#[derive(Debug, Clone)]
pub struct HeldText {
    text: String,
    /// The text's length in UTF-16 code units, as Electron measures it, kept as pieces arrive.
    units: usize,
    /// What is kept of each end once cut.
    end: usize,
}

impl Default for HeldText {
    fn default() -> Self {
        Self::keeping(HELD_END)
    }
}

impl HeldText {
    /// A held text that keeps `end` UTF-16 units of each end once it outgrows `4 * end`.
    pub fn keeping(end: usize) -> Self {
        Self { text: String::new(), units: 0, end }
    }

    /// `heldText(held + piece)`: appends the piece, and cuts the whole to both ends once it is longer than `4 * end`.
    pub fn push(&mut self, piece: &str) {
        self.text.push_str(piece);
        self.units += piece.encode_utf16().count();
        if self.units > self.end * 4 {
            let head = ends(self.text.chars(), self.end);
            let mut tail = ends(self.text.chars().rev(), self.end);
            tail.reverse();
            let mut cut = String::with_capacity(self.end * 2 + CUT.len());
            cut.extend(head);
            cut.push_str(CUT);
            cut.extend(tail);
            self.units = cut.encode_utf16().count();
            self.text = cut;
        }
    }

    /// Replaces what is held with a whole text, held as if it had arrived in one piece.
    pub fn set(&mut self, whole: &str) {
        self.text.clear();
        self.units = 0;
        self.push(whole);
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// What is held, leaving nothing.
    pub fn take(&mut self) -> String {
        self.units = 0;
        std::mem::take(&mut self.text)
    }
}

/// The characters from one end whose UTF-16 length fits in `units` (a pair is never split, as JS `slice` might).
fn ends(chars: impl Iterator<Item = char>, units: usize) -> Vec<char> {
    let mut used = 0;
    chars
        .take_while(|c| {
            used += c.len_utf16();
            used <= units
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holds_what_it_was_given_until_it_outgrows_four_ends() {
        let mut held = HeldText::keeping(4);
        for piece in ["abc", "def", "ghij", "klmnop"] {
            held.push(piece);
        }
        assert_eq!(held.as_str(), "abcdefghijklmnop", "16 units is four ends, not more");
        held.push("q");
        assert_eq!(held.as_str(), "abcd\n…\nnopq", "past four ends it keeps each end");
        held.push("rs");
        assert_eq!(held.as_str(), "abcd\n…\nnopqrs", "and grows again from the cut");
        assert_eq!(held.units, held.as_str().encode_utf16().count(), "its length is kept as it grows");
    }

    #[test]
    fn measures_as_electron_does_and_never_splits_a_character() {
        let mut held = HeldText::keeping(2);
        held.set("😀😀😀😀😀");
        // Each face is two UTF-16 units: ten units is past eight, so one face is kept of each end.
        assert_eq!(held.as_str(), "😀\n…\n😀");
        assert_eq!(held.take(), "😀\n…\n😀");
        assert!(held.is_empty());
        held.push("x");
        assert_eq!(held.as_str(), "x", "taken, it starts again empty");
    }

    #[test]
    fn holding_a_long_answer_a_token_at_a_time_costs_its_length_not_its_square() {
        // 400,000 two-character pieces through the default bound: a few cuts, each piece measured once. Rebuilding the
        // whole held text a piece (the fault) is ~10^10 character steps here and does not finish in the time allowed.
        let started = std::time::Instant::now();
        let mut held = HeldText::default();
        for i in 0..400_000 {
            held.push(if i % 2 == 0 { "ab" } else { "cd" });
            // Checked as it goes, so a holder that rebuilds the whole fails here in seconds, not after minutes.
            if i % 1_000 == 0 {
                assert!(started.elapsed() < std::time::Duration::from_secs(5), "{i} pieces took {:?}", started.elapsed());
            }
        }
        assert!(held.units <= HELD_END * 4);
        assert!(held.as_str().starts_with("abcdabcd"));
        assert!(held.as_str().ends_with("abcd"));
    }
}
