//! Unified HTTP/3 layer: QUIC/HTTP/3 wire capture parsers and the `sbrs-h3d` client & pool.
//!
//! - [`capture`]: RFC 9001 Initial packet decryption, TLS `ClientHello` / transport parameter
//!   extraction, and HTTP/3 frame / SETTINGS / QPACK decoding (available with `network`).
//! - Client & pool (`feature = "http3"`): [`Http3Client`], [`Http3Pool`], [`Http3Options`],
//!   [`Http3Request`], [`Http3RequestKind`], [`Http3Response`], and [`Http3Error`], speaking
//!   over stdin/stdout JSON lines to the standalone `sbrs-h3d` helper (`h3d/`).
//!
//! `sbrs-h3d` runs out of process because `phantom-http`'s BoringSSL and `wreq`'s BoringSSL
//! export 6,047 colliding unprefixed C symbols (`docs/design/http3.md`).

pub mod capture;

#[cfg(feature = "http3")]
mod client;
#[cfg(feature = "http3")]
mod request;

#[cfg(feature = "http3")]
pub use client::{Http3Client, Http3Error, Http3Options, Http3Pool, HELPER_VAR};
#[cfg(feature = "http3")]
pub use request::{Http3Request, Http3RequestKind, Http3Response};

#[cfg(all(test, unix, feature = "http3"))]
mod tests;
