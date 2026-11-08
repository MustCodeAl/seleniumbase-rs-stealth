//! Wire-level fingerprints: TLS and HTTP/2 specs measured from a local browser, named by core's `Browser`.
//!
//! The main types are re-exported here: the parsers (`ClientHello`, `Ja3`, `Ja4`, `Ja4Raw`,
//! `Peetprint`, `Akamai`), the specs (`TlsSpec`, `H2Spec`, `HeaderSpec`), and the measured table
//! (`WireMeasurement`), with their errors `TlsSpecError`, `H2SpecError` and
//! `WireMeasurementError`. The lookups of the measured table are the functions of `measurement`
//! (`measurement::all`, `measurement::for_kind`, `measurement::get`), named for the module path
//! they are called through.
//!
//! The wire layer is experimental, like the rest of the stealth layer: its types may change
//! between releases.

#![forbid(unsafe_code)]

pub mod akamai;
pub mod client_hello;
pub mod h2_frames;
pub mod h2_spec;
pub mod header_spec;
pub mod ids;
pub mod ja3;
pub mod ja4;
pub mod measurement;
pub mod tls_spec;

pub use akamai::{Akamai, AkamaiError};
pub use client_hello::{ClientHello, ClientHelloError};
pub use h2_spec::{H2Spec, H2SpecError};
pub use header_spec::{ClientHeaders, HeaderSpec, HeaderSpecError, RequestShape};
pub use ja3::{Ja3, Ja3Error, Peetprint, PeetprintError};
pub use ja4::{Ja4, Ja4Error, Ja4Raw};
pub use measurement::{WireMeasurement, WireMeasurementError};
pub use tls_spec::{KnownGap, TlsSpec, TlsSpecError};
