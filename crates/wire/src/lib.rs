//! Browser-identical TLS and HTTP/2 through wreq and `BoringSSL`, for seleniumbase-rs.
//!
//! The main types are re-exported here: the profile and the client (`WireFormat`,
//! `WireFormatBuilder`, `wire_client`), plain HTTP that continues a browser's session
//! (`HttpSession`, `HttpSessionOptions`, `HttpRequest`, `HttpResponse`, `HttpError` and `Method`), the
//! loopback proxy for a real Chrome (`MitmProxy`, `MitmOptions`, `MitmError`, `MitmStats`), the
//! dual-mode session (`StealthSession`, `StealthSessionBuilder`, `StealthFetch`, `StealthError`)
//! and the in-memory CA (`EphemeralCa`).
//!
//! Five names of the HTTP session are generic, and are re-exported with a prefix so that they do
//! not read as something else at this level: `HttpAttempt`, `HttpFetch`, `HttpRules` and `HttpVerdict` of the
//! judging calls become `HttpAttempt`, `HttpFetch`, `HttpRules` and `HttpVerdict`, and the
//! reply `ResponseHeaders` become `ResponseHeaders`. The original names stay in `http_session`.
//!
//! The stealth layer is experimental: its types may change between releases. It is measured on
//! macOS with Chrome 155 only.

#![forbid(unsafe_code)]

pub mod capture_h2;
pub mod client;
pub mod convert;
pub mod http_session;
pub mod mitm;
pub mod stealth_session;
mod sync;
pub mod tls_server;

pub use client::{
    wire_client, WireFormatBuilder, WireFormatError, WireFormat,
    WireProtocol,
};
pub use http_session::{
    HttpAttempt, HttpFetch, ResponseHeaders, HttpError,
    HttpSessionOptions, HttpResponse, HttpRequest, HttpSession, Method, ReplyProtocol, HttpRules,
    HttpVerdict,
};
pub use mitm::{MitmError, MitmOptions, MitmProxy, MitmStats};
pub use stealth_session::{StealthError, StealthFetch, StealthSession, StealthSessionBuilder};
pub use tls_server::{CaOptions, EphemeralCa, LeafCert, TlsServerError};
