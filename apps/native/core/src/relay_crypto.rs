//! THE LOCK ON YOUR PHONE'S MESSAGES (Electron `main/relay-crypto.ts`, 0.826).
//!
//! Standard RustCrypto-backed implementation of:
//! - base64url (unpadded) & OS randomness (`getrandom`)
//! - HMAC-SHA256 (`hmac`) & HKDF-SHA256 (`hkdf`, RFC 5869)
//! - AES-128-GCM & AES-256-GCM (`aes-gcm`, NIST SP 800-38D, 12-byte nonce)
//! - X25519 key agreement (`x25519-dalek`, RFC 7748)
//! - NIST P-256 (secp256r1) ECDH (`p256`, for RFC 8291 Web Push)
//! - Locust relay key derivation, sealing, opening, and pairing tokens/proofs.

use aes_gcm::{
    Aes128Gcm, Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use p256::elliptic_curve::sec1::ToEncodedPoint;
use serde_json::Value;
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

type HmacSha256 = Hmac<Sha256>;

pub const RELAY_CRYPTO_VERSION: &str = "locust-relay-v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    PcToDevice,
    DeviceToPc,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PcToDevice => "pc-to-device",
            Self::DeviceToPc => "device-to-pc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkSide {
    Pc,
    Device,
}

// ---- base64url, random, constant-time compare -----------------------------------------------------------------------

pub fn to_base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((n >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(TABLE[(n & 0x3f) as usize] as char);
        }
    }
    out
}

pub fn from_base64url(text: &str) -> Result<Vec<u8>, String> {
    let bytes = text.as_bytes();
    let trimmed = match bytes.iter().rposition(|&b| b != b'=') {
        Some(last) => &bytes[..=last],
        None => &[],
    };
    if trimmed.len() % 4 == 1 {
        return Err("Invalid base64url length.".into());
    }
    let decode = |b: u8| -> Result<u32, String> {
        match b {
            b'A'..=b'Z' => Ok((b - b'A') as u32),
            b'a'..=b'z' => Ok((b - b'a' + 26) as u32),
            b'0'..=b'9' => Ok((b - b'0' + 52) as u32),
            b'-' | b'+' => Ok(62),
            b'_' | b'/' => Ok(63),
            _ => Err("Invalid base64url byte.".into()),
        }
    };
    let mut out = Vec::with_capacity((trimmed.len() * 3) / 4);
    for chunk in trimmed.chunks(4) {
        let v0 = decode(chunk[0])?;
        let v1 = decode(chunk[1])?;
        out.push(((v0 << 2) | (v1 >> 4)) as u8);
        if chunk.len() >= 3 {
            let v2 = decode(chunk[2])?;
            out.push((((v1 & 0x0f) << 4) | (v2 >> 2)) as u8);
            if chunk.len() == 4 {
                let v3 = decode(chunk[3])?;
                out.push((((v2 & 0x03) << 6) | v3) as u8);
            }
        }
    }
    Ok(out)
}

pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    fill_random(&mut out);
    out
}

pub fn fill_random(dest: &mut [u8]) {
    if dest.is_empty() {
        return;
    }
    getrandom::fill(dest).expect("OS random number generator failed");
}

pub fn random_text(bytes: usize) -> String {
    to_base64url(&random_bytes(bytes))
}

pub fn sha256_b64url(text: &str) -> String {
    to_base64url(&Sha256::digest(text.as_bytes()))
}

pub fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    use subtle::ConstantTimeEq;
    // Lengths are not secret (tokens and proofs have fixed lengths); the bytes are compared in constant time.
    a.len() == b.len() && bool::from(a.ct_eq(b))
}

pub fn same_text(a: &str, b: &str) -> bool {
    same_bytes(a.as_bytes(), b.as_bytes())
}

// ---- HMAC-SHA256 & HKDF-SHA256 (RFC 5869) ---------------------------------------------------------------------------

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).expect("HMAC accepts any key size");
    mac.update(data);
    mac.finalize().into_bytes().into()
}

pub fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let salt = if salt.is_empty() { None } else { Some(salt) };
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut okm = vec![0u8; len];
    hk.expand(info, &mut okm)
        .expect("HKDF output length exceeds maximum");
    okm
}

// ---- AES-128 & AES-256 + GCM (NIST SP 800-38D) ---------------------------------------------------------------------

