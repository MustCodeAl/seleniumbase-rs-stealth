//! HTTP/3 through `sbrs-h3d`, a helper program (feature `http3`, off by default).
//!
//! This crate's own client speaks HTTP/1.1 and HTTP/2 through wreq and `BoringSSL`. HTTP/3 is sent
//! by phantom-http, which brings a second `BoringSSL` that cannot be linked into the same binary
//! (`docs/design/http3.md`). So it runs in a process of its own, `sbrs-h3d` (the package in
//! `crates/h3d`, installed with `cargo install --path crates/h3d`), and [`Http3Client`] talks to
//! it over stdin and stdout, one JSON object per line. What its requests share with Chrome's and
//! what they do not is in `docs/http3.md`.
//!
//! Every request goes over HTTP/3 only; nothing falls back to HTTP/2. A program that wants a
//! fallback sends the request again through [`HttpSession`](crate::HttpSession).

use std::fmt;
use std::net::IpAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use seleniumbase_rs::evasion::fingerprint::{BrowserType, OsType};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;

/// The environment variable that names the helper program; without it, `sbrs-h3d` is looked up
/// on the `PATH`.
pub const HELPER_VAR: &str = "SBRS_H3D";

/// Why an HTTP/3 request got no response.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Http3Error {
    /// The helper program could not be started.
    #[error(
        "the HTTP/3 helper {} could not be started ({source}); install it with \
         `cargo install --path crates/h3d` or name it in {HELPER_VAR}",
        program.display()
    )]
    HelperMissing {
        /// The program that was tried.
        program: PathBuf,
        /// What starting it said.
        source: std::io::Error,
    },
    /// The helper ended, so it can take no more requests.
    #[error("the HTTP/3 helper ended")]
    HelperEnded,
    /// Writing to or reading from the helper failed.
    #[error("talking to the HTTP/3 helper failed: {0}")]
    Io(#[from] std::io::Error),
    /// The helper answered something that is not an answer to this request.
    #[error("the HTTP/3 helper answered something unexpected: {0}")]
    Reply(String),
    /// The helper could not send the request, or got no response.
    #[error("HTTP/3 request failed ({kind}): {message}")]
    Failed {
        /// `request`, `send`, `timeout` or `body`, as the helper names it.
        kind: String,
        /// The helper's message.
        message: String,
    },
}

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

/// How to start the helper.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[must_use]
pub struct Http3Options {
    program: PathBuf,
    profile: Option<String>,
    request_kind: Option<Http3RequestKind>,
    root_cert_der: Option<PathBuf>,
    resolve: Vec<(String, IpAddr)>,
    proxy: Option<String>,
}

impl Default for Http3Options {
    fn default() -> Self {
        Self {
            program: std::env::var_os(HELPER_VAR)
                .map_or_else(|| PathBuf::from("sbrs-h3d"), PathBuf::from),
            profile: None,
            request_kind: None,
            root_cert_der: None,
            resolve: Vec::new(),
            proxy: None,
        }
    }
}

impl Http3Options {
    /// The helper named by [`HELPER_VAR`], else `sbrs-h3d` on the `PATH`, with its default profile.
    pub fn new() -> Self {
        Self::default()
    }

    /// The helper program to start.
    pub fn program(mut self, program: impl Into<PathBuf>) -> Self {
        self.program = program.into();
        self
    }

    /// The phantom profile the helper sends with, such as `chrome-156-macos` (its default).
    pub fn profile(mut self, name: impl Into<String>) -> Self {
        self.profile = Some(name.into());
        self
    }

    /// Sets the profile to match the specified browser family, major version, and operating system.
    ///
    /// `sbrs-h3d` resolves the generated selector to the exact `phantom-profile` catalog entry when
    /// available or the closest matching browser family and OS entry.
    pub fn for_browser(mut self, browser: BrowserType, major: u32, os: OsType) -> Self {
        let family = match browser {
            BrowserType::Firefox => "firefox",
            BrowserType::MobileSafari => "safari",
            BrowserType::Chromium | _ => "chrome",
        };
        let os_slug = match os {
            OsType::Windows => "windows",
            OsType::Linux => "linux",
            OsType::Android => "android",
            OsType::Ios => {
                if browser == BrowserType::MobileSafari {
                    "ios"
                } else {
                    "macos"
                }
            }
            OsType::Macos | _ => "macos",
        };
        self.profile = Some(format!("{family}-{major}-{os_slug}"));
        self
    }

