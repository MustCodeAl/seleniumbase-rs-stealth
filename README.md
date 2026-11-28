# seleniumbase-rs-stealth

The layered stealth module for [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs). Import it when core's built-in undetected mode (`undetected::patcher`, `sb_cdp::human`) is not enough and you need coherent browser identity, multi-surface JS/CDP masking, wire-level TLS/HTTP2/HTTP3 impersonation, or resilient anti-bot sessions.

Documentation: <https://mustcodeal-seleniumbase-rs.docs7.io>. Start with
[Stealth](https://mustcodeal-seleniumbase-rs.docs7.io/stealth/introduction),
[Advanced stealth](https://mustcodeal-seleniumbase-rs.docs7.io/stealth/advanced/overview), and
[Stealthy scraping](https://mustcodeal-seleniumbase-rs.docs7.io/scraping/stealthy/overview).

> [!IMPORTANT]
> User docs live on the core site under `docs/stealth` (and `docs/scraping`) in
> [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs): Docs7 builds only that
> repository's `docs/`. This repository's `docs/` holds design and contributor notes.

## Four independent layers

`seleniumbase-rs-stealth` is a single library crate organized into four strictly separated layers (plus one standalone `h3d/` HTTP/3 helper binary). Every layer ships with sensible out-of-the-box defaults and opens up full deep controls underneath, so you opt into only the layer you need:

| Layer | Feature | Zero BoringSSL? | What it owns | Good default |
|---|---|---|---|---|
| **1. `identity`** | Always on (`--no-default-features`) | Yes | Pure persona & fingerprint modeling: unified `fingerprint` module (`Fingerprint`, `ProfileFingerprint`, `FingerprintGenerator`, `FingerprintOptions`, `BayesianSampler`, `HeaderGenerator`, `FingerprintConverter`, `audit_coherence`, `compare`, `consistency`, `file`, `schema`), `Spoof<T>`, `StealthPolicy`, `mimic` specimens, `profile_payloads`, `capture` probes | `Fingerprint::randomized(OsType::Macos, 42)` / `BayesianSampler::sample(&SampleConstraints::default())?` |
| **2. `client`** | `client` (in default) | Yes | Client-side browser masking: `StealthLayer` (`Default`, `balanced`, `strict`, `minimal`), 42-surface `StealthScriptBundle` / `StealthScriptOptions`, 32 modular JS `EvasionProvider` passes, `StealthPlan` CDP overrides, launch/page extension traits (`LaunchOptionsFingerprintExt`, `BrowserConfigFingerprintExt`, `PageFingerprintExt`, `BaseCaseFingerprintExt`), core Layer-0 re-exports | `LaunchOptions::default().with_stealth(&StealthLayer::default())?` |
| **3. `network`** | `network-spec` *(pure)* / `network` + `http3` *(BoringSSL)* | `network-spec`: Yes; `network`/`http3`: BoringSSL | Pure `spec` (TLS `ClientHello`, JA3/JA4/peetprint, Akamai H2, H3, headers, measured browser tables) and `proxy` (`ProxyPool`, `ProxyRoute`, `GeoResolver`, redaction, `ProxySwitch`); opt-in BoringSSL `wire` (`WireFormat`), `mitm` (`MitmProxy`), `tls_server` (`EphemeralCa`), wire `capture`, and unified `http3` (`Http3Client`, `Http3Pool`, `capture`) | `WireFormat::random(...)`, `MitmProxy::start(MitmOptions::default()).await?`, `Http3Client::start(&Http3Options::new())?` |
| **4. `session`** | `session` (**default**) | Yes (CDP); BoringSSL only if `network` enabled | Resilient multi-request workflows: `CdpSession` (CDP-only, default), `HttpSession` (wire HTTP/2+3, with `network`), `HybridSession` (dual-mode HTTP and real Chrome behind `MitmProxy`, with `network`), plus `challenge` detection & `MitigationPolicy`, `CookieJar`, `Ledger`, `Passport`, `SessionError`, `SessionEvent` | `CdpSession::builder().build().await?`, `HttpSession::new(...)`, `HybridSession::builder().build()?` |
| **`sbrs-h3d`** (`h3d/`) | Standalone binary | `phantom-http` BoringSSL | Out-of-process HTTP/3 helper binary (`cargo install --path h3d`). Kept as a separate process strictly because `wreq`'s `btls-sys` and `phantom-http`'s `phantom-btls-sys` export 6,047 colliding unprefixed BoringSSL C symbols (`docs/design/http3.md`). | `Http3Client::start(&Http3Options::new())?` |

## Choosing only the layer you need

```toml
# Layer 1 only: pure fingerprint synthesis & coherence auditing (zero BoringSSL):
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false }

# Layers 1 + 2: fingerprints + in-browser JS/CDP masking on your own Page/LaunchOptions (zero BoringSSL):
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", default-features = false, features = ["client"] }

# Layers 1-4 default: includes CdpSession, challenge handling, proxy pool, cookie jar, ledger (still zero BoringSSL):
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }

# Full stack + TLS/HTTP2 impersonation, MitmProxy, HttpSession, HybridSession (needs cmake + C/C++):
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["network"] }

# Full stack + HTTP/3 client/pool & Alt-Svc upgrade (also run `cargo install --path h3d`):
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["http3"] }
```

`seleniumbase-rs-stealth` re-exports all of `seleniumbase-rs` (`sb_cdp`, `BaseCase`, `BrowserConfig`, `undetected`, etc.), so downstream projects need only one dependency.

## Quick examples

### Layer 2 (`client`): 2-line browser masking with good defaults
```rust,no_run
use seleniumbase_rs_stealth::sb_cdp::{Browser, LaunchOptions};
use seleniumbase_rs_stealth::client::{LaunchOptionsFingerprintExt, StealthLayer};

# async fn demo() -> seleniumbase_rs_stealth::Result<()> {
let layer = StealthLayer::default();
let options = LaunchOptions::default().with_stealth(&layer)?;
let browser = Browser::launch(options).await?;
let page = browser.default_page().await?;
page.goto("https://example.com").await?;
browser.close().await?;
# Ok(())
# }
```

### Layer 4 (`session`): `CdpSession`, `HttpSession`, and `HybridSession`
```rust,ignore
use seleniumbase_rs_stealth::session::{CdpSession, HttpSession, HybridSession};
use seleniumbase_rs_stealth::network::{MitmProxy, WireFormat};
```

## Status & honesty notes

- **Experimental:** types and paths may evolve between releases; nothing here is published to crates.io.
- **Wire measurements:** embedded wire tables currently measure Chrome 155 on macOS; unmeasured OS/version pairs return typed `NotMeasured` / `VersionMismatch` errors rather than silently borrowing values from another OS.
- **HTTP/3:** `h3d/` pins `phantom-http = "=0.6.0"` (`chrome-156-macos` default). See `docs/http3.md` and `docs/design/http3.md` for measured matches and known QUIC Initial / control-stream differences.
- **Code quality:** `#![forbid(unsafe_code)]`, `thiserror` error enums, pure domain purity enforced by `tests/session_domain_purity.rs`.
- **Testing:** everything is verified against local servers, mocked CDP, Node V8, and local Chrome, never against live third-party services.

## Building & testing

```sh
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy --no-default-features --all-targets -- -D warnings
cargo clippy --no-default-features --features client --all-targets -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo clippy --features "http3 test-util test-fixtures" --all-targets -- -D warnings
cargo test --features "http3 test-util test-fixtures"
```

To build against a local checkout of `seleniumbase-rs`, add an uncommitted `.cargo/config.toml`:

```toml
[patch."https://github.com/MustCodeAl/seleniumbase-rs"]
seleniumbase-rs = { path = "../seleniumbase-rs" }
```

Chrome tests (`*_chrome.rs`) are `#[ignore]`d; run one binary at a time with `-- --ignored --test-threads=1`.

## License

MIT, see [LICENSE](LICENSE).
