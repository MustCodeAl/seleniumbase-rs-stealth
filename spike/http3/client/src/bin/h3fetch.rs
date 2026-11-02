//! Question (b): fetch one page from the local `h3srv` with phantom over an exact protocol, and
//! say which protocol the response really came over.
//!
//! The proof of HTTP/3 is not this program's word alone: `h3srv` logs a `quic_connection`
//! event (ALPN `h3`, SNI, a QUIC handshake) and the request's HEADERS frame, and its reply body
//! names the transport it saw. This program prints what phantom reports (`ResponseInfo`) and the
//! body, so the three can be compared.
//!
//! ```text
//! h3fetch <port> <cert.der> <h1|h2|h3> [--profile NAME] [--path /x] [--qlog DIR]
//! ```
//!
//! Every request goes to `localhost` / 127.0.0.1 and nowhere else.

use std::net::{IpAddr, Ipv4Addr};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use phantom::profile::catalog::{profile_for, RequestTemplateKind};
use phantom::{Client, HttpProtocol, RequestTimeouts, ResponseInfo};

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const BODY_LIMIT: usize = 1 << 20;

/// The command line, parsed.
struct Args {
    port: u16,
    cert: PathBuf,
    protocol: HttpProtocol,
    profile: String,
    path: String,
    qlog: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut profile = "chrome-156-macos".to_owned();
    let mut path = "/h3fetch".to_owned();
    let mut qlog = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => profile = args.next().ok_or("--profile needs a value")?,
            "--path" => path = args.next().ok_or("--path needs a value")?,
            "--qlog" => qlog = Some(PathBuf::from(args.next().ok_or("--qlog needs a value")?)),
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}")),
            _ => positional.push(arg),
        }
    }
    let [port, cert, protocol] = positional.as_slice() else {
        return Err("usage: h3fetch <port> <cert.der> <h1|h2|h3> [--profile NAME] [--path /x] [--qlog DIR]".into());
    };
    let protocol = match protocol.as_str() {
        "h1" => HttpProtocol::Http1,
        "h2" => HttpProtocol::Http2,
        "h3" => HttpProtocol::Http3,
        other => return Err(format!("protocol must be h1, h2 or h3, not {other}")),
    };
    Ok(Args {
        port: port.parse().map_err(|_| "the port is not a number".to_owned())?,
        cert: PathBuf::from(cert),
        protocol,
        profile,
        path,
        qlog,
    })
}

async fn fetch(args: &Args) -> Result<(), String> {
    let cert = std::fs::read(&args.cert).map_err(|e| format!("cannot read {}: {e}", args.cert.display()))?;
    let profile = profile_for(&args.profile, RequestTemplateKind::Navigation).map_err(|e| e.to_string())?;
    let mut builder = Client::builder(profile)
        .add_root_certificate_der(cert)
        .resolve("localhost", [LOOPBACK])
        .request_timeouts(RequestTimeouts::new().total(Duration::from_secs(15)));
    if let Some(dir) = &args.qlog {
        builder = builder.qlog_dir(dir.clone());
    }
    let client = builder.build().map_err(|e| e.to_string())?;
    let url = format!("https://localhost:{}{}", args.port, args.path);
    let mut request = client.get_with_protocol(args.protocol, &url).map_err(|e| e.to_string())?;
        let timeline = request.diagnostics(NonZeroUsize::new(256).unwrap_or(NonZeroUsize::MIN));
        let response = match request.send().await {
            Ok(response) => response,
            Err(e) => return Err(format!("{e} | {e:?} | timeline: {:?}", timeline.snapshot())),
        };
    let status = response.status();
    let seen = response
        .extensions()
        .get::<ResponseInfo>()
        .map(|info| format!("{:?}", info.protocol()))
        .unwrap_or_else(|| "unknown".to_owned());
    let body = response
        .into_body()
        .collect_with_limit(BODY_LIMIT)
        .await
        .map_err(|e| e.to_string())?;
    println!(
        "phantom profile={} asked={:?} got={seen} status={status} url={url}",
        args.profile, args.protocol
    );
    println!("body: {}", String::from_utf8_lossy(&body).trim_end());
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("h3fetch: {error}");
            return ExitCode::from(2);
        }
    };
    match fetch(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            println!("FAIL asked={:?}: {error}", args.protocol);
            ExitCode::FAILURE
        }
    }
}