    /// Sets the profile to match the specified Chromium major and operating system.
    ///
    /// Uses the requested Chromium major and OS. Callers that need a specific
    /// profile string should use [`Self::profile`].
    pub fn for_version(self, major: u32, os: OsType) -> Self {
        self.for_browser(BrowserType::Chromium, major, os)
    }

    /// Sets the profile to match a [`Fingerprint`](seleniumbase_rs::Fingerprint).
    ///
    /// When the fingerprint carries no version, uses the current stable Chromium major.
    pub fn for_fingerprint(mut self, fingerprint: &seleniumbase_rs::Fingerprint) -> Self {
        let major = fingerprint
            .core_version()
            .unwrap_or(seleniumbase_rs::evasion::randomize::CURRENT_STABLE_MAJOR);
        let os = fingerprint.os_type();
        let ua = fingerprint.user_agent().value().map_or("", String::as_str);
        if ua.contains("Edg/") {
            let slug = match os {
                OsType::Android => "android",
                _ => "windows",
            };
            self.profile = Some(format!("edge-{major}-{slug}"));
            return self;
        }
        if ua.contains("OPR/") {
            let slug = match os {
                OsType::Android => "android",
                _ => "windows",
            };
            self.profile = Some(format!("opera-{major}-{slug}"));
            return self;
        }
        self.for_browser(fingerprint.browser_type(), major, os)
    }

    /// Sets the request header template category (`navigation` or `subresource`).
    pub fn request_kind(mut self, kind: Http3RequestKind) -> Self {
        self.request_kind = Some(kind);
        self
    }

    /// One more certificate to trust, as a DER file: for a local test server.
    pub fn root_cert_der(mut self, path: impl Into<PathBuf>) -> Self {
        self.root_cert_der = Some(path.into());
        self
    }

    /// A fixed address for `host`: for a local test server.
    pub fn resolve(mut self, host: impl Into<String>, ip: IpAddr) -> Self {
        self.resolve.push((host.into(), ip));
        self
    }

    /// A proxy to route requests through (SOCKS5 UDP relay or CONNECT-UDP / MASQUE).
    pub fn proxy(mut self, proxy: impl Into<String>) -> Self {
        self.proxy = Some(proxy.into());
        self
    }

    /// Routes QUIC datagrams through a SOCKS5 UDP relay (`socks5://` or `socks5h://`).
    ///
    /// Bare `host:port` values default to `socks5h://` so target hostnames resolve at the proxy.
    pub fn socks5(mut self, proxy: impl Into<String>) -> Self {
        let raw = proxy.into();
        let normalized = if raw.contains("://") {
            raw
        } else {
            format!("socks5h://{raw}")
        };
        self.proxy = Some(normalized);
        self
    }

    /// Routes QUIC datagrams through an RFC 9298 CONNECT-UDP (MASQUE) endpoint or URI template.
    ///
    /// Bare `host:port` values default to `connect-udp://host:port`.
    pub fn connect_udp(mut self, template: impl Into<String>) -> Self {
        let raw = template.into();
        let normalized = if raw.contains("://") {
            raw
        } else {
            format!("connect-udp://{raw}")
        };
        self.proxy = Some(normalized);
        self
    }

    /// Builds a reusable [`Http3Pool`] retaining up to `max_idle` helper processes for these options.
    #[must_use]
    pub fn pool(self, max_idle: usize) -> Http3Pool {
        Http3Pool::with_max_idle(self, max_idle)
    }

    fn arguments(&self) -> Vec<String> {
        let mut arguments = Vec::new();
        if let Some(profile) = &self.profile {
            arguments.extend(["--profile".to_owned(), profile.clone()]);
        }
        if let Some(kind) = self.request_kind {
            arguments.extend(["--request-kind".to_owned(), kind.as_str().to_owned()]);
        }
        if let Some(path) = &self.root_cert_der {
            arguments.extend(["--root-cert-der".to_owned(), path.display().to_string()]);
        }
        for (host, ip) in &self.resolve {
            arguments.extend(["--resolve".to_owned(), format!("{host}={ip}")]);
        }
        if let Some(proxy) = &self.proxy {
            arguments.extend(["--proxy".to_owned(), proxy.clone()]);
        }
        arguments
    }
}