/// AES-128-GCM or AES-256-GCM encrypt with a 12-byte nonce. Appends the 16-byte tag to the ciphertext.
pub fn aes_gcm_encrypt(key: &[u8], nonce: &[u8], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    if nonce.len() != 12 {
        return Err("AES-GCM nonce must be 12 bytes.".into());
    }
    let nonce = Nonce::from_slice(nonce);
    let payload = Payload { msg: plaintext, aad };
    match key.len() {
        16 => Aes128Gcm::new_from_slice(key)
            .map_err(|_| "AES key must be 16 or 32 bytes.".to_owned())?
            .encrypt(nonce, payload)
            .map_err(|_| "AES-GCM encryption failed.".to_owned()),
        32 => Aes256Gcm::new_from_slice(key)
            .map_err(|_| "AES key must be 16 or 32 bytes.".to_owned())?
            .encrypt(nonce, payload)
            .map_err(|_| "AES-GCM encryption failed.".to_owned()),
        _ => Err("AES key must be 16 or 32 bytes.".into()),
    }
}

/// AES-128-GCM or AES-256-GCM decrypt with a 12-byte nonce. Expects ciphertext || 16-byte tag.
pub fn aes_gcm_decrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    ciphertext_and_tag: &[u8],
) -> Result<Vec<u8>, String> {
    if nonce.len() != 12 {
        return Err("AES-GCM nonce must be 12 bytes.".into());
    }
    if ciphertext_and_tag.len() < 16 {
        return Err("Ciphertext too short.".into());
    }
    let nonce = Nonce::from_slice(nonce);
    let payload = Payload {
        msg: ciphertext_and_tag,
        aad,
    };
    match key.len() {
        16 => Aes128Gcm::new_from_slice(key)
            .map_err(|_| "AES key must be 16 or 32 bytes.".to_owned())?
            .decrypt(nonce, payload)
            .map_err(|_| "Authentication tag mismatch.".to_owned()),
        32 => Aes256Gcm::new_from_slice(key)
            .map_err(|_| "AES key must be 16 or 32 bytes.".to_owned())?
            .decrypt(nonce, payload)
            .map_err(|_| "Authentication tag mismatch.".to_owned()),
        _ => Err("AES key must be 16 or 32 bytes.".into()),
    }
}

// ---- X25519 (RFC 7748) ---------------------------------------------------------------------------------------------

pub fn x25519(scalar: &[u8; 32], point: &[u8; 32]) -> [u8; 32] {
    let secret = StaticSecret::from(*scalar);
    let peer = X25519PublicKey::from(*point);
    secret.diffie_hellman(&peer).to_bytes()
}

pub fn x25519_public_key(private_key: &[u8; 32]) -> [u8; 32] {
    let secret = StaticSecret::from(*private_key);
    X25519PublicKey::from(&secret).to_bytes()
}

// ---- NIST P-256 (secp256r1) ECDH ------------------------------------------------------------------------------------

