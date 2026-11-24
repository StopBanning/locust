//! YOUR LOCUST IS KEPT CURRENT AT THE RELAY
//! (Electron `main/profile-sync-crypto.ts` + `main/profile-sync.ts`).
//!
//! Seals the profile's backup set (`profile_backup::carried_files`) in `CHUNK_BYTES` pieces with
//! AES-256-GCM (keys from HKDF-SHA256, chunk IDs from HMAC-SHA256), sends only changed pieces,
//! writes the manifest last, removes obsolete pieces after, and pulls another computer's copy
//! into a backup folder readable by `profile_backup::read_backup`.

use crate::profile_backup::{
    BACKUP_MANIFEST, BACKUP_SCHEMA, BackupCounts, LEFT_OUT, carried_files, carries, count_profile,
};
use crate::relay_crypto::{
    aes_gcm_decrypt, aes_gcm_encrypt, fill_random, from_base64url, hkdf_sha256, hmac_sha256,
    random_bytes, sha256_b64url, to_base64url, x25519, x25519_public_key,
};
use chrono::{DateTime, Utc};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const VERSION: &str = "locust-sync-v1";
pub const CHUNK_BYTES: usize = 4 * 1024 * 1024;
pub const SEAL_OVERHEAD: usize = 12 + 16;
pub const PROFILE_KEY_BYTES: usize = 32;
pub const PROFILE_SYNC_FILE: &str = "profile-sync.json";
pub const SYNC_SCHEMA: u32 = 1;
pub const SYNC_QUIET_MS: u64 = 60_000;

pub fn manifest_id() -> String {
    sha256_b64url(&format!("{VERSION}|manifest"))
}

