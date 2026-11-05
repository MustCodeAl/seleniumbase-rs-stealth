//! `sbrs-h3d`: HTTP/3 requests for the wire crate, sent by phantom-http with a Chrome profile.
//!
//! phantom builds its own `BoringSSL`, which cannot be linked into one binary with the wire
//! crate's (`docs/design/http3.md`), so the wire crate starts this program and talks to it over
//! stdin and stdout, one JSON object per line:
//!
//! ```text
//! sbrs-h3d [--profile NAME] [--root-cert-der PATH] [--resolve HOST=IP]...
//! ```
//!
//! A request line is `{"id": 1, "method": "GET", "url": "https://…", "headers": [["name",
//! "value"]], "body": "<base64>", "timeout_ms": 30000}` (only `id` and `url` are required). Its
//! answer is `{"id": 1, "status": 200, "protocol": "h3", "headers": [["name", "value"]], "body":
//! "<base64>"}`, or `{"id": 1, "error": {"kind": "…", "message": "…"}}`. Every request is sent
//! over HTTP/3 only, never falling back to HTTP/2 or HTTP/1.1. Requests are answered in order,
//! and the program ends when its input ends.
//!
//! `--root-cert-der` and `--resolve` exist for local test servers: an extra trusted certificate
//! and a fixed address for a host name.

use std::net::IpAddr;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use phantom::profile::catalog::{profile_for, RequestTemplateKind};
use phantom::{Client, HttpProtocol, RequestHeader, RequestTimeouts, ResponseInfo};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader};

/// The phantom profile used when none is named: the one the HTTP/3 spike compared with Chrome 156.
const DEFAULT_PROFILE: &str = "chrome-156-macos";

/// The largest response body read, in bytes; a longer one is an error.
const BODY_LIMIT: usize = 16 << 20;

/// The time limit of a request that names none.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// The longest a request may ask for.
const MAX_TIMEOUT: Duration = Duration::from_secs(300);

/// The command line, parsed.
#[derive(Debug)]
struct Options {
    profile: String,
    root_cert_der: Option<PathBuf>,
    resolve: Vec<(String, IpAddr)>,
}

/// One request line.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    id: u64,
    #[serde(default)]
    method: Option<String>,
    url: String,
    #[serde(default)]
    headers: Vec<(String, String)>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

/// One answer line.
#[derive(Debug, Serialize)]
#[serde(untagged)]
enum Answer {
    Response {
        id: u64,
        status: u16,
        protocol: String,
        headers: Vec<(String, String)>,
        body: String,
    },
    Failure {
        id: Option<u64>,
        error: Failure,
    },
}

/// Why a request got no response.
#[derive(Debug, Serialize)]
struct Failure {
    /// `request` (the line could not be read), `send` (phantom failed), `timeout` or `body`.
    kind: &'static str,
    message: String,
}

