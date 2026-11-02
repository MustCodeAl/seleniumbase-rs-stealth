//! A local server for the spike: HTTP/3 over QUIC (UDP) and HTTP/1.1 over TLS (TCP), both on
//! 127.0.0.1 with a throwaway certificate, logging what each client sent.
//!
//! The QUIC side is quinn with rustls and nothing else: it reads the HTTP/3 control stream,
//! the QPACK streams and the request stream itself, so the SETTINGS, their order, the stream
//! ids and the QPACK field lines are exactly what arrived, not what a library kept.
//!
//! ```text
//! h3srv --cert certs/cert.pem --key certs/key.pem --out out/h3srv.jsonl
//!       [--tcp 127.0.0.1:4433] [--udp 127.0.0.1:4434]
//!       [--qpack-capacity 0] [--qpack-blocked 0] [--alt-svc]
//! ```

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use http3_spike_tools::h3wire::{
    decode_field_section, encode_response_headers, frame_name, hex, is_h3_grease, parse_frames,
    parse_settings, read_varint, setting_name, stream_type_name, write_frame, write_varint,
};
use quinn::crypto::rustls::{HandshakeData, QuicServerConfig};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::TlsAcceptor;

/// How long a stream may stay silent before what arrived so far is taken as complete.
const IDLE: Duration = Duration::from_millis(500);

/// The JSON-lines log, shared by every task.
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

struct Config {
    tcp: SocketAddr,
    udp: SocketAddr,
    cert: PathBuf,
    key: PathBuf,
    out: PathBuf,
    qpack_capacity: u64,
    qpack_blocked: u64,
    alt_svc: bool,
}

fn parse_args() -> Result<Config, String> {
    let mut config = Config {
        tcp: "127.0.0.1:4433".parse().map_err(|_| "tcp")?,
        udp: "127.0.0.1:4434".parse().map_err(|_| "udp")?,
        cert: PathBuf::from("certs/cert.pem"),
        key: PathBuf::from("certs/key.pem"),
        out: PathBuf::from("out/h3srv.jsonl"),
        qpack_capacity: 0,
        qpack_blocked: 0,
        alt_svc: false,
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or(format!("{flag} needs a value"));
        match flag.as_str() {
            "--tcp" => config.tcp = value()?.parse().map_err(|_| "bad --tcp")?,
            "--udp" => config.udp = value()?.parse().map_err(|_| "bad --udp")?,
            "--cert" => config.cert = value()?.into(),
            "--key" => config.key = value()?.into(),
            "--out" => config.out = value()?.into(),
            "--qpack-capacity" => config.qpack_capacity = value()?.parse().map_err(|_| "bad capacity")?,
            "--qpack-blocked" => config.qpack_blocked = value()?.parse().map_err(|_| "bad blocked")?,
            "--alt-svc" => config.alt_svc = true,
            other => return Err(format!("unknown flag {other}")),
        }
    }
    for addr in [config.tcp, config.udp] {
        if !addr.ip().is_loopback() {
            return Err(format!("{addr} is not a loopback address: the spike serves 127.0.0.1 only"));
        }
    }
    Ok(config)
}

/// Appends to `out` until `idle` passes without data, the stream ends, or `cap` bytes are held.
/// Returns whether the peer finished the stream.
async fn read_until_idle(stream: &mut quinn::RecvStream, out: &mut Vec<u8>, idle: Duration, cap: usize) -> bool {
    let mut buf = [0_u8; 4096];
    loop {
        match tokio::time::timeout(idle, stream.read(&mut buf)).await {
            Ok(Ok(Some(n))) => {
                out.extend_from_slice(&buf[..n]);
                if out.len() >= cap {
                    return false;
                }
            }
            Ok(Ok(None)) => return true,
            Ok(Err(_)) | Err(_) => return false,
        }
    }
}

fn stream_id_number(id: quinn::StreamId) -> u64 {
    quinn::VarInt::from(id).into_inner()
}

