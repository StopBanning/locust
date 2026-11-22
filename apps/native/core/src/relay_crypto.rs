//! THE LOCK ON YOUR PHONE'S MESSAGES (Electron `main/relay-crypto.ts`, 0.826).
//!
//! Pure-Rust implementation (no new crates beyond `sha2`) of:
//! - base64url (unpadded) & OS randomness
//! - HMAC-SHA256 & HKDF-SHA256 (RFC 5869)
//! - AES-128-GCM & AES-256-GCM (NIST SP 800-38D, 12-byte nonce)
//! - X25519 key agreement (RFC 7748)
//! - NIST P-256 (secp256r1) ECDH (for RFC 8291 Web Push)
//! - Locust relay key derivation, sealing, opening, and pairing tokens/proofs.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;

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
    #[cfg(unix)]
    {
        if let Ok(mut f) = std::fs::File::open("/dev/urandom")
            && f.read_exact(dest).is_ok()
        {
            return;
        }
    }
    // Fallback if /dev/urandom is unavailable (e.g. non-Unix without OS RNG wrapper):
    // mix high-resolution time + counter through SHA-256.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let mut offset = 0;
    while offset < dest.len() {
        let mut h = Sha256::new();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        h.update(now.to_le_bytes());
        h.update(
            COUNTER
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                .to_le_bytes(),
        );
        h.update(std::process::id().to_le_bytes());
        let block = h.finalize();
        let take = (dest.len() - offset).min(block.len());
        dest[offset..offset + take].copy_from_slice(&block[..take]);
        offset += take;
    }
}

pub fn random_text(bytes: usize) -> String {
    to_base64url(&random_bytes(bytes))
}

pub fn sha256_b64url(text: &str) -> String {
    to_base64url(&Sha256::digest(text.as_bytes()))
}

pub fn same_bytes(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

pub fn same_text(a: &str, b: &str) -> bool {
    same_bytes(a.as_bytes(), b.as_bytes())
}

// ---- HMAC-SHA256 & HKDF-SHA256 (RFC 5869) ---------------------------------------------------------------------------

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        let digest = Sha256::digest(key);
        k[..32].copy_from_slice(&digest);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOCK];
    let mut opad = [0x5cu8; BLOCK];
    for i in 0..BLOCK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    let mut out = [0u8; 32];
    out.copy_from_slice(&outer.finalize());
    out
}

pub fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let prk = if salt.is_empty() {
        hmac_sha256(&[0u8; 32], ikm)
    } else {
        hmac_sha256(salt, ikm)
    };
    let mut okm = Vec::with_capacity(len);
    let mut prev: Vec<u8> = Vec::new();
    let mut counter: u8 = 1;
    while okm.len() < len {
        let mut msg = Vec::with_capacity(prev.len() + info.len() + 1);
        msg.extend_from_slice(&prev);
        msg.extend_from_slice(info);
        msg.push(counter);
        let t = hmac_sha256(&prk, &msg);
        let need = (len - okm.len()).min(32);
        okm.extend_from_slice(&t[..need]);
        prev = t.to_vec();
        counter = counter.wrapping_add(1);
    }
    okm
}

// ---- AES-128 & AES-256 + GCM (NIST SP 800-38D) ---------------------------------------------------------------------

const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

const RCON: [u8; 11] = [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1b, 0x36];

struct AesKey {
    round_keys: Vec<[u8; 16]>,
}

