//! A text chunk in a PNG (`main/png-text.ts`): how a team card carries its team.
//!
//! A PNG is an 8-byte signature and then chunks — length, type, data, CRC —
//! and a `tEXt` chunk is `keyword NUL text`, where `text` is base64 of UTF-8 JSON.

use serde_json::Value;

const SIGNATURE: &[u8; 8] = &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const MAX_TEXT_BYTES: usize = 256 * 1024;

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((triple >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn decode_base64(text: &str) -> Option<Vec<u8>> {
    fn val(b: u8) -> Option<u32> {
        match b {
            b'A'..=b'Z' => Some((b - b'A') as u32),
            b'a'..=b'z' => Some((b - b'a' + 26) as u32),
            b'0'..=b'9' => Some((b - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks_exact(4) {
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c = if chunk[2] == b'=' { 0 } else { val(chunk[2])? };
        let d = if chunk[3] == b'=' { 0 } else { val(chunk[3])? };
        let triple = (a << 18) | (b << 12) | (c << 6) | d;
        out.push(((triple >> 16) & 0xff) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xff) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xff) as u8);
        }
    }
    Some(out)
}

fn make_chunk(chunk_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(12 + data.len());
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    let crc = crc32(&out[4..]);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

pub fn is_png(bytes: &[u8]) -> bool {
    bytes.len() > 8 && &bytes[..8] == SIGNATURE
}

struct Chunk<'a> {
    kind: &'a [u8],
    data: &'a [u8],
    raw: &'a [u8],
}

fn chunks_of(png: &[u8]) -> Vec<Chunk<'_>> {
    let mut out = Vec::new();
    let mut at = 8;
    while at + 12 <= png.len() {
        let len = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let Some(end_of) = at.checked_add(12).and_then(|v| v.checked_add(len)) else {
            break;
        };
        if end_of > png.len() {
            break;
        }
        out.push(Chunk {
            kind: &png[at + 4..at + 8],
            data: &png[at + 8..at + 8 + len],
            raw: &png[at..end_of],
        });
        at = end_of;
    }
    out
}

/// The PNG with `text` under `keyword`, replacing one already there.
pub fn with_text_chunk(png: &[u8], keyword: &str, text: &str) -> Result<Vec<u8>, String> {
    if !is_png(png) {
        return Err("That is not a PNG image.".into());
    }
    let mut out = Vec::with_capacity(png.len() + text.len() * 2 + 64);
    out.extend_from_slice(SIGNATURE);
    let mut end: Option<&[u8]> = None;
    let kw_bytes = keyword.as_bytes();
    for found in chunks_of(png) {
        if found.kind == b"IEND" {
            end = Some(found.raw);
            break;
        }
        if found.kind == b"tEXt"
            && let Some(zero) = found.data.iter().position(|&b| b == 0)
            && &found.data[..zero] == kw_bytes
        {
            continue;
        }
        out.extend_from_slice(found.raw);
    }
    let Some(end_raw) = end else {
        return Err("That PNG image has no end.".into());
    };
    let encoded = encode_base64(text.as_bytes());
    let mut body = Vec::with_capacity(kw_bytes.len() + 1 + encoded.len());
    body.extend_from_slice(kw_bytes);
    body.push(0);
    body.extend_from_slice(encoded.as_bytes());
    out.extend_from_slice(&make_chunk(b"tEXt", &body));
    out.extend_from_slice(end_raw);
    Ok(out)
}

/// The text under `keyword`, or None when the image carries none (or is not a PNG).
pub fn read_text_chunk(png: &[u8], keyword: &str) -> Option<String> {
    if !is_png(png) {
        return None;
    }
    let kw_bytes = keyword.as_bytes();
    for found in chunks_of(png) {
        if found.kind != b"tEXt" {
            continue;
        }
        let zero = found.data.iter().position(|&b| b == 0)?;
        if &found.data[..zero] != kw_bytes {
            continue;
        }
        let encoded_bytes = &found.data[zero + 1..];
        if encoded_bytes.len() > MAX_TEXT_BYTES {
            return None;
        }
        let encoded = std::str::from_utf8(encoded_bytes).ok()?;
        let decoded = decode_base64(encoded)?;
        return String::from_utf8(decoded).ok();
    }
    None
}

fn hue_rgb(hue: &str) -> [u8; 4] {
    match hue {
        "lime" => [132, 204, 22, 255],
        "violet" => [139, 92, 246, 255],
        "blue" => [59, 130, 246, 255],
        "teal" => [20, 184, 166, 255],
        "butter" => [234, 179, 8, 255],
        "slate" => [100, 116, 139, 255],
        "rose" => [244, 63, 94, 255],
        "clay" => [217, 119, 6, 255],
        "pearl" => [148, 163, 184, 255],
        _ => [132, 204, 22, 255],
    }
}

/// Render a clean minimal RGBA PNG for a team card so export works headless with no OS window.
pub fn render_team_card_png(teammates: &[Value]) -> Vec<u8> {
    let cols = teammates.len().clamp(1, 4) as u32;
    let rows = (teammates.len().max(1).div_ceil(4)) as u32;
    let width = 32 + cols * 80;
    let height = 40 + rows * 64;
    let stride = 1 + width as usize * 4;
    let mut raw = vec![0u8; stride * height as usize];

    // Fill background (#18181b) and subtle border (#3f3f46)
    for y in 0..height as usize {
        let row_start = y * stride;
        raw[row_start] = 0; // PNG filter: None
        for x in 0..width as usize {
            let px = row_start + 1 + x * 4;
            let border = x == 0 || y == 0 || x + 1 == width as usize || y + 1 == height as usize;
            let color = if border {
                [63, 63, 70, 255]
            } else {
                [24, 24, 27, 255]
            };
            raw[px..px + 4].copy_from_slice(&color);
        }
    }

    // Draw a coloured badge for each teammate
    for (idx, mate) in teammates.iter().take(12).enumerate() {
        let col = (idx % 4) as usize;
        let row = (idx / 4) as usize;
        let bx = 24 + col * 80;
        let by = 24 + row * 64;
        let color = hue_rgb(mate["hue"].as_str().unwrap_or("lime"));
        for dy in 0..40 {
            for dx in 0..40 {
                let x = bx + dx;
                let y = by + dy;
                if x < width as usize && y < height as usize {
                    let px = y * stride + 1 + x * 4;
                    raw[px..px + 4].copy_from_slice(&color);
                }
            }
        }
    }

    // Build zlib stream using uncompressed DEFLATE blocks (max 65535 bytes per block).
    let mut zlib = Vec::with_capacity(raw.len() + 64);
    zlib.push(0x78);
    zlib.push(0x01);
    let mut offset = 0;
    while offset < raw.len() {
        let end = (offset + 65535).min(raw.len());
        let block = &raw[offset..end];
        let is_last = end == raw.len();
        zlib.push(if is_last { 0x01 } else { 0x00 });
        let len = block.len() as u16;
        zlib.extend_from_slice(&len.to_le_bytes());
        zlib.extend_from_slice(&(!len).to_le_bytes());
        zlib.extend_from_slice(block);
        offset = end;
    }
    zlib.extend_from_slice(&adler32(&raw).to_be_bytes());

    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace

    let mut png = Vec::new();
    png.extend_from_slice(SIGNATURE);
    png.extend_from_slice(&make_chunk(b"IHDR", &ihdr));
    png.extend_from_slice(&make_chunk(b"IDAT", &zlib));
    png.extend_from_slice(&make_chunk(b"IEND", &[]));
    png
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_utf8_text_chunk_and_replaces_existing() {
        let png = render_team_card_png(&[serde_json::json!({"name": "Wren", "hue": "lime"})]);
        assert!(is_png(&png));
        assert_eq!(read_text_chunk(&png, "locust-team"), None);

        let written = with_text_chunk(&png, "locust-team", r#"{"name":"Zoë 🦗"}"#).unwrap();
        assert_eq!(
            read_text_chunk(&written, "locust-team").as_deref(),
            Some(r#"{"name":"Zoë 🦗"}"#)
        );

        let twice = with_text_chunk(&written, "locust-team", "second").unwrap();
        assert_eq!(read_text_chunk(&twice, "locust-team").as_deref(), Some("second"));

        assert_eq!(read_text_chunk(b"not an image", "locust-team"), None);
        assert_eq!(
            with_text_chunk(b"not an image", "locust-team", "x").unwrap_err(),
            "That is not a PNG image."
        );
    }
}
