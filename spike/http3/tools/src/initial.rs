//! Reads the first flight of a QUIC client without the client's keys.
//!
//! An Initial packet is protected with keys derived from its own Destination Connection ID
//! (RFC 9001 section 5.2), so anyone who sees the datagram can decrypt it. This module does
//! that, reassembles the CRYPTO frames and parses the TLS `ClientHello` inside, including the
//! `quic_transport_parameters` extension in wire order.
//!
//! Versions: QUIC v1 (RFC 9001) and v2 (RFC 9369). The salts and labels are from those RFCs;
//! the unit tests decrypt the RFC 9001 appendix A.2 packet and compare the derived keys with
//! appendix A.1.

use std::collections::BTreeMap;

use ring::{aead, hkdf};
use serde_json::{json, Value};

use crate::h3wire::{hex, read_varint};

/// QUIC version 1.
pub const VERSION_1: u32 = 1;
/// QUIC version 2 (RFC 9369).
pub const VERSION_2: u32 = 0x6b33_43cf;

const SALT_V1: &str = "38762cf7f55934b34d179ae6a4c80cadccbb7f0a";
const SALT_V2: &str = "0dede3def700a6db819381be6e269dcbf9bd2ed9";

/// Parses hex text, ignoring anything that is not a hex digit.
pub fn unhex(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text.bytes().filter(u8::is_ascii_hexdigit).collect();
    digits
        .chunks(2)
        .filter_map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

struct Len(usize);

impl hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

/// TLS 1.3 `HKDF-Expand-Label` with an empty context (RFC 8446 section 7.1).
fn expand_label(prk: &hkdf::Prk, label: &str, len: usize) -> Result<Vec<u8>, String> {
    let full = format!("tls13 {label}");
    let mut info = Vec::new();
    info.extend_from_slice(&(len as u16).to_be_bytes());
    info.push(full.len() as u8);
    info.extend_from_slice(full.as_bytes());
    info.push(0);
    let info = [info.as_slice()];
    let okm = prk
        .expand(&info, Len(len))
        .map_err(|_| "hkdf expand".to_owned())?;
    let mut out = vec![0_u8; len];
    okm.fill(&mut out).map_err(|_| "hkdf fill".to_owned())?;
    Ok(out)
}

/// The client's Initial packet protection keys.
#[derive(Debug, PartialEq, Eq)]
pub struct InitialKeys {
    /// AEAD key (AES-128-GCM).
    pub key: Vec<u8>,
    /// AEAD IV.
    pub iv: Vec<u8>,
    /// Header protection key (AES-128).
    pub hp: Vec<u8>,
}

/// Derives the client Initial keys for a Destination Connection ID.
pub fn client_initial_keys(version: u32, dcid: &[u8]) -> Result<InitialKeys, String> {
    let (salt, key_label, iv_label, hp_label) = match version {
        VERSION_1 => (SALT_V1, "quic key", "quic iv", "quic hp"),
        VERSION_2 => (SALT_V2, "quicv2 key", "quicv2 iv", "quicv2 hp"),
        other => return Err(format!("QUIC version {other:#x} is not decoded")),
    };
    let initial_secret = hkdf::Salt::new(hkdf::HKDF_SHA256, &unhex(salt)).extract(dcid);
    let client_secret = expand_label(&initial_secret, "client in", 32)?;
    let prk = hkdf::Prk::new_less_safe(hkdf::HKDF_SHA256, &client_secret);
    Ok(InitialKeys {
        key: expand_label(&prk, key_label, 16)?,
        iv: expand_label(&prk, iv_label, 12)?,
        hp: expand_label(&prk, hp_label, 16)?,
    })
}

/// The unprotected fields of a long header that opens an Initial packet.
#[derive(Clone, Debug)]
pub struct LongHeader {
    /// QUIC version.
    pub version: u32,
    /// Destination Connection ID.
    pub dcid: Vec<u8>,
    /// Source Connection ID.
    pub scid: Vec<u8>,
    /// Packet type bits as written for this version (v1: 0 Initial; v2: 1 Initial).
    pub packet_type: u8,
    /// Retry/address-validation token.
    pub token: Vec<u8>,
    /// The Length field: packet number plus payload plus tag.
    pub length: usize,
    /// Offset of the (protected) packet number.
    pub pn_offset: usize,
}

/// Reads the long header of the packet at the start of `dgram`.
pub fn parse_long_header(dgram: &[u8]) -> Result<LongHeader, String> {
    let first = *dgram.first().ok_or("empty datagram")?;
    if first & 0x80 == 0 {
        return Err("short header".into());
    }
    let version = u32::from_be_bytes(
        dgram
            .get(1..5)
            .ok_or("short")?
            .try_into()
            .map_err(|_| "short")?,
    );
    let mut pos = 5;
    let dcid_len = usize::from(*dgram.get(pos).ok_or("short")?);
    pos += 1;
    let dcid = dgram.get(pos..pos + dcid_len).ok_or("short dcid")?.to_vec();
    pos += dcid_len;
    let scid_len = usize::from(*dgram.get(pos).ok_or("short")?);
    pos += 1;
    let scid = dgram.get(pos..pos + scid_len).ok_or("short scid")?.to_vec();
    pos += scid_len;
    let packet_type = (first >> 4) & 0x03;
    let initial_type = if version == VERSION_2 { 1 } else { 0 };
    if packet_type != initial_type {
        return Err(format!("not an Initial packet (type bits {packet_type})"));
    }
    let token_len = read_varint(dgram, &mut pos).ok_or("short token length")?;
    let token_len = usize::try_from(token_len).map_err(|_| "token length")?;
    let token = dgram
        .get(pos..pos + token_len)
        .ok_or("short token")?
        .to_vec();
    pos += token_len;
    let length = read_varint(dgram, &mut pos).ok_or("short length")?;
    let length = usize::try_from(length).map_err(|_| "length")?;
    if pos + length > dgram.len() {
        return Err("packet longer than datagram".into());
    }
    Ok(LongHeader {
        version,
        dcid,
        scid,
        packet_type,
        token,
        length,
        pn_offset: pos,
    })
}

/// A decrypted Initial packet.
#[derive(Clone, Debug)]
pub struct Initial {
    /// The unprotected header fields.
    pub header: LongHeader,
    /// The packet number.
    pub packet_number: u64,
    /// Packet number length in bytes.
    pub pn_len: usize,
    /// The decrypted frames.
    pub frames: Vec<u8>,
    /// Bytes of the datagram this packet used.
    pub packet_len: usize,
}

/// Removes header protection and decrypts the Initial packet at the start of `dgram`.
pub fn decrypt_initial(dgram: &[u8]) -> Result<Initial, String> {
    let header = parse_long_header(dgram)?;
    let keys = client_initial_keys(header.version, &header.dcid)?;
    let packet_len = header.pn_offset + header.length;
    let mut packet = dgram[..packet_len].to_vec();

    let sample_start = header.pn_offset + 4;
    let sample = packet
        .get(sample_start..sample_start + 16)
        .ok_or("packet too short to sample")?;
    let hp = aead::quic::HeaderProtectionKey::new(&aead::quic::AES_128, &keys.hp)
        .map_err(|_| "hp key")?;
    let mask = hp.new_mask(sample).map_err(|_| "hp mask")?;

    packet[0] ^= mask[0] & 0x0f;
    let pn_len = usize::from(packet[0] & 0x03) + 1;
    let mut packet_number = 0_u64;
    for i in 0..pn_len {
        packet[header.pn_offset + i] ^= mask[1 + i];
        packet_number = (packet_number << 8) | u64::from(packet[header.pn_offset + i]);
    }

    let mut nonce = [0_u8; 12];
    nonce.copy_from_slice(&keys.iv);
    for (slot, byte) in nonce[4..].iter_mut().zip(packet_number.to_be_bytes()) {
        *slot ^= byte;
    }
    let payload_start = header.pn_offset + pn_len;
    let aad = packet[..payload_start].to_vec();
    let mut body = packet[payload_start..].to_vec();
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_128_GCM, &keys.key).map_err(|_| "aead key")?,
    );
    let plain = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(aad),
            &mut body,
        )
        .map_err(|_| "AEAD open failed (wrong keys or not an Initial of this connection)")?;
    Ok(Initial {
        header,
        packet_number,
        pn_len,
        frames: plain.to_vec(),
        packet_len,
    })
}