impl AesKey {
    fn new(key: &[u8]) -> Result<Self, String> {
        let (nk, nr) = match key.len() {
            16 => (4usize, 10usize),
            32 => (8usize, 14usize),
            _ => return Err("AES key must be 16 or 32 bytes.".into()),
        };
        let mut w = vec![[0u8; 4]; 4 * (nr + 1)];
        for i in 0..nk {
            w[i].copy_from_slice(&key[4 * i..4 * i + 4]);
        }
        for i in nk..4 * (nr + 1) {
            let mut temp = w[i - 1];
            if i % nk == 0 {
                temp = [
                    SBOX[temp[1] as usize] ^ RCON[i / nk],
                    SBOX[temp[2] as usize],
                    SBOX[temp[3] as usize],
                    SBOX[temp[0] as usize],
                ];
            } else if nk > 6 && i % nk == 4 {
                temp = [
                    SBOX[temp[0] as usize],
                    SBOX[temp[1] as usize],
                    SBOX[temp[2] as usize],
                    SBOX[temp[3] as usize],
                ];
            }
            for b in 0..4 {
                w[i][b] = w[i - nk][b] ^ temp[b];
            }
        }
        let mut round_keys = Vec::with_capacity(nr + 1);
        for r in 0..=nr {
            let mut rk = [0u8; 16];
            for c in 0..4 {
                rk[4 * c..4 * c + 4].copy_from_slice(&w[4 * r + c]);
            }
            round_keys.push(rk);
        }
        Ok(Self { round_keys })
    }

    fn encrypt_block(&self, input: &[u8; 16]) -> [u8; 16] {
        let mut s = *input;
        add_round_key(&mut s, &self.round_keys[0]);
        let nr = self.round_keys.len() - 1;
        for r in 1..nr {
            sub_bytes(&mut s);
            shift_rows(&mut s);
            mix_columns(&mut s);
            add_round_key(&mut s, &self.round_keys[r]);
        }
        sub_bytes(&mut s);
        shift_rows(&mut s);
        add_round_key(&mut s, &self.round_keys[nr]);
        s
    }
}

fn add_round_key(s: &mut [u8; 16], rk: &[u8; 16]) {
    for i in 0..16 {
        s[i] ^= rk[i];
    }
}

fn sub_bytes(s: &mut [u8; 16]) {
    for b in s.iter_mut() {
        *b = SBOX[*b as usize];
    }
}

fn shift_rows(s: &mut [u8; 16]) {
    let t = *s;
    // Row 1: shift left by 1
    s[1] = t[5];
    s[5] = t[9];
    s[9] = t[13];
    s[13] = t[1];
    // Row 2: shift left by 2
    s[2] = t[10];
    s[6] = t[14];
    s[10] = t[2];
    s[14] = t[6];
    // Row 3: shift left by 3
    s[3] = t[15];
    s[7] = t[3];
    s[11] = t[7];
    s[15] = t[11];
}

#[inline]
fn xtime(a: u8) -> u8 {
    (a << 1) ^ (if (a & 0x80) != 0 { 0x1b } else { 0 })
}

fn mix_columns(s: &mut [u8; 16]) {
    for c in 0..4 {
        let i = 4 * c;
        let a0 = s[i];
        let a1 = s[i + 1];
        let a2 = s[i + 2];
        let a3 = s[i + 3];
        let t = a0 ^ a1 ^ a2 ^ a3;
        s[i] ^= t ^ xtime(a0 ^ a1);
        s[i + 1] ^= t ^ xtime(a1 ^ a2);
        s[i + 2] ^= t ^ xtime(a2 ^ a3);
        s[i + 3] ^= t ^ xtime(a3 ^ a0);
    }
}

fn gf128_mul(x: u128, y: u128) -> u128 {
    let mut z: u128 = 0;
    let mut v: u128 = y;
    for i in 0..128 {
        if ((x >> (127 - i)) & 1) != 0 {
            z ^= v;
        }
        let lsb = v & 1;
        v >>= 1;
        if lsb != 0 {
            v ^= 0xe100_0000_0000_0000_0000_0000_0000_0000u128;
        }
    }
    z
}

fn ghash(h_block: &[u8; 16], aad: &[u8], ciphertext: &[u8]) -> [u8; 16] {
    let h = u128::from_be_bytes(*h_block);
    let mut y: u128 = 0;
    for chunk in aad.chunks(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        y = gf128_mul(y ^ u128::from_be_bytes(block), h);
    }
    for chunk in ciphertext.chunks(16) {
        let mut block = [0u8; 16];
        block[..chunk.len()].copy_from_slice(chunk);
        y = gf128_mul(y ^ u128::from_be_bytes(block), h);
    }
    let mut len_block = [0u8; 16];
    len_block[..8].copy_from_slice(&((aad.len() as u64) * 8).to_be_bytes());
    len_block[8..].copy_from_slice(&((ciphertext.len() as u64) * 8).to_be_bytes());
    y = gf128_mul(y ^ u128::from_be_bytes(len_block), h);
    y.to_be_bytes()
}