pub fn new_profile_key() -> [u8; PROFILE_KEY_BYTES] {
    let mut k = [0u8; PROFILE_KEY_BYTES];
    fill_random(&mut k);
    k
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncKeys {
    pub seal: [u8; 32],
    pub name: [u8; 32],
    pub account: u64,
}

pub fn sync_keys(profile_key: &[u8], account: u64) -> Result<SyncKeys, String> {
    if profile_key.len() != PROFILE_KEY_BYTES {
        return Err("A profile key is 32 bytes.".into());
    }
    let seal_vec = hkdf_sha256(VERSION.as_bytes(), profile_key, b"seal", 32);
    let name_vec = hkdf_sha256(VERSION.as_bytes(), profile_key, b"name", 32);
    let mut seal = [0u8; 32];
    let mut name = [0u8; 32];
    seal.copy_from_slice(&seal_vec);
    name.copy_from_slice(&name_vec);
    Ok(SyncKeys { seal, name, account })
}

pub fn file_id_of(keys: &SyncKeys, name: &str) -> String {
    let mac = hmac_sha256(&keys.name, format!("{VERSION}|{name}").as_bytes());
    to_base64url(&mac)
}

pub fn chunk_id_of(keys: &SyncKeys, path: &str, index: usize) -> String {
    file_id_of(keys, &format!("file|{path}|{index}"))
}

fn aad_of(keys: &SyncKeys, id: &str) -> Vec<u8> {
    format!("{VERSION}|{}|{id}", keys.account).into_bytes()
}

pub fn seal_file(keys: &SyncKeys, id: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    let nonce = random_bytes(12);
    let aad = aad_of(keys, id);
    let sealed = aes_gcm_encrypt(&keys.seal, &nonce, &aad, bytes)?;
    let mut out = Vec::with_capacity(12 + sealed.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&sealed);
    Ok(out)
}

pub fn open_file(keys: &SyncKeys, id: &str, boxed: &[u8]) -> Result<Vec<u8>, String> {
    if boxed.len() < SEAL_OVERHEAD {
        return Err("That box is too short to hold anything.".into());
    }
    let aad = aad_of(keys, id);
    aes_gcm_decrypt(&keys.seal, &boxed[..12], &aad, &boxed[12..])
}

pub fn chunks_of(size: usize) -> Vec<(usize, usize)> {
    if size == 0 {
        return vec![(0, 0)];
    }
    let mut pieces = Vec::new();
    let mut start = 0;
    while start < size {
        let end = (start + CHUNK_BYTES).min(size);
        pieces.push((start, end));
        start = end;
    }
    pieces
}

// ---- Recovery code (Crockford base32 + 2-byte SHA-256 check) --------------------------------------------------------

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Encodes a 32-byte profile key with a 2-byte SHA-256 check in Crockford base32 (55 chars in groups of 5).
pub fn recovery_code(profile_key: &[u8]) -> Result<String, String> {
    if profile_key.len() != PROFILE_KEY_BYTES {
        return Err("A profile key is 32 bytes.".into());
    }
    let digest = Sha256::digest(profile_key);
    let mut bytes = [0u8; PROFILE_KEY_BYTES + 2];
    bytes[..PROFILE_KEY_BYTES].copy_from_slice(profile_key);
    bytes[PROFILE_KEY_BYTES] = digest[0];
    bytes[PROFILE_KEY_BYTES + 1] = digest[1];

    let mut bits: u32 = 0;
    let mut value: u32 = 0;
    let mut raw = String::with_capacity(55);
    for byte in bytes {
        value = (value << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            raw.push(CROCKFORD[((value >> (bits - 5)) & 31) as usize] as char);
            bits -= 5;
        }
        value &= (1u32 << bits) - 1;
    }
    if bits > 0 {
        raw.push(CROCKFORD[((value << (5 - bits)) & 31) as usize] as char);
    }
    let mut grouped = String::with_capacity(55 + 10);
    for (i, ch) in raw.chars().enumerate() {
        if i > 0 && i % 5 == 0 {
            grouped.push('-');
        }
        grouped.push(ch);
    }
    Ok(grouped)
}

/// Decodes a Crockford base32 recovery code back into its 32-byte profile key, or `None` if malformed or check fails.
pub fn profile_key_from_code(code: &str) -> Option<[u8; PROFILE_KEY_BYTES]> {
    let mut cleaned = String::with_capacity(55);
    for ch in code.chars() {
        if ch.is_whitespace() || ch == '-' {
            continue;
        }
        let upper = ch.to_ascii_uppercase();
        let mapped = match upper {
            'I' | 'L' => '1',
            'O' => '0',
            c => c,
        };
        cleaned.push(mapped);
    }
    if cleaned.len() != 55 {
        return None;
    }
    let mut bytes = Vec::with_capacity(PROFILE_KEY_BYTES + 2);
    let mut bits: u32 = 0;
    let mut value: u32 = 0;
    for ch in cleaned.bytes() {
        let idx = CROCKFORD.iter().position(|&c| c == ch)? as u32;
        value = (value << 5) | idx;
        bits += 5;
        if bits >= 8 {
            bytes.push(((value >> (bits - 8)) & 0xff) as u8);
            bits -= 8;
            value &= (1u32 << bits) - 1;
        }
    }
    if bytes.len() != PROFILE_KEY_BYTES + 2 {
        return None;
    }
    let mut key = [0u8; PROFILE_KEY_BYTES];
    key.copy_from_slice(&bytes[..PROFILE_KEY_BYTES]);
    let digest = Sha256::digest(key);
    if digest[0] == bytes[PROFILE_KEY_BYTES] && digest[1] == bytes[PROFILE_KEY_BYTES + 1] {
        Some(key)
    } else {
        None
    }
}

// ---- Slice 4b: Approving a new computer (X25519 + HKDF-SHA256 + AES-256-GCM) ----------------------------------------

/// Where a new computer leaves its ask in its own room at the relay.
pub fn ask_id() -> String {
    sha256_b64url(&format!("{VERSION}|ask"))
}

/// Where an approving computer leaves its grant for `room` in its own place at the relay.
pub fn grant_id_of(room: &str) -> String {
    sha256_b64url(&format!("{VERSION}|grant|{room}"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AskKeys {
    pub private_key: [u8; 32],
    /// Raw X25519 public key, base64url (43 chars).
    pub public_key: String,
}

pub fn new_ask_keys() -> AskKeys {
    let mut private_key = [0u8; 32];
    fill_random(&mut private_key);
    let pub_raw = x25519_public_key(&private_key);
    AskKeys {
        private_key,
        public_key: to_base64url(&pub_raw),
    }
}

/// The code both screens show for one ask's key: `XXXX-XXXX` in Crockford base32 (40 bits).
pub fn match_of(public_key: &str) -> String {
    let digest = Sha256::digest(format!("{VERSION}|match|{public_key}").as_bytes());
    let mut bits: u64 = 0;
    for &byte in &digest[..5] {
        bits = (bits << 8) | u64::from(byte);
    }
    let mut text = String::with_capacity(9);
    for i in 0..8 {
        if i == 4 {
            text.push('-');
        }
        let shift = 35 - i * 5;
        text.push(CROCKFORD[((bits >> shift) & 31) as usize] as char);
    }
    text
}

fn grant_info(from: &str, to: &str) -> String {
    format!("{VERSION}|grant|{from}|{to}")
}

fn grant_key(private_key: &[u8; 32], peer_public_b64: &str, info: &str) -> Result<[u8; 32], String> {
    let peer_vec = from_base64url(peer_public_b64)?;
    if peer_vec.len() != 32 {
        return Err("Invalid X25519 public key length.".into());
    }
    let mut peer = [0u8; 32];
    peer.copy_from_slice(&peer_vec);
    let shared = x25519(private_key, &peer);
    if shared.iter().all(|&b| b == 0) {
        return Err("That key locks nothing.".into());
    }
    let okm = hkdf_sha256(VERSION.as_bytes(), &shared, info.as_bytes(), 32);
    let mut out = [0u8; 32];
    out.copy_from_slice(&okm);
    Ok(out)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenedGrant {
    pub profile_key: [u8; PROFILE_KEY_BYTES],
    pub from: String,
}

/// Seals the profile key to an ask's X25519 public key, bound to both rooms (`from` -> `to`).
pub fn seal_grant(
    profile_key: &[u8],
    ask_public_key: &str,
    from: &str,
    to: &str,
) -> Result<Vec<u8>, String> {
    if profile_key.len() != PROFILE_KEY_BYTES {
        return Err("A profile key is 32 bytes.".into());
    }
    let mine = new_ask_keys();
    let info = grant_info(from, to);
    let key = grant_key(&mine.private_key, ask_public_key, &info)?;
    let nonce = random_bytes(12);
    let sealed = aes_gcm_encrypt(&key, &nonce, info.as_bytes(), profile_key)?;
    let body = json!({
        "v": 1,
        "from": from,
        "key": mine.public_key,
        "n": to_base64url(&nonce),
        "c": to_base64url(&sealed),
    });
    serde_json::to_vec(&body).map_err(|e| e.to_string())
}

/// Opens a grant for room `to` using this computer's ask key, or returns `None`.
pub fn open_grant(grant: &[u8], ask: &AskKeys, to: &str) -> Option<OpenedGrant> {
    let val: Value = serde_json::from_slice(grant).ok()?;
    let obj = val.as_object()?;
    if obj.get("v")?.as_u64()? != 1 {
        return None;
    }
    let from = obj.get("from")?.as_str()?;
    let peer_key = obj.get("key")?.as_str()?;
    let n = obj.get("n")?.as_str()?;
    let c = obj.get("c")?.as_str()?;
    let info = grant_info(from, to);
    let key = grant_key(&ask.private_key, peer_key, &info).ok()?;
    let nonce = from_base64url(n).ok()?;
    let ct = from_base64url(c).ok()?;
    let opened = aes_gcm_decrypt(&key, &nonce, info.as_bytes(), &ct).ok()?;
    if opened.len() != PROFILE_KEY_BYTES {
        return None;
    }
    let mut profile_key = [0u8; PROFILE_KEY_BYTES];
    profile_key.copy_from_slice(&opened);
    Some(OpenedGrant {
        profile_key,
        from: from.to_owned(),
    })
}

// ---- Sync types & traits --------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncPiece {
    pub bytes: usize,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncedFile {
    pub path: String,
    pub bytes: usize,
    pub sha256: String,
    pub pieces: Vec<SyncPiece>,
}

impl SyncedFile {
    fn to_json(&self) -> Value {
        json!({
            "path": self.path,
            "bytes": self.bytes,
            "sha256": self.sha256,
            "pieces": self.pieces.iter().map(|p| json!({ "bytes": p.bytes, "sha256": p.sha256 })).collect::<Vec<_>>(),
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let path = obj.get("path")?.as_str()?.to_owned();
        let bytes = obj.get("bytes")?.as_u64()? as usize;
        let sha256 = obj.get("sha256")?.as_str()?.to_owned();
        let pieces = obj
            .get("pieces")?
            .as_array()?
            .iter()
            .map(|p| {
                Some(SyncPiece {
                    bytes: p.get("bytes")?.as_u64()? as usize,
                    sha256: p.get("sha256")?.as_str()?.to_owned(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            path,
            bytes,
            sha256,
            pieces,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncManifest {
    pub schema: u32,
    pub app: String,
    pub app_version: String,
    pub created_at: String,
    pub computer: String,
    pub counts: BackupCounts,
    pub files: Vec<SyncedFile>,
}

impl SyncManifest {
    fn to_json(&self) -> Value {
        json!({
            "schema": self.schema,
            "app": self.app,
            "appVersion": self.app_version,
            "createdAt": self.created_at,
            "computer": self.computer,
            "counts": self.counts.to_json(),
            "files": self.files.iter().map(SyncedFile::to_json).collect::<Vec<_>>(),
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let schema = obj.get("schema")?.as_u64()? as u32;
        let app = obj.get("app")?.as_str()?.to_owned();
        let app_version = obj.get("appVersion").and_then(Value::as_str).unwrap_or("").to_owned();
        let created_at = obj.get("createdAt").and_then(Value::as_str).unwrap_or("").to_owned();
        let computer = obj.get("computer").and_then(Value::as_str).unwrap_or("").to_owned();
        let counts = obj.get("counts").map(BackupCounts::from_json).unwrap_or_default();
        let files = obj
            .get("files")?
            .as_array()?
            .iter()
            .map(SyncedFile::from_json)
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            schema,
            app,
            app_version,
            created_at,
            computer,
            counts,
            files,
        })
    }
}

#[derive(Clone, Debug)]
struct SyncState {
    schema: u32,
    account: u64,
    room: String,
    pushed_at: String,
    files: Vec<SyncedFile>,
}

impl SyncState {
    fn to_json(&self) -> Value {
        json!({
            "schema": self.schema,
            "account": self.account,
            "room": self.room,
            "pushedAt": self.pushed_at,
            "files": self.files.iter().map(SyncedFile::to_json).collect::<Vec<_>>(),
        })
    }

    fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        let schema = obj.get("schema")?.as_u64()? as u32;
        let account = obj.get("account")?.as_u64()?;
        let room = obj.get("room")?.as_str()?.to_owned();
        let pushed_at = obj.get("pushedAt")?.as_str()?.to_owned();
        let files = obj
            .get("files")?
            .as_array()?
            .iter()
            .map(SyncedFile::from_json)
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            schema,
            account,
            room,
            pushed_at,
            files,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncRefused {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for SyncRefused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub trait SyncRemote: Send + Sync {
    fn put(&self, id: &str, boxed: &[u8]) -> Result<(), SyncRefused>;
    fn get(&self, from: &str, id: &str) -> Result<Option<Vec<u8>>, SyncRefused>;
    fn remove(&self, id: &str) -> Result<(), SyncRefused>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyncUsage {
    pub used: u64,
    pub quota: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedCopy {
    pub room: String,
    pub bytes: u64,
    pub at: u64,
    pub ids: Vec<String>,
}

/// What Settings › Your account asks of the relay besides: storage usage, listed copies, and deleting every copy.
pub trait SyncAccountRemote: SyncRemote {
    fn usage(&self) -> Result<SyncUsage, SyncRefused>;
    fn copies(&self) -> Result<Vec<ListedCopy>, SyncRefused>;
    fn remove_all(&self) -> Result<(), SyncRefused>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastSync {
    pub at: String,
    pub files: usize,
    pub bytes: usize,
}

/// What this computer last sent, for "Last synced": when, how many files and their bytes; `None` before the first sync.
pub fn last_sync(profile: &Path, account: u64, room: &str) -> Option<LastSync> {
    let state = read_state(profile, account, room)?;
    Some(LastSync {
        at: state.pushed_at,
        files: state.files.len(),
        bytes: state.files.iter().map(|f| f.bytes).sum(),
    })
}

/// Forgets what was sent, so the next sync sends everything: after the copy at the relay is deleted.
pub fn forget_sync(profile: &Path) {
    let _ = fs::remove_file(profile.join(PROFILE_SYNC_FILE));
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncPushed {
    Ok {
        sent: usize,
        bytes: usize,
        removed: usize,
        files: usize,
        at: String,
    },
    Err {
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncPulled {
    Ok {
        folder: PathBuf,
        computer: String,
        created_at: String,
        counts: BackupCounts,
        files: usize,
        bytes: usize,
    },
    Err {
        reason: String,
    },
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

pub fn syncable(path: &str) -> bool {
    let first = path.split('/').next().unwrap_or("");
    carries(path)
        && !LEFT_OUT.iter().any(|(k, _)| *k == first)
        && !path.split('/').any(|p| p == ".." || p.is_empty())
}

fn read_state(profile: &Path, account: u64, room: &str) -> Option<SyncState> {
    let text = fs::read_to_string(profile.join(PROFILE_SYNC_FILE)).ok()?;
    let val: Value = serde_json::from_str(&text).ok()?;
    let st = SyncState::from_json(&val)?;
    if st.schema == SYNC_SCHEMA && st.account == account && st.room == room {
        Some(st)
    } else {
        None
    }
}

pub struct PushOptions<'a> {
    pub keys: &'a SyncKeys,
    pub remote: &'a dyn SyncRemote,
    pub room: &'a str,
    pub computer: &'a str,
    pub app_version: &'a str,
    pub now: DateTime<Utc>,
}

pub fn push_profile(profile: &Path, opts: &PushOptions<'_>) -> SyncPushed {
    let before = read_state(profile, opts.keys.account, opts.room);
    let sent_before: HashMap<&str, &SyncedFile> = before
        .as_ref()
        .map(|s| s.files.iter().map(|f| (f.path.as_str(), f)).collect())
        .unwrap_or_default();

    let mut files: Vec<SyncedFile> = Vec::new();
    let mut sent = 0usize;
    let mut bytes = 0usize;

    for rel in carried_files(profile).into_iter().filter(|p| syncable(p)) {
        let mut full = profile.to_path_buf();
        for part in rel.split('/') {
            full.push(part);
        }
        let Ok(content) = fs::read(&full) else {
            continue;
        };
        let mut pieces = Vec::new();
        let old = sent_before.get(rel.as_str()).copied();
        for (index, (start, end)) in chunks_of(content.len()).into_iter().enumerate() {
            let slice = &content[start..end];
            let entry = SyncPiece {
                bytes: slice.len(),
                sha256: sha256_hex(slice),
            };
            let unchanged = old
                .and_then(|o| o.pieces.get(index))
                .is_some_and(|w| w.sha256 == entry.sha256 && w.bytes == entry.bytes);
            pieces.push(entry);
            if unchanged {
                continue;
            }
            let id = chunk_id_of(opts.keys, &rel, index);
            let sealed = match seal_file(opts.keys, &id, slice) {
                Ok(b) => b,
                Err(e) => return SyncPushed::Err { reason: e },
            };
            if let Err(err) = opts.remote.put(&id, &sealed) {
                return SyncPushed::Err { reason: err.message };
            }
            sent += 1;
            bytes += slice.len();
        }
        files.push(SyncedFile {
            path: rel,
            bytes: content.len(),
            sha256: sha256_hex(&content),
            pieces,
        });
    }

    let at = opts
        .now
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let manifest = SyncManifest {
        schema: SYNC_SCHEMA,
        app: "Locust".into(),
        app_version: opts.app_version.to_owned(),
        created_at: at.clone(),
        computer: opts.computer.to_owned(),
        counts: count_profile(profile),
        files: files.clone(),
    };
    let mid = manifest_id();
    let manifest_bytes = match serde_json::to_vec(&manifest.to_json()) {
        Ok(b) => b,
        Err(e) => return SyncPushed::Err { reason: e.to_string() },
    };
    let sealed_manifest = match seal_file(opts.keys, &mid, &manifest_bytes) {
        Ok(b) => b,
        Err(e) => return SyncPushed::Err { reason: e },
    };
    if let Err(err) = opts.remote.put(&mid, &sealed_manifest) {
        return SyncPushed::Err { reason: err.message };
    }

    let mut removed = 0usize;
    let kept: HashMap<&str, usize> = files.iter().map(|f| (f.path.as_str(), f.pieces.len())).collect();
    if let Some(prev) = &before {
        for file in &prev.files {
            let start_idx = kept.get(file.path.as_str()).copied().unwrap_or(0);
            for idx in start_idx..file.pieces.len() {
                let id = chunk_id_of(opts.keys, &file.path, idx);
                if let Err(err) = opts.remote.remove(&id) {
                    return SyncPushed::Err { reason: err.message };
                }
                removed += 1;
            }
        }
    }

    let state = SyncState {
        schema: SYNC_SCHEMA,
        account: opts.keys.account,
        room: opts.room.to_owned(),
        pushed_at: at.clone(),
        files: files.clone(),
    };
    let state_body = format!("{}\n", state.to_json());
    if let Err(e) = fs::write(profile.join(PROFILE_SYNC_FILE), state_body) {
        return SyncPushed::Err { reason: e.to_string() };
    }
    SyncPushed::Ok {
        sent,
        bytes,
        removed,
        files: files.len(),
        at,
    }
}

pub fn pull_profile(
    keys: &SyncKeys,
    remote: &dyn SyncRemote,
    from: &str,
    folder: &Path,
) -> SyncPulled {
    if fs::read_dir(folder)
        .ok()
        .is_some_and(|mut it| it.next().is_some())
    {
        return SyncPulled::Err {
            reason: "That folder already has files in it.".into(),
        };
    }
    let mid = manifest_id();
    let boxed = match remote.get(from, &mid) {
        Ok(Some(b)) => b,
        Ok(None) => {
            return SyncPulled::Err {
                reason: "That computer has not kept a copy yet.".into(),
            };
        }
        Err(err) => return SyncPulled::Err { reason: err.message },
    };
    let manifest_val: Value = match open_file(keys, &mid, &boxed)
        .ok()
        .and_then(|plain| serde_json::from_slice(&plain).ok())
    {
        Some(v) => v,
        None => {
            return SyncPulled::Err {
                reason: "That copy did not open with this key. Check the recovery code, or approve this computer from one that has your Locust.".into(),
            };
        }
    };
    let Some(manifest) = SyncManifest::from_json(&manifest_val) else {
        let ver = manifest_val
            .get("appVersion")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        return SyncPulled::Err {
            reason: format!("That copy was made by a newer Locust ({ver}). Update Locust, then try again."),
        };
    };
    if manifest.app != "Locust" || manifest.schema > SYNC_SCHEMA {
        return SyncPulled::Err {
            reason: format!(
                "That copy was made by a newer Locust ({}). Update Locust, then try again.",
                manifest.app_version
            ),
        };
    }

    let mut entries = Vec::new();
    let mut total_bytes = 0usize;
    for file in &manifest.files {
        if !syncable(&file.path) {
            return SyncPulled::Err {
                reason: format!("That copy names a file it should not carry: {}.", file.path),
            };
        }
        let mut content = Vec::with_capacity(file.bytes);
        for (index, piece) in file.pieces.iter().enumerate() {
            let id = chunk_id_of(keys, &file.path, index);
            let held = match remote.get(from, &id) {
                Ok(h) => h,
                Err(err) => return SyncPulled::Err { reason: err.message },
            };
            let opened = held.as_deref().and_then(|b| open_file(keys, &id, b).ok());
            let Some(opened) = opened else {
                return SyncPulled::Err {
                    reason: format!("{} was syncing while this was read. Try again in a minute.", manifest.computer),
                };
            };
            if opened.len() != piece.bytes || sha256_hex(&opened) != piece.sha256 {
                return SyncPulled::Err {
                    reason: format!("{} was syncing while this was read. Try again in a minute.", manifest.computer),
                };
            }
            content.extend_from_slice(&opened);
        }
        if content.len() != file.bytes || sha256_hex(&content) != file.sha256 {
            return SyncPulled::Err {
                reason: format!("{} in that copy does not match its own list.", file.path),
            };
        }
        let mut to = folder.join("profile");
        for part in file.path.split('/') {
            to.push(part);
        }
        if let Some(parent) = to.parent()
            && let Err(e) = fs::create_dir_all(parent)
        {
            return SyncPulled::Err { reason: e.to_string() };
        }
        if let Err(e) = fs::write(&to, &content) {
            return SyncPulled::Err { reason: e.to_string() };
        }
        entries.push(json!({
            "path": file.path,
            "bytes": file.bytes,
            "sha256": file.sha256,
        }));
        total_bytes += file.bytes;
    }

    let left_out: Vec<String> = LEFT_OUT.iter().map(|(k, _)| (*k).to_owned()).collect();
    let mut backup = Map::new();
    backup.insert("schema".into(), json!(BACKUP_SCHEMA));
    backup.insert("app".into(), json!("Locust"));
    backup.insert("appVersion".into(), json!(manifest.app_version));
    backup.insert("createdAt".into(), json!(manifest.created_at));
    backup.insert("counts".into(), manifest.counts.to_json());
    backup.insert("files".into(), Value::Array(entries));
    backup.insert("leftOut".into(), json!(left_out));
    if let Err(e) = fs::create_dir_all(folder) {
        return SyncPulled::Err { reason: e.to_string() };
    }
    let body = format!(
        "{}\n",
        serde_json::to_string_pretty(&Value::Object(backup)).unwrap_or_default()
    );
    if let Err(e) = fs::write(folder.join(BACKUP_MANIFEST), body) {
        return SyncPulled::Err { reason: e.to_string() };
    }

    SyncPulled::Ok {
        folder: folder.to_path_buf(),
        computer: manifest.computer,
        created_at: manifest.created_at,
        counts: manifest.counts,
        files: manifest.files.len(),
        bytes: total_bytes,
    }
}

// ---- HTTP remote (httpSyncRemote) -----------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SyncHttpReply {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait SyncHttpFetch: Send + Sync {
    fn send(
        &self,
        url: &str,
        method: &str,
        auth: &str,
        body: Option<&[u8]>,
    ) -> Result<SyncHttpReply, String>;
}

pub struct HttpSyncRemote {
    base: String,
    auth: String,
    fetch: Arc<dyn SyncHttpFetch>,
}

impl HttpSyncRemote {
    pub fn new(relay: &str, room: &str, token: &str, fetch: Arc<dyn SyncHttpFetch>) -> Self {
        Self {
            base: format!("{}/v1/room/{room}/sync", relay.trim_end_matches('/')),
            auth: format!("Bearer {token}"),
            fetch,
        }
    }

    fn refused(reply: &SyncHttpReply) -> SyncRefused {
        let error = serde_json::from_slice::<Value>(&reply.body)
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned));
        let msg = match reply.status {
            507 => "Your copy on Locust’s relay is full (1 GB). Your Locust is safe here; nothing more is sent until there is room.".to_owned(),
            429 => "Locust synced a lot this hour. The next sync carries on.".to_owned(),
            401 | 403 => "Locust’s relay did not know this computer. Sign in again in Settings.".to_owned(),
            501 => "Locust’s relay does not keep copies yet.".to_owned(),
            s => match error {
                Some(e) => format!("Locust’s relay answered {s} ({e}). The next sync tries again."),
                None => format!("Locust’s relay answered {s}. The next sync tries again."),
            },
        };
        SyncRefused {
            status: reply.status,
            message: msg,
        }
    }
}

impl SyncRemote for HttpSyncRemote {
    fn put(&self, id: &str, boxed: &[u8]) -> Result<(), SyncRefused> {
        let reply = self
            .fetch
            .send(&format!("{}/{id}", self.base), "PUT", &self.auth, Some(boxed))
            .map_err(|e| SyncRefused { status: 0, message: e })?;
        if (200..300).contains(&reply.status) {
            Ok(())
        } else {
            Err(Self::refused(&reply))
        }
    }

    fn get(&self, from: &str, id: &str) -> Result<Option<Vec<u8>>, SyncRefused> {
        let reply = self
            .fetch
            .send(&format!("{}/{from}/{id}", self.base), "GET", &self.auth, None)
            .map_err(|e| SyncRefused { status: 0, message: e })?;
        if reply.status == 404 {
            return Ok(None);
        }
        if (200..300).contains(&reply.status) {
            Ok(Some(reply.body))
        } else {
            Err(Self::refused(&reply))
        }
    }

    fn remove(&self, id: &str) -> Result<(), SyncRefused> {
        let reply = self
            .fetch
            .send(&format!("{}/{id}", self.base), "DELETE", &self.auth, None)
            .map_err(|e| SyncRefused { status: 0, message: e })?;
        if (200..300).contains(&reply.status) {
            Ok(())
        } else {
            Err(Self::refused(&reply))
        }
    }
}

fn is_b64url_len(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

impl SyncAccountRemote for HttpSyncRemote {
    fn usage(&self) -> Result<SyncUsage, SyncRefused> {
        let reply = self
            .fetch
            .send(&self.base, "GET", &self.auth, None)
            .map_err(|e| SyncRefused { status: 0, message: e })?;
        if !(200..300).contains(&reply.status) {
            return Err(Self::refused(&reply));
        }
        let val: Value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
        let used = val.get("used").and_then(Value::as_u64).unwrap_or(0);
        let quota = val.get("quota").and_then(Value::as_u64).unwrap_or(0);
        Ok(SyncUsage { used, quota })
    }

    fn copies(&self) -> Result<Vec<ListedCopy>, SyncRefused> {
        let reply = self
            .fetch
            .send(&self.base, "GET", &self.auth, None)
            .map_err(|e| SyncRefused { status: 0, message: e })?;
        if !(200..300).contains(&reply.status) {
            return Err(Self::refused(&reply));
        }
        let val: Value = serde_json::from_slice(&reply.body).unwrap_or(Value::Null);
        let mut out = Vec::new();
        let Some(computers) = val.get("computers").and_then(Value::as_array) else {
            return Ok(out);
        };
        for one in computers {
            let Some(obj) = one.as_object() else {
                continue;
            };
            let Some(room) = obj.get("room").and_then(Value::as_str) else {
                continue;
            };
            if !is_b64url_len(room, 22) {
                continue;
            }
            let bytes = obj.get("bytes").and_then(Value::as_u64).unwrap_or(0);
            let files = obj.get("files").and_then(Value::as_array);
            let mut max_at = 0u64;
            let mut ids = Vec::new();
            if let Some(files) = files {
                for f in files {
                    if let Some(at) = f.get("at").and_then(Value::as_u64) {
                        max_at = max_at.max(at);
                    }
                    if let Some(id) = f.get("id").and_then(Value::as_str)
                        && is_b64url_len(id, 43)
                    {
                        ids.push(id.to_owned());
                    }
                }
            }
            out.push(ListedCopy {
                room: room.to_owned(),
                bytes,
                at: max_at,
                ids,
            });
        }
        Ok(out)
    }

    fn remove_all(&self) -> Result<(), SyncRefused> {
        let reply = self
            .fetch
            .send(&self.base, "DELETE", &self.auth, None)
            .map_err(|e| SyncRefused { status: 0, message: e })?;
        if (200..300).contains(&reply.status) {
            Ok(())
        } else {
            Err(Self::refused(&reply))
        }
    }
}

/// Real HTTP transport for `HttpSyncRemote` using `ureq`.
pub struct UreqSyncHttpFetch;

impl SyncHttpFetch for UreqSyncHttpFetch {
    fn send(
        &self,
        url: &str,
        method: &str,
        auth: &str,
        body: Option<&[u8]>,
    ) -> Result<SyncHttpReply, String> {
        use std::io::Read;
        let req = ureq::request(method, url).set("authorization", auth);
        let resp = match body {
            Some(bytes) => req
                .set("content-type", "application/octet-stream")
                .send_bytes(bytes),
            None => req.call(),
        };
        let (status, mut reader) = match resp {
            Ok(r) => (r.status(), r.into_reader()),
            Err(ureq::Error::Status(code, r)) => (code, r.into_reader()),
            Err(ureq::Error::Transport(t)) => return Err(t.to_string()),
        };
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        Ok(SyncHttpReply { status, body: buf })
    }
}

// ---- SyncSchedule (createSyncSchedule) ------------------------------------------------------------------------------

struct ScheduleInner {
    quiet_ms: u64,
    deadline: Option<u64>,
    running: bool,
    again: bool,
    stopped: bool,
}

/// Deterministic sync scheduler: fires `quiet_ms` after the last `touched(now)`, never while
/// `busy` or another push is running, and re-arms once more if `touched` happened while running.
pub struct SyncSchedule {
    inner: Mutex<ScheduleInner>,
}

impl SyncSchedule {
    pub fn new(quiet_ms: u64) -> Self {
        Self {
            inner: Mutex::new(ScheduleInner {
                quiet_ms,
                deadline: None,
                running: false,
                again: false,
                stopped: false,
            }),
        }
    }

    pub fn touched(&self, now_ms: u64) {
        let mut st = self.inner.lock().unwrap();
        if st.stopped {
            return;
        }
        if st.running {
            st.again = true;
            return;
        }
        st.deadline = Some(now_ms + st.quiet_ms);
    }

    /// Advances to `now_ms`. Returns `true` if a push should start now (caller calls `finished`
    /// when the push completes).
    pub fn tick(&self, now_ms: u64, busy: bool) -> bool {
        let mut st = self.inner.lock().unwrap();
        if st.stopped || st.running {
            return false;
        }
        let Some(due) = st.deadline else {
            return false;
        };
        if now_ms < due {
            return false;
        }
        st.deadline = None;
        if busy {
            st.deadline = Some(now_ms + st.quiet_ms);
            return false;
        }
        st.running = true;
        st.again = false;
        true
    }

    pub fn finished(&self, now_ms: u64) {
        let mut st = self.inner.lock().unwrap();
        st.running = false;
        if st.again && !st.stopped {
            st.again = false;
            st.deadline = Some(now_ms + st.quiet_ms);
        }
    }

    pub fn stop(&self) {
        let mut st = self.inner.lock().unwrap();
        st.stopped = true;
        st.deadline = None;
    }
}