/// A CRYPTO frame.
#[derive(Clone, Debug)]
pub struct CryptoFrame {
    /// Offset in the crypto stream.
    pub offset: u64,
    /// The data.
    pub data: Vec<u8>,
}

/// Frames of an Initial payload in wire order: a description for each, and the CRYPTO data.
pub fn read_frames(plain: &[u8]) -> (Vec<String>, Vec<CryptoFrame>) {
    let mut names = Vec::new();
    let mut crypto = Vec::new();
    let mut pos = 0;
    while pos < plain.len() {
        let Some(ty) = read_varint(plain, &mut pos) else {
            break;
        };
        match ty {
            0x00 => {
                let mut run = 1;
                while plain.get(pos) == Some(&0) {
                    pos += 1;
                    run += 1;
                }
                names.push(format!("PADDING x{run}"));
            }
            0x01 => names.push("PING".into()),
            0x02 | 0x03 => {
                let mut fields = Vec::new();
                for _ in 0..3 {
                    fields.push(read_varint(plain, &mut pos));
                }
                let first_range = read_varint(plain, &mut pos);
                let ranges = fields[2].unwrap_or(0);
                for _ in 0..ranges {
                    let _ = read_varint(plain, &mut pos);
                    let _ = read_varint(plain, &mut pos);
                }
                if ty == 0x03 {
                    for _ in 0..3 {
                        let _ = read_varint(plain, &mut pos);
                    }
                }
                names.push(format!(
                    "ACK largest={:?} first_range={first_range:?}",
                    fields[0]
                ));
            }
            0x06 => {
                let (Some(offset), Some(len)) =
                    (read_varint(plain, &mut pos), read_varint(plain, &mut pos))
                else {
                    break;
                };
                let Ok(len) = usize::try_from(len) else {
                    break;
                };
                let Some(data) = plain.get(pos..pos + len) else {
                    names.push("CRYPTO (truncated)".into());
                    break;
                };
                pos += len;
                names.push(format!("CRYPTO off={offset} len={len}"));
                crypto.push(CryptoFrame {
                    offset,
                    data: data.to_vec(),
                });
            }
            other => {
                names.push(format!("frame type {other:#x} (stop)"));
                break;
            }
        }
    }
    (names, crypto)
}

