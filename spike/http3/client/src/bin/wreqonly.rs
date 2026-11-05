//! Control for question (a): one request through the stealth crates (wreq over btls 0.5.6) and
//! nothing from phantom, so only one BoringSSL is linked. If this links and runs while `clash`
//! does not, the failure belongs to having two TLS stacks in one binary.
//!
//! ```text
//! wreqonly <port> <cert.der>
//! ```
//!
//! Every request goes to `localhost` / 127.0.0.1 and nowhere else.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::process::ExitCode;
use std::time::Duration;

use seleniumbase_rs_codec::BrowserKind;
use seleniumbase_rs_wire::client::{impersonation_client, ImpersonationProfile};
use wreq::tls::trust::CertStore;

const LOOPBACK: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);

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
    Ok(format!("wreq {version:?} status={status} body={:?}", String::from_utf8_lossy(&body)))
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(port), Some(cert_path)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: wreqonly <port> <cert.der>");
        return ExitCode::from(2);
    };
    let Ok(port) = port.parse::<u16>() else {
        eprintln!("wreqonly: the port is not a number");
        return ExitCode::from(2);
    };
    let cert_der = match std::fs::read(cert_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("wreqonly: cannot read {cert_path}: {error}");
            return ExitCode::from(2);
        }
    };
    match via_wreq(port, &cert_der).await {
        Ok(text) => {
            println!("OK   {text}");
            ExitCode::SUCCESS
        }
        Err(text) => {
            println!("FAIL {text}");
            ExitCode::FAILURE
        }
    }
}
