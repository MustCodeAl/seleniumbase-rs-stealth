//! The HTTP/3 specification: QUIC transport parameters, HTTP/3 SETTINGS, and pseudo-header ordering.
//!
//! [`H3Spec`] describes the HTTP/3 and QUIC parameters reproduced from a browser:
//! the QUIC transport parameters in wire order, the HTTP/3 SETTINGS frame entries,
//! and the pseudo-header order (`:method`, `:authority`, `:scheme`, `:path`).

use crate::identity::fingerprint::OsType;
use serde::{Deserialize, Serialize};
use thiserror::Error;

fn default_major() -> u32 {
    157
}

fn default_os() -> OsType {
    OsType::Macos
}

/// SETTINGS identifiers (RFC 9114 section 7.2.4.1 and extensions).
pub mod setting_id {
    /// `SETTINGS_QPACK_MAX_TABLE_CAPACITY` (RFC 9204 section 5).
    pub const QPACK_MAX_TABLE_CAPACITY: u64 = 0x01;
    /// `SETTINGS_MAX_FIELD_SECTION_SIZE` (RFC 9114 section 7.2.4.1).
    pub const MAX_FIELD_SECTION_SIZE: u64 = 0x06;
    /// `SETTINGS_QPACK_BLOCKED_STREAMS` (RFC 9204 section 5).
    pub const QPACK_BLOCKED_STREAMS: u64 = 0x07;
    /// `SETTINGS_ENABLE_CONNECT_PROTOCOL` (RFC 9220).
    pub const ENABLE_CONNECT_PROTOCOL: u64 = 0x08;
    /// `SETTINGS_H3_DATAGRAM` (RFC 9297).
    pub const H3_DATAGRAM: u64 = 0x33;
}

/// QUIC transport parameter identifiers (RFC 9000 section 18.2).
pub mod transport_param_id {
    /// `original_destination_connection_id`.
    pub const ORIGINAL_DESTINATION_CONNECTION_ID: u64 = 0x00;
    /// `max_idle_timeout`.
    pub const MAX_IDLE_TIMEOUT: u64 = 0x01;
    /// `stateless_reset_token`.
    pub const STATELESS_RESET_TOKEN: u64 = 0x02;
    /// `max_udp_payload_size`.
    pub const MAX_UDP_PAYLOAD_SIZE: u64 = 0x03;
    /// `initial_max_data`.
    pub const INITIAL_MAX_DATA: u64 = 0x04;
    /// `initial_max_stream_data_bidi_local`.
    pub const INITIAL_MAX_STREAM_DATA_BIDI_LOCAL: u64 = 0x05;
    /// `initial_max_stream_data_bidi_remote`.
    pub const INITIAL_MAX_STREAM_DATA_BIDI_REMOTE: u64 = 0x06;
    /// `initial_max_stream_data_uni`.
    pub const INITIAL_MAX_STREAM_DATA_UNI: u64 = 0x07;
    /// `initial_max_streams_bidi`.
    pub const INITIAL_MAX_STREAMS_BIDI: u64 = 0x08;
    /// `initial_max_streams_uni`.
    pub const INITIAL_MAX_STREAMS_UNI: u64 = 0x09;
    /// `ack_delay_exponent`.
    pub const ACK_DELAY_EXPONENT: u64 = 0x0a;
    /// `max_ack_delay`.
    pub const MAX_ACK_DELAY: u64 = 0x0b;
    /// `disable_active_migration`.
    pub const DISABLE_ACTIVE_MIGRATION: u64 = 0x0c;
    /// `preferred_address`.
    pub const PREFERRED_ADDRESS: u64 = 0x0d;
    /// `active_connection_id_limit`.
    pub const ACTIVE_CONNECTION_ID_LIMIT: u64 = 0x0e;
    /// `initial_source_connection_id`.
    pub const INITIAL_SOURCE_CONNECTION_ID: u64 = 0x0f;
    /// `retry_source_connection_id`.
    pub const RETRY_SOURCE_CONNECTION_ID: u64 = 0x10;
}