/// One HTTP/3 request.
#[derive(Clone, Debug)]
#[must_use]
pub struct Http3Request {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<Vec<u8>>,
    timeout: Option<Duration>,
    proxy: Option<String>,
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
    status: u16,
    protocol: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
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
struct Line<'a> {
    id: u64,
    method: &'a str,
    url: &'a str,
    headers: &'a [(String, String)],
    #[serde(skip_serializing_if = "Option::is_none")]
    body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    proxy: Option<&'a str>,
}

/// An answer line, as `sbrs-h3d` writes it.
#[derive(Debug, Deserialize)]
struct Answer {
    id: Option<u64>,
    status: Option<u16>,
    protocol: Option<String>,
    #[serde(default)]
    headers: Vec<(String, String)>,
    body: Option<String>,
    error: Option<Failure>,
}

#[derive(Debug, Deserialize)]
struct Failure {
    kind: String,
    message: String,
}

/// A running `sbrs-h3d`. Requests are sent one at a time; the helper ends when this is dropped.
#[derive(Debug)]
pub struct Http3Client {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl Http3Client {
    /// Starts the helper.
    ///
    /// # Errors
    ///
    /// [`Http3Error::HelperMissing`] when the program cannot be started.
    pub fn start(options: &Http3Options) -> Result<Self, Http3Error> {
        let missing = |source| Http3Error::HelperMissing {
            program: options.program.clone(),
            source,
        };
        let mut child = tokio::process::Command::new(&options.program)
            .args(options.arguments())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(missing)?;
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            return Err(missing(std::io::Error::other("no stdin or stdout")));
        };
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_id: 1,
        })
    }

    /// Whether the helper process is still running.
    ///
    /// A helper whose exit status cannot be read counts as ended.
    pub fn is_alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Sends `request` over HTTP/3 and waits for its response.
    ///
    /// # Errors
    ///
    /// [`Http3Error::Failed`] when the helper could not send it or got no response,
    /// [`Http3Error::HelperEnded`] when the helper is gone, [`Http3Error::Reply`] for an answer
    /// that does not belong to this request.
    pub async fn send(&mut self, request: Http3Request) -> Result<Http3Response, Http3Error> {
        let id = self.next_id;
        self.next_id += 1;
        let line = Line {
            id,
            method: &request.method,
            url: &request.url,
            headers: &request.headers,
            body: request.body.as_deref().map(|body| STANDARD.encode(body)),
            timeout_ms: request
                .timeout
                .map(|timeout| u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX)),
            proxy: request.proxy.as_deref(),
        };
        let mut text =
            serde_json::to_string(&line).map_err(|error| Http3Error::Reply(error.to_string()))?;
        text.push('\n');
        if let Err(error) = self.write(text.as_bytes()).await {
            return Err(ended_or(error));
        }
        let Some(reply) = self.stdout.next_line().await.map_err(ended_or)? else {
            return Err(Http3Error::HelperEnded);
        };
        let answer: Answer = serde_json::from_str(&reply)
            .map_err(|error| Http3Error::Reply(format!("{error}: {reply}")))?;
        if answer.id != Some(id) {
            return Err(Http3Error::Reply(format!(
                "an answer for {:?}, not {id}",
                answer.id
            )));
        }
        if let Some(failure) = answer.error {
            return Err(Http3Error::Failed {
                kind: failure.kind,
                message: failure.message,
            });
        }
        let (Some(status), Some(protocol), Some(body)) =
            (answer.status, answer.protocol, answer.body)
        else {
            return Err(Http3Error::Reply(format!(
                "no status, protocol or body: {reply}"
            )));
        };
        let body = STANDARD
            .decode(body)
            .map_err(|error| Http3Error::Reply(format!("the body is not base64: {error}")))?;
        Ok(Http3Response {
            status,
            protocol,
            headers: answer.headers,
            body,
        })
    }

    async fn write(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.stdin.write_all(bytes).await?;
        self.stdin.flush().await
    }
}

/// A pool of reusable [`Http3Client`] workers.
///
/// Keeps active `Http3Client` process instances to avoid the latency of repeated process spawns.
#[derive(Clone, Debug)]
pub struct Http3Pool {
    options: Http3Options,
    clients: Arc<Mutex<Vec<Http3Client>>>,
    max_idle: usize,
}

impl Http3Pool {
    /// Creates a new pool for the given `options` with a default capacity of 4 idle clients.
    #[must_use]
    pub fn new(options: Http3Options) -> Self {
        Self::with_max_idle(options, 4)
    }

