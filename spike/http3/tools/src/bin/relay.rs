//! A UDP relay that reads the QUIC Initial packets of the clients going through it.
//!
//! It forwards every datagram unchanged to the real server (`h3srv`), and for each client
//! connection it decrypts the client's Initial packets (RFC 9001 section 5.2: the keys come
//! from the Destination Connection ID, so no key log is needed), joins their CRYPTO frames,
//! parses the TLS `ClientHello` and prints its QUIC transport parameters in wire order.
//!
//! ```text
//! relay --listen 127.0.0.1:4433 --to 127.0.0.1:4434 --out out/relay.jsonl
//! ```
//!
//! Loopback only: it refuses any other address.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use http3_spike_tools::h3wire::hex;
use http3_spike_tools::initial::{
    client_hello_json, decrypt_initial, parse_client_hello, parse_long_header, read_frames,
    CryptoStream, HelloState,
};
use serde_json::{json, Value};
use tokio::net::UdpSocket;

/// How many datagrams per client address get a one-line summary.
const SUMMARY_LIMIT: usize = 12;

#[derive(Clone)]
struct Log {
    file: Arc<Mutex<File>>,
    start: Instant,
}

impl Log {
    fn emit(&self, mut value: Value) {
        value["t_ms"] = json!(self.start.elapsed().as_millis());
        let line = value.to_string();
        if let Ok(mut file) = self.file.lock() {
            let _ = writeln!(file, "{line}");
        }
        eprintln!("{line}");
    }
}

/// What the relay knows about one client connection (keyed by the first Destination Connection ID).
#[derive(Default)]
struct Flight {
    crypto: CryptoStream,
    packets: Vec<Value>,
    reported: bool,
}

fn parse_args() -> Result<(SocketAddr, SocketAddr, PathBuf), String> {
    let mut listen: SocketAddr = "127.0.0.1:4433".parse().map_err(|_| "listen")?;
    let mut to: SocketAddr = "127.0.0.1:4434".parse().map_err(|_| "to")?;
    let mut out = PathBuf::from("out/relay.jsonl");
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--listen" => listen = value()?.parse().map_err(|_| "bad --listen")?,
            "--to" => to = value()?.parse().map_err(|_| "bad --to")?,
            "--out" => out = value()?.into(),
            other => return Err(format!("unknown flag {other}")),
        }
    }
    for addr in [listen, to] {
        if !addr.ip().is_loopback() {
            return Err(format!("{addr} is not a loopback address: the spike relays 127.0.0.1 only"));
        }
    }
    Ok((listen, to, out))
}

/// Looks at one client datagram: logs a short summary, and decrypts any Initial packet in it.
fn analyze(
    datagram: &[u8],
    from: SocketAddr,
    seen: &mut HashMap<SocketAddr, usize>,
    flights: &mut HashMap<Vec<u8>, Flight>,
    log: &Log,
) {
    let count = seen.entry(from).or_insert(0);
    *count += 1;
    let number = *count;
    let first = datagram.first().copied().unwrap_or(0);
    let long = first & 0x80 != 0;
    let mut summary = json!({
        "ev": "client_datagram", "from_port": from.port(), "n": number, "len": datagram.len(),
        "first_byte_protected": format!("{first:#04x}"), "long_header": long,
    });
    if long {
        match decrypt_initial(datagram) {
            Ok(initial) => {
                let (frame_names, crypto) = read_frames(&initial.frames);
                let flight = flights.entry(initial.header.dcid.clone()).or_default();
                for frame in &crypto {
                    flight.crypto.add(frame);
                }
                let packet = json!({
                    "datagram_n": number,
                    "datagram_len": datagram.len(),
                    "packet_len": initial.packet_len,
                    "coalesced_after_this_packet": datagram.len() - initial.packet_len,
                    "version": format!("{:#010x}", initial.header.version),
                    "dcid": hex(&initial.header.dcid),
                    "scid": hex(&initial.header.scid),
                    "token_len": initial.header.token.len(),
                    "packet_number": initial.packet_number,
                    "pn_len": initial.pn_len,
                    "frames": frame_names,
                });
                summary["initial"] = packet.clone();
                flight.packets.push(packet);
                if !flight.reported {
                    match parse_client_hello(&flight.crypto.contiguous()) {
                        Ok(hello) => {
                            flight.reported = true;
                            log.emit(json!({
                                "ev": "client_hello",
                                "dcid": hex(&initial.header.dcid),
                                "client_hello": client_hello_json(&hello),
                                "initial_packets": flight.packets,
                            }));
                        }
                        Err(HelloState::Incomplete) => {}
                        Err(HelloState::Malformed(why)) => {
                            flight.reported = true;
                            log.emit(json!({ "ev": "client_hello_malformed", "why": why }));
                        }
                    }
                }
            }
            Err(error) => {
                summary["not_decoded"] = json!(error);
                if let Ok(header) = parse_long_header(datagram) {
                    summary["version"] = json!(format!("{:#010x}", header.version));
                    summary["dcid_len"] = json!(header.dcid.len());
                }
            }
        }
    }
    if number <= SUMMARY_LIMIT {
        log.emit(summary);
    }
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let (listen, to, out) = match parse_args() {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("relay: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = match OpenOptions::new().create(true).append(true).open(&out) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("relay: cannot open {}: {error}", out.display());
            return std::process::ExitCode::from(2);
        }
    };
    let log = Log { file: Arc::new(Mutex::new(file)), start: Instant::now() };
    let socket = match UdpSocket::bind(listen).await {
        Ok(socket) => Arc::new(socket),
        Err(error) => {
            eprintln!("relay: cannot bind {listen}: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    log.emit(json!({ "ev": "relay_listening", "listen": listen.to_string(), "to": to.to_string() }));

    let mut upstreams: HashMap<SocketAddr, Arc<UdpSocket>> = HashMap::new();
    let mut seen = HashMap::new();
    let mut flights = HashMap::new();
    let mut buf = vec![0_u8; 65536];
    loop {
        let (n, from) = tokio::select! {
            received = socket.recv_from(&mut buf) => match received {
                Ok(pair) => pair,
                Err(error) => {
                    eprintln!("relay: recv: {error}");
                    continue;
                }
            },
            _ = tokio::signal::ctrl_c() => return std::process::ExitCode::SUCCESS,
        };
        analyze(&buf[..n], from, &mut seen, &mut flights, &log);
        let upstream = match upstreams.get(&from) {
            Some(upstream) => upstream.clone(),
            None => {
                let Ok(upstream) = UdpSocket::bind("127.0.0.1:0").await else {
                    continue;
                };
                if upstream.connect(to).await.is_err() {
                    continue;
                }
                let upstream = Arc::new(upstream);
                upstreams.insert(from, upstream.clone());
                let (back, listen_socket) = (upstream.clone(), socket.clone());
                tokio::spawn(async move {
                    let mut reply = vec![0_u8; 65536];
                    while let Ok(m) = back.recv(&mut reply).await {
                        if listen_socket.send_to(&reply[..m], from).await.is_err() {
                            break;
                        }
                    }
                });
                upstream
            }
        };
        let _ = upstream.send(&buf[..n]).await;
    }
}