/// Why an HTTP/3 specification is invalid or cannot be applied.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum H3SpecError {
    /// A transport parameter identifier is malformed or invalid.
    #[error("invalid transport parameter id {0}")]
    InvalidTransportParameter(u64),
    /// A SETTINGS identifier is invalid.
    #[error("invalid SETTINGS id {0}")]
    InvalidSetting(u64),
    /// A SETTINGS identifier appears more than once.
    #[error("SETTINGS id {0} appears more than once")]
    DuplicateSetting(u64),
    /// The pseudo-header list is empty.
    #[error("pseudo-headers cannot be empty")]
    EmptyPseudoHeaders,
}

/// An entry in the QUIC transport parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportParam {
    /// The transport parameter identifier.
    pub id: u64,
    /// The raw payload bytes.
    pub value: Vec<u8>,
}

/// The HTTP/3 and QUIC specification a client reproduces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct H3Spec {
    /// The Chromium major version this specification represents.
    #[serde(default = "default_major")]
    pub major: u32,
    /// The operating system family.
    #[serde(default = "default_os")]
    pub os: OsType,
    /// QUIC transport parameters, in wire order.
    pub transport_parameters: Vec<TransportParam>,
    /// HTTP/3 SETTINGS frame entries (id, value), in wire order.
    pub settings: Vec<(u64, u64)>,
    /// Pseudo-header names, in wire order (e.g. `:method`, `:authority`, `:scheme`, `:path`).
    pub pseudo_headers: Vec<String>,
    /// Initial packet layout traits (e.g. 1200 or 1250 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_packet_size: Option<usize>,
}

impl H3Spec {
    /// Builds an HTTP/3 specification from its components.
    ///
    /// # Errors
    ///
    /// [`H3SpecError::EmptyPseudoHeaders`] if `pseudo_headers` is empty, or
    /// [`H3SpecError::DuplicateSetting`] if a SETTINGS id is duplicated.
    pub fn new(
        transport_parameters: Vec<TransportParam>,
        settings: Vec<(u64, u64)>,
        pseudo_headers: Vec<String>,
    ) -> Result<Self, H3SpecError> {
        Self::new_with_version(
            157,
            OsType::Macos,
            transport_parameters,
            settings,
            pseudo_headers,
        )
    }

    /// Builds an HTTP/3 specification with a specific Chromium major version and OS.
    ///
    /// # Errors
    ///
    /// [`H3SpecError::EmptyPseudoHeaders`] if `pseudo_headers` is empty, or
    /// [`H3SpecError::DuplicateSetting`] if a SETTINGS id is duplicated.
    pub fn new_with_version(
        major: u32,
        os: OsType,
        transport_parameters: Vec<TransportParam>,
        settings: Vec<(u64, u64)>,
        pseudo_headers: Vec<String>,
    ) -> Result<Self, H3SpecError> {
        if pseudo_headers.is_empty() {
            return Err(H3SpecError::EmptyPseudoHeaders);
        }
        let mut seen = std::collections::HashSet::new();
        for &(id, _) in &settings {
            if !seen.insert(id) {
                return Err(H3SpecError::DuplicateSetting(id));
            }
        }
        Ok(Self {
            major,
            os,
            transport_parameters,
            settings,
            pseudo_headers,
            initial_packet_size: None,
        })
    }

    /// Sets the expected initial packet size.
    #[must_use]
    pub fn with_initial_packet_size(mut self, size: usize) -> Self {
        self.initial_packet_size = Some(size);
        self
    }

