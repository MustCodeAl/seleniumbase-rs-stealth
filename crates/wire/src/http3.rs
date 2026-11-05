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

use std::net::IpAddr;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};

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

/// How to start the helper.
#[derive(Clone, Debug)]
#[must_use]
pub struct Http3Options {
    program: PathBuf,
    profile: Option<String>,
    root_cert_der: Option<PathBuf>,
    resolve: Vec<(String, IpAddr)>,
}

impl Default for Http3Options {
    fn default() -> Self {
        Self {
            program: std::env::var_os(HELPER_VAR)
                .map_or_else(|| PathBuf::from("sbrs-h3d"), PathBuf::from),
            profile: None,
            root_cert_der: None,
            resolve: Vec::new(),
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

    fn arguments(&self) -> Vec<String> {
        let mut arguments = Vec::new();
        if let Some(profile) = &self.profile {
            arguments.extend(["--profile".to_owned(), profile.clone()]);
        }
        if let Some(path) = &self.root_cert_der {
            arguments.extend(["--root-cert-der".to_owned(), path.display().to_string()]);
        }
        for (host, ip) in &self.resolve {
            arguments.extend(["--resolve".to_owned(), format!("{host}={ip}")]);
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
        }
    }

    /// A `GET` request for `url`.
    pub fn get(url: impl Into<String>) -> Self {
        Self::new("GET", url)
    }

    /// One more header. A header the profile also sends keeps the profile's place and takes
    /// this value; the others follow the profile's headers. Use lowercase names.
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// The request body; set its `content-type` with [`header`](Self::header).
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// How long to wait for the response (the helper's default is 30 seconds, its limit five
    /// minutes).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
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

    /// The response body.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
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
    _child: Child,
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
            _child: child,
            stdin,
            stdout: BufReader::new(stdout).lines(),
            next_id: 1,
        })
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
            .root_cert_der("/tmp/cert.der")
            .resolve("localhost", IpAddr::from([127, 0, 0, 1]));
        assert_eq!(
            options.arguments(),
            [
                "--profile",
                "chrome-154-macos",
                "--root-cert-der",
                "/tmp/cert.der",
                "--resolve",
                "localhost=127.0.0.1"
            ]
        );
    }
}
