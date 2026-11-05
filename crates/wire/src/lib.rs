//! Browser-identical TLS and HTTP/2 through wreq and `BoringSSL`, for seleniumbase-rs.
//!
//! The main types are re-exported here: the profile and the client (`WireFormat`,
//! `WireFormatBuilder`, `WireFormat::client_builder`), plain HTTP that continues a browser's session
//! (`HttpSession`, `HttpSessionOptions`, `HttpRequest`, `HttpResponse`, `HttpError` and `Method`), the
//! loopback proxy for a real Chrome (`MitmProxy`, `MitmOptions`, `MitmError`, `MitmStats`), the
//! dual-mode session (`StealthSession`, `StealthSessionBuilder`, `StealthFetch`, `StealthError`)
//! and the in-memory CA (`EphemeralCa`).
//!
//! The judging types of the HTTP session (`HttpAttempt`, `HttpFetch`, `HttpRules`, `HttpVerdict`)
//! and its reply headers (`ResponseHeaders`) are re-exported here under their own names.
//!
//! The stealth layer is experimental: its types may change between releases. It is measured on
//! macOS with Chrome 155 only.

#![forbid(unsafe_code)]

pub mod capture_h2;
pub mod client;
pub mod convert;
#[cfg(feature = "http3")]
pub mod http3;
pub mod http_session;
pub mod mitm;
pub mod stealth_session;
mod sync;
pub mod tls_server;

pub use client::{WireFormat, WireFormatBuilder, WireFormatError, WireProtocol};
#[cfg(feature = "http3")]
pub use http3::{Http3Client, Http3Error, Http3Options, Http3Request, Http3Response};
pub use http_session::{
    HttpAttempt, HttpError, HttpFetch, HttpRequest, HttpResponse, HttpRules, HttpSession,
    HttpSessionOptions, HttpVerdict, Method, ResponseHeaders,
};
pub use mitm::{MitmError, MitmOptions, MitmProxy, MitmStats};
pub use stealth_session::{StealthError, StealthFetch, StealthSession, StealthSessionBuilder};
pub use tls_server::{CaOptions, EphemeralCa, LeafCert, TlsServerError};
