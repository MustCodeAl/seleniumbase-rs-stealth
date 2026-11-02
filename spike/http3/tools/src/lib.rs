//! Tools of the HTTP/3 spike: the wire parsers its two binaries share.
//!
//! - [`initial`]: decrypts a client's QUIC Initial packet (RFC 9001 section 5) and reads the
//!   TLS `ClientHello` inside it, with the QUIC transport parameters in wire order.
//! - [`h3wire`]: HTTP/3 frames, SETTINGS and a QPACK field-section reader (RFC 9114, RFC 9204).
//!
//! Spike code: it parses what the clients under test send and does not defend against hostile
//! input beyond not panicking.

pub mod h3wire;
pub mod initial;