fn settings_json(pairs: &[(u64, u64)]) -> Value {
    json!(pairs
        .iter()
        .map(|(id, value)| json!({
            "id": format!("{id:#x}"),
            "name": setting_name(*id),
            "grease": is_h3_grease(*id),
            "value": value,
        }))
        .collect::<Vec<_>>())
}

fn frames_json(buf: &[u8]) -> (Value, usize) {
    let (frames, used) = parse_frames(buf);
    let list = frames
        .iter()
        .map(|frame| {
            let mut entry = json!({
                "type": frame_name(frame.ty),
                "type_raw": format!("{:#x}", frame.ty),
                "len": frame.payload.len(),
            });
            match frame.ty {
                0x04 => {
                    entry["settings"] = parse_settings(&frame.payload)
                        .map(|p| settings_json(&p))
                        .unwrap_or(Value::Null);
                }
                0x01 => {
                    entry["block_hex"] = json!(hex(&frame.payload));
                    entry["section"] = match decode_field_section(&frame.payload) {
                        Ok(section) => json!({
                            "required_insert_count": section.required_insert_count,
                            "base_sign": section.base.0,
                            "delta_base": section.base.1,
                            "fields": section.fields.iter().map(|f| json!({
                                "repr": f.repr, "name": f.name, "value": f.value, "huffman": f.huffman,
                            })).collect::<Vec<_>>(),
                        }),
                        Err(error) => json!({ "error": error }),
                    };
                }
                0x00 => {
                    entry["data_hex"] = json!(hex(&frame.payload[..frame.payload.len().min(64)]));
                }
                _ => {
                    entry["payload_hex"] = json!(hex(&frame.payload[..frame.payload.len().min(64)]));
                }
            }
            entry
        })
        .collect::<Vec<_>>();
    (json!(list), used)
}

/// Opens the server's three unidirectional streams: control with SETTINGS, QPACK encoder and
/// decoder (type byte only).
async fn open_server_streams(conn: quinn::Connection, config: Arc<Config>) {
    let mut control = Vec::new();
    write_varint(&mut control, 0x00);
    let mut settings = Vec::new();
    for (id, value) in [(0x01_u64, config.qpack_capacity), (0x07, config.qpack_blocked)] {
        write_varint(&mut settings, id);
        write_varint(&mut settings, value);
    }
    write_frame(&mut control, 0x04, &settings);
    for (ty, payload) in [(None, control), (Some(0x02_u64), Vec::new()), (Some(0x03), Vec::new())] {
        let Ok(mut send) = conn.open_uni().await else {
            return;
        };
        let mut bytes = Vec::new();
        if let Some(ty) = ty {
            write_varint(&mut bytes, ty);
        }
        bytes.extend_from_slice(&payload);
        if send.write_all(&bytes).await.is_err() {
            return;
        }
        // The streams stay open for the life of the connection: finishing the control stream
        // would be a protocol error.
        tokio::spawn({ let conn = conn.clone(); async move { let _keep = send; let _ = conn.closed().await; } });
    }
}

async fn handle_uni(mut stream: quinn::RecvStream, conn_no: u64, seq: u64, log: Log) {
    let id = stream_id_number(stream.id());
    let mut data = Vec::new();
    let fin = read_until_idle(&mut stream, &mut data, IDLE, 1 << 16).await;
    let mut pos = 0;
    let Some(ty) = read_varint(&data, &mut pos) else {
        log.emit(json!({ "ev": "uni_stream", "conn": conn_no, "seq": seq, "stream_id": id, "error": "no type", "bytes_hex": hex(&data) }));
        return;
    };
    let mut entry = json!({
        "ev": "uni_stream", "conn": conn_no, "seq": seq, "stream_id": id,
        "stream_type": stream_type_name(ty), "stream_type_raw": format!("{ty:#x}"),
        "fin": fin, "bytes": data.len(),
    });
    match ty {
        0x00 => {
            let (frames, used) = frames_json(&data[pos..]);
            entry["frames"] = frames;
            entry["unparsed_tail_bytes"] = json!(data.len() - pos - used);
        }
        _ => {
            entry["bytes_hex"] = json!(hex(&data[pos..data.len().min(pos + 256)]));
        }
    }
    log.emit(entry);
}

