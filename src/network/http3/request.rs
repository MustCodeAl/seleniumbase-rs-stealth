//! HTTP/3 request builder, response container, and request-kind selector.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use super::client::Http3Error;

/// Request header template category selected by `sbrs-h3d`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Http3RequestKind {
    /// Top-level document navigation (`--request-kind navigation`).
    #[default]
    Navigation,
    /// Subresource or CORS fetch (`--request-kind subresource`).
    Subresource,
}

impl Http3RequestKind {
    /// Returns the CLI identifier accepted by `sbrs-h3d --request-kind`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Navigation => "navigation",
            Self::Subresource => "subresource",
        }
    }
}

impl fmt::Display for Http3RequestKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Http3RequestKind {
    type Err = Http3Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "navigation" => Ok(Self::Navigation),
            "subresource" => Ok(Self::Subresource),
            other => Err(Http3Error::Reply(format!(
                "unknown HTTP/3 request kind: {other}"
            ))),
        }
    }
}

/// One HTTP/3 request.
#[derive(Clone, Debug)]
#[must_use]
pub struct Http3Request {
    pub(super) method: String,
    pub(super) url: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Option<Vec<u8>>,
    pub(super) timeout: Option<Duration>,
    pub(super) proxy: Option<String>,
}

impl Http3Request {
    /// A request with `method` (such as `POST`) for `url`.
    pub fn new(method: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            url: url.into(),
            headers: Vec::new(),
            body: None,
            timeout: None,
            proxy: None,
        }
    }

    /// A `GET` request for `url`.
    pub fn get(url: impl Into<String>) -> Self {
        Self::new("GET", url)
    }

    /// A `POST` request for `url`.
    pub fn post(url: impl Into<String>) -> Self {
        Self::new("POST", url)
    }

    /// A `PUT` request for `url`.
    pub fn put(url: impl Into<String>) -> Self {
        Self::new("PUT", url)
    }

    /// A `DELETE` request for `url`.
    pub fn delete(url: impl Into<String>) -> Self {
        Self::new("DELETE", url)
    }

    /// A `PATCH` request for `url`.
    pub fn patch(url: impl Into<String>) -> Self {
        Self::new("PATCH", url)
    }

    /// A `HEAD` request for `url`.
    pub fn head(url: impl Into<String>) -> Self {
        Self::new("HEAD", url)
    }

    /// One more header. A header the profile also sends keeps the profile's place and takes
    /// this value; the others follow the profile's headers. Use lowercase names.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Appends multiple request headers.
    pub fn headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.headers
            .extend(headers.into_iter().map(|(k, v)| (k.into(), v.into())));
        self
    }

    /// Sets an `authorization: Bearer <token>` header.
    pub fn bearer_auth(self, token: impl AsRef<str>) -> Self {
        self.header("authorization", format!("Bearer {}", token.as_ref()))
    }

    /// Sets an `authorization: Basic <base64>` header from username and password.
    pub fn basic_auth(self, username: impl AsRef<str>, password: impl AsRef<str>) -> Self {
        let raw = format!("{}:{}", username.as_ref(), password.as_ref());
        self.header("authorization", format!("Basic {}", STANDARD.encode(raw)))
    }

    /// Sets the `referer` request header.
    pub fn referer(self, referer: impl Into<String>) -> Self {
        self.header("referer", referer)
    }

    /// Sets the `origin` request header.
    pub fn origin(self, origin: impl Into<String>) -> Self {
        self.header("origin", origin)
    }

    /// Sets the `accept-language` request header.
    pub fn accept_language(self, value: impl Into<String>) -> Self {
        self.header("accept-language", value)
    }

    /// Sets the `cookie` request header.
    pub fn cookie(self, cookie: impl Into<String>) -> Self {
        self.header("cookie", cookie)
    }

    /// Sets a UTF-8 plain-text body and `content-type: text/plain; charset=utf-8`.
    pub fn text_body(mut self, text: impl Into<String>) -> Self {
        self.body = Some(text.into().into_bytes());
        self.headers
            .retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
        self.headers.push((
            "content-type".to_owned(),
            "text/plain; charset=utf-8".to_owned(),
        ));
        self
    }

    /// Appends URL-encoded query parameters to the request URL.
    pub fn query<I, K, V>(mut self, params: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let encoded = encode_form_pairs(params);
        if !encoded.is_empty() {
            let sep = if self.url.contains('?') { '&' } else { '?' };
            self.url.push(sep);
            self.url.push_str(&encoded);
        }
        self
    }

    /// Sets an `application/x-www-form-urlencoded` body and matching `content-type` header.
    pub fn form<I, K, V>(mut self, params: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let encoded = encode_form_pairs(params);
        self.body = Some(encoded.into_bytes());
        self.headers
            .retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
        self.headers.push((
            "content-type".to_owned(),
            "application/x-www-form-urlencoded".to_owned(),
        ));
        self
    }

    /// The request body; set its `content-type` with [`header`](Self::header).
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Serializes `value` as JSON body and sets `content-type: application/json`.
    ///
    /// # Errors
    ///
    /// Returns [`serde_json::Error`] if `value` cannot be serialized as JSON.
    pub fn json<T: serde::Serialize>(mut self, value: &T) -> Result<Self, serde_json::Error> {
        let bytes = serde_json::to_vec(value)?;
        self.body = Some(bytes);
        self.headers
            .retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
        self.headers
            .push(("content-type".to_owned(), "application/json".to_owned()));
        Ok(self)
    }

    /// How long to wait for the response (the helper's default is 30 seconds, its limit five
    /// minutes).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// An optional per-request proxy override.
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }
}