    /// Creates a new pool with the specified idle client capacity.
    #[must_use]
    pub fn with_max_idle(options: Http3Options, max_idle: usize) -> Self {
        Self {
            options,
            clients: Arc::new(Mutex::new(Vec::new())),
            max_idle,
        }
    }

    /// The options every helper of this pool is started with.
    #[must_use]
    pub fn options(&self) -> &Http3Options {
        &self.options
    }

    /// How many idle helpers the pool keeps at most.
    #[must_use]
    pub fn max_idle(&self) -> usize {
        self.max_idle
    }

    /// Acquires an idle client from the pool or starts a new one.
    ///
    /// An idle helper that has exited since it was released is dropped, never returned.
    ///
    /// # Errors
    ///
    /// [`Http3Error::HelperMissing`] when a new helper cannot be started.
    pub async fn acquire(&self) -> Result<Http3Client, Http3Error> {
        let mut guard = self.clients.lock().await;
        while let Some(mut client) = guard.pop() {
            if client.is_alive() {
                return Ok(client);
            }
        }
        drop(guard);
        Http3Client::start(&self.options)
    }

    /// Releases a client back to the pool for reuse if it is alive and the pool is under
    /// `max_idle`.
    pub async fn release(&self, mut client: Http3Client) {
        if !client.is_alive() {
            return;
        }
        let mut guard = self.clients.lock().await;
        if guard.len() < self.max_idle {
            guard.push(client);
        }
    }

    /// Sends a request using a pooled client, returning the client to the pool upon completion.
    ///
    /// # Errors
    ///
    /// See [`Http3Client::send`]; a failed send drops its helper instead of pooling it.
    pub async fn send(&self, request: Http3Request) -> Result<Http3Response, Http3Error> {
        let mut client = self.acquire().await?;
        let res = client.send(request).await;
        if res.is_ok() {
            self.release(client).await;
        }
        res
    }
}

