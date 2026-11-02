//! HTTP/3 frames, SETTINGS, and a static-table QPACK field-section reader.
//!
//! Sources, all fetched while writing this (nothing from memory):
//! - RFC 9114 (HTTP/3): frame and stream layout, SETTINGS.
//! - RFC 9204 (QPACK): field-line representations; the static table is `qpack_static_table.txt`,
//!   extracted from Appendix A.
//! - RFC 7541 (HPACK) Appendix B: the Huffman code, `huffman_table.txt`.
//!
//! The reader decodes field sections that use only the static table. A section that refers to
//! the dynamic table is reported as such (`dynamic:<index>`), not decoded; the spike server
//! advertises a dynamic table of size 0 by default so the clients cannot use one.

use std::collections::HashMap;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------------------------
// Variable-length integers (RFC 9000 section 16)
// ---------------------------------------------------------------------------------------------

/// Reads a QUIC variable-length integer at `pos` and moves `pos` past it.
pub fn read_varint(buf: &[u8], pos: &mut usize) -> Option<u64> {
    let first = *buf.get(*pos)?;
    let len = 1_usize << (first >> 6);
    let bytes = buf.get(*pos..*pos + len)?;
    let mut value = u64::from(first & 0x3f);
    for byte in &bytes[1..] {
        value = (value << 8) | u64::from(*byte);
    }
    *pos += len;
    Some(value)
}

/// Appends `value` as the shortest QUIC variable-length integer.
pub fn write_varint(out: &mut Vec<u8>, value: u64) {
    if value < 1 << 6 {
        out.push(value as u8);
    } else if value < 1 << 14 {
        out.extend_from_slice(&((value as u16) | 0x4000).to_be_bytes());
    } else if value < 1 << 30 {
        out.extend_from_slice(&((value as u32) | 0x8000_0000).to_be_bytes());
    } else {
        out.extend_from_slice(&(value | 0xc000_0000_0000_0000).to_be_bytes());
    }
}

// ---------------------------------------------------------------------------------------------
// HTTP/3 frames and SETTINGS (RFC 9114 sections 6 and 7)
// ---------------------------------------------------------------------------------------------

/// One HTTP/3 frame: its type and payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The frame type (0x00 DATA, 0x01 HEADERS, 0x04 SETTINGS, ...).
    pub ty: u64,
    /// The payload bytes.
    pub payload: Vec<u8>,
}

/// Splits `buf` into complete frames. The second value is how many bytes the complete frames
/// used; a frame cut off at the end of `buf` is left unread.
pub fn parse_frames(buf: &[u8]) -> (Vec<Frame>, usize) {
    let mut frames = Vec::new();
    let mut pos = 0;
    loop {
        let start = pos;
        let (Some(ty), Some(len)) = (read_varint(buf, &mut pos), read_varint(buf, &mut pos)) else {
            return (frames, start);
        };
        let Ok(len) = usize::try_from(len) else {
            return (frames, start);
        };
        let Some(payload) = buf.get(pos..pos + len) else {
            return (frames, start);
        };
        pos += len;
        frames.push(Frame {
            ty,
            payload: payload.to_vec(),
        });
    }
}

/// Appends one frame.
pub fn write_frame(out: &mut Vec<u8>, ty: u64, payload: &[u8]) {
    write_varint(out, ty);
    write_varint(out, payload.len() as u64);
    out.extend_from_slice(payload);
}

/// The identifier/value pairs of a SETTINGS payload, in wire order.
pub fn parse_settings(payload: &[u8]) -> Option<Vec<(u64, u64)>> {
    let mut pos = 0;
    let mut pairs = Vec::new();
    while pos < payload.len() {
        let id = read_varint(payload, &mut pos)?;
        let value = read_varint(payload, &mut pos)?;
        pairs.push((id, value));
    }
    Some(pairs)
}

/// True for the reserved "grease" values `0x1f * N + 0x21` (RFC 9114 section 7.2.8 for frame
/// types, 11.2.2 for settings and stream types).
pub fn is_h3_grease(value: u64) -> bool {
    value >= 0x21 && (value - 0x21) % 0x1f == 0
}