fn inc32(counter: &mut [u8; 16]) {
    let c = u32::from_be_bytes([counter[12], counter[13], counter[14], counter[15]]).wrapping_add(1);
    counter[12..16].copy_from_slice(&c.to_be_bytes());
}

fn gctr(aes: &AesKey, icb: [u8; 16], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut cb = icb;
    for chunk in data.chunks(16) {
        let ks = aes.encrypt_block(&cb);
        for i in 0..chunk.len() {
            out.push(chunk[i] ^ ks[i]);
        }
        inc32(&mut cb);
    }
    out
}

/// AES-128-GCM or AES-256-GCM encrypt with a 12-byte nonce. Appends the 16-byte tag to the ciphertext.
pub fn aes_gcm_encrypt(key: &[u8], nonce: &[u8], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, String> {
    if nonce.len() != 12 {
        return Err("AES-GCM nonce must be 12 bytes.".into());
    }
    let aes = AesKey::new(key)?;
    let h = aes.encrypt_block(&[0u8; 16]);
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;
    let mut icb = j0;
    inc32(&mut icb);
    let mut ct = gctr(&aes, icb, plaintext);
    let s = ghash(&h, aad, &ct);
    let j0_enc = aes.encrypt_block(&j0);
    let mut tag = [0u8; 16];
    for i in 0..16 {
        tag[i] = s[i] ^ j0_enc[i];
    }
    ct.extend_from_slice(&tag);
    Ok(ct)
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
    let aes = AesKey::new(key)?;
    let h = aes.encrypt_block(&[0u8; 16]);
    let mut j0 = [0u8; 16];
    j0[..12].copy_from_slice(nonce);
    j0[15] = 1;
    let (ct, tag) = ciphertext_and_tag.split_at(ciphertext_and_tag.len() - 16);
    let s = ghash(&h, aad, ct);
    let j0_enc = aes.encrypt_block(&j0);
    let mut expected_tag = [0u8; 16];
    for i in 0..16 {
        expected_tag[i] = s[i] ^ j0_enc[i];
    }
    if !same_bytes(tag, &expected_tag) {
        return Err("Authentication tag mismatch.".into());
    }
    let mut icb = j0;
    inc32(&mut icb);
    Ok(gctr(&aes, icb, ct))
}

// ---- X25519 (RFC 7748, TweetNaCl radix-2^16 field arithmetic) ------------------------------------------------------

type Gf25519 = [i64; 16];

fn gf_zero() -> Gf25519 {
    [0i64; 16]
}

fn car25519(o: &mut Gf25519) {
    for i in 0..16 {
        o[i] += 1 << 16;
        let c = o[i] >> 16;
        o[(i + 1) * usize::from(i < 15)] += c - 1 + 37 * (c - 1) * i64::from(i == 15);
        o[i] -= c << 16;
    }
}

fn sel25519(p: &mut Gf25519, q: &mut Gf25519, b: i64) {
    let c = !(b - 1);
    for i in 0..16 {
        let t = c & (p[i] ^ q[i]);
        p[i] ^= t;
        q[i] ^= t;
    }
}

fn pack25519(n: &Gf25519) -> [u8; 32] {
    let mut m = gf_zero();
    let mut t = *n;
    car25519(&mut t);
    car25519(&mut t);
    car25519(&mut t);
    for _ in 0..2 {
        m[0] = t[0] - 0xffed;
        for i in 1..15 {
            m[i] = t[i] - 0xffff - ((m[i - 1] >> 16) & 1);
            m[i - 1] &= 0xffff;
        }
        m[15] = t[15] - 0x7fff - ((m[14] >> 16) & 1);
        let b = (m[15] >> 16) & 1;
        m[14] &= 0xffff;
        sel25519(&mut t, &mut m, 1 - b);
    }
    let mut o = [0u8; 32];
    for i in 0..16 {
        o[2 * i] = (t[i] & 0xff) as u8;
        o[2 * i + 1] = ((t[i] >> 8) & 0xff) as u8;
    }
    o
}

fn unpack25519(n: &[u8; 32]) -> Gf25519 {
    let mut o = gf_zero();
    for i in 0..16 {
        o[i] = (n[2 * i] as i64) + ((n[2 * i + 1] as i64) << 8);
    }
    o[15] &= 0x7fff;
    o
}

fn gf_add(a: &Gf25519, b: &Gf25519) -> Gf25519 {
    let mut o = gf_zero();
    for i in 0..16 {
        o[i] = a[i] + b[i];
    }
    o
}

fn gf_sub(a: &Gf25519, b: &Gf25519) -> Gf25519 {
    let mut o = gf_zero();
    for i in 0..16 {
        o[i] = a[i] - b[i];
    }
    o
}

fn gf_mul(a: &Gf25519, b: &Gf25519) -> Gf25519 {
    let mut t = [0i64; 31];
    for i in 0..16 {
        for j in 0..16 {
            t[i + j] += a[i] * b[j];
        }
    }
    for i in 0..15 {
        t[i] += 38 * t[i + 16];
    }
    let mut o = gf_zero();
    o.copy_from_slice(&t[..16]);
    car25519(&mut o);
    car25519(&mut o);
    o
}

fn gf_sqr(a: &Gf25519) -> Gf25519 {
    gf_mul(a, a)
}

fn inv25519(i: &Gf25519) -> Gf25519 {
    let mut c = *i;
    for a in (0..=253).rev() {
        c = gf_sqr(&c);
        if a != 2 && a != 4 {
            c = gf_mul(&c, i);
        }
    }
    c
}

pub fn x25519(scalar: &[u8; 32], point: &[u8; 32]) -> [u8; 32] {
    let mut z = *scalar;
    z[0] &= 248;
    z[31] &= 127;
    z[31] |= 64;

    let x = unpack25519(point);
    let mut a = gf_zero();
    a[0] = 1;
    let mut b = x;
    let mut c = gf_zero();
    let mut d = gf_zero();
    d[0] = 1;
    let mut g121665 = gf_zero();
    g121665[0] = 0xdb41;
    g121665[1] = 1;

    for i in (0..=254).rev() {
        let r = ((z[i >> 3] >> (i & 7)) & 1) as i64;
        sel25519(&mut a, &mut b, r);
        sel25519(&mut c, &mut d, r);
        let e = gf_add(&a, &c);
        a = gf_sub(&a, &c);
        c = gf_add(&b, &d);
        b = gf_sub(&b, &d);
        d = gf_sqr(&e);
        let f = gf_sqr(&a);
        a = gf_mul(&c, &a);
        c = gf_mul(&b, &e);
        let e2 = gf_add(&a, &c);
        a = gf_sub(&a, &c);
        b = gf_sqr(&a);
        c = gf_sub(&d, &f);
        a = gf_mul(&c, &g121665);
        a = gf_add(&a, &d);
        c = gf_mul(&c, &a);
        a = gf_mul(&d, &f);
        d = gf_mul(&b, &x);
        b = gf_sqr(&e2);
        sel25519(&mut a, &mut b, r);
        sel25519(&mut c, &mut d, r);
    }
    let inv = inv25519(&c);
    let res = gf_mul(&a, &inv);
    pack25519(&res)
}

pub fn x25519_public_key(private_key: &[u8; 32]) -> [u8; 32] {
    let mut base = [0u8; 32];
    base[0] = 9;
    x25519(private_key, &base)
}

// ---- NIST P-256 (secp256r1) ECDH ------------------------------------------------------------------------------------

/// Field element mod p = 2^256 - 2^224 + 2^192 + 2^96 - 1, stored in Montgomery form (little-endian `[u64; 4]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct P256Fe([u64; 4]);

const P256_P: [u64; 4] = [
    0xffff_ffff_ffff_ffff,
    0x0000_0000_ffff_ffff,
    0x0000_0000_0000_0000,
    0xffff_ffff_0000_0001,
];

const P256_N: [u64; 4] = [
    0xf3b9_cac2_fc63_2551,
    0xbce6_faad_a717_9e84,
    0xffff_ffff_ffff_ffff,
    0xffff_ffff_0000_0000,
];

/// R mod p = 2^256 mod p = 2^224 - 2^192 - 2^96 + 1
const P256_R: [u64; 4] = [
    0x0000_0000_0000_0001,
    0xffff_ffff_0000_0000,
    0xffff_ffff_ffff_ffff,
    0x0000_0000_ffff_fffe,
];

/// R^2 mod p
const P256_R2: [u64; 4] = [
    0x0000_0000_0000_0003,
    0xffff_fffb_ffff_ffff,
    0xffff_ffff_ffff_fffe,
    0x0000_0004_ffff_fffd,
];

const P256_B_RAW: [u64; 4] = [
    0x3bce_3c3e_27d2_604b,
    0x651d_06b0_cc53_b0f6,
    0xb3eb_bd55_7698_86bc,
    0x5ac6_35d8_aa3a_93e7,
];

const P256_GX_RAW: [u64; 4] = [
    0xf4a1_3945_d898_c296,
    0x7703_7d81_2deb_33a0,
    0xf8bc_e6e5_63a4_40f2,
    0x6b17_d1f2_e12c_4247,
];

const P256_GY_RAW: [u64; 4] = [
    0xcbb6_4068_37bf_51f5,
    0x2bce_3357_6b31_5ece,
    0x8ee7_eb4a_7c0f_9e16,
    0x4fe3_42e2_fe1a_7f9b,
];

fn u256_gte(a: &[u64; 4], b: &[u64; 4]) -> bool {
    for i in (0..4).rev() {
        if a[i] != b[i] {
            return a[i] > b[i];
        }
    }
    true
}

fn u256_add_raw(a: &[u64; 4], b: &[u64; 4]) -> ([u64; 4], u64) {
    let mut out = [0u64; 4];
    let mut carry = 0u128;
    for i in 0..4 {
        let sum = (a[i] as u128) + (b[i] as u128) + carry;
        out[i] = sum as u64;
        carry = sum >> 64;
    }
    (out, carry as u64)
}

fn u256_sub_raw(a: &[u64; 4], b: &[u64; 4]) -> ([u64; 4], u64) {
    let mut out = [0u64; 4];
    let mut borrow = 0u128;
    for i in 0..4 {
        let ai = a[i] as u128;
        let bi = (b[i] as u128) + borrow;
        if ai >= bi {
            out[i] = (ai - bi) as u64;
            borrow = 0;
        } else {
            out[i] = ((1u128 << 64) + ai - bi) as u64;
            borrow = 1;
        }
    }
    (out, borrow as u64)
}

impl P256Fe {
    const ZERO: Self = Self([0; 4]);
    const ONE: Self = Self(P256_R);

    fn is_zero(self) -> bool {
        self.0 == [0; 4]
    }

    fn from_raw(raw: [u64; 4]) -> Self {
        Self(raw).mul(Self(P256_R2))
    }

    fn to_raw(self) -> [u64; 4] {
        self.mul(Self([1, 0, 0, 0])).0
    }

    fn from_be_bytes(bytes: &[u8; 32]) -> Result<Self, String> {
        let raw = [
            u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
            u64::from_be_bytes(bytes[16..24].try_into().unwrap()),
            u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
            u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
        ];
        if u256_gte(&raw, &P256_P) {
            return Err("P-256 coordinate out of range.".into());
        }
        Ok(Self::from_raw(raw))
    }

    fn to_be_bytes(self) -> [u8; 32] {
        let raw = self.to_raw();
        let mut out = [0u8; 32];
        out[0..8].copy_from_slice(&raw[3].to_be_bytes());
        out[8..16].copy_from_slice(&raw[2].to_be_bytes());
        out[16..24].copy_from_slice(&raw[1].to_be_bytes());
        out[24..32].copy_from_slice(&raw[0].to_be_bytes());
        out
    }

    fn add(self, rhs: Self) -> Self {
        let (sum, carry) = u256_add_raw(&self.0, &rhs.0);
        if carry != 0 || u256_gte(&sum, &P256_P) {
            Self(u256_sub_raw(&sum, &P256_P).0)
        } else {
            Self(sum)
        }
    }

    fn sub(self, rhs: Self) -> Self {
        let (diff, borrow) = u256_sub_raw(&self.0, &rhs.0);
        if borrow != 0 {
            Self(u256_add_raw(&diff, &P256_P).0)
        } else {
            Self(diff)
        }
    }

    fn double(self) -> Self {
        self.add(self)
    }

    /// CIOS Montgomery multiplication. Since `P256_P[0] = 2^64 - 1`, `-p^{-1} mod 2^64 = 1`, so `m = t[0]`.
    fn mul(self, rhs: Self) -> Self {
        let mut t = [0u64; 6];
        for i in 0..4 {
            // t += self.0[i] * rhs.0
            let mut carry = 0u128;
            for j in 0..4 {
                let prod = (self.0[i] as u128) * (rhs.0[j] as u128) + (t[j] as u128) + carry;
                t[j] = prod as u64;
                carry = prod >> 64;
            }
            let sum = (t[4] as u128) + carry;
            t[4] = sum as u64;
            t[5] = (sum >> 64) as u64;

            // Montgomery reduction step: m = t[0] * 1 mod 2^64 = t[0]
            let m = t[0] as u128;
            let mut carry = (m * (P256_P[0] as u128) + (t[0] as u128)) >> 64;
            for j in 1..4 {
                let prod = m * (P256_P[j] as u128) + (t[j] as u128) + carry;
                t[j - 1] = prod as u64;
                carry = prod >> 64;
            }
            let sum = (t[4] as u128) + carry;
            t[3] = sum as u64;
            t[4] = t[5] + ((sum >> 64) as u64);
            t[5] = 0;
        }
        let res = [t[0], t[1], t[2], t[3]];
        if t[4] != 0 || u256_gte(&res, &P256_P) {
            Self(u256_sub_raw(&res, &P256_P).0)
        } else {
            Self(res)
        }
    }

    fn sqr(self) -> Self {
        self.mul(self)
    }

    /// Modular inverse via Fermat's little theorem: a^(p-2) mod p.
    fn inv(self) -> Self {
        // p - 2 = 0xffffffff_00000001_00000000_00000000_00000000_ffffffff_ffffffff_fffffffd
        let exp: [u64; 4] = [
            0xffff_ffff_ffff_fffd,
            0x0000_0000_ffff_ffff,
            0x0000_0000_0000_0000,
            0xffff_ffff_0000_0001,
        ];
        let mut res = Self::ONE;
        let mut base = self;
        for limb in exp {
            for bit in 0..64 {
                if ((limb >> bit) & 1) != 0 {
                    res = res.mul(base);
                }
                base = base.sqr();
            }
        }
        res
    }
}

#[derive(Clone, Copy, Debug)]
struct P256Point {
    x: P256Fe,
    y: P256Fe,
    z: P256Fe,
}

impl P256Point {
    const INFINITY: Self = Self {
        x: P256Fe::ZERO,
        y: P256Fe::ONE,
        z: P256Fe::ZERO,
    };

    fn is_infinity(self) -> bool {
        self.z.is_zero()
    }

    fn generator() -> Self {
        Self {
            x: P256Fe::from_raw(P256_GX_RAW),
            y: P256Fe::from_raw(P256_GY_RAW),
            z: P256Fe::ONE,
        }
    }

    fn from_uncompressed(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != 65 || bytes[0] != 4 {
            return Err("That push key is not a P-256 public key.".into());
        }
        let x = P256Fe::from_be_bytes(bytes[1..33].try_into().unwrap())?;
        let y = P256Fe::from_be_bytes(bytes[33..65].try_into().unwrap())?;
        // Check y^2 == x^3 - 3x + b
        let lhs = y.sqr();
        let x2 = x.sqr();
        let x3 = x2.mul(x);
        let three_x = x.double().add(x);
        let b = P256Fe::from_raw(P256_B_RAW);
        let rhs = x3.sub(three_x).add(b);
        if lhs != rhs {
            return Err("That push key is not a P-256 public key.".into());
        }
        Ok(Self { x, y, z: P256Fe::ONE })
    }

    fn to_affine(self) -> Option<(P256Fe, P256Fe)> {
        if self.is_infinity() {
            return None;
        }
        let z_inv = self.z.inv();
        let z_inv2 = z_inv.sqr();
        let z_inv3 = z_inv2.mul(z_inv);
        Some((self.x.mul(z_inv2), self.y.mul(z_inv3)))
    }

    fn to_uncompressed(self) -> Option<[u8; 65]> {
        let (x, y) = self.to_affine()?;
        let mut out = [0u8; 65];
        out[0] = 4;
        out[1..33].copy_from_slice(&x.to_be_bytes());
        out[33..65].copy_from_slice(&y.to_be_bytes());
        Some(out)
    }

    /// Jacobian doubling for a = -3 (EFD dbl-2001-b).
    fn double(self) -> Self {
        if self.is_infinity() || self.y.is_zero() {
            return Self::INFINITY;
        }
        let delta = self.z.sqr();
        let gamma = self.y.sqr();
        let beta = self.x.mul(gamma);
        let alpha_t = self.x.sub(delta).mul(self.x.add(delta));
        let alpha = alpha_t.double().add(alpha_t);
        let beta4 = beta.double().double();
        let beta8 = beta4.double();
        let x3 = alpha.sqr().sub(beta8);
        let z3 = self.y.add(self.z).sqr().sub(gamma).sub(delta);
        let gamma2 = gamma.sqr();
        let gamma2_8 = gamma2.double().double().double();
        let y3 = alpha.mul(beta4.sub(x3)).sub(gamma2_8);
        Self { x: x3, y: y3, z: z3 }
    }

    /// General Jacobian addition (EFD add-2007-bl).
    fn add(self, rhs: Self) -> Self {
        if self.is_infinity() {
            return rhs;
        }
        if rhs.is_infinity() {
            return self;
        }
        let z1z1 = self.z.sqr();
        let z2z2 = rhs.z.sqr();
        let u1 = self.x.mul(z2z2);
        let u2 = rhs.x.mul(z1z1);
        let s1 = self.y.mul(rhs.z).mul(z2z2);
        let s2 = rhs.y.mul(self.z).mul(z1z1);
        if u1 == u2 {
            if s1 == s2 {
                return self.double();
            } else {
                return Self::INFINITY;
            }
        }
        let h = u2.sub(u1);
        let i = h.double().sqr();
        let j = h.mul(i);
        let r = s2.sub(s1).double();
        let v = u1.mul(i);
        let x3 = r.sqr().sub(j).sub(v.double());
        let y3 = r.mul(v.sub(x3)).sub(s1.mul(j).double());
        let z3 = self.z.add(rhs.z).sqr().sub(z1z1).sub(z2z2).mul(h);
        Self { x: x3, y: y3, z: z3 }
    }

    fn scalar_mul(self, scalar_be: &[u8; 32]) -> Self {
        let mut acc = Self::INFINITY;
        for &byte in scalar_be {
            for bit in (0..8).rev() {
                acc = acc.double();
                if ((byte >> bit) & 1) != 0 {
                    acc = acc.add(self);
                }
            }
        }
        acc
    }
}

fn p256_valid_scalar(bytes: &[u8; 32]) -> bool {
    let limbs = [
        u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
        u64::from_be_bytes(bytes[16..24].try_into().unwrap()),
        u64::from_be_bytes(bytes[8..16].try_into().unwrap()),
        u64::from_be_bytes(bytes[0..8].try_into().unwrap()),
    ];
    limbs != [0; 4] && !u256_gte(&limbs, &P256_N)
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
        if !p256_valid_scalar(&priv_bytes) {
            continue;
        }
        if let Some(pub_bytes) = P256Point::generator().scalar_mul(&priv_bytes).to_uncompressed() {
            return P256KeyPair {
                private_key: priv_bytes,
                public_key: pub_bytes,
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
    let _ = P256Point::from_uncompressed(&pub_vec)?;
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
    let peer = P256Point::from_uncompressed(peer_uncompressed)?;
    let shared = peer.scalar_mul(private_key);
    let (x, _) = shared
        .to_affine()
        .ok_or_else(|| "P-256 ECDH produced point at infinity.".to_owned())?;
    Ok(x.to_be_bytes())
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
}
