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
| `client/src/bin/clash.rs` | Question (a): one binary that links phantom and the stealth crates (wreq over btls 0.5.6) and calls both TLS stacks alternately, against the local server. **It does not link on macOS arm64** (duplicate BoringSSL symbols); `build.sh` leaves it out. |
| `client/src/bin/wreqonly.rs` | Control for (a): the stealth crates alone, one request. Links and runs. |
| `client/src/bin/h3fetch.rs` | Question (b): phantom requests over an exact protocol (`h1`, `h2`, `h3`) or negotiated (`auto`, with `--alt-svc` and `--repeat`); prints the protocol phantom reports for each. |
| `tools/src/bin/h3srv.rs` | Local server: HTTP/3 over QUIC (quinn + rustls, no h3 library) and HTTP/1.1 over TLS. Logs the HTTP/3 control stream (SETTINGS in order), QPACK streams and the request HEADERS as they arrived. |
| `tools/src/bin/relay.rs` | UDP relay in front of the server. Decrypts the client's QUIC Initial packet (RFC 9001 section 5) and prints the TLS ClientHello with the QUIC transport parameters in wire order. |
| `tools/src/initial.rs`, `h3wire.rs` | The parsers behind them; unit tests include the RFC 9001 appendix A.2 client Initial. |
| `scripts/` | `build.sh`, `mkcert.sh`, `serve-and-run.sh`, the three `client-*.sh`, `summarize.sh`, `samples.sh`, `claims.sh`, `symbols.sh`, `link-experiments.sh`. |
| `results/` | Small captures quoted in `docs/design/http3.md`. `claims-14-captures.txt` is not a capture: `scripts/claims.sh` computed it from the raw logs of 14 captures in `out/`. |
| `certs/`, `out/`, `target/` | Gitignored: certificate and key, raw logs, build output. |

## Run it again

Needs Rust (the workspace pins nothing newer than the repository's), CMake and Clang for the two
BoringSSL builds, `openssl` for the certificate, and Chrome for question (c).

```sh
cd spike/http3
scripts/build.sh                 # every binary that links, copied to bin/ (two BoringSSL builds: slow once)
cargo test -p http3-spike-tools  # the wire parsers, including the RFC 9001 A.2 test vector
scripts/mkcert.sh                # certs/{key.pem,cert.pem,cert.der,spki.b64}

# (a) both TLS stacks in one process: this does NOT link on macOS arm64; the script shows how
scripts/link-experiments.sh      # builds `clash` three ways, runs symbols.sh, runs any binary that links
CLASH_BIN=wreqonly scripts/serve-and-run.sh wreqonly scripts/client-clash.sh   # the stealth stack alone

# (b) phantom over HTTP/3 against the local server
scripts/serve-and-run.sh phantom-h3 scripts/client-phantom.sh h3

# (c) the same server, headless Chrome, then compare
scripts/serve-and-run.sh chrome scripts/client-chrome.sh
scripts/summarize.sh out/chrome ; scripts/summarize.sh out/phantom-h3
scripts/samples.sh 5             # 5 runs of each (order and GREASE are random), one block per run
scripts/claims.sh > results/claims-14-captures.txt   # the "equal in every run" numbers of the note, from out/ of the 14 captures (needs jq)

# Alt-Svc, and UDP silently dropped (H3_RELAY_TO points the relay at a port nobody serves)
H3SRV_ARGS="--alt-svc" scripts/serve-and-run.sh altsvc-open scripts/client-phantom.sh auto --alt-svc --repeat 3
H3_RELAY_TO=4449 H3SRV_ARGS="--alt-svc" scripts/serve-and-run.sh altsvc-blocked scripts/client-phantom.sh auto --alt-svc --repeat 3

# a server whose QPACK dynamic table is allowed (the labels chrome-qpack and phantom-qpack are what claims.sh reads)
H3SRV_ARGS="--qpack-capacity 4096 --qpack-blocked 100" scripts/serve-and-run.sh chrome-qpack scripts/client-chrome.sh
H3SRV_ARGS="--qpack-capacity 4096 --qpack-blocked 100" scripts/serve-and-run.sh phantom-qpack scripts/client-phantom.sh h3
```

`serve-and-run.sh <label> <command...>` starts `h3srv` (TCP and UDP) and `relay`, runs the
command, stops them, and leaves `out/<label>/h3srv.jsonl` and `out/<label>/relay.jsonl`
(`out/` is gitignored; the captures and numbers quoted by the design note are in `results/`;
the single captures there, `chrome-156-single.txt` and `phantom-h3-single.txt`, are `summarize.sh` over `out/chrome` and `out/phantom-h3`).
Set `CHROME=/path/to/chrome` to use another Chrome. `scripts/symbols.sh` compares the two BoringSSL
static libraries and needs the `target/` of a finished build.

Builds go through whatever your `cargo` is. With mr-boxington managing `target/` and a disk under its
free-space minimum, it deleted `target/` after some builds while this spike was written; that is why
`build.sh` copies the binaries to `bin/` and why `link-experiments.sh` runs `symbols.sh` right after the
first link.

Keep every request on loopback: `h3srv` and `relay` refuse any other address, the clients only dial
`localhost`, and Chrome is told to resolve every name to 127.0.0.1.
