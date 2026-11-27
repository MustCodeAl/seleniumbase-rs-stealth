# Examples

> [!IMPORTANT]
> This page is not on the docs site. Its user-facing copy is
> [Runnable examples](https://mustcodeal-seleniumbase-rs.docs7.io/scraping/advanced-stealthy/examples)
> on the core site (`docs/scraping/advanced-stealthy/examples.md` in the seleniumbase-rs
> repository); a change users should read goes there.

Every example is a file under `examples/`. Run them from the repository root with
`cargo run --example <name>` (or `--features network --example <name>` for BoringSSL wire
examples). None of them reaches the outside network: the sites they talk to are started on
`127.0.0.1` by the example itself.

Only Chrome 155 on macOS is measured. An example that needs a measured system stops at once with
the `NotMeasured` error anywhere else; it never borrows the values of another system.

## Default feature (`session`, zero BoringSSL)

| Example | What it shows | Needs |
|---|---|---|
| `cookie_jar` | The RFC 6265 cookie jar: storing `Set-Cookie` headers, the ones it refuses and why, and the `Cookie` header it would send. | Nothing: no browser, no network, any system. |
| `cdp_session` | A `session::CdpSession` against a loopback page behind a script challenge: detection, solving and the reload. | A Chrome on this machine (run headless). Any system. |
| `capture_tls` | Measures the TLS `ClientHello` of the local Chrome and writes it to `src/network/spec/measured`, with the raw bytes in `tests/fixtures/measured`. | The local Chrome. Writes into the repository; `--help` lists the options (`--out`, `--fixtures`, ...). |
| `capture_hints` | Measures the client hints, `User-Agent` and `Accept-Language` of the local Chrome into the same file. `--snippet` prints a console script for a browser the tool cannot drive. | The local Chrome. Writes into the repository. |

```sh
cargo run --example cookie_jar
cargo run --example cdp_session
cargo run --example capture_tls -- --help
cargo run --example capture_hints -- --snippet
```

## Network feature (`--features network`, BoringSSL / wreq)

These build BoringSSL through `btls`, so they need `cmake` and a C/C++ toolchain, and about 1 GB
of target directory on the first build.

| Example | What it shows | Needs |
|---|---|---|
| `custom_profile` | A client from the measured Chrome 155 emulation, one part overridden with `WireFormatBuilder`, and what the stack accepts and refuses. Opens no socket. | A measured system. |
| `http_session` | A `session::HttpSession` against a loopback TLS shop: plain HTTP with the measured Chrome handshake and headers, and the cookie jar following the shop. | A measured system. No browser. |
| `hybrid_session` | A `session::HybridSession` moving between plain HTTP and a real Chrome to pass a script challenge on a loopback shop. | A measured system and that Chrome installed, with the same major version as the measurement (155). |
| `capture_h2` | Measures the HTTP/2 connection start and the request headers of the local Chrome and adds them to its measured file. Run `capture_tls` and `capture_hints` first. | The local Chrome (headed). Writes into the repository. |

```sh
cargo run --features network --example custom_profile
cargo run --features network --example http_session
cargo run --features network --example hybrid_session
cargo run --features network --example capture_h2 -- --help
```

Only one Chrome-driving program should run on the machine at a time.

## Re-measuring when Chrome updates

Run `capture_tls`, then `capture_hints`, then `capture_h2` against the new Chrome. They rewrite
`src/network/spec/measured/<browser>-<major>-<os>.json` and the raw fixtures under
`tests/fixtures/measured/`; `cargo test --features "network-spec test-fixtures" --test wire_measurement`
and `cargo test --features "network test-fixtures" --test impersonate_capture_h2` then check the
files against each other. The full procedure is in [docs/tutorial.md](tutorial.md).

## Tests that start a real Chrome

They are the `*_chrome.rs` files under `tests/` and are `#[ignore]`d. Run them one binary at a
time, for example:

```sh
cargo test --test cdp_session_chrome -- --ignored --test-threads=1
cargo test --features network --test hybrid_session_chrome -- --ignored --test-threads=1
```