impl Answer {
    fn failure(id: Option<u64>, kind: &'static str, message: impl Into<String>) -> Self {
        Answer::Failure {
            id,
            error: Failure {
                kind,
                message: message.into(),
            },
        }
    }
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Options, String> {
    let mut options = Options {
        profile: DEFAULT_PROFILE.to_owned(),
        root_cert_der: None,
        resolve: Vec::new(),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => options.profile = args.next().ok_or("--profile needs a name")?,
            "--root-cert-der" => {
                options.root_cert_der = Some(PathBuf::from(
                    args.next().ok_or("--root-cert-der needs a path")?,
                ));
            }
            "--resolve" => {
                let spec = args.next().ok_or("--resolve needs HOST=IP")?;
                let (host, ip) = spec
                    .split_once('=')
                    .ok_or_else(|| format!("--resolve {spec}: write HOST=IP"))?;
                let ip: IpAddr = ip
                    .parse()
                    .map_err(|error| format!("--resolve {spec}: {error}"))?;
                options.resolve.push((host.to_owned(), ip));
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(options)
}

fn client(options: &Options) -> Result<Client, String> {
    let profile = profile_for(&options.profile, RequestTemplateKind::Navigation)
        .map_err(|error| format!("profile {}: {error}", options.profile))?;
    let mut builder =
        Client::builder(profile).request_timeouts(RequestTimeouts::new().total(MAX_TIMEOUT));
    if let Some(path) = &options.root_cert_der {
        let der = std::fs::read(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        builder = builder.add_root_certificate_der(der);
    }
    for (host, ip) in &options.resolve {
        builder = builder.resolve(host.as_str(), [*ip]);
    }
    builder.build().map_err(|error| error.to_string())
}

/// The error and its causes, one after the other.
fn chain(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

/// The protocol a response came over, as ALPN names it.
fn protocol_name(info: Option<&ResponseInfo>) -> String {
    match info.map(|info| format!("{:?}", info.protocol())) {
        Some(name) if name == "Http3" => "h3".to_owned(),
        Some(name) if name == "Http2" => "h2".to_owned(),
        Some(name) if name == "Http1" => "http/1.1".to_owned(),
        Some(name) => name.to_lowercase(),
        None => "unknown".to_owned(),
    }
}

async fn answer(client: &Client, request: Request) -> Answer {
    let id = Some(request.id);
    let method = request.method.as_deref().unwrap_or("GET");
    let Ok(method) = http::Method::from_bytes(method.as_bytes()) else {
        return Answer::failure(id, "request", format!("{method} is not an HTTP method"));
    };
    let body = match request
        .body
        .as_deref()
        .map(|body| STANDARD.decode(body))
        .transpose()
    {
        Ok(body) => body,
        Err(error) => {
            return Answer::failure(id, "request", format!("the body is not base64: {error}"))
        }
    };
    let mut builder = match client.request_with_protocol(HttpProtocol::Http3, method, &request.url)
    {
        Ok(builder) => builder,
        Err(error) => return Answer::failure(id, "request", chain(&error)),
    };
    for (name, value) in request.headers {
        builder = builder.header(RequestHeader::new(name, value));
    }
    if let Some(body) = body {
        builder = builder.body(body);
    }
    let limit = request
        .timeout_ms
        .map_or(DEFAULT_TIMEOUT, Duration::from_millis)
        .min(MAX_TIMEOUT);
    let response = match tokio::time::timeout(limit, builder.send()).await {
        Ok(Ok(response)) => response,
        Ok(Err(error)) => return Answer::failure(id, "send", chain(&error)),
        Err(_elapsed) => {
            return Answer::failure(
                id,
                "timeout",
                format!("no response within {} ms", limit.as_millis()),
            );
        }
    };
    let status = response.status().as_u16();
    let protocol = protocol_name(response.extensions().get::<ResponseInfo>());
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    match response.into_body().collect_with_limit(BODY_LIMIT).await {
        Ok(body) => Answer::Response {
            id: request.id,
            status,
            protocol,
            headers,
            body: STANDARD.encode(&body),
        },
        Err(error) => Answer::failure(id, "body", chain(&error)),
    }
}

async fn serve(client: Client) -> std::io::Result<()> {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }
        let answer = match serde_json::from_str::<Request>(&line) {
            Ok(request) => answer(&client, request).await,
            Err(error) => Answer::failure(None, "request", format!("not a request: {error}")),
        };
        let mut text = serde_json::to_string(&answer).map_err(std::io::Error::other)?;
        text.push('\n');
        stdout.write_all(text.as_bytes()).await?;
        stdout.flush().await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let options = match parse_args(std::env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("sbrs-h3d: {error}");
            return ExitCode::from(2);
        }
    };
    let client = match client(&options) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("sbrs-h3d: {error}");
            return ExitCode::from(2);
        }
    };
    match serve(client).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("sbrs-h3d: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Result<Options, String> {
        parse_args(list.iter().map(|arg| (*arg).to_owned()))
    }

    #[test]
    fn the_command_line_takes_a_profile_a_certificate_and_addresses() {
        let options = args(&[
            "--profile",
            "chrome-154-macos",
            "--root-cert-der",
            "/tmp/cert.der",
            "--resolve",
            "localhost=127.0.0.1",
        ])
        .expect("valid");
        assert_eq!(options.profile, "chrome-154-macos");
        assert_eq!(options.root_cert_der, Some(PathBuf::from("/tmp/cert.der")));
        assert_eq!(
            options.resolve,
            [("localhost".to_owned(), IpAddr::from([127, 0, 0, 1]))]
        );
        assert_eq!(args(&[]).expect("valid").profile, DEFAULT_PROFILE);
    }

    #[test]
    fn a_wrong_command_line_says_what_is_wrong() {
        assert!(args(&["--resolve", "localhost"]).is_err_and(|error| error.contains("HOST=IP")));
        assert!(args(&["--resolve", "localhost=nowhere"]).is_err());
        assert!(args(&["--profile"]).is_err());
        assert!(args(&["--nosuch"]).is_err_and(|error| error.contains("--nosuch")));
    }

    #[test]
    fn a_request_line_needs_an_id_and_a_url_and_nothing_unknown() {
        let request: Request =
            serde_json::from_str(r#"{"id":7,"url":"https://localhost/"}"#).expect("minimal");
        assert_eq!((request.id, request.method, request.body), (7, None, None));
        assert!(serde_json::from_str::<Request>(r#"{"url":"https://localhost/"}"#).is_err());
        assert!(serde_json::from_str::<Request>(r#"{"id":1,"url":"x","proxy":"y"}"#).is_err());
    }

    #[test]
    fn answers_are_one_json_object_each() {
        let failure = Answer::failure(Some(3), "timeout", "no response within 10 ms");
        assert_eq!(
            serde_json::to_value(&failure).expect("json"),
            serde_json::json!({ "id": 3, "error": { "kind": "timeout", "message": "no response within 10 ms" } })
        );
        let response = Answer::Response {
            id: 4,
            status: 204,
            protocol: "h3".to_owned(),
            headers: vec![("server".to_owned(), "local".to_owned())],
            body: String::new(),
        };
        assert_eq!(
            serde_json::to_value(&response).expect("json"),
            serde_json::json!({ "id": 4, "status": 204, "protocol": "h3", "headers": [["server", "local"]], "body": "" })
        );
    }
}
