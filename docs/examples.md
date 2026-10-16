# Examples

Every example is a file under `crates/<crate>/examples/`. Run them from the workspace root with
`cargo run -p <package> --example <name>`. None of them reaches the outside network: the sites
they talk to are started on `127.0.0.1` by the example itself.

Only Chrome 155 on macOS is measured. An example that needs a measured system stops at once with
the `NotMeasured` error anywhere else; it never borrows the values of another system.

## seleniumbase-rs-scrape

| Example | What it shows | Needs |
|---|---|---|
| `cookie_jar` | The RFC 6265 cookie jar: storing `Set-Cookie` headers, the ones it refuses and why, and the `Cookie` header it would send. | Nothing: no browser, no network, any system. |
| `browser_scraper` | A `BrowserScraper` against a loopback page behind a script challenge: detection, solving and the reload. | A Chrome on this machine (run headless). Any system. |
| `capture_tls` | Measures the TLS `ClientHello` of the local Chrome and writes it to `crates/codec/src/measured`, with the raw bytes in `crates/codec/tests/fixtures/measured`. | The local Chrome. Writes into the repository; `--help` lists the options (`--out`, `--fixtures`, ...). |
| `capture_hints` | Measures the client hints, `User-Agent` and `Accept-Language` of the local Chrome into the same file. `--snippet` prints a console script for a browser the tool cannot drive. | The local Chrome. Writes into the repository. |

```sh
cargo run -p seleniumbase-rs-scrape --example cookie_jar
cargo run -p seleniumbase-rs-scrape --example browser_scraper
cargo run -p seleniumbase-rs-scrape --example capture_tls -- --help
cargo run -p seleniumbase-rs-scrape --example capture_hints -- --snippet
```

## seleniumbase-rs-mimic

These build BoringSSL through `btls`, so they need `cmake` and a C/C++ toolchain, and about 1 GB
of target directory on the first build.

| Example | What it shows | Needs |
|---|---|---|
| `custom_profile` | A client from the measured Chrome 155 emulation, one part overridden with `CustomProfileBuilder`, and what the stack accepts and refuses. Opens no socket. | A measured system. |
| `http_session` | An `HttpSession` against a loopback TLS shop: plain HTTP with the measured Chrome handshake and headers, and the cookie jar following the shop. | A measured system. No browser. |
| `stealth_session` | A `StealthSession` moving between plain HTTP and a real Chrome to pass a script challenge on a loopback shop. | A measured system and that Chrome installed, with the same major version as the measurement (155). |
| `capture_h2` | Measures the HTTP/2 connection start and the request headers of the local Chrome and adds them to its measured file. Run `capture_tls` and `capture_hints` first. | The local Chrome (headed). Writes into the repository. |

```sh
cargo run -p seleniumbase-rs-mimic --example custom_profile
cargo run -p seleniumbase-rs-mimic --example http_session
cargo run -p seleniumbase-rs-mimic --example stealth_session
cargo run -p seleniumbase-rs-mimic --example capture_h2 -- --help
```

Only one Chrome-driving program should run on the machine at a time.

## Re-measuring when Chrome updates

Run `capture_tls`, then `capture_hints`, then `capture_h2` against the new Chrome. They rewrite
`crates/codec/src/measured/<browser>-<major>-<os>.json` and the raw fixtures next to the wire
tests; `cargo test -p seleniumbase-rs-codec --test wire_emulation` and
`cargo test -p seleniumbase-rs-mimic --test impersonate_capture_h2` then check the files
against each other. The full procedure is in `docs/tutorials/stealth_scraping.md` of
[seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs).

## Tests that start a real Chrome

They are the `*_chrome.rs` files and are `#[ignore]`d. Run them one binary at a time, for example:

```sh
cargo test -p seleniumbase-rs-scrape --test scrape_browser_chrome -- --ignored --test-threads=1
cargo test -p seleniumbase-rs-mimic --test stealth_session_chrome -- --ignored --test-threads=1
```
