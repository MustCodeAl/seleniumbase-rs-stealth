//! Layer 3 (`network`): wire protocol specs, proxy routing, wire capture, and
//! opt-in BoringSSL TLS/HTTP2/HTTP3/MITM transport.
//!
//! - [`spec`] *(feature `network-spec`, pure Rust, zero I/O, zero BoringSSL)*:
//!   TLS `ClientHello`, JA3/JA4/peetprint, Akamai HTTP/2, HTTP/3, header
//!   specs, and measured browser tables (`WireMeasurement`).
//! - [`proxy`] *(feature `network-spec`, zero BoringSSL)*: country-aware
//!   `ProxyPool`, `ProxyRoute`, `StaticGeoResolver`, URL redaction, and
//!   `ProxySwitch`.
//! - [`capture`] *(feature `session` for TLS/hints; feature `network` for H2)*:
//!   local Chrome measurement capture tools.
//! - [`wire`], [`convert`], [`mitm`], [`tls_server`], [`http3`] *(feature
//!   `network`, plus `http3` for `sbrs-h3d` client/pool)*: BoringSSL/wreq wire
//!   impersonation, loopback `MitmProxy`, `EphemeralCa`, and unified HTTP/3.

#[cfg(feature = "session")]
pub mod capture;
#[cfg(feature = "network")]
pub mod convert;
#[cfg(feature = "network")]
pub mod http3;
#[cfg(feature = "network")]
pub mod mitm;
pub mod proxy;
pub mod spec;
#[cfg(feature = "network")]
pub mod tls_server;
#[cfg(feature = "network")]
pub mod wire;

#[cfg(feature = "session")]
pub use proxy::ProxySwitch;
pub use proxy::{
    CountryCode, GeoResolver, LocaleExt, ProxyKey, ProxyPool, ProxyRoute, RedactedUrl, Secret,
    StaticGeoResolver,
};

pub use spec::{
    Akamai, AkamaiError, ClientHeaders, ClientHello, ClientHelloError, H2Spec, H2SpecError, H3Spec,
    H3SpecError, HeaderSpec, HeaderSpecError, Ja3, Ja3Error, Ja4, Ja4Error, Ja4Raw, KnownGap,
    Peetprint, PeetprintError, RequestShape, TlsSpec, TlsSpecError, TransportParam,
    WireMeasurement, WireMeasurementError,
};

#[cfg(feature = "network")]
pub use mitm::{MitmError, MitmOptions, MitmProxy, MitmStats};
#[cfg(feature = "network")]
pub use tls_server::{CaOptions, EphemeralCa, LeafCert, TlsServerError};
#[cfg(feature = "network")]
pub use wire::{WireFormat, WireFormatBuilder, WireFormatError, WireProtocol};

#[cfg(feature = "http3")]
pub use http3::{
    Http3Client, Http3Error, Http3Options, Http3Pool, Http3Request, Http3RequestKind, Http3Response,
};
