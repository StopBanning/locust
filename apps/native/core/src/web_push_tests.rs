use crate::relay_crypto::{from_base64url, to_base64url};
use crate::web_push::{FixedPush, encrypt_push, sender_keys_for_test};

// RFC 8291 Section 5 public worked example vectors (identical to Electron's an-alert-is-locked-for-its-phone-alone.test.ts).
const RFC_AUTH: &str = "BTBZMqHH6r4Tts7J_aSIgg";
const RFC_RECEIVER_PUBLIC: &str =
    "BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4";
const RFC_SENDER_PRIVATE: &str = "yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw";
const RFC_SENDER_PUBLIC: &str =
    "BP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A8";
const RFC_BODY: &str = "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN";
const RFC_TEXT: &str = "When I grow up, I want to be a watermelon";

#[test]
// case: makes the RFC's own example body from its keys and salt
fn makes_the_rfcs_own_example_body_from_its_keys_and_salt() {
    let expected = from_base64url(RFC_BODY).unwrap();
    let mut salt = [0u8; 16];
    salt.copy_from_slice(&expected[..16]);
    let keys = sender_keys_for_test(RFC_SENDER_PRIVATE, RFC_SENDER_PUBLIC).unwrap();
    let body = encrypt_push(
        RFC_RECEIVER_PUBLIC,
        RFC_AUTH,
        RFC_TEXT,
        Some(FixedPush {
            salt: Some(salt),
            keys: Some(keys),
        }),
    )
    .unwrap();
    assert_eq!(to_base64url(&body), RFC_BODY);
}

#[test]
// case: uses a fresh salt and key for every alert
fn uses_a_fresh_salt_and_key_for_every_alert() {
    let one = encrypt_push(
        RFC_RECEIVER_PUBLIC,
        RFC_AUTH,
        "Mira needs your approval",
        None,
    )
    .unwrap();
    let two = encrypt_push(
        RFC_RECEIVER_PUBLIC,
        RFC_AUTH,
        "Mira needs your approval",
        None,
    )
    .unwrap();
    assert_ne!(&one[0..16], &two[0..16]);
    assert_ne!(&one[21..86], &two[21..86]);
}

#[test]
// case: refuses keys that are not a browser's push keys
fn refuses_keys_that_are_not_a_browsers_push_keys() {
    let bad_pub = encrypt_push("AAAA", RFC_AUTH, "x", None).unwrap_err();
    assert!(bad_pub.contains("not a P-256 public key"), "{bad_pub}");
    let bad_auth = encrypt_push(RFC_RECEIVER_PUBLIC, "AAAA", "x", None).unwrap_err();
    assert!(bad_auth.contains("not 16 bytes"), "{bad_auth}");
}