/// The response to an [`Http3Request`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Http3Response {
    pub(super) status: u16,
    pub(super) protocol: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Vec<u8>,
}

impl Http3Response {
    /// The status code.
    #[must_use]
    pub fn status(&self) -> u16 {
        self.status
    }

    /// The protocol the response came over, as ALPN names it: `h3`.
    #[must_use]
    pub fn protocol(&self) -> &str {
        &self.protocol
    }

    /// The response headers, in the order they came.
    #[must_use]
    pub fn headers(&self) -> &[(String, String)] {
        &self.headers
    }

    /// The first value of the header `name`, compared without case.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// All values of the header `name`, compared without case.
    #[must_use]
    pub fn headers_all(&self, name: &str) -> Vec<&str> {
        self.headers
            .iter()
            .filter(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .collect()
    }

    /// All `Set-Cookie` header values from the response.
    #[must_use]
    pub fn cookies(&self) -> Vec<&str> {
        self.headers_all("set-cookie")
    }

    /// The response `Content-Type` header value, if present.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.header("content-type")
    }

    /// Returns `true` if the status code is in `200..=299`.
    #[must_use]
    pub fn is_success(&self) -> bool {
        (200..=299).contains(&self.status)
    }

    /// Returns `true` if the status code is in `300..=399`.
    #[must_use]
    pub fn is_redirect(&self) -> bool {
        (300..=399).contains(&self.status)
    }

    /// Returns `true` if the status code is in `400..=499`.
    #[must_use]
    pub fn is_client_error(&self) -> bool {
        (400..=499).contains(&self.status)
    }

    /// Returns `true` if the status code is in `500..=599`.
    #[must_use]
    pub fn is_server_error(&self) -> bool {
        (500..=599).contains(&self.status)
    }

    /// The response body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// Decodes the response body as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns [`std::string::FromUtf8Error`] if the body is not valid UTF-8.
    pub fn text(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.body.clone())
    }

    /// Decodes the response body lossily as a UTF-8 string.
    #[must_use]
    pub fn text_lossy(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    /// Deserializes the response body as JSON.
    ///
    /// # Errors
    ///
    /// Returns [`serde_json::Error`] if the body is not valid JSON for `T`.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }
}

/// Percent-encodes `key=value` pairs for query strings and `application/x-www-form-urlencoded` bodies.
fn encode_form_pairs<I, K, V>(params: I) -> String
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    fn pct(input: &str, out: &mut String) {
        use std::fmt::Write as _;
        for b in input.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(char::from(b));
                }
                b' ' => out.push('+'),
                other => {
                    let _ = write!(out, "%{other:02X}");
                }
            }
        }
    }
    let mut out = String::new();
    for (idx, (k, v)) in params.into_iter().enumerate() {
        if idx > 0 {
            out.push('&');
        }
        pct(k.as_ref(), &mut out);
        out.push('=');
        pct(v.as_ref(), &mut out);
    }
    out
}

/// A request line, as `sbrs-h3d` reads it.
#[derive(Debug, Serialize)]
pub(super) struct Line<'a> {
    pub(super) id: u64,
    pub(super) method: &'a str,
    pub(super) url: &'a str,
    pub(super) headers: &'a [(String, String)],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) proxy: Option<&'a str>,
}

/// An answer line, as `sbrs-h3d` writes it.
#[derive(Debug, Deserialize)]
pub(super) struct Answer {
    pub(super) id: Option<u64>,
    pub(super) status: Option<u16>,
    pub(super) protocol: Option<String>,
    #[serde(default)]
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Option<String>,
    pub(super) error: Option<Failure>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Failure {
    pub(super) kind: String,
    pub(super) message: String,
}