/// A broken pipe means the helper ended.
fn ended_or(error: std::io::Error) -> Http3Error {
    if error.kind() == std::io::ErrorKind::BrokenPipe {
        Http3Error::HelperEnded
    } else {
        Http3Error::Io(error)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fmt::Write as _;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::Path;

    use super::*;

    /// A stand-in for `sbrs-h3d`: a shell script that writes each request line it reads to
    /// `requests.txt` and answers with the lines given, in turn.
    fn fake_helper(dir: &Path, answers: &[&str]) -> PathBuf {
        let mut script = String::from("#!/bin/sh\n");
        for answer in answers {
            script.push_str("IFS= read -r line || exit 0\n");
            writeln!(
                script,
                "printf '%s\\n' \"$line\" >> '{}'",
                dir.join("requests.txt").display()
            )
            .expect("a String takes any text");
            writeln!(script, "printf '%s\\n' '{answer}'").expect("a String takes any text");
        }
        let path = dir.join("fake-h3d");
        std::fs::write(&path, script).expect("the script is written");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("it can run");
        path
    }

    #[tokio::test]
    async fn a_missing_helper_says_how_to_install_it() {
        let error = Http3Client::start(&Http3Options::new().program("/nonexistent/sbrs-h3d"))
            .expect_err("missing");
        assert!(
            matches!(error, Http3Error::HelperMissing { .. }),
            "{error:?}"
        );
        assert!(
            error
                .to_string()
                .contains("cargo install --path crates/h3d"),
            "{error}"
        );
    }

    #[tokio::test]
    async fn a_request_is_one_line_and_its_answer_becomes_a_response() {
        let dir = tempfile::tempdir().expect("a folder");
        let program = fake_helper(
            dir.path(),
            &[
                r#"{"id":1,"status":201,"protocol":"h3","headers":[["X-Served-By","fake"]],"body":"aGk="}"#,
                r#"{"id":2,"error":{"kind":"send","message":"no route"}}"#,
            ],
        );
        let mut client =
            Http3Client::start(&Http3Options::new().program(&program)).expect("it starts");

        let request = Http3Request::new("POST", "https://localhost:4433/form")
            .header("content-type", "text/plain")
            .body("a=1")
            .timeout(Duration::from_millis(1500));
        let response = client.send(request).await.expect("a response");
        assert_eq!(
            (response.status(), response.protocol(), response.body()),
            (201, "h3", &b"hi"[..])
        );
        assert_eq!(response.header("x-served-by"), Some("fake"));

        let error = client
            .send(Http3Request::get("https://localhost:4433/"))
            .await
            .expect_err("a failure");
        assert!(
            matches!(&error, Http3Error::Failed { kind, .. } if kind == "send"),
            "{error:?}"
        );

        let written =
            std::fs::read_to_string(dir.path().join("requests.txt")).expect("the requests");
        let lines: Vec<serde_json::Value> = written
            .lines()
            .map(|line| serde_json::from_str(line).expect("JSON"))
            .collect();
        assert_eq!(
            lines[0],
            serde_json::json!({
                "id": 1, "method": "POST", "url": "https://localhost:4433/form",
                "headers": [["content-type", "text/plain"]], "body": "YT0x", "timeout_ms": 1500,
            })
        );
        assert_eq!(
            lines[1],
            serde_json::json!({ "id": 2, "method": "GET", "url": "https://localhost:4433/", "headers": [] })
        );
    }

    #[tokio::test]
    async fn an_answer_for_another_request_is_refused() {
        let dir = tempfile::tempdir().expect("a folder");
        let program = fake_helper(
            dir.path(),
            &[r#"{"id":9,"status":200,"protocol":"h3","body":""}"#],
        );
        let mut client =
            Http3Client::start(&Http3Options::new().program(&program)).expect("it starts");
        let error = client
            .send(Http3Request::get("https://localhost/"))
            .await
            .expect_err("refused");
        assert!(matches!(error, Http3Error::Reply(_)), "{error:?}");
    }

    #[tokio::test]
    async fn a_helper_that_ended_is_reported_as_ended() {
        let dir = tempfile::tempdir().expect("a folder");
        let program = fake_helper(dir.path(), &[]);
        let mut client =
            Http3Client::start(&Http3Options::new().program(&program)).expect("it starts");
        let error = client
            .send(Http3Request::get("https://localhost/"))
            .await
            .expect_err("ended");
        assert!(matches!(error, Http3Error::HelperEnded), "{error:?}");
    }

    #[test]
    fn the_options_become_the_helpers_command_line() {
        let options = Http3Options::new()
            .profile("chrome-154-macos")
            .request_kind(Http3RequestKind::Subresource)
            .root_cert_der("/tmp/cert.der")
            .resolve("localhost", IpAddr::from([127, 0, 0, 1]))
            .proxy("socks5://127.0.0.1:1080");
        assert_eq!(
            options.arguments(),
            [
                "--profile",
                "chrome-154-macos",
                "--request-kind",
                "subresource",
                "--root-cert-der",
                "/tmp/cert.der",
                "--resolve",
                "localhost=127.0.0.1",
                "--proxy",
                "socks5://127.0.0.1:1080",
            ]
        );
    }

    #[test]
    fn options_build_browser_selectors_and_proxy_shortcuts() {
        let chrome = Http3Options::new().for_browser(BrowserType::Chromium, 157, OsType::Windows);
        assert!(chrome
            .arguments()
            .contains(&"chrome-157-windows".to_owned()));

        let firefox = Http3Options::new().for_browser(BrowserType::Firefox, 157, OsType::Windows);
        assert!(firefox
            .arguments()
            .contains(&"firefox-157-windows".to_owned()));

        let safari = Http3Options::new().for_browser(BrowserType::MobileSafari, 26, OsType::Ios);
        assert!(safari.arguments().contains(&"safari-26-ios".to_owned()));

        let socks = Http3Options::new().socks5("127.0.0.1:1080");
        assert!(socks
            .arguments()
            .contains(&"socks5h://127.0.0.1:1080".to_owned()));

        let masque = Http3Options::new().connect_udp("masque.example:443");
        assert!(masque
            .arguments()
            .contains(&"connect-udp://masque.example:443".to_owned()));
    }

    #[tokio::test]
    async fn pool_drops_exited_helper_before_returning_it() {
        let dir = tempfile::tempdir().expect("a folder");
        // First helper answers one request and exits immediately; second answers request 2.
        let program = fake_helper(
            dir.path(),
            &[r#"{"id":1,"status":200,"protocol":"h3","headers":[],"body":""}"#],
        );
        let pool = Http3Options::new().program(&program).pool(2);
        let mut client = pool.acquire().await.expect("starts");
        let resp = client
            .send(Http3Request::get("https://localhost/one"))
            .await
            .expect("first ok");
        assert_eq!(resp.status(), 200);
        // Wait briefly for the fake shell script to exit.
        tokio::time::sleep(Duration::from_millis(50)).await;
        pool.release(client).await;
        assert_eq!(pool.clients.lock().await.len(), 0);
    }
}
