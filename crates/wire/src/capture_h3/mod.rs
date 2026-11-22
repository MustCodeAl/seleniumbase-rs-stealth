//! HTTP/3 capture and diagnostic parsers: RFC 9001 Initial packet decryption,
//! CRYPTO reassembly, TLS `ClientHello` and transport parameters extraction,
//! and HTTP/3 frames, SETTINGS and QPACK decoding.

#![allow(
    clippy::cast_possible_truncation,
    reason = "packet decoders work with variable-length and bounded RFC integer conversions"
)]
#![allow(
    clippy::must_use_candidate,
    reason = "diagnostic and parser utilities are used in tests and tools"
)]
#![allow(
    clippy::missing_errors_doc,
    reason = "internal capture parser error variants are documented on the error enum"
)]
#![allow(
    clippy::map_err_ignore,
    reason = "packet parser discards low-level crypto/codec error details for structured parse failures"
)]
#![allow(
    clippy::too_many_lines,
    reason = "state machine and RFC decoding routines are kept contiguous for clarity"
)]
#![allow(
    clippy::manual_is_multiple_of,
    reason = "compatibility across compiler targets"
)]
#![allow(
    clippy::collapsible_if,
    reason = "matches RFC decoding layout"
)]
#![allow(
    clippy::match_same_arms,
    reason = "explicit RFC wire type mapping"
)]
#![allow(
    clippy::doc_markdown,
    reason = "RFC terminology in doc comments"
)]

pub mod h3wire;
pub mod initial;

pub use h3wire::{decode_field_section, parse_frames, parse_settings, Frame};
pub use initial::{client_initial_keys, parse_long_header, InitialKeys, LongHeader};