fn parse_p256_uncompressed(bytes: &[u8]) -> Result<p256::PublicKey, String> {
    if bytes.len() != 65 || bytes[0] != 4 {
        return Err("That push key is not a P-256 public key.".into());
    }
    p256::PublicKey::from_sec1_bytes(bytes)
        .map_err(|_| "That push key is not a P-256 public key.".to_owned())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct P256KeyPair {
    pub private_key: [u8; 32],
    pub public_key: [u8; 65],
}

pub fn p256_generate_keypair() -> P256KeyPair {
    loop {
        let mut priv_bytes = [0u8; 32];
        fill_random(&mut priv_bytes);
        if let Ok(secret) = p256::SecretKey::from_slice(&priv_bytes) {
            let encoded = secret.public_key().to_encoded_point(false);
            let mut public_key = [0u8; 65];
            public_key.copy_from_slice(encoded.as_bytes());
            return P256KeyPair {
                private_key: priv_bytes,
                public_key,
            };
        }
    }
}

pub fn p256_keypair_from_raw(private_b64: &str, public_b64: &str) -> Result<P256KeyPair, String> {
    let priv_vec = from_base64url(private_b64)?;
    let pub_vec = from_base64url(public_b64)?;
    if priv_vec.len() != 32 {
        return Err("Invalid P-256 private key length.".into());
    }
    let _ = p256::SecretKey::from_slice(&priv_vec)
        .map_err(|_| "Invalid P-256 private key.".to_owned())?;
    let _ = parse_p256_uncompressed(&pub_vec)?;
    let mut private_key = [0u8; 32];
    private_key.copy_from_slice(&priv_vec);
    let mut public_key = [0u8; 65];
    public_key.copy_from_slice(&pub_vec);
    Ok(P256KeyPair {
        private_key,
        public_key,
    })
}

pub fn p256_ecdh(private_key: &[u8; 32], peer_uncompressed: &[u8]) -> Result<[u8; 32], String> {
    let secret = p256::SecretKey::from_slice(private_key)
        .map_err(|_| "Invalid P-256 private key.".to_owned())?;
    let peer = parse_p256_uncompressed(peer_uncompressed)?;
    let shared = p256::ecdh::diffie_hellman(secret.to_nonzero_scalar(), peer.as_affine());
    let mut out = [0u8; 32];
    out.copy_from_slice(shared.raw_secret_bytes());
    Ok(out)
}

// ---- High-level relay-crypto.ts API --------------------------------------------------------------------------------

/// What a phone shows the relay to be let in while a code is open.
pub fn pairing_token(secret: &str) -> String {
    sha256_b64url(&format!("{RELAY_CRYPTO_VERSION}|pair-relay|{secret}"))
}

/// What a phone shows the PC inside its sealed hello: the relay never sees it.
pub fn pairing_proof(secret: &str) -> String {
    sha256_b64url(&format!("{RELAY_CRYPTO_VERSION}|pair-proof|{secret}"))
}

/// A device's id, cut from its public key.
pub fn device_id_of(public_key: &str) -> String {
    sha256_b64url(&format!("{RELAY_CRYPTO_VERSION}|device|{public_key}"))
        .chars()
        .take(22)
        .collect()
}

pub fn aad_of(room: &str, device: &str, direction: Direction) -> String {
    format!("{RELAY_CRYPTO_VERSION}|{room}|{device}|{}", direction.as_str())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcKeys {
    pub private_key: [u8; 32],
    pub public_key: String,
}

pub fn new_pc_keys() -> PcKeys {
    let mut private_key = [0u8; 32];
    fill_random(&mut private_key);
    let pub_raw = x25519_public_key(&private_key);
    PcKeys {
        private_key,
        public_key: to_base64url(&pub_raw),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LinkKeys {
    pub seal: [u8; 32],
    pub open: [u8; 32],
}

pub fn link_keys(
    private_key: &[u8; 32],
    peer_public_key: &str,
    room: &str,
    side: LinkSide,
) -> Result<LinkKeys, String> {
    let peer_bytes = from_base64url(peer_public_key)?;
    if peer_bytes.len() != 32 {
        return Err("Invalid X25519 public key length.".into());
    }
    let mut peer = [0u8; 32];
    peer.copy_from_slice(&peer_bytes);
    let shared = x25519(private_key, &peer);
    if shared.iter().all(|&b| b == 0) {
        return Err("That key locks nothing.".into());
    }
    let salt = format!("{RELAY_CRYPTO_VERSION}|{room}");
    let derive = |dir: Direction| -> [u8; 32] {
        let okm = hkdf_sha256(salt.as_bytes(), &shared, dir.as_str().as_bytes(), 32);
        let mut out = [0u8; 32];
        out.copy_from_slice(&okm);
        out
    };
    let (mine, theirs) = match side {
        LinkSide::Pc => (Direction::PcToDevice, Direction::DeviceToPc),
        LinkSide::Device => (Direction::DeviceToPc, Direction::PcToDevice),
    };
    Ok(LinkKeys {
        seal: derive(mine),
        open: derive(theirs),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedBox {
    pub n: String,
    pub c: String,
}

pub fn seal_json(key: &[u8; 32], value: &Value, aad: &str) -> Result<SealedBox, String> {
    let mut iv = [0u8; 12];
    fill_random(&mut iv);
    let plain = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    let sealed = aes_gcm_encrypt(key, &iv, aad.as_bytes(), &plain)?;
    Ok(SealedBox {
        n: to_base64url(&iv),
        c: to_base64url(&sealed),
    })
}

pub fn open_json(key: &[u8; 32], n: &str, c: &str, aad: &str) -> Result<Value, String> {
    let iv = from_base64url(n)?;
    let ct = from_base64url(c)?;
    let plain = aes_gcm_decrypt(key, &iv, aad.as_bytes(), &ct)?;
    serde_json::from_slice(&plain).map_err(|e| e.to_string())
}

#[cfg(test)]
#[path = "relay_crypto_tests.rs"]
mod relay_crypto_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn x25519_matches_rfc_7748_vector() {
        let hex = |s: &str| -> [u8; 32] {
            let mut out = [0u8; 32];
            for i in 0..32 {
                out[i] = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
            }
            out
        };
        let a_priv = hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let a_pub = hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
        let b_priv = hex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
        let b_pub = hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        let shared = hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(x25519_public_key(&a_priv), a_pub);
        assert_eq!(x25519_public_key(&b_priv), b_pub);
        assert_eq!(x25519(&a_priv, &b_pub), shared);
        assert_eq!(x25519(&b_priv, &a_pub), shared);
    }

    #[test]
    fn aes_gcm_round_trip_and_aad_binding() {
        let key = [0x42u8; 32];
        let nonce = [0x11u8; 12];
        let ct = aes_gcm_encrypt(&key, &nonce, b"aad-1", b"hello locust").unwrap();
        let pt = aes_gcm_decrypt(&key, &nonce, b"aad-1", &ct).unwrap();
        assert_eq!(pt, b"hello locust");
        assert!(aes_gcm_decrypt(&key, &nonce, b"aad-2", &ct).is_err());
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    // case: AES-GCM refuses a key that is neither 16 nor 32 bytes
    fn aes_gcm_refuses_a_key_that_is_neither_16_nor_32_bytes() {
        let nonce = [0x11u8; 12];
        assert!(aes_gcm_encrypt(&[0u8; 24], &nonce, b"", b"x").is_err());
        assert!(aes_gcm_decrypt(&[0u8; 24], &nonce, b"", &[0u8; 17]).is_err());
    }

    #[test]
    // case: AES-128-GCM matches NIST SP 800-38D test case 4
    fn aes_128_gcm_matches_nist_sp_800_38d_test_case_4() {
        let key = hex_bytes("feffe9928665731c6d6a8f9467308308");
        let iv = hex_bytes("cafebabefacedbaddecaf888");
        let pt = hex_bytes(
            "d9313225f88406e5a55909c5aff5269a\
             86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525\
             b16aedf5aa0de657ba637b39",
        );
        let aad = hex_bytes("feedfacedeadbeeffeedfacedeadbeefabaddad2");
        let expected_ct = hex_bytes(
            "42831ec2217774244b7221b784d0d49c\
             e3aa212f2c02a4e035c17e2329aca12e\
             21d514b25466931c7d8f6a5aac84aa05\
             1ba30b396a0aac973d58e091",
        );
        let expected_tag = hex_bytes("5bc94fbc3221a5db94fae95ae7121a47");
        let mut expected = expected_ct;
        expected.extend_from_slice(&expected_tag);

        let sealed = aes_gcm_encrypt(&key, &iv, &aad, &pt).unwrap();
        assert_eq!(sealed, expected);
        let opened = aes_gcm_decrypt(&key, &iv, &aad, &sealed).unwrap();
        assert_eq!(opened, pt);
    }

    #[test]
    // case: AES-256-GCM matches NIST SP 800-38D test case 16
    fn aes_256_gcm_matches_nist_sp_800_38d_test_case_16() {
        let key = hex_bytes(
            "feffe9928665731c6d6a8f9467308308\
             feffe9928665731c6d6a8f9467308308",
        );
        let iv = hex_bytes("cafebabefacedbaddecaf888");
        let pt = hex_bytes(
            "d9313225f88406e5a55909c5aff5269a\
             86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525\
             b16aedf5aa0de657ba637b39",
        );
        let aad = hex_bytes("feedfacedeadbeeffeedfacedeadbeefabaddad2");
        let expected_ct = hex_bytes(
            "522dc1f099567d07f47f37a32a84427d\
             643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838\
             c5f61e6393ba7a0abcc9f662",
        );
        let expected_tag = hex_bytes("76fc6ece0f4e1768cddf8853bb2d551b");
        let mut expected = expected_ct;
        expected.extend_from_slice(&expected_tag);

        let sealed = aes_gcm_encrypt(&key, &iv, &aad, &pt).unwrap();
        assert_eq!(sealed, expected);
        let opened = aes_gcm_decrypt(&key, &iv, &aad, &sealed).unwrap();
        assert_eq!(opened, pt);
    }

    #[test]
    // case: HKDF-SHA256 matches RFC 5869 test case 1
    fn hkdf_sha256_matches_rfc_5869_test_case_1() {
        let ikm = hex_bytes("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b");
        let salt = hex_bytes("000102030405060708090a0b0c");
        let info = hex_bytes("f0f1f2f3f4f5f6f7f8f9");
        let expected_okm = hex_bytes(
            "3cb25f25faacd57a90434f64d0362f2a\
             2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
             34007208d5b887185865",
        );
        assert_eq!(hkdf_sha256(&salt, &ikm, &info, 42), expected_okm);
    }
}
