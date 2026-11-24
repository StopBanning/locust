//! Carries `apps/desktop/src/main/the-lock-is-the-same-on-both-sides.test.ts` (all 5 cases):
//! proves Rust's `relay_crypto` and the browser page's `phone_page::RELAY_LOCK` produce and open
//! the exact same bytes both ways.

use std::collections::HashSet;
use std::process::Command;

use serde_json::{Value, json};

use crate::phone_page::{PhonePageVia, RELAY_LOCK, phone_page};
use crate::relay_crypto::{
    Direction, LinkSide, aad_of, device_id_of, from_base64url, link_keys, new_pc_keys, open_json,
    pairing_proof, pairing_token, random_bytes, random_text, seal_json, to_base64url,
};

fn run_page_lock_js(body: &str) -> Value {
    let script = format!(
        r#"
const vm = require('node:vm');
const RELAY_LOCK = {relay_lock};
const context = vm.createContext({{ crypto: globalThis.crypto, TextEncoder, TextDecoder, btoa, atob }});
vm.runInContext(RELAY_LOCK, context);
const lock = context.lock;
(async () => {{
  {body}
}})().then(
  (out) => process.stdout.write(JSON.stringify(out)),
  (err) => {{
    process.stderr.write(String(err && err.stack || err));
    process.exit(1);
  }}
);
"#,
        relay_lock = serde_json::to_string(RELAY_LOCK).expect("serialize RELAY_LOCK"),
        body = body,
    );
    let output = Command::new("node")
        .arg("-e")
        .arg(&script)
        .output()
        .expect("node must be available to run RELAY_LOCK parity tests");
    assert!(
        output.status.success(),
        "node exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON from node")
}

#[test]
// case: writes and reads bytes as the PC does
fn writes_and_reads_bytes_as_the_pc_does() {
    let samples: Vec<Vec<u8>> = [0usize, 1, 2, 3, 16, 32, 33, 255]
        .into_iter()
        .map(random_bytes)
        .collect();
    let encoded_by_pc: Vec<String> = samples.iter().map(|b| to_base64url(b)).collect();
    let res = run_page_lock_js(&format!(
        r#"
const samples = {samples};
const pcB64 = {pc_b64};
const pageB64 = samples.map((arr) => lock.b64(new Uint8Array(arr)));
const pageUnb64 = pcB64.map((text) => Array.from(lock.unb64(text)));
return {{ pageB64, pageUnb64 }};
"#,
        samples = serde_json::to_string(&samples).unwrap(),
        pc_b64 = serde_json::to_string(&encoded_by_pc).unwrap(),
    ));
    let page_b64 = res["pageB64"].as_array().unwrap();
    let page_unb64 = res["pageUnb64"].as_array().unwrap();
    for (i, bytes) in samples.iter().enumerate() {
        let p_b64 = page_b64[i].as_str().unwrap();
        assert_eq!(p_b64, encoded_by_pc[i]);
        let decoded_by_page: Vec<u8> = page_unb64[i]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap() as u8)
            .collect();
        assert_eq!(&decoded_by_page, bytes);
        assert_eq!(&from_base64url(p_b64).unwrap(), bytes);
    }
    // Concrete vector checks independent of both implementations.
    assert_eq!(to_base64url(&[]), "");
    assert_eq!(to_base64url(&[0xfb, 0xff, 0xfe]), "-__-");
    assert_eq!(from_base64url("-__-").unwrap(), vec![0xfb, 0xff, 0xfe]);
}