/// A name for a frame type.
pub fn frame_name(ty: u64) -> &'static str {
    match ty {
        0x00 => "DATA",
        0x01 => "HEADERS",
        0x03 => "CANCEL_PUSH",
        0x04 => "SETTINGS",
        0x05 => "PUSH_PROMISE",
        0x07 => "GOAWAY",
        0x0d => "MAX_PUSH_ID",
        t if is_h3_grease(t) => "GREASE",
        _ => "unknown",
    }
}

/// A name for a SETTINGS identifier (RFC 9114, RFC 9204, RFC 9220, RFC 9297).
pub fn setting_name(id: u64) -> &'static str {
    match id {
        0x01 => "QPACK_MAX_TABLE_CAPACITY",
        0x06 => "MAX_FIELD_SECTION_SIZE",
        0x07 => "QPACK_BLOCKED_STREAMS",
        0x08 => "ENABLE_CONNECT_PROTOCOL",
        0x33 => "H3_DATAGRAM",
        i if is_h3_grease(i) => "GREASE",
        _ => "unknown",
    }
}

/// A name for a unidirectional stream type.
pub fn stream_type_name(ty: u64) -> &'static str {
    match ty {
        0x00 => "control",
        0x01 => "push",
        0x02 => "qpack-encoder",
        0x03 => "qpack-decoder",
        t if is_h3_grease(t) => "grease",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------------------------
// Prefixed integers (RFC 7541 section 5.1, reused by QPACK)
// ---------------------------------------------------------------------------------------------

/// Reads an integer with an `n`-bit prefix. Returns the byte's flag bits (above the prefix)
/// and the value.
fn read_prefixed(buf: &[u8], pos: &mut usize, n: u8) -> Option<(u8, u64)> {
    let first = *buf.get(*pos)?;
    *pos += 1;
    let mask = ((1_u16 << n) - 1) as u8;
    let flags = first & !mask;
    let mut value = u64::from(first & mask);
    if value < u64::from(mask) {
        return Some((flags, value));
    }
    let mut shift = 0;
    loop {
        let byte = *buf.get(*pos)?;
        *pos += 1;
        value = value.checked_add(u64::from(byte & 0x7f).checked_shl(shift)?)?;
        if byte & 0x80 == 0 {
            return Some((flags, value));
        }
        shift += 7;
        if shift > 56 {
            return None;
        }
    }
}

/// Appends an integer with an `n`-bit prefix, `flags` already placed above the prefix.
pub fn write_prefixed(out: &mut Vec<u8>, flags: u8, n: u8, mut value: u64) {
    let max = (1_u64 << n) - 1;
    if value < max {
        out.push(flags | value as u8);
        return;
    }
    out.push(flags | max as u8);
    value -= max;
    while value >= 128 {
        out.push((value % 128) as u8 | 0x80);
        value /= 128;
    }
    out.push(value as u8);
}

// ---------------------------------------------------------------------------------------------
// Huffman (RFC 7541 Appendix B)
// ---------------------------------------------------------------------------------------------

/// `(code, length)` for each of the 257 symbols, read from the RFC table.
fn huffman_codes() -> &'static HashMap<(u8, u32), u16> {
    static TABLE: OnceLock<HashMap<(u8, u32), u16>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        for line in include_str!("huffman_table.txt").lines() {
            let mut parts = line.split_whitespace();
            let (Some(sym), Some(code), Some(len)) = (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            if let (Ok(sym), Ok(code), Ok(len)) = (
                sym.parse::<u16>(),
                u32::from_str_radix(code, 16),
                len.parse::<u8>(),
            ) {
                map.insert((len, code), sym);
            }
        }
        map
    })
}

/// Decodes a Huffman-coded string.
pub fn huffman_decode(data: &[u8]) -> Option<Vec<u8>> {
    let table = huffman_codes();
    let mut out = Vec::new();
    let mut code = 0_u32;
    let mut len = 0_u8;
    for byte in data {
        for bit in (0..8).rev() {
            code = (code << 1) | u32::from((byte >> bit) & 1);
            len += 1;
            if let Some(sym) = table.get(&(len, code)) {
                if *sym == 256 {
                    return None;
                }
                out.push(*sym as u8);
                code = 0;
                len = 0;
            } else if len > 30 {
                return None;
            }
        }
    }
    // What is left must be the start of EOS (all ones) and shorter than a byte.
    let padding_ok = len < 8 && code == (1_u32 << len) - 1;
    padding_ok.then_some(out)
}

