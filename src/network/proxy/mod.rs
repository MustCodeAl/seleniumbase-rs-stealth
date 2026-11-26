//! Proxy routing, rotation pool, geo-aligned locale resolution, URL redaction,
//! and proxy-switching seam (zero BoringSSL).

pub mod geo;
pub mod pool;
pub mod redact;
pub mod route;
#[cfg(feature = "session")]
pub mod switch;

pub use geo::{CountryCode, GeoResolver, LocaleExt, StaticGeoResolver};
pub use pool::ProxyPool;
pub use redact::{RedactedUrl, Secret};
pub use route::{ProxyKey, ProxyRoute};
#[cfg(feature = "session")]
pub use switch::ProxySwitch;