async fn handle_bi(mut send: quinn::SendStream, mut recv: quinn::RecvStream, conn_no: u64, seq: u64, sni: String, log: Log) {
    let id = stream_id_number(recv.id());
    let mut data = Vec::new();
    let fin = read_until_idle(&mut recv, &mut data, IDLE, 1 << 20).await;
    let (frames, used) = frames_json(&data);
    log.emit(json!({
        "ev": "request", "conn": conn_no, "seq": seq, "stream_id": id, "fin": fin,
        "bytes": data.len(), "frames": frames, "unparsed_tail_bytes": data.len() - used,
    }));
    let body = format!("proto=h3 alpn=h3 sni={sni} stream={id}\n");
    let mut reply = Vec::new();
    write_frame(
        &mut reply,
        0x01,
        &encode_response_headers(&[("content-type", "text/plain"), ("x-spike-transport", "h3")]),
    );
    write_frame(&mut reply, 0x00, body.as_bytes());
    if send.write_all(&reply).await.is_ok() {
        let _ = send.finish();
    }
}

async fn handle_conn(incoming: quinn::Incoming, conn_no: u64, config: Arc<Config>, log: Log) {
    let remote = incoming.remote_address();
    let conn = match incoming.await {
        Ok(conn) => conn,
        Err(error) => {
            log.emit(json!({ "ev": "quic_handshake_failed", "conn": conn_no, "remote": remote.to_string(), "error": error.to_string() }));
            return;
        }
    };
    let (alpn, sni) = conn
        .handshake_data()
        .and_then(|data| data.downcast::<HandshakeData>().ok())
        .map(|data| {
            (
                data.protocol.map(|p| String::from_utf8_lossy(&p).into_owned()),
                data.server_name.unwrap_or_default(),
            )
        })
        .unwrap_or_default();
    log.emit(json!({
        "ev": "quic_connection", "conn": conn_no, "remote": remote.to_string(),
        "alpn": alpn, "sni": sni, "max_datagram_size": conn.max_datagram_size(),
    }));
    tokio::spawn(open_server_streams(conn.clone(), config));
    let seq = Arc::new(AtomicU64::new(0));
    loop {
        tokio::select! {
            uni = conn.accept_uni() => match uni {
                Ok(stream) => {
                    let n = seq.fetch_add(1, Ordering::Relaxed);
                    tokio::spawn(handle_uni(stream, conn_no, n, log.clone()));
                }
                Err(_) => break,
            },
            bi = conn.accept_bi() => match bi {
                Ok((send, recv)) => {
                    let n = seq.fetch_add(1, Ordering::Relaxed);
                    tokio::spawn(handle_bi(send, recv, conn_no, n, sni.clone(), log.clone()));
                }
                Err(_) => break,
            },
        }
    }
    log.emit(json!({ "ev": "quic_closed", "conn": conn_no, "reason": format!("{:?}", conn.close_reason()) }));
}

