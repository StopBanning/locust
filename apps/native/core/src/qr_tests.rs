//! Square for square what Electron's qrcode-generator 2.0.4 draws: each path's SHA-256 was made by that library with
//! PhoneSettings.tsx's own path loop (`qrcode(0, 'M')`, `addData`, `make`), in ~/work/scratch/phone/qr/gold.cjs.

use super::*;
use sha2::{Digest, Sha256};

fn digest(text: &str) -> (usize, String) {
    let code = qr_code(text).expect("fits");
    let sha = Sha256::digest(code.svg_path().as_bytes());
    (code.size, sha.iter().map(|b| format!("{b:02x}")).collect())
}

#[test]
fn draws_the_code_electron_draws_for_a_phones_link_and_for_short_and_long_texts() {
    let cases = [
        ("hello".to_owned(), 21, "f5520e2f8167d98dbb755ca517db75a879e75ec75cc157b36abd4ff47545e0b4"),
        (
            "http://192.168.1.20:47740/#Qm9va1RoZVNlY29uZE9uZVBsZWFzZQ".to_owned(),
            33,
            "5cc8a37636dcaf6f20832b5cfb7da454b34d6ec3de00e16ddb71f0ef6a6d2827",
        ),
        (
            "http://10.0.0.5:51234/#abcdefghijklmnopqrstuvwxyz012345".to_owned(),
            33,
            "9a8fdb00f098131ce4688a6de508ec3f002d4a058bed5856287128300e923d66",
        ),
        (
            format!("http://172.16.254.199:47740/#{}", "Zz-_09".repeat(6)),
            37,
            "0682419e27f8a95fb57be22ae72159be022ba1212d2b1feac588cf08b51301a1",
        ),
        // Version 9: the version blocks beside the corners.
        (format!("https://example.com/{}", "a".repeat(140)), 53, "5552e50a47b16300638a86deb643965b390cc183b917adc9c5e9e61ba7d58fb7"),
        // Version 10: a 16-bit length.
        ("x".repeat(200), 57, "a0a348b67b2a533202ce1a80bfcbd46ede82a891963b84c3d880de35c12f6870"),
    ];
    for (text, size, sha) in cases {
        assert_eq!(digest(&text), (size, sha.to_owned()), "{text}");
    }
}

#[test]
fn draws_dark_squares_inside_a_four_square_margin_and_holds_at_most_213_bytes() {
    let code = qr_code("hello").unwrap();
    // The top-left finder's corner is dark, drawn four squares in.
    assert!(code.is_dark(0, 0));
    assert!(code.svg_path().starts_with("M4 4h1v1h-1z"));
    assert!(qr_code(&"x".repeat(213)).is_some());
    assert_eq!(qr_code(&"x".repeat(214)), None);
}
