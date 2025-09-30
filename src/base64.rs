pub fn is_binary_tag(tag: Option<&str>) -> bool {
    match tag {
        Some(t) => matches!(
            t,
            "!!binary" | "!binary" | "tag:yaml.org,2002:binary" | "tag:yaml.org,2002:!binary"
        ),
        None => false,
    }
}

use crate::sf_serde::Error;

/// Lookup table translating ASCII bytes to their base64 sextet value.
///
/// Invalid bytes are mapped to 0xFF which we treat as an error.
fn decode_val(b: u8) -> Result<u8, Error> {
    match b {
        b'A'..=b'Z' => Ok(b - b'A'),
        b'a'..=b'z' => Ok(b - b'a' + 26),
        b'0'..=b'9' => Ok(b - b'0' + 52),
        b'+' => Ok(62),
        b'/' => Ok(63),
        _ => Err(Error::msg("invalid !!binary base64")),
    }
}

/// Decode a YAML !!binary scalar string (may contain newlines or spaces).
pub fn decode_base64_yaml(s: &str) -> Result<Vec<u8>, Error> {
    // YAML allows ASCII whitespace inside the base64 text.
    let cleaned: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();

    if cleaned.len() % 4 != 0 {
        return Err(Error::msg("invalid !!binary base64"));
    }

    let mut out = Vec::with_capacity(cleaned.len() / 4 * 3);

    for chunk in cleaned.chunks_exact(4) {
        let pad = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        let a = decode_val(chunk[0])? as u32;
        let b = decode_val(chunk[1])? as u32;

        let c = match chunk[2] {
            b'=' => {
                if pad < 2 {
                    return Err(Error::msg("invalid !!binary base64"));
                }
                0
            }
            byte => decode_val(byte)? as u32,
        };

        let d = match chunk[3] {
            b'=' => {
                if pad == 0 {
                    return Err(Error::msg("invalid !!binary base64"));
                }
                0
            }
            byte => decode_val(byte)? as u32,
        };

        let triple = (a << 18) | (b << 12) | (c << 6) | d;

        out.push(((triple >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if pad == 0 {
            out.push((triple & 0xFF) as u8);
        }

        match pad {
            0 | 1 | 2 => {}
            _ => return Err(Error::msg("invalid !!binary base64")),
        }
    }

    Ok(out)
}