async fn handle_tcp(tcp: TcpStream, acceptor: TlsAcceptor, conn_no: u64, config: Arc<Config>, log: Log) {
    let peer = tcp.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    let mut tls = match acceptor.accept(tcp).await {
        Ok(tls) => tls,
        Err(error) => {
            log.emit(json!({ "ev": "tls_handshake_failed", "conn": conn_no, "peer": peer, "error": error.to_string() }));
            return;
        }
    };
    let (alpn, sni, version) = {
        let (_, session) = tls.get_ref();
        (
            session.alpn_protocol().map(|p| String::from_utf8_lossy(p).into_owned()),
            session.server_name().map(str::to_owned),
            session.protocol_version().map(|v| format!("{v:?}")),
        )
    };
    let mut request = Vec::new();
    let mut buf = [0_u8; 4096];
    while !request.windows(4).any(|w| w == b"\r\n\r\n") && request.len() < 16 * 1024 {
        match tokio::time::timeout(Duration::from_secs(5), tls.read(&mut buf)).await {
            Ok(Ok(n)) if n > 0 => request.extend_from_slice(&buf[..n]),
            _ => break,
        }
    }
    let text = String::from_utf8_lossy(&request).into_owned();
    let lines: Vec<&str> = text.split("\r\n").take_while(|l| !l.is_empty()).collect();
    log.emit(json!({
        "ev": "tcp_request", "conn": conn_no, "peer": peer, "tls_version": version,
        "alpn": alpn, "sni": sni, "request_line": lines.first(), "header_lines_in_order": lines.get(1..),
    }));
    let body = format!("proto=http/1.1 over TLS/TCP alpn={} sni={}\n", alpn.unwrap_or_default(), sni.unwrap_or_default());
    let alt = if config.alt_svc {
        format!("Alt-Svc: h3=\":{}\"; ma=60\r\n", config.tcp.port())
    } else {
        String::new()
    };
    let reply = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n{alt}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = tls.write_all(reply.as_bytes()).await;
    let _ = tls.shutdown().await;
}

fn run_config() -> Result<(Config, Arc<rustls::ServerConfig>), String> {
    let config = parse_args()?;
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(&config.cert)
        .map_err(|e| format!("cert: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("cert: {e}"))?;
    let key = PrivateKeyDer::from_pem_file(&config.key).map_err(|e| format!("key: {e}"))?;
    let tls = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("tls config: {e}"))?;
    Ok((config, Arc::new(tls)))
}

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let (config, base_tls) = match run_config() {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("h3srv: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    let config = Arc::new(config);
    if let Some(dir) = config.out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = match OpenOptions::new().create(true).append(true).open(&config.out) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("h3srv: cannot open {}: {error}", config.out.display());
            return std::process::ExitCode::from(2);
        }
    };
    let log = Log { file: Arc::new(Mutex::new(file)), start: Instant::now() };

    // QUIC: ALPN h3 only.
    let mut quic_tls = (*base_tls).clone();
    quic_tls.alpn_protocols = vec![b"h3".to_vec()];
    let quic_crypto = match QuicServerConfig::try_from(quic_tls) {
        Ok(crypto) => crypto,
        Err(error) => {
            eprintln!("h3srv: quic tls: {error}");
            return std::process::ExitCode::from(2);
        }
    };
    let server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_crypto));
    let endpoint = match quinn::Endpoint::server(server_config, config.udp) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            eprintln!("h3srv: udp {}: {error}", config.udp);
            return std::process::ExitCode::from(2);
        }
    };

    // TCP: ALPN http/1.1 only (the spike server does not speak HTTP/2).
    let mut tcp_tls = (*base_tls).clone();
    tcp_tls.alpn_protocols = vec![b"http/1.1".to_vec()];
    let acceptor = TlsAcceptor::from(Arc::new(tcp_tls));
    let listener = match TcpListener::bind(config.tcp).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("h3srv: tcp {}: {error}", config.tcp);
            return std::process::ExitCode::from(2);
        }
    };
    log.emit(json!({ "ev": "listening", "tcp": config.tcp.to_string(), "udp": config.udp.to_string(),
                     "qpack_capacity": config.qpack_capacity, "qpack_blocked": config.qpack_blocked }));

    let counter = Arc::new(AtomicU64::new(0));
    {
        let (config, log, counter) = (config.clone(), log.clone(), counter.clone());
        tokio::spawn(async move {
            while let Some(incoming) = endpoint.accept().await {
                let n = counter.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(handle_conn(incoming, n, config.clone(), log.clone()));
            }
        });
    }
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                if let Ok((tcp, _)) = accepted {
                    let n = counter.fetch_add(1, Ordering::Relaxed);
                    tokio::spawn(handle_tcp(tcp, acceptor.clone(), n, config.clone(), log.clone()));
                }
            }
            _ = tokio::signal::ctrl_c() => break,
        }
    }
    std::process::ExitCode::SUCCESS
}
