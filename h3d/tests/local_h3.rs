//! `sbrs-h3d` against a local HTTP/3 server: quinn and h3 on 127.0.0.1 with a fresh self-signed
//! certificate. Nothing leaves the machine.

use std::net::{Ipv4Addr, SocketAddr};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use bytes::{Buf as _, Bytes};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout};
use tokio::sync::mpsc;

/// What the server saw of one request.
#[derive(Debug)]
struct Seen {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Seen {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(seen, _)| seen == name)
            .map(|(_, value)| value.as_str())
    }
}

/// A local HTTP/3 server that answers every request with `<METHOD> <path> over h3` and reports
/// what it saw.
struct Server {
    address: SocketAddr,
    seen: mpsc::UnboundedReceiver<Seen>,
    certificate: tempfile::NamedTempFile,
}

async fn handle(
    resolver: h3::server::RequestResolver<h3_quinn::Connection, Bytes>,
    seen: mpsc::UnboundedSender<Seen>,
) {
    let Ok((request, mut stream)) = resolver.resolve_request().await else {
        return;
    };
    let mut body = Vec::new();
    while let Ok(Some(mut chunk)) = stream.recv_data().await {
        body.extend_from_slice(&chunk.copy_to_bytes(chunk.remaining()));
    }
    let text = format!("{} {} over h3", request.method(), request.uri().path());
    let _ = seen.send(Seen {
        method: request.method().to_string(),
        path: request.uri().path().to_owned(),
        headers: request
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect(),
        body,
    });
    let response = http::Response::builder()
        .status(200)
        .header("x-served-by", "local-h3")
        .body(())
        .expect("a response head");
    if stream.send_response(response).await.is_ok()
        && stream.send_data(Bytes::from(text)).await.is_ok()
    {
        let _ = stream.finish().await;
    }
}

async fn serve(endpoint: quinn::Endpoint, seen: mpsc::UnboundedSender<Seen>) {
    while let Some(incoming) = endpoint.accept().await {
        let seen = seen.clone();
        tokio::spawn(async move {
            let Ok(connection) = incoming.await else {
                return;
            };
            let Ok(mut h3) =
                h3::server::Connection::<_, Bytes>::new(h3_quinn::Connection::new(connection))
                    .await
            else {
                return;
            };
            while let Ok(Some(resolver)) = h3.accept().await {
                tokio::spawn(handle(resolver, seen.clone()));
            }
        });
    }
}

fn start_server() -> Server {
    let rcgen::CertifiedKey { cert, key_pair } =
        rcgen::generate_simple_self_signed(vec!["localhost".to_owned()]).expect("a certificate");
    let der: CertificateDer<'static> = cert.der().clone();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key_pair.serialize_der()));
    let mut certificate = tempfile::NamedTempFile::new().expect("a temporary file");
    std::io::Write::write_all(&mut certificate, der.as_ref()).expect("the certificate is written");

    let mut tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![der], key)
        .expect("a TLS configuration");
    tls.alpn_protocols = vec![b"h3".to_vec()];
    let crypto =
        quinn::crypto::rustls::QuicServerConfig::try_from(tls).expect("a QUIC TLS configuration");
    let endpoint = quinn::Endpoint::server(
        quinn::ServerConfig::with_crypto(Arc::new(crypto)),
        SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
    )
    .expect("a UDP port");
    let address = endpoint.local_addr().expect("its address");
    let (sender, seen) = mpsc::unbounded_channel();
    tokio::spawn(serve(endpoint, sender));
    Server {
        address,
        seen,
        certificate,
    }
}

/// The helper, started the way the wire crate starts it.
struct Helper {
    _child: Child,
    stdin: ChildStdin,
    stdout: Lines<BufReader<ChildStdout>>,
}