/// CRYPTO data from several packets, joined into the contiguous prefix of the stream.
#[derive(Default, Debug)]
pub struct CryptoStream {
    chunks: BTreeMap<u64, Vec<u8>>,
}

impl CryptoStream {
    /// Adds a frame.
    pub fn add(&mut self, frame: &CryptoFrame) {
        let slot = self.chunks.entry(frame.offset).or_default();
        if slot.len() < frame.data.len() {
            *slot = frame.data.clone();
        }
    }

    /// The bytes from offset 0 up to the first gap.
    pub fn contiguous(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (offset, data) in &self.chunks {
            let have = out.len() as u64;
            if *offset > have {
                break;
            }
            let skip = usize::try_from(have - offset).unwrap_or(usize::MAX);
            if skip < data.len() {
                out.extend_from_slice(&data[skip..]);
            }
        }
        out
    }
}

/// A parsed TLS `ClientHello`.
#[derive(Clone, Debug)]
pub struct ClientHello {
    /// `legacy_version`.
    pub legacy_version: u16,
    /// Length of the legacy session id.
    pub session_id_len: usize,
    /// Cipher suites in wire order.
    pub cipher_suites: Vec<u16>,
    /// Compression methods.
    pub compression: Vec<u8>,
    /// Extensions in wire order: `(type, data)`.
    pub extensions: Vec<(u16, Vec<u8>)>,
    /// Bytes the whole handshake message used (header included).
    pub message_len: usize,
}

