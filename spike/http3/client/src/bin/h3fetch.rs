//! Question (b): fetch pages from the local `h3srv` with phantom, over an exact protocol or
//! negotiated, and say which protocol each response really came over.
//!
//! The proof of HTTP/3 is not this program's word alone: `h3srv` logs a `quic_connection`
//! event (ALPN `h3`, SNI, a QUIC handshake) and the request's HEADERS frame, and its reply body
//! names the transport it saw. This program prints what phantom reports (`ResponseInfo`) and the
//! body, so the three can be compared.
//!
//! ```text
//! h3fetch <port> <cert.der> <h1|h2|h3|auto> [--profile NAME] [--path /x] [--qlog DIR]
//!         [--alt-svc] [--repeat N]
//! ```
//!
//! `auto` is phantom's negotiated request (TLS picks HTTP/1.1 or HTTP/2); with `--alt-svc` the
//! client learns `Alt-Svc: h3` from a response and uses it for later negotiated requests.
//! Every request goes to `localhost` / 127.0.0.1 and nowhere else.

use std::net::{IpAddr, Ipv4Addr};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use phantom::profile::catalog::{profile_for, RequestTemplateKind};
use phantom::{Client, HttpProtocol, RequestTimeouts, ResponseInfo};

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const BODY_LIMIT: usize = 1 << 20;

/// What the command line asked for.
#[derive(Clone, Copy, Debug)]
enum Mode {
    /// An exact protocol, never falling back.
    Exact(HttpProtocol),
    /// A negotiated request.
    Auto,
}

/// The command line, parsed.
struct Args {
    port: u16,
    cert: PathBuf,
    mode: Mode,
    profile: String,
    path: String,
    qlog: Option<PathBuf>,
    alt_svc: bool,
    repeat: u32,
}

fn parse_args() -> Result<Args, String> {
    let mut positional = Vec::new();
    let mut profile = "chrome-156-macos".to_owned();
    let mut path = "/h3fetch".to_owned();
    let mut qlog = None;
    let mut alt_svc = false;
    let mut repeat = 1_u32;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--profile" => profile = args.next().ok_or("--profile needs a value")?,
            "--path" => path = args.next().ok_or("--path needs a value")?,
            "--qlog" => qlog = Some(PathBuf::from(args.next().ok_or("--qlog needs a value")?)),
            "--alt-svc" => alt_svc = true,
            "--repeat" => {
                repeat = args
                    .next()
                    .ok_or("--repeat needs a value")?
                    .parse()
                    .map_err(|_| "--repeat is not a number")?;
            }
            flag if flag.starts_with("--") => return Err(format!("unknown flag {flag}")),
            _ => positional.push(arg),
        }
    }
    let [port, cert, mode] = positional.as_slice() else {
        return Err("usage: h3fetch <port> <cert.der> <h1|h2|h3|auto> [--profile NAME] [--path /x] [--qlog DIR] [--alt-svc] [--repeat N]".into());
    };
    let mode = match mode.as_str() {
        "h1" => Mode::Exact(HttpProtocol::Http1),
        "h2" => Mode::Exact(HttpProtocol::Http2),
        "h3" => Mode::Exact(HttpProtocol::Http3),
        "auto" => Mode::Auto,
        other => return Err(format!("mode must be h1, h2, h3 or auto, not {other}")),
    };
    Ok(Args {
        port: port.parse().map_err(|_| "the port is not a number".to_owned())?,
        cert: PathBuf::from(cert),
        mode,
        profile,
        path,
        qlog,
        alt_svc,
        repeat,
    })
}

/// The `source()` chain of an error, innermost last.
fn chain(error: &dyn std::error::Error) -> String {
    let mut parts = Vec::new();
    let mut current = error.source();
    while let Some(source) = current {
        parts.push(format!("{source} ({source:?})"));
        current = source.source();
    }
    if parts.is_empty() {
        "no source".to_owned()
    } else {
        parts.join(" <- ")
    }
}

/// One request; returns the line to print.
async fn one_request(client: &Client, args: &Args, url: &str) -> Result<String, String> {
    let mut request = match args.mode {
        Mode::Exact(protocol) => client.get_with_protocol(protocol, url),
        Mode::Auto => client.get(url),
    }
    .map_err(|e| e.to_string())?;
    let timeline = request.diagnostics(NonZeroUsize::new(256).unwrap_or(NonZeroUsize::MIN));
    let response = match request.send().await {
        Ok(response) => response,
        Err(e) => {
            return Err(format!(
                "{e} | source chain: {} | timeline: {:?}",
                chain(&e),
                timeline.snapshot()
            ))
        }
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
    Ok(format!(
        "got={seen} status={status} body: {}",
        String::from_utf8_lossy(&body).trim_end()
    ))
}

async fn run(args: &Args) -> Result<bool, String> {
    let cert = std::fs::read(&args.cert).map_err(|e| format!("cannot read {}: {e}", args.cert.display()))?;
    let profile = profile_for(&args.profile, RequestTemplateKind::Navigation).map_err(|e| e.to_string())?;
    let mut builder = Client::builder(profile)
        .add_root_certificate_der(cert)
        .resolve("localhost", [LOOPBACK])
        .request_timeouts(RequestTimeouts::new().total(Duration::from_secs(15)));
    if let Some(dir) = &args.qlog {
        builder = builder.qlog_dir(dir.clone());
    }
    if args.alt_svc {
        builder = builder.alt_svc(NonZeroUsize::new(16).unwrap_or(NonZeroUsize::MIN));
    }
    let client = builder.build().map_err(|e| e.to_string())?;
    let url = format!("https://localhost:{}{}", args.port, args.path);
    let mut all_ok = true;
    for n in 1..=args.repeat {
        let started = Instant::now();
        let outcome = one_request(&client, args, &url).await;
        let ms = started.elapsed().as_millis();
        match outcome {
            Ok(line) => println!(
                "#{n} profile={} asked={:?} {line} ({ms} ms)",
                args.profile, args.mode
            ),
            Err(error) => {
                all_ok = false;
                println!("#{n} FAIL asked={:?} ({ms} ms): {error}", args.mode);
            }
        }
    }
    Ok(all_ok)
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
    match run(&args).await {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            println!("FAIL setup: {error}");
            ExitCode::FAILURE
        }
    }
}
