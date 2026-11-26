//! `sbrs-h3d` process client, pool, options, and error types.

use std::net::IpAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::Mutex;

use crate::identity::fingerprint::{BrowserType, OsType};

use super::request::{Answer, Http3Request, Http3RequestKind, Http3Response, Line};

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
         `cargo install --path h3d` or name it in {HELPER_VAR}",
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
    default_headers: Vec<(String, String)>,
    default_timeout: Option<Duration>,
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
            default_headers: Vec::new(),
            default_timeout: None,
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
            BrowserType::Chromium => "chrome",
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
            OsType::Macos => "macos",
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

    /// Sets the profile to match a [`Fingerprint`](crate::identity::Fingerprint).
    ///
    /// When the fingerprint carries no version, uses the current stable Chromium major.
    pub fn for_fingerprint(mut self, fingerprint: &crate::identity::Fingerprint) -> Self {
        let major = fingerprint
            .core_version()
            .unwrap_or(crate::identity::randomize::CURRENT_STABLE_MAJOR);
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

    /// Adds a default header merged into every request sent by clients created from these options.
    pub fn default_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.default_headers.push((name.into(), value.into()));
        self
    }

    /// Adds multiple default headers merged into every request.
    pub fn default_headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.default_headers
            .extend(headers.into_iter().map(|(k, v)| (k.into(), v.into())));
        self
    }

    /// Sets a default `user-agent` header for every request.
    pub fn user_agent(self, ua: impl Into<String>) -> Self {
        self.default_header("user-agent", ua)
    }

    /// Sets a default per-request timeout applied when a request does not specify its own.
    pub fn default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = Some(timeout);
        self
    }

    /// Builds a reusable [`Http3Pool`] retaining up to `max_idle` helper processes for these options.
    #[must_use]
    pub fn pool(self, max_idle: usize) -> Http3Pool {
        Http3Pool::with_max_idle(self, max_idle)
    }

    pub(super) fn arguments(&self) -> Vec<String> {
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

/// A running `sbrs-h3d`. Requests are sent one at a time; the helper ends when this is dropped.
#[derive(Debug)]
pub struct Http3Client {
    child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
    next_id: u64,
    default_headers: Vec<(String, String)>,
    default_timeout: Option<Duration>,
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
            default_headers: options.default_headers.clone(),
            default_timeout: options.default_timeout,
        })
    }

    /// Sends a `GET` request for `url` using this client's defaults.
    ///
    /// # Errors
    ///
    /// See [`Self::send`].
    pub async fn get(&mut self, url: impl Into<String>) -> Result<Http3Response, Http3Error> {
        self.send(Http3Request::get(url)).await
    }

    /// Serializes `payload` as JSON and sends a `POST` request to `url`.
    ///
    /// # Errors
    ///
    /// Returns [`Http3Error::Reply`] on JSON serialization failure or any [`Self::send`] error.
    pub async fn post_json<T: serde::Serialize>(
        &mut self,
        url: impl Into<String>,
        payload: &T,
    ) -> Result<Http3Response, Http3Error> {
        let req = Http3Request::post(url)
            .json(payload)
            .map_err(|e| Http3Error::Reply(e.to_string()))?;
        self.send(req).await
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
        let mut merged_headers =
            Vec::with_capacity(self.default_headers.len() + request.headers.len());
        for (k, v) in &self.default_headers {
            if !request
                .headers
                .iter()
                .any(|(rk, _)| rk.eq_ignore_ascii_case(k))
            {
                merged_headers.push((k.clone(), v.clone()));
            }
        }
        merged_headers.extend(request.headers);
        let effective_timeout = request.timeout.or(self.default_timeout);
        let line = Line {
            id,
            method: &request.method,
            url: &request.url,
            headers: &merged_headers,
            body: request.body.as_deref().map(|body| STANDARD.encode(body)),
            timeout_ms: effective_timeout
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
    pub(super) clients: Arc<Mutex<Vec<Http3Client>>>,
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

    /// Sends a `GET` request for `url` using a pooled client.
    ///
    /// # Errors
    ///
    /// See [`Self::send`].
    pub async fn get(&self, url: impl Into<String>) -> Result<Http3Response, Http3Error> {
        self.send(Http3Request::get(url)).await
    }

    /// Serializes `payload` as JSON and sends a `POST` request to `url` using a pooled client.
    ///
    /// # Errors
    ///
    /// Returns [`Http3Error::Reply`] on JSON serialization failure or any [`Self::send`] error.
    pub async fn post_json<T: serde::Serialize>(
        &self,
        url: impl Into<String>,
        payload: &T,
    ) -> Result<Http3Response, Http3Error> {
        let req = Http3Request::post(url)
            .json(payload)
            .map_err(|e| Http3Error::Reply(e.to_string()))?;
        self.send(req).await
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