    /// Constructs an HTTP/3 specification for the given Chromium major version and operating system.
    #[must_use]
    pub fn chrome(major: u32, os: OsType) -> Self {
        let settings = vec![
            (setting_id::QPACK_MAX_TABLE_CAPACITY, 0),
            (setting_id::MAX_FIELD_SECTION_SIZE, 262_144),
            (setting_id::QPACK_BLOCKED_STREAMS, 100),
            (setting_id::ENABLE_CONNECT_PROTOCOL, 1),
            (setting_id::H3_DATAGRAM, 1),
        ];

        let pseudo_headers = vec![
            ":method".to_owned(),
            ":authority".to_owned(),
            ":scheme".to_owned(),
            ":path".to_owned(),
        ];

        let transport_parameters = vec![
            TransportParam {
                id: transport_param_id::MAX_IDLE_TIMEOUT,
                value: 30_000u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::MAX_UDP_PAYLOAD_SIZE,
                value: 1472u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::INITIAL_MAX_DATA,
                value: 15_728_640u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::INITIAL_MAX_STREAM_DATA_BIDI_LOCAL,
                value: 6_291_456u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::INITIAL_MAX_STREAM_DATA_BIDI_REMOTE,
                value: 6_291_456u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::INITIAL_MAX_STREAM_DATA_UNI,
                value: 6_291_456u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::INITIAL_MAX_STREAMS_BIDI,
                value: 100u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::INITIAL_MAX_STREAMS_UNI,
                value: 103u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::ACK_DELAY_EXPONENT,
                value: 3u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::MAX_ACK_DELAY,
                value: 25u64.to_be_bytes().to_vec(),
            },
            TransportParam {
                id: transport_param_id::ACTIVE_CONNECTION_ID_LIMIT,
                value: 8u64.to_be_bytes().to_vec(),
            },
        ];

        Self {
            major,
            os,
            transport_parameters,
            settings,
            pseudo_headers,
            initial_packet_size: Some(1250),
        }
    }

    /// The baseline measured for Chrome 156 on macOS.
    #[must_use]
    pub fn chrome_156_macos() -> Self {
        Self::chrome(156, OsType::Macos)
    }

    /// The baseline measured for Chrome 157 on macOS.
    #[must_use]
    pub fn chrome_157_macos() -> Self {
        Self::chrome(157, OsType::Macos)
    }

    /// The HTTP/3 profile for the current stable Chromium major (Chrome 157).
    #[must_use]
    pub fn current_stable() -> Self {
        Self::chrome(157, OsType::Macos)
    }

    /// Looks up a transport parameter value by its identifier.
    #[must_use]
    pub fn transport_parameter(&self, id: u64) -> Option<&[u8]> {
        self.transport_parameters
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.value.as_slice())
    }

    /// Looks up a SETTINGS value by its identifier.
    #[must_use]
    pub fn setting(&self, id: u64) -> Option<u64> {
        self.settings
            .iter()
            .find(|(sid, _)| *sid == id)
            .map(|(_, val)| *val)
    }

    /// Returns the pseudo-header order.
    #[must_use]
    pub fn pseudo_headers(&self) -> &[String] {
        &self.pseudo_headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrome_156_spec_has_expected_shape() {
        let spec = H3Spec::chrome_156_macos();
        assert_eq!(spec.pseudo_headers().len(), 4);
        assert_eq!(spec.pseudo_headers()[0], ":method");
        assert_eq!(spec.pseudo_headers()[1], ":authority");
        assert_eq!(spec.pseudo_headers()[2], ":scheme");
        assert_eq!(spec.pseudo_headers()[3], ":path");

        assert_eq!(
            spec.setting(setting_id::MAX_FIELD_SECTION_SIZE),
            Some(262_144)
        );
        assert_eq!(spec.setting(setting_id::QPACK_BLOCKED_STREAMS), Some(100));
        assert!(
            spec.transport_parameter(transport_param_id::MAX_IDLE_TIMEOUT)
                .is_some()
        );
        assert_eq!(spec.initial_packet_size, Some(1250));
    }

    #[test]
    fn duplicate_settings_are_refused() {
        let err = H3Spec::new(
            Vec::new(),
            vec![(1, 100), (1, 200)],
            vec![":method".to_owned()],
        );
        assert_eq!(err, Err(H3SpecError::DuplicateSetting(1)));
    }

    #[test]
    fn empty_pseudo_headers_are_refused() {
        let err = H3Spec::new(Vec::new(), Vec::new(), Vec::new());
        assert_eq!(err, Err(H3SpecError::EmptyPseudoHeaders));
    }
}