impl Helper {
    fn start(certificate: &std::path::Path) -> Self {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_sbrs-h3d"))
            .arg("--root-cert-der")
            .arg(certificate)
            .args(["--resolve", "localhost=127.0.0.1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("sbrs-h3d starts");
        let stdin = child.stdin.take().expect("its stdin");
        let stdout = BufReader::new(child.stdout.take().expect("its stdout")).lines();
        Helper {
            _child: child,
            stdin,
            stdout,
        }
    }

    async fn ask(&mut self, line: &str) -> Value {
        self.stdin
            .write_all(line.as_bytes())
            .await
            .expect("a request is written");
        self.stdin
            .write_all(b"\n")
            .await
            .expect("a newline is written");
        self.stdin.flush().await.expect("stdin flushes");
        let answer = tokio::time::timeout(Duration::from_secs(30), self.stdout.next_line())
            .await
            .expect("an answer within 30 s")
            .expect("stdout reads")
            .expect("a line, not the end of the output");
        serde_json::from_str(&answer).expect("the answer is JSON")
    }
}

fn body_of(answer: &Value) -> String {
    let encoded = answer["body"].as_str().expect("a body");
    String::from_utf8(STANDARD.decode(encoded).expect("base64")).expect("UTF-8")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_get_goes_over_http3_with_the_chrome_profile_headers() {
    let mut server = start_server();
    let mut helper = Helper::start(server.certificate.path());
    let url = format!("https://localhost:{}/hello", server.address.port());

    let answer = helper
        .ask(&json!({ "id": 1, "url": url }).to_string())
        .await;

    assert_eq!(answer["id"], 1, "{answer}");
    assert_eq!(answer["status"], 200, "{answer}");
    assert_eq!(answer["protocol"], "h3", "{answer}");
    assert_eq!(body_of(&answer), "GET /hello over h3");
    let served_by = answer["headers"]
        .as_array()
        .expect("headers")
        .iter()
        .any(|pair| pair[0] == "x-served-by" && pair[1] == "local-h3");
    assert!(served_by, "{answer}");

    let seen = server
        .seen
        .recv()
        .await
        .expect("the server saw the request");
    assert_eq!(
        (seen.method.as_str(), seen.path.as_str()),
        ("GET", "/hello")
    );
    let agent = seen.header("user-agent").expect("a User-Agent");
    assert!(agent.contains("Chrome/"), "{agent}");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_post_carries_its_body_and_the_callers_headers() {
    let mut server = start_server();
    let mut helper = Helper::start(server.certificate.path());
    let url = format!("https://localhost:{}/form", server.address.port());
    let request = json!({
        "id": 2,
        "method": "POST",
        "url": url,
        "headers": [["content-type", "application/x-www-form-urlencoded"]],
        "body": STANDARD.encode("a=1&b=2"),
    });

    let answer = helper.ask(&request.to_string()).await;

    assert_eq!(
        (answer["status"].as_u64(), answer["protocol"].as_str()),
        (Some(200), Some("h3")),
        "{answer}"
    );
    assert_eq!(body_of(&answer), "POST /form over h3");
    let seen = server
        .seen
        .recv()
        .await
        .expect("the server saw the request");
    assert_eq!(seen.body, b"a=1&b=2");
    assert_eq!(
        seen.header("content-type"),
        Some("application/x-www-form-urlencoded")
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_that_never_answers_ends_in_an_error_within_the_time_limit() {
    let server = start_server();
    // A UDP port that is open but never answers: QUIC hears nothing back.
    let silent = std::net::UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).expect("a UDP port");
    let mut helper = Helper::start(server.certificate.path());
    let url = format!(
        "https://localhost:{}/",
        silent.local_addr().expect("its address").port()
    );

    let started = Instant::now();
    let answer = helper
        .ask(&json!({ "id": 3, "url": url, "timeout_ms": 1500 }).to_string())
        .await;

    assert_eq!(answer["id"], 3, "{answer}");
    let kind = answer["error"]["kind"].as_str().expect("an error");
    assert!(kind == "timeout" || kind == "send", "{answer}");
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_line_that_is_not_a_request_is_answered_and_the_next_one_still_works() {
    let server = start_server();
    let mut helper = Helper::start(server.certificate.path());

    let refused = helper.ask("not json").await;
    assert_eq!(refused["id"], Value::Null, "{refused}");
    assert_eq!(refused["error"]["kind"], "request", "{refused}");

    let url = format!("https://localhost:{}/again", server.address.port());
    let answer = helper
        .ask(&json!({ "id": 4, "url": url }).to_string())
        .await;
    assert_eq!(
        (answer["id"].as_u64(), answer["status"].as_u64()),
        (Some(4), Some(200)),
        "{answer}"
    );
}
