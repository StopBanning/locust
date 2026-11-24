//! Electron's `packages/runtime-adapters/test/more-secrets-are-scrubbed.test.ts`, case by case: four more shapes of
//! secret (a private key block, a bare JWT, a Google API key, a Hugging Face token) are scrubbed by redactSecrets and
//! redactText (events.rs `redact_secrets`, `redact_text`), and so from the text every normalizer keeps (`text`).
//! Built by concatenation, as Electron's are, so no real-looking secret sits in the source.
use super::*;

fn pem() -> String {
    ["-----BEGIN", " PRIVATE KEY-----\n", "MIIEvQIBADAN", "BgkqhkiG9w0BAQEFAASC\n", "-----END", " PRIVATE KEY-----"].concat()
}
fn rsa() -> String {
    ["-----BEGIN RSA", " PRIVATE KEY-----\nabc\n-----END RSA", " PRIVATE KEY-----"].concat()
}
fn jwt() -> String {
    ["eyJ", "hbGciOiJIUzI1NiJ9", ".eyJ", "zdWIiOiIxMjM0NTY3ODkwIn0", "."].concat() + &"d".repeat(43)
}
fn google() -> String {
    format!("AIza{}", "S".repeat(35))
}
fn hugging_face() -> String {
    format!("hf_{}", "a".repeat(34))
}

// case: ${scrub.name} scrubs a private key, a bare JWT, a Google key and a Hugging Face token
#[test]
fn each_scrubber_takes_out_a_private_key_a_jwt_a_google_key_and_a_hugging_face_token() {
    let input = format!("key:\n{}\nrsa: {}\njwt {} google {} hf {} end", pem(), rsa(), jwt(), google(), hugging_face());
    let message_text = |value: &str| text(value, 16384);
    let scrubbers: [(&str, &dyn Fn(&str) -> String); 3] =
        [("redact_secrets", &redact_secrets), ("redact_text", &redact_text), ("text", &message_text)];
    for (name, scrub) in scrubbers {
        let said = scrub(&input);
        for secret in [pem(), rsa(), jwt(), google(), hugging_face()] {
            assert!(!said.contains(&secret), "{name} kept {secret:?}: {said}");
        }
        assert!(said.contains("end"), "{name}: {said}");
    }
}

// case: leaves ordinary words alone, including ones that merely start alike
#[test]
fn ordinary_words_that_merely_start_alike_are_left_alone() {
    assert_eq!(redact_secrets("eyJ is how a JWT starts; AIza and hf_ too."), "eyJ is how a JWT starts; AIza and hf_ too.");
}