/// Parse state of a `ClientHello` that may be split over several packets.
#[derive(Debug)]
pub enum HelloState {
    /// More CRYPTO data is needed.
    Incomplete,
    /// The data is not a `ClientHello`.
    Malformed(String),
}

fn be16(buf: &[u8], pos: &mut usize) -> Option<u16> {
    let value = u16::from_be_bytes(buf.get(*pos..*pos + 2)?.try_into().ok()?);
    *pos += 2;
    Some(value)
}

/// Parses the `ClientHello` at the start of the crypto stream.
pub fn parse_client_hello(stream: &[u8]) -> Result<ClientHello, HelloState> {
    if stream.len() < 4 {
        return Err(HelloState::Incomplete);
    }
    if stream[0] != 1 {
        return Err(HelloState::Malformed(format!(
            "handshake type {} is not ClientHello",
            stream[0]
        )));
    }
    let len = (usize::from(stream[1]) << 16) | (usize::from(stream[2]) << 8) | usize::from(stream[3]);
    if stream.len() < 4 + len {
        return Err(HelloState::Incomplete);
    }
    let body = &stream[4..4 + len];
    let bad = |what: &str| HelloState::Malformed(what.to_owned());
    let mut pos = 0;
    let legacy_version = be16(body, &mut pos).ok_or_else(|| bad("version"))?;
    pos += 32; // random
    let sid_len = usize::from(*body.get(pos).ok_or_else(|| bad("session id"))?);
    pos += 1 + sid_len;
    let suites_len = usize::from(be16(body, &mut pos).ok_or_else(|| bad("suites"))?);
    let suite_bytes = body.get(pos..pos + suites_len).ok_or_else(|| bad("suites"))?;
    let cipher_suites = suite_bytes
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    pos += suites_len;
    let comp_len = usize::from(*body.get(pos).ok_or_else(|| bad("compression"))?);
    pos += 1;
    let compression = body
        .get(pos..pos + comp_len)
        .ok_or_else(|| bad("compression"))?
        .to_vec();
    pos += comp_len;
    let ext_total = usize::from(be16(body, &mut pos).ok_or_else(|| bad("extensions"))?);
    let ext_end = pos + ext_total;
    if ext_end > body.len() {
        return Err(bad("extensions overrun"));
    }
    let mut extensions = Vec::new();
    while pos < ext_end {
        let ty = be16(body, &mut pos).ok_or_else(|| bad("extension type"))?;
        let elen = usize::from(be16(body, &mut pos).ok_or_else(|| bad("extension length"))?);
        let data = body.get(pos..pos + elen).ok_or_else(|| bad("extension data"))?;
        pos += elen;
        extensions.push((ty, data.to_vec()));
    }
    Ok(ClientHello {
        legacy_version,
        session_id_len: sid_len,
        cipher_suites,
        compression,
        extensions,
        message_len: 4 + len,
    })
}

/// GREASE values of TLS (RFC 8701): both bytes equal, low nibble 0xa.
pub fn is_tls_grease(value: u16) -> bool {
    value >> 8 == value & 0xff && value & 0x000f == 0x000a
}

/// GREASE transport parameter ids: `31 * N + 27` (RFC 9000 section 18.1).
pub fn is_tp_grease(id: u64) -> bool {
    id >= 27 && (id - 27) % 31 == 0
}

