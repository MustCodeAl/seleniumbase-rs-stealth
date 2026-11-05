//! Question (a): do phantom-http (its own BoringSSL) and the stealth crates (wreq over btls
//! 0.5.6, another BoringSSL) live in one process?
//!
//! This binary links both and calls both TLS stacks against one local server, alternating, so a
//! symbol clash (a link error) or a stack that breaks after the other has run (a runtime error)
//! shows up as a failed step. Run `h3srv` first; see spike/http3/README.md.
//!
//! ```text
//! clash <port> <cert.der> [--tcp-only]
//! ```
//!
//! Every request goes to `localhost` / 127.0.0.1 and nowhere else.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::ExitCode;
use std::time::Duration;

use phantom::profile::catalog::{profile_for, RequestTemplateKind};
use phantom::{Client, HttpProtocol, ResponseInfo};
use seleniumbase_rs_codec::BrowserKind;
use seleniumbase_rs_wire::client::{impersonation_client, ImpersonationProfile};
use wreq::tls::trust::CertStore;

/// The phantom profile used for every phantom request: Chrome 156 on Windows, navigation headers.
const PHANTOM_PROFILE: &str = "chrome-156-windows";
const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const BODY_LIMIT: usize = 1 << 20;

/// One line of the result table.
struct Step {
    label: String,
    outcome: Result<String, String>,
}

/// A request through wreq with the measured Chrome profile of the stealth crates.
async fn via_wreq(port: u16, cert_der: &[u8]) -> Result<String, String> {
    let profile = ImpersonationProfile::measured(BrowserKind::Chrome).map_err(|e| e.to_string())?;
    let store = CertStore::from_der_certs([cert_der]).map_err(|e| e.to_string())?;
    let client = impersonation_client(&profile)
        .map_err(|e| e.to_string())?
        .tls_cert_store(store)
        .resolve("localhost", SocketAddr::new(LOOPBACK, port))
        .no_proxy()
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(format!("https://localhost:{port}/via-wreq"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = response.status();
    let version = response.version();
    let body = response.bytes().await.map_err(|e| e.to_string())?;
    Ok(format!(
        "wreq {version:?} status={status} body={:?}",
        String::from_utf8_lossy(&body)
    ))
}

/// A request through phantom with an exact protocol.
async fn via_phantom(port: u16, cert_der: &[u8], protocol: HttpProtocol) -> Result<String, String> {
    let profile = profile_for(PHANTOM_PROFILE, RequestTemplateKind::Navigation)
        .map_err(|e| e.to_string())?;
    let client = Client::builder(profile)
        .add_root_certificate_der(cert_der.to_vec())
        .resolve("localhost", [LOOPBACK])
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get_with_protocol(protocol, &format!("https://localhost:{port}/via-phantom"))
        .map_err(|e| e.to_string())?
        .send()
        .await
        .map_err(|e| e.to_string())?;
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
        "phantom asked={protocol:?} got={seen} status={status} body={:?}",
        String::from_utf8_lossy(&body)
    ))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(port), Some(cert_path)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: clash <port> <cert.der> [--tcp-only]");
        return ExitCode::from(2);
    };
    let Ok(port) = port.parse::<u16>() else {
        eprintln!("clash: the port is not a number");
        return ExitCode::from(2);
    };
    let tcp_only = args.iter().any(|a| a == "--tcp-only");
    let cert_der = match std::fs::read(cert_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("clash: cannot read {cert_path}: {error}");
            return ExitCode::from(2);
        }
    };

    // The order alternates the stacks on purpose: each one runs again after the other has run.
    let mut steps = Vec::new();
    steps.push(Step {
        label: "1 wreq (btls 0.5.6) over TCP".into(),
        outcome: via_wreq(port, &cert_der).await,
    });
    steps.push(Step {
        label: "2 phantom (phantom-btls) over TCP h1 (h3srv offers no h2)".into(),
        outcome: via_phantom(port, &cert_der, HttpProtocol::Http1).await,
    });
    steps.push(Step {
        label: "3 wreq again".into(),
        outcome: via_wreq(port, &cert_der).await,
    });
    if !tcp_only {
        steps.push(Step {
            label: "4 phantom over QUIC h3".into(),
            outcome: via_phantom(port, &cert_der, HttpProtocol::Http3).await,
        });
        steps.push(Step {
            label: "5 wreq after h3".into(),
            outcome: via_wreq(port, &cert_der).await,
        });
        steps.push(Step {
            label: "6 phantom h3 again".into(),
            outcome: via_phantom(port, &cert_der, HttpProtocol::Http3).await,
        });
    }
    steps.push(Step {
        label: "7 phantom over TCP h1".into(),
        outcome: via_phantom(port, &cert_der, HttpProtocol::Http1).await,
    });
    steps.push(Step {
        label: "8 wreq last".into(),
        outcome: via_wreq(port, &cert_der).await,
    });

    let mut failed = 0_u32;
    for step in &steps {
        match &step.outcome {
            Ok(text) => println!("OK   {}: {text}", step.label),
            Err(text) => {
                failed += 1;
                println!("FAIL {}: {text}", step.label);
            }
        }
    }
    println!("{} steps, {failed} failed", steps.len());
    if failed == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