#[test]
// case: makes the same pairing token, proof and device id from the same code
fn makes_the_same_pairing_token_proof_and_device_id_from_the_same_code() {
    let secret = random_text(16);
    let res = run_page_lock_js(&format!(
        r#"
const secret = {secret};
const pair = await globalThis.crypto.subtle.generateKey({{ name: 'X25519' }}, false, ['deriveBits']);
const pub = lock.b64(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', pair.publicKey)));
return {{
  token: await lock.pairingToken(secret),
  proof: await lock.pairingProof(secret),
  publicKey: pub,
  deviceId: await lock.deviceIdOf(pub),
  aad: lock.aad('r', 'd', 'device-to-pc'),
}};
"#,
        secret = serde_json::to_string(&secret).unwrap(),
    ));
    let pc_token = pairing_token(&secret);
    let pc_proof = pairing_proof(&secret);
    assert_eq!(res["token"].as_str().unwrap(), pc_token);
    assert_eq!(res["proof"].as_str().unwrap(), pc_proof);
    // The relay sees the token; the proof it never sees must not follow from it.
    assert_ne!(pc_proof, pc_token);

    let phone_pub = res["publicKey"].as_str().unwrap();
    let pc_device_id = device_id_of(phone_pub);
    assert_eq!(pc_device_id.len(), 22);
    assert_eq!(res["deviceId"].as_str().unwrap(), pc_device_id);
    assert_eq!(
        aad_of("r", "d", Direction::DeviceToPc),
        "locust-relay-v1|r|d|device-to-pc"
    );
    assert_eq!(
        res["aad"].as_str().unwrap(),
        aad_of("r", "d", Direction::DeviceToPc)
    );
}

#[test]
// case: opens on the PC what the page sealed, and in the page what the PC sealed
fn opens_on_the_pc_what_the_page_sealed_and_in_the_page_what_the_pc_sealed() {
    let room = random_text(16);
    let pc = new_pc_keys();

    // 1. Page generates its X25519 pair, derives link keys, and seals a request to the PC.
    let step1 = run_page_lock_js(&format!(
        r#"
const room = {room};
const pcPub = {pc_pub};
const pair = await globalThis.crypto.subtle.generateKey({{ name: 'X25519' }}, true, ['deriveBits']);
const privateJwk = await globalThis.crypto.subtle.exportKey('jwk', pair.privateKey);
const publicKey = lock.b64(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', pair.publicKey)));
const device = await lock.deviceIdOf(publicKey);
const onPhone = await lock.linkKeys(pair.privateKey, pcPub, room, 'device');
const asked = await lock.seal(
  onPhone.seal,
  {{ kind: 'req', path: '/api/state', words: 'Grüße ✓' }},
  lock.aad(room, device, 'device-to-pc')
);
return {{ privateJwk, publicKey, device, asked }};
"#,
        room = serde_json::to_string(&room).unwrap(),
        pc_pub = serde_json::to_string(&pc.public_key).unwrap(),
    ));

    let phone_pub = step1["publicKey"].as_str().unwrap();
    let device = device_id_of(phone_pub);
    assert_eq!(step1["device"].as_str().unwrap(), device);

    let on_pc = link_keys(&pc.private_key, phone_pub, &room, LinkSide::Pc).unwrap();
    let asked_n = step1["asked"]["n"].as_str().unwrap();
    let asked_c = step1["asked"]["c"].as_str().unwrap();
    let opened_on_pc = open_json(
        &on_pc.open,
        asked_n,
        asked_c,
        &aad_of(&room, &device, Direction::DeviceToPc),
    )
    .unwrap();
    assert_eq!(
        opened_on_pc,
        json!({ "kind": "req", "path": "/api/state", "words": "Grüße ✓" })
    );

    // 2. PC seals a response, and the page opens it with RELAY_LOCK.
    let answered = seal_json(
        &on_pc.seal,
        &json!({ "kind": "res", "status": 200 }),
        &aad_of(&room, &device, Direction::PcToDevice),
    )
    .unwrap();

    let step2 = run_page_lock_js(&format!(
        r#"
const room = {room};
const pcPub = {pc_pub};
const device = {device};
const privateJwk = {private_jwk};
const answered = {{ n: {n}, c: {c} }};
const privKey = await globalThis.crypto.subtle.importKey('jwk', privateJwk, {{ name: 'X25519' }}, false, ['deriveBits']);
const onPhone = await lock.linkKeys(privKey, pcPub, room, 'device');
return await lock.open(onPhone.open, answered.n, answered.c, lock.aad(room, device, 'pc-to-device'));
"#,
        room = serde_json::to_string(&room).unwrap(),
        pc_pub = serde_json::to_string(&pc.public_key).unwrap(),
        device = serde_json::to_string(&device).unwrap(),
        private_jwk = serde_json::to_string(&step1["privateJwk"]).unwrap(),
        n = serde_json::to_string(&answered.n).unwrap(),
        c = serde_json::to_string(&answered.c).unwrap(),
    ));
    assert_eq!(step2, json!({ "kind": "res", "status": 200 }));

    // Caller check: the rendered relay phone page embeds RELAY_LOCK.
    let html = phone_page(PhonePageVia::Relay, None);
    assert!(
        html.contains(RELAY_LOCK),
        "relay phone page must embed RELAY_LOCK"
    );
}

#[test]
// case: does not open a box sent back to the side that sealed it, or named for another device or room
fn does_not_open_a_box_sent_back_to_the_side_that_sealed_it_or_named_for_another_device_or_room() {
    let room = random_text(16);
    let other_room = random_text(16);
    let other_device: String = random_text(16).chars().take(22).collect();
    let pc = new_pc_keys();

    let step1 = run_page_lock_js(
        r#"
const pair = await globalThis.crypto.subtle.generateKey({ name: 'X25519' }, true, ['deriveBits']);
const privateJwk = await globalThis.crypto.subtle.exportKey('jwk', pair.privateKey);
const publicKey = lock.b64(new Uint8Array(await globalThis.crypto.subtle.exportKey('raw', pair.publicKey)));
return { privateJwk, publicKey };
"#,
    );
    let phone_pub = step1["publicKey"].as_str().unwrap();
    let device = device_id_of(phone_pub);
    let on_pc = link_keys(&pc.private_key, phone_pub, &room, LinkSide::Pc).unwrap();

    let answered = seal_json(
        &on_pc.seal,
        &json!({ "kind": "res" }),
        &aad_of(&room, &device, Direction::PcToDevice),
    )
    .unwrap();

    // Reflected to the PC: its opening key is the other direction's.
    assert!(
        open_json(
            &on_pc.open,
            &answered.n,
            &answered.c,
            &aad_of(&room, &device, Direction::PcToDevice),
        )
        .is_err()
    );

    // Read on the phone as if for another device, another room, wrong direction, or keys derived for another room.
    let checks = run_page_lock_js(&format!(
        r#"
const room = {room};
const otherRoom = {other_room};
const device = {device};
const otherDevice = {other_device};
const pcPub = {pc_pub};
const privateJwk = {private_jwk};
const answered = {{ n: {n}, c: {c} }};
const privKey = await globalThis.crypto.subtle.importKey('jwk', privateJwk, {{ name: 'X25519' }}, false, ['deriveBits']);
const onPhone = await lock.linkKeys(privKey, pcPub, room, 'device');
const elsewhere = await lock.linkKeys(privKey, pcPub, otherRoom, 'device');
async function fails(fn) {{
  try {{ await fn(); return false; }} catch (_) {{ return true; }}
}}
return {{
  wrongDevice: await fails(() => lock.open(onPhone.open, answered.n, answered.c, lock.aad(room, otherDevice, 'pc-to-device'))),
  wrongRoom: await fails(() => lock.open(onPhone.open, answered.n, answered.c, lock.aad(otherRoom, device, 'pc-to-device'))),
  wrongDirection: await fails(() => lock.open(onPhone.open, answered.n, answered.c, lock.aad(room, device, 'device-to-pc'))),
  elsewhereKeys: await fails(() => lock.open(elsewhere.open, answered.n, answered.c, lock.aad(room, device, 'pc-to-device'))),
}};
"#,
        room = serde_json::to_string(&room).unwrap(),
        other_room = serde_json::to_string(&other_room).unwrap(),
        device = serde_json::to_string(&device).unwrap(),
        other_device = serde_json::to_string(&other_device).unwrap(),
        pc_pub = serde_json::to_string(&pc.public_key).unwrap(),
        private_jwk = serde_json::to_string(&step1["privateJwk"]).unwrap(),
        n = serde_json::to_string(&answered.n).unwrap(),
        c = serde_json::to_string(&answered.c).unwrap(),
    ));
    assert_eq!(checks["wrongDevice"], json!(true));
    assert_eq!(checks["wrongRoom"], json!(true));
    assert_eq!(checks["wrongDirection"], json!(true));
    assert_eq!(checks["elsewhereKeys"], json!(true));
}

#[test]
// case: never seals two boxes with the same nonce
fn never_seals_two_boxes_with_the_same_nonce() {
    let pc = new_pc_keys();
    let phone = new_pc_keys();
    let keys = link_keys(&pc.private_key, &phone.public_key, &random_text(16), LinkSide::Pc).unwrap();
    let mut nonces = HashSet::new();
    for i in 0..200 {
        nonces.insert(seal_json(&keys.seal, &json!(i), "aad").unwrap().n);
    }
    assert_eq!(nonces.len(), 200);
}