// ---------------------------------------------------------------------------------------------
// QPACK field sections (RFC 9204 section 4.5)
// ---------------------------------------------------------------------------------------------

/// The static table: `(name, value)` by index.
pub fn static_table() -> &'static Vec<(String, String)> {
    static TABLE: OnceLock<Vec<(String, String)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut rows: Vec<(usize, String, String)> = Vec::new();
        for line in include_str!("qpack_static_table.txt").lines() {
            let mut parts = line.splitn(3, '|');
            if let (Some(index), Some(name), Some(value)) = (parts.next(), parts.next(), parts.next())
            {
                if let Ok(index) = index.parse::<usize>() {
                    rows.push((index, name.to_owned(), value.to_owned()));
                }
            }
        }
        rows.sort_by_key(|row| row.0);
        rows.into_iter().map(|(_, n, v)| (n, v)).collect()
    })
}

/// One decoded field line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// How the line was written on the wire, e.g. `indexed-static`.
    pub repr: String,
    /// The field name, or `dynamic:<n>` / `post-base:<n>` when it needs the dynamic table.
    pub name: String,
    /// The value, or empty when not known.
    pub value: String,
    /// Whether the value was Huffman-coded on the wire (`None` for an indexed line).
    pub huffman: Option<bool>,
}

/// A decoded QPACK field section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldSection {
    /// Required Insert Count as written (encoded form).
    pub required_insert_count: u64,
    /// The sign bit and Delta Base of the prefix.
    pub base: (bool, u64),
    /// The field lines in wire order.
    pub fields: Vec<Field>,
}

fn read_string(buf: &[u8], pos: &mut usize, flag_mask: u8, n: u8) -> Option<(String, bool)> {
    let (flags, len) = read_prefixed(buf, pos, n)?;
    let len = usize::try_from(len).ok()?;
    let raw = buf.get(*pos..*pos + len)?;
    *pos += len;
    let huffman = flags & flag_mask != 0;
    let bytes = if huffman {
        huffman_decode(raw)?
    } else {
        raw.to_vec()
    };
    Some((String::from_utf8_lossy(&bytes).into_owned(), huffman))
}

