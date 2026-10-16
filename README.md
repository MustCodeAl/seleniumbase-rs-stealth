# seleniumbase-rs-stealth

The experimental stealth layer of [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs),
the Rust port of SeleniumBase, as three crates of their own:

| Crate | What it is |
|---|---|
| `seleniumbase-rs-codec` (`crates/codec`) | Pure wire formats, no I/O: a TLS `ClientHello` parser, JA3, JA4 and peetprint, the Akamai HTTP/2 fingerprint, HTTP/2 frame and HPACK reading, the TLS, HTTP/2 and header specs, and the measured browser table (`emulation`) embedded from `src/measured/*.json`. |
| `seleniumbase-rs-scrape` (`crates/scrape`) | Stealth scraping without TLS spoofing, over the pure CDP engine of seleniumbase-rs. Pure modules for challenge detection and the mitigation policy, egress (proxy pool, geo locale, URL redaction), an RFC 6265 cookie jar, the per-site ledger and its events, and the stealth identity with its session policy; the `runtime` module holds the I/O: `BrowserScraper`, `EgressSwitch`, `JsonStateStore`, the identity file, the tracing sink and the capture tools. Pure Rust. |
| `seleniumbase-rs-mimic` (`crates/mimic`) | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile` and `CustomProfileBuilder`, `HttpSession` (plain HTTP that continues a browser's session), `MitmProxy` (a loopback proxy that gives a real Chrome the measured TLS), `StealthSession` (moves between plain HTTP and Chrome), `EphemeralCa`, and the HTTP/2 capture tool. Builds BoringSSL, so it needs `cmake` and a C/C++ toolchain. |

`mimic` depends on `scrape`, which depends on `codec`; all three depend on `seleniumbase-rs`.

## Status

**Experimental.** The types and paths may change between commits; nothing here is published to
crates.io.

**Measured only for Chrome 155 on macOS.** The wire tables hold one entry. On any other system,
or against another Chrome major version, the impersonation refuses to run (`NotMeasured`,
`VersionMismatch`) instead of borrowing the values of another system. `codec` and `scrape` work
everywhere; `scrape` does no TLS spoofing.

Everything was tested against local servers and a local Chrome, never against live services.

## Using the crates

Depend on them by git:

```toml
[dependencies]
seleniumbase-rs = { git = "https://github.com/MustCodeAl/seleniumbase-rs" }
seleniumbase-rs-scrape = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
# Only if you need browser-identical TLS (builds BoringSSL):
seleniumbase-rs-mimic = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

Cargo finds each package inside the repository by its name. These crates take `seleniumbase-rs`
by its plain git URL, so write it exactly as above (no `branch`, `tag` or `rev`) when your code
uses its types (`sb_cdp::Browser`, `Fingerprint`, ...): the same source spelling makes Cargo
resolve both to one commit, recorded in your `Cargo.lock`. A different spelling, such as a
`tag`, would make them two different crates to Cargo and their types would not match.

These crates need some core items to be public (in `seleniumbase_rs::sb_cdp` and
`seleniumbase_rs::stealth::randomize`). Until that change is on the `main` branch of
seleniumbase-rs, build against a local checkout of it, as below.

## Building this workspace

The workspace builds against a checkout of seleniumbase-rs next to it: the `[patch]` section of
`Cargo.toml` points the GitHub dependency at `../seleniumbase-rs`.

```sh
git clone https://github.com/MustCodeAl/seleniumbase-rs
git clone https://github.com/MustCodeAl/seleniumbase-rs-stealth
cd seleniumbase-rs-stealth

cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The first build of `mimic` compiles BoringSSL (about 1 GB of target directory). Tests that
start a real Chrome are the `*_chrome.rs` files and are `#[ignore]`d; run them one binary at a time
with `-- --ignored --test-threads=1`.

[docs/examples.md](docs/examples.md) lists the examples, what each needs, and how to run them.

## Related

- [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs): the core crate, its CDP engine
  and the stealth tutorial, now in this repository: [docs/tutorial.md](docs/tutorial.md).
- [SeleniumBase](https://github.com/seleniumbase/SeleniumBase): the Python original.

## License

MIT, see [LICENSE](LICENSE).
