# HTTP/3 spike

Scratch code behind `docs/design/http3.md`: can phantom (crate `phantom-http`) give the stealth
crates an HTTP/3 transport next to wreq/btls? It is a Cargo workspace of its own (its own
`[workspace]` table), so the root workspace of this repository does not build it and the root
`Cargo.toml` / `Cargo.lock` are untouched. Nothing here is published.

Everything runs on loopback: the server, the relay and every client talk to `127.0.0.1` /
`localhost`, with a throwaway certificate. Chrome runs headless and is told to resolve every
host name to 127.0.0.1.

## Layout

| Path | What |
|---|---|
| `client/src/bin/clash.rs` | Question (a): one binary that links phantom and the stealth crates (wreq over btls 0.5.6) and calls both TLS stacks alternately, against the local server. |
| `client/src/bin/h3fetch.rs` | Question (b): one phantom request over an exact protocol (`h1`, `h2`, `h3`); prints the protocol phantom reports. |
| `tools/src/bin/h3srv.rs` | Local server: HTTP/3 over QUIC (quinn + rustls, no h3 library) and HTTP/1.1 over TLS. Logs the HTTP/3 control stream (SETTINGS in order), QPACK streams and the request HEADERS as they arrived. |
| `tools/src/bin/relay.rs` | UDP relay in front of the server. Decrypts the client's QUIC Initial packet (RFC 9001 section 5) and prints the TLS ClientHello with the QUIC transport parameters in wire order. |
| `tools/src/initial.rs`, `h3wire.rs` | The parsers behind them; unit tests include the RFC 9001 appendix A.2 client Initial. |
| `scripts/` | `mkcert.sh`, `serve-and-run.sh` and the three `client-*.sh`. |
| `results/` | Small, curated captures quoted in `docs/design/http3.md`. |
| `certs/`, `out/`, `target/` | Gitignored: certificate and key, raw logs, build output. |

## Run it again

Needs Rust (the workspace pins nothing newer than the repository's), CMake and Clang for the two
BoringSSL builds, `openssl` for the certificate, and Chrome for question (c).

```sh
cd spike/http3
cargo build                      # builds both packages; two BoringSSL builds, slow the first time
cargo test -p http3-spike-tools  # the wire parsers, including the RFC 9001 A.2 test vector
scripts/mkcert.sh                # certs/{key.pem,cert.pem,cert.der,spki.b64}

# (a) both TLS stacks in one process
scripts/serve-and-run.sh clash scripts/client-clash.sh

# (b) phantom over HTTP/3 against the local server
scripts/serve-and-run.sh phantom-h3 scripts/client-phantom.sh h3

# (c) the same server, headless Chrome, then compare out/phantom-h3 with out/chrome
scripts/serve-and-run.sh chrome scripts/client-chrome.sh
```

`serve-and-run.sh <label> <command...>` starts `h3srv` (TCP and UDP) and `relay`, runs the
command, stops them, and leaves `out/<label>/h3srv.jsonl` and `out/<label>/relay.jsonl`.
`scripts/symbols.sh` compares the two BoringSSL static libraries and the final `clash` binary.
Set `CHROME=/path/to/chrome` to use another Chrome.