/// Decodes a field section (the payload of a HEADERS frame).
pub fn decode_field_section(block: &[u8]) -> Result<FieldSection, String> {
    let table = static_table();
    let mut pos = 0;
    let (_, required_insert_count) =
        read_prefixed(block, &mut pos, 8).ok_or("truncated prefix")?;
    let (sign, delta_base) = read_prefixed(block, &mut pos, 7).ok_or("truncated prefix")?;
    let mut fields = Vec::new();
    while pos < block.len() {
        let first = block[pos];
        if first & 0x80 != 0 {
            // Indexed field line: 1 T index(6+)
            let (flags, index) = read_prefixed(block, &mut pos, 6).ok_or("truncated indexed")?;
            let index = index as usize;
            if flags & 0x40 != 0 {
                let (name, value) = table.get(index).ok_or("static index out of range")?;
                fields.push(Field {
                    repr: format!("indexed-static[{index}]"),
                    name: name.clone(),
                    value: value.clone(),
                    huffman: None,
                });
            } else {
                fields.push(Field {
                    repr: format!("indexed-dynamic[{index}]"),
                    name: format!("dynamic:{index}"),
                    value: String::new(),
                    huffman: None,
                });
            }
        } else if first & 0xc0 == 0x40 {
            // Literal with name reference: 01 N T index(4+) then value
            let (flags, index) = read_prefixed(block, &mut pos, 4).ok_or("truncated literal")?;
            let never = flags & 0x20 != 0;
            let is_static = flags & 0x10 != 0;
            let index = index as usize;
            let name = if is_static {
                table.get(index).ok_or("static index out of range")?.0.clone()
            } else {
                format!("dynamic:{index}")
            };
            let (value, huffman) =
                read_string(block, &mut pos, 0x80, 7).ok_or("bad literal value")?;
            fields.push(Field {
                repr: format!(
                    "literal-name-ref-{}[{index}]{}",
                    if is_static { "static" } else { "dynamic" },
                    if never { "-never-indexed" } else { "" }
                ),
                name,
                value,
                huffman: Some(huffman),
            });
        } else if first & 0xe0 == 0x20 {
            // Literal with literal name: 001 N H name-length(3+) name, then value
            let (flags, name_len) = read_prefixed(block, &mut pos, 3).ok_or("truncated literal")?;
            let never = flags & 0x10 != 0;
            let name_huffman = flags & 0x08 != 0;
            let name_len = name_len as usize;
            let raw = block.get(pos..pos + name_len).ok_or("truncated name")?;
            pos += name_len;
            let name_bytes = if name_huffman {
                huffman_decode(raw).ok_or("bad huffman name")?
            } else {
                raw.to_vec()
            };
            let (value, huffman) =
                read_string(block, &mut pos, 0x80, 7).ok_or("bad literal value")?;
            fields.push(Field {
                repr: format!(
                    "literal-literal-name{}{}",
                    if name_huffman { "-huffman-name" } else { "" },
                    if never { "-never-indexed" } else { "" }
                ),
                name: String::from_utf8_lossy(&name_bytes).into_owned(),
                value,
                huffman: Some(huffman),
            });
        } else if first & 0xf0 == 0x10 {
            // Indexed with post-base index: 0001 index(4+)
            let (_, index) = read_prefixed(block, &mut pos, 4).ok_or("truncated post-base")?;
            fields.push(Field {
                repr: format!("indexed-post-base[{index}]"),
                name: format!("post-base:{index}"),
                value: String::new(),
                huffman: None,
            });
        } else {
            // Literal with post-base name reference: 0000 N index(3+) then value
            let (flags, index) = read_prefixed(block, &mut pos, 3).ok_or("truncated literal")?;
            let never = flags & 0x08 != 0;
            let (value, huffman) =
                read_string(block, &mut pos, 0x80, 7).ok_or("bad literal value")?;
            fields.push(Field {
                repr: format!(
                    "literal-post-base-name-ref[{index}]{}",
                    if never { "-never-indexed" } else { "" }
                ),
                name: format!("post-base:{index}"),
                value,
                huffman: Some(huffman),
            });
        }
    }
    Ok(FieldSection {
        required_insert_count,
        base: (sign & 0x80 != 0, delta_base),
        fields,
    })
}

/// Encodes a response field section with the static table only: `:status 200` (static index
/// 25) followed by literal lines, with no Huffman coding. Names are written as literals.
pub fn encode_response_headers(extra: &[(&str, &str)]) -> Vec<u8> {
    let mut out = vec![0x00, 0x00];
    // Indexed field line, static table (T = 1), index 25 = `:status 200`.
    write_prefixed(&mut out, 0xc0, 6, 25);
    for (name, value) in extra {
        // Literal with literal name: 001 N=0 H=0, name length with a 3-bit prefix.
        write_prefixed(&mut out, 0x20, 3, name.len() as u64);
        out.extend_from_slice(name.as_bytes());
        // Value: H = 0, length with a 7-bit prefix.
        write_prefixed(&mut out, 0x00, 7, value.len() as u64);
        out.extend_from_slice(value.as_bytes());
    }
    out
}