/// Names of the TLS extensions that matter for a QUIC client.
pub fn extension_name(id: u16) -> &'static str {
    match id {
        0x0000 => "server_name",
        0x0005 => "status_request",
        0x000a => "supported_groups",
        0x000b => "ec_point_formats",
        0x000d => "signature_algorithms",
        0x0010 => "application_layer_protocol_negotiation",
        0x0012 => "signed_certificate_timestamp",
        0x0015 => "padding",
        0x0017 => "extended_master_secret",
        0x001b => "compress_certificate",
        0x0023 => "session_ticket",
        0x0029 => "pre_shared_key",
        0x002b => "supported_versions",
        0x002d => "psk_key_exchange_modes",
        0x0033 => "key_share",
        0x0039 => "quic_transport_parameters",
        0x4469 => "application_settings (old codepoint)",
        0x44cd => "application_settings",
        0xfe0d => "encrypted_client_hello",
        0xff01 => "renegotiation_info",
        0xffa5 => "quic_transport_parameters (draft codepoint)",
        v if is_tls_grease(v) => "GREASE",
        _ => "unlabelled",
    }
}

/// The RFC-defined transport parameter names; anything else is left unlabelled on purpose.
pub fn tp_name(id: u64) -> &'static str {
    match id {
        0x00 => "original_destination_connection_id",
        0x01 => "max_idle_timeout",
        0x02 => "stateless_reset_token",
        0x03 => "max_udp_payload_size",
        0x04 => "initial_max_data",
        0x05 => "initial_max_stream_data_bidi_local",
        0x06 => "initial_max_stream_data_bidi_remote",
        0x07 => "initial_max_stream_data_uni",
        0x08 => "initial_max_streams_bidi",
        0x09 => "initial_max_streams_uni",
        0x0a => "ack_delay_exponent",
        0x0b => "max_ack_delay",
        0x0c => "disable_active_migration",
        0x0d => "preferred_address",
        0x0e => "active_connection_id_limit",
        0x0f => "initial_source_connection_id",
        0x10 => "retry_source_connection_id",
        0x11 => "version_information",
        0x20 => "max_datagram_frame_size",
        0x2ab2 => "grease_quic_bit",
        v if is_tp_grease(v) => "GREASE",
        _ => "unlabelled",
    }
}

/// One transport parameter as written.
#[derive(Clone, Debug)]
pub struct TransportParam {
    /// The parameter id.
    pub id: u64,
    /// The raw value.
    pub value: Vec<u8>,
}

/// Parses a `quic_transport_parameters` extension body into wire order.
pub fn parse_transport_params(data: &[u8]) -> Result<Vec<TransportParam>, String> {
    let mut pos = 0;
    let mut out = Vec::new();
    while pos < data.len() {
        let id = read_varint(data, &mut pos).ok_or("transport parameter id")?;
        let len = read_varint(data, &mut pos).ok_or("transport parameter length")?;
        let len = usize::try_from(len).map_err(|_| "length")?;
        let value = data
            .get(pos..pos + len)
            .ok_or("transport parameter value")?
            .to_vec();
        pos += len;
        out.push(TransportParam { id, value });
    }
    Ok(out)
}

/// The parameter ids whose value is one variable-length integer.
fn tp_is_integer(id: u64) -> bool {
    matches!(id, 0x01 | 0x03..=0x0b | 0x0e | 0x20)
}

/// A transport parameter as JSON: raw id and hex always, a decimal when the value is one varint.
pub fn tp_json(param: &TransportParam) -> Value {
    let mut decimal = Value::Null;
    if tp_is_integer(param.id) {
        let mut pos = 0;
        if let Some(value) = read_varint(&param.value, &mut pos) {
            if pos == param.value.len() {
                decimal = json!(value);
            }
        }
    }
    json!({
        "id": format!("{:#x}", param.id),
        "name": tp_name(param.id),
        "grease": is_tp_grease(param.id),
        "len": param.value.len(),
        "value_hex": hex(&param.value),
        "value_int": decimal,
    })
}

