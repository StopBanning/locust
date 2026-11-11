//! Ports antigravity-projects.ts: workspace -> project id from agyhub summaries protobuf.
use std::collections::HashMap;

const PROJECT_ID_TAG: [u8; 3] = [0x92, 0x01, 0x24];
const PROJECT_ID_LENGTH: usize = 36;
const WORKSPACE_TAG: u8 = 0x3a;

fn read_varint(bytes: &[u8], offset: usize) -> Option<(u32, usize)> {
    let mut value = 0u32;
    let mut shift = 0u32;
    for index in 0..5 {
        let byte = *bytes.get(offset + index)?;
        value |= (u32::from(byte) & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some((value, index + 1));
        }
        shift += 7;
    }
    None
}

pub fn normalize_workspace(value: &str) -> String {
    let mut path = value.to_owned();
    if let Some(rest) = path.strip_prefix("file://") {
        path = rest.to_owned();
    }
    if let Ok(decoded) = urlencoding_decode(&path) {
        path = decoded;
    }
    if path.len() >= 3
        && path.as_bytes()[0] == b'/'
        && path.as_bytes()[1].is_ascii_alphabetic()
        && path.as_bytes()[2] == b':'
    {
        path = path[1..].to_owned();
    }
    path = path.replace('\\', "/");
    while path.len() > 1 && path.ends_with('/') {
        path.pop();
    }
    if path.len() >= 2 && path.as_bytes()[0].is_ascii_alphabetic() && path.as_bytes()[1] == b':' {
        let mut chars = path.chars();
        let first = chars.next().unwrap().to_ascii_lowercase();
        format!("{first}{}", chars.collect::<String>())
    } else {
        path
    }
}

fn urlencoding_decode(s: &str) -> Result<String, ()> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h = |c: u8| -> Option<u8> {
                match c {
                    b'0'..=b'9' => Some(c - b'0'),
                    b'a'..=b'f' => Some(c - b'a' + 10),
                    b'A'..=b'F' => Some(c - b'A' + 10),
                    _ => None,
                }
            };
            if let (Some(a), Some(b)) = (h(bytes[i + 1]), h(bytes[i + 2])) {
                out.push((a << 4) | b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).map_err(|_| ())
}

fn read_workspace_uri(bytes: &[u8], offset: usize) -> Option<String> {
    let (len, n) = read_varint(bytes, offset + 1)?;
    if len == 0 {
        return None;
    }
    let start = offset + 1 + n;
    let end = start + len as usize;
    if end > bytes.len() {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes[start..end]).into_owned();
    text.starts_with("file:").then_some(text)
}

fn read_project_id(bytes: &[u8], offset: usize) -> Option<String> {
    if bytes.len() < offset + PROJECT_ID_TAG.len() + PROJECT_ID_LENGTH {
        return None;
    }
    if bytes[offset..offset + PROJECT_ID_TAG.len()] != PROJECT_ID_TAG {
        return None;
    }
    let start = offset + PROJECT_ID_TAG.len();
    let end = start + PROJECT_ID_LENGTH;
    let text = String::from_utf8_lossy(&bytes[start..end]).into_owned();
    (text.len() == 36 && text.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')).then_some(text)
}

pub fn parse_projects(bytes: &[u8]) -> HashMap<String, String> {
    let mut projects = HashMap::new();
    let mut pending: Option<String> = None;
    for offset in 0..bytes.len() {
        if bytes[offset] == WORKSPACE_TAG {
            if let Some(uri) = read_workspace_uri(bytes, offset) {
                pending = Some(normalize_workspace(&uri));
            }
            continue;
        }
        let Some(workspace) = pending.as_ref() else {
            continue;
        };
        if let Some(id) = read_project_id(bytes, offset) {
            projects.insert(workspace.clone(), id);
            pending = None;
        }
    }
    projects
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalize_collapses_encodings() {
        assert_eq!(
            normalize_workspace("file:///C:/Users/x/antigravtest"),
            normalize_workspace("file:///c%3A%5CUsers%5Cx%5Cantigravtest")
        );
    }
    #[test]
    fn parses_synthetic_record() {
        let uri = b"file:///c:/Users/x/proj";
        let mut bytes = Vec::new();
        bytes.push(0x3a);
        bytes.push(uri.len() as u8);
        bytes.extend_from_slice(uri);
        bytes.extend_from_slice(&[0x92, 0x01, 0x24]);
        bytes.extend_from_slice(b"01234567-89ab-cdef-0123-456789abcdef");
        let projects = parse_projects(&bytes);
        assert_eq!(
            projects.get("c:/Users/x/proj").map(String::as_str),
            Some("01234567-89ab-cdef-0123-456789abcdef")
        );
    }
}
