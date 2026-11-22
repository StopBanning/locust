//! AN ALERT LOCKED FOR ONE PHONE'S BROWSER (Electron `main/web-push.ts`, 0.827).
//!
//! Encrypts a phone's alert on the PC to the browser's push key per RFC 8291 (with RFC 8188's `aes128gcm`),
//! so the relay and push service pass on what they cannot read.

use crate::relay_crypto::{
    P256KeyPair, aes_gcm_decrypt, aes_gcm_encrypt, fill_random, from_base64url, hkdf_sha256,
    p256_ecdh, p256_generate_keypair, p256_keypair_from_raw,
};

/// One record is plenty: an alert is a few hundred bytes.
pub const PUSH_RECORD_SIZE: u32 = 4096;

/// What a browser hands back when it subscribes: where to send, and the two keys to encrypt with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PushSubscriptionKeys {
    pub endpoint: String,
    /// The browser's P-256 public key, uncompressed, base64url.
    pub p256dh: String,
    /// The 16-byte authentication secret, base64url.
    pub auth: String,
}

#[derive(Clone, Debug, Default)]
pub struct FixedPush {
    pub salt: Option<[u8; 16]>,
    pub keys: Option<P256KeyPair>,
}

/// The body of one push message: salt, record size, this message's own public key, then the sealed text.
pub fn encrypt_push(
    p256dh: &str,
    auth_b64: &str,
    text: &str,
    fixed: Option<FixedPush>,
) -> Result<Vec<u8>, String> {
    let receiver_raw = from_base64url(p256dh)?;
    let auth = from_base64url(auth_b64)?;
    if receiver_raw.len() != 65 || receiver_raw[0] != 4 {
        return Err("That push key is not a P-256 public key.".into());
    }
    if auth.len() != 16 {
        return Err("That push secret is not 16 bytes.".into());
    }
    let fixed = fixed.unwrap_or_default();
    let sender = fixed.keys.unwrap_or_else(p256_generate_keypair);
    let shared = p256_ecdh(&sender.private_key, &receiver_raw)?;

    let mut key_info = Vec::with_capacity(14 + 65 + 65);
    key_info.extend_from_slice(b"WebPush: info\0");
    key_info.extend_from_slice(&receiver_raw);
    key_info.extend_from_slice(&sender.public_key);
    let ikm = hkdf_sha256(&auth, &shared, &key_info, 32);

    let salt = match fixed.salt {
        Some(s) => s,
        None => {
            let mut s = [0u8; 16];
            fill_random(&mut s);
            s
        }
    };
    let cek = hkdf_sha256(&salt, &ikm, b"Content-Encoding: aes128gcm\0", 16);
    let nonce = hkdf_sha256(&salt, &ikm, b"Content-Encoding: nonce\0", 12);

    let mut plain = Vec::with_capacity(text.len() + 1);
    plain.extend_from_slice(text.as_bytes());
    plain.push(2); // final record delimiter

    let sealed = aes_gcm_encrypt(&cek, &nonce, &[], &plain)?;
    if sealed.len() + 86 > PUSH_RECORD_SIZE as usize {
        return Err("That alert is too long for one push.".into());
    }

    let mut out = Vec::with_capacity(21 + sender.public_key.len() + sealed.len());
    out.extend_from_slice(&salt);
    out.extend_from_slice(&PUSH_RECORD_SIZE.to_be_bytes());
    out.push(sender.public_key.len() as u8);
    out.extend_from_slice(&sender.public_key);
    out.extend_from_slice(&sealed);
    Ok(out)
}

/// The private key of a test's fixed sender, from its raw parts.
pub fn sender_keys_for_test(private_key: &str, public_key: &str) -> Result<P256KeyPair, String> {
    p256_keypair_from_raw(private_key, public_key)
}

/// Opens a push body as the phone's browser does (RFC 8291), returning the UTF-8 plaintext.
pub fn open_push(body: &[u8], receiver: &P256KeyPair, auth: &[u8]) -> Result<String, String> {
    if body.len() < 21 + 65 + 17 {
        return Err("Push body too short.".into());
    }
    let salt = &body[0..16];
    let key_len = body[20] as usize;
    if body.len() < 21 + key_len + 17 {
        return Err("Push body truncated.".into());
    }
    let sender_raw = &body[21..21 + key_len];
    let sealed = &body[21 + key_len..];
    let shared = p256_ecdh(&receiver.private_key, sender_raw)?;

    let mut key_info = Vec::with_capacity(14 + 65 + sender_raw.len());
    key_info.extend_from_slice(b"WebPush: info\0");
    key_info.extend_from_slice(&receiver.public_key);
    key_info.extend_from_slice(sender_raw);
    let ikm = hkdf_sha256(auth, &shared, &key_info, 32);

    let cek = hkdf_sha256(salt, &ikm, b"Content-Encoding: aes128gcm\0", 16);
    let nonce = hkdf_sha256(salt, &ikm, b"Content-Encoding: nonce\0", 12);
    let plain = aes_gcm_decrypt(&cek, &nonce, &[], sealed)?;
    if plain.last() != Some(&2) {
        return Err("Missing RFC 8188 final record delimiter.".into());
    }
    String::from_utf8(plain[..plain.len() - 1].to_vec()).map_err(|e| e.to_string())
}