fn u16_list(data: &[u8]) -> Vec<String> {
    data.chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| {
            let v = u16::from_be_bytes([c[0], c[1]]);
            if is_tls_grease(v) {
                format!("{v:#06x}(GREASE)")
            } else {
                format!("{v:#06x}")
            }
        })
        .collect()
}

fn length_prefixed_strings(data: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(len) = data.get(pos) {
        let len = usize::from(*len);
        pos += 1;
        let Some(item) = data.get(pos..pos + len) else {
            break;
        };
        out.push(String::from_utf8_lossy(item).into_owned());
        pos += len;
    }
    out
}

/// A readable form of one extension's body.
pub fn describe_extension(id: u16, data: &[u8]) -> Value {
    let decoded = match id {
        0x0000 => {
            let mut pos = 2;
            let host = match data.get(pos) {
                Some(0) => {
                    pos += 1;
                    let len = be16(data, &mut pos).map(usize::from).unwrap_or(0);
                    data.get(pos..pos + len)
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                }
                _ => None,
            };
            json!({ "host_name": host })
        }
        0x000a | 0x000d => json!(u16_list(data.get(2..).unwrap_or_default())),
        0x000b | 0x002d => json!(hex(data.get(1..).unwrap_or_default())),
        0x0010 | 0x4469 | 0x44cd => json!(length_prefixed_strings(data.get(2..).unwrap_or_default())),
        0x001b => json!(u16_list(data.get(1..).unwrap_or_default())),
        0x002b => json!(u16_list(data.get(1..).unwrap_or_default())),
        0x0033 => {
            let mut shares = Vec::new();
            let mut pos = 2;
            while let Some(group) = be16(data, &mut pos) {
                let Some(len) = be16(data, &mut pos).map(usize::from) else {
                    break;
                };
                pos += len;
                let group_text = if is_tls_grease(group) {
                    format!("{group:#06x}(GREASE)")
                } else {
                    format!("{group:#06x}")
                };
                shares.push(format!("{group_text} keylen={len}"));
            }
            json!(shares)
        }
        0xfe0d => {
            // ECHClientHello (draft-ietf-tls-esni): type, kdf, aead, config id, enc, payload.
            let mut pos = 0;
            let ty = data.first().copied();
            pos += 1;
            let kdf = be16(data, &mut pos);
            let aead = be16(data, &mut pos);
            let config_id = data.get(pos).copied();
            pos += 1;
            let enc_len = be16(data, &mut pos).map(usize::from).unwrap_or(0);
            pos += enc_len;
            let payload_len = be16(data, &mut pos);
            json!({ "type": ty, "kdf": kdf, "aead": aead, "config_id": config_id,
                    "enc_len": enc_len, "payload_len": payload_len })
        }
        0x0015 => json!({ "padding_len": data.len() }),
        _ => json!(hex(&data[..data.len().min(32)])),
    };
    json!({
        "id": format!("{id:#06x}"),
        "name": extension_name(id),
        "grease": is_tls_grease(id),
        "len": data.len(),
        "decoded": decoded,
    })
}