/// Lower-case hex of `bytes`.
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unhex(s: &str) -> Vec<u8> {
        let digits: Vec<u8> = s.bytes().filter(u8::is_ascii_hexdigit).collect();
        digits
            .chunks(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    #[test]
    fn varints_match_rfc_9000_appendix_a1() {
        for (wire, value) in [
            ("c2197c5eff14e88c", 151_288_809_941_952_652_u64),
            ("9d7f3e7d", 494_878_333),
            ("7bbd", 15_293),
            ("25", 37),
        ] {
            let bytes = unhex(wire);
            let mut pos = 0;
            assert_eq!(read_varint(&bytes, &mut pos), Some(value), "{wire}");
            assert_eq!(pos, bytes.len());
            let mut out = Vec::new();
            write_varint(&mut out, value);
            assert_eq!(out, bytes, "{wire} written back");
        }
    }

    #[test]
    fn huffman_codes_form_a_complete_prefix_code() {
        // Kraft sum over the 257 symbols is exactly 1 for a complete prefix code.
        let mut sum = 0_u128;
        let mut count = 0;
        for line in include_str!("huffman_table.txt").lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let len: u32 = parts[2].parse().unwrap();
            let code = u32::from_str_radix(parts[1], 16).unwrap();
            assert!(u64::from(code) < 1_u64 << len, "code fits its length: {line}");
            sum += 1_u128 << (30 - len);
            count += 1;
        }
        assert_eq!(count, 257);
        assert_eq!(sum, 1_u128 << 30);
    }

    #[test]
    fn huffman_decodes_the_rfc_7541_c4_examples() {
        assert_eq!(
            huffman_decode(&unhex("f1e3c2e5f23a6ba0ab90f4ff")).unwrap(),
            b"www.example.com"
        );
        assert_eq!(huffman_decode(&unhex("a8eb10649cbf")).unwrap(), b"no-cache");
        assert_eq!(
            huffman_decode(&unhex("25a849e95ba97d7f")).unwrap(),
            b"custom-key"
        );
        assert_eq!(
            huffman_decode(&unhex("25a849e95bb8e8b4bf")).unwrap(),
            b"custom-value"
        );
    }

    #[test]
    fn the_static_table_has_99_entries_in_order() {
        let table = static_table();
        assert_eq!(table.len(), 99);
        assert_eq!(table[0].0, ":authority");
        assert_eq!(table[17], (":method".into(), "GET".into()));
        assert_eq!(table[23], (":scheme".into(), "https".into()));
        assert_eq!(table[25], (":status".into(), "200".into()));
        assert_eq!(table[98].0, "x-frame-options");
    }

    #[test]
    fn a_literal_with_static_name_reference_decodes_as_in_rfc_9204_b1() {
        // RFC 9204 Appendix B.1: prefix 0000, then 0x51 = literal, static name index 1 (:path),
        // value "/index.html".
        let block = unhex("0000510b2f696e6465782e68746d6c");
        let section = decode_field_section(&block).unwrap();
        assert_eq!(section.fields.len(), 1);
        assert_eq!(section.fields[0].name, ":path");
        assert_eq!(section.fields[0].value, "/index.html");
        assert_eq!(section.fields[0].huffman, Some(false));
    }

    #[test]
    fn indexed_static_lines_decode() {
        // 0xd1 = indexed static 17 (:method GET), 0xd7 = 23 (:scheme https), 0xd9 = 25.
        let section = decode_field_section(&unhex("0000d1d7d9")).unwrap();
        let names: Vec<_> = section.fields.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, [":method", ":scheme", ":status"]);
        assert_eq!(section.fields[2].value, "200");
    }

    #[test]
    fn our_response_headers_decode_back() {
        let block = encode_response_headers(&[("x-spike-transport", "h3")]);
        let section = decode_field_section(&block).unwrap();
        assert_eq!(section.fields[0].name, ":status");
        assert_eq!(section.fields[1].name, "x-spike-transport");
        assert_eq!(section.fields[1].value, "h3");
    }

    #[test]
    fn settings_and_frames_round_trip() {
        let mut payload = Vec::new();
        for (id, value) in [(0x01_u64, 0_u64), (0x07, 0), (0x1f * 5 + 0x21, 1)] {
            write_varint(&mut payload, id);
            write_varint(&mut payload, value);
        }
        let mut wire = Vec::new();
        write_frame(&mut wire, 0x04, &payload);
        let (frames, used) = parse_frames(&wire);
        assert_eq!(used, wire.len());
        assert_eq!(frames.len(), 1);
        let pairs = parse_settings(&frames[0].payload).unwrap();
        assert_eq!(pairs, [(1, 0), (7, 0), (0x1f * 5 + 0x21, 1)]);
        assert!(is_h3_grease(pairs[2].0));
        assert!(!is_h3_grease(0x01));
    }

    #[test]
    fn a_cut_frame_is_left_unread() {
        let mut wire = Vec::new();
        write_frame(&mut wire, 0x01, b"abcdef");
        let (frames, used) = parse_frames(&wire[..wire.len() - 1]);
        assert!(frames.is_empty());
        assert_eq!(used, 0);
    }
}