/// Everything the spike reports about a `ClientHello`, as JSON.
pub fn client_hello_json(hello: &ClientHello) -> Value {
    let suites: Vec<String> = hello
        .cipher_suites
        .iter()
        .map(|s| {
            if is_tls_grease(*s) {
                format!("{s:#06x}(GREASE)")
            } else {
                format!("{s:#06x}")
            }
        })
        .collect();
    let transport = hello
        .extensions
        .iter()
        .find(|(id, _)| *id == 0x0039 || *id == 0xffa5)
        .map(|(id, data)| {
            let params = parse_transport_params(data);
            json!({
                "extension": format!("{id:#06x}"),
                "params": params.as_ref().map(|p| p.iter().map(tp_json).collect::<Vec<_>>()).unwrap_or_default(),
                "error": params.err(),
                "raw_hex": hex(data),
            })
        });
    json!({
        "legacy_version": format!("{:#06x}", hello.legacy_version),
        "session_id_len": hello.session_id_len,
        "cipher_suites": suites,
        "compression": hex(&hello.compression),
        "extension_order": hello.extensions.iter().map(|(id, _)| format!("{id:#06x}")).collect::<Vec<_>>(),
        "extensions": hello.extensions.iter().map(|(id, data)| describe_extension(*id, data)).collect::<Vec<_>>(),
        "transport_parameters": transport,
        "message_len": hello.message_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const A2_PACKET: &str = include_str!("../tests/fixtures/rfc9001_a2_client_initial.hex");

    #[test]
    fn keys_match_rfc_9001_appendix_a1() {
        let keys = client_initial_keys(VERSION_1, &unhex("8394c8f03e515708")).unwrap();
        assert_eq!(hex(&keys.key), "1f369613dd76d5467730efcbe3b1a22d");
        assert_eq!(hex(&keys.iv), "fa044b2f42a3fd3b46fb255c");
        assert_eq!(hex(&keys.hp), "9f50449e04a0e810283a1e9933adedd2");
    }

    #[test]
    fn the_rfc_9001_a2_client_initial_decrypts() {
        let wire = unhex(A2_PACKET);
        assert_eq!(wire.len(), 1200);
        let initial = decrypt_initial(&wire).unwrap();
        assert_eq!(initial.packet_number, 2);
        assert_eq!(initial.pn_len, 4);
        assert_eq!(hex(&initial.header.dcid), "8394c8f03e515708");
        assert_eq!(initial.frames.len(), 1162);
        assert_eq!(&initial.frames[..4], &unhex("060040f1")[..]);

        let (names, crypto) = read_frames(&initial.frames);
        assert_eq!(names[0], "CRYPTO off=0 len=241");
        let mut stream = CryptoStream::default();
        for frame in &crypto {
            stream.add(frame);
        }
        let hello = parse_client_hello(&stream.contiguous()).unwrap();
        let ids: Vec<u16> = hello.extensions.iter().map(|(id, _)| *id).collect();
        // The RFC example offers server_name first and quic_transport_parameters last.
        // (the ids between them are not asserted here)
        // The transport parameters are asserted below, in wire order.
        assert_eq!(ids.first(), Some(&0x0000));
        assert_eq!(ids.last(), Some(&0x0039));

        let (_, data) = hello.extensions.iter().find(|(id, _)| *id == 0x0039).unwrap();
        let params = parse_transport_params(data).unwrap();
        let order: Vec<u64> = params.iter().map(|p| p.id).collect();
        assert_eq!(order, [0x04, 0x05, 0x07, 0x08, 0x01, 0x09, 0x0f, 0x06]);
        assert_eq!(hex(&params[6].value), "8394c8f03e515708");
    }

    #[test]
    fn a_flipped_byte_fails_authentication() {
        let mut wire = unhex(A2_PACKET);
        wire[700] ^= 1;
        assert!(decrypt_initial(&wire).is_err());
    }

    #[test]
    fn a_short_header_is_not_an_initial() {
        assert!(decrypt_initial(&[0x40, 1, 2, 3]).is_err());
    }

    #[test]
    fn grease_values() {
        assert!(is_tls_grease(0x0a0a));
        assert!(is_tls_grease(0xfafa));
        assert!(!is_tls_grease(0x1301));
        assert!(is_tp_grease(27));
        assert!(is_tp_grease(27 + 31 * 1000));
        assert!(!is_tp_grease(0x04));
    }

    #[test]
    fn crypto_frames_reassemble_out_of_order_and_overlapping() {
        let mut stream = CryptoStream::default();
        stream.add(&CryptoFrame { offset: 3, data: b"def".to_vec() });
        assert_eq!(stream.contiguous(), b"");
        stream.add(&CryptoFrame { offset: 0, data: b"abcd".to_vec() });
        assert_eq!(stream.contiguous(), b"abcdef");
    }
}
