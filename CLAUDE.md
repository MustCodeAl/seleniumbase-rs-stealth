# seleniumbase-rs-stealth: knowledge map

A short map of the repository for a coding assistant. Keep it correct; fix a line here when the
code stops matching it. `README.md` is the overview for people.

## What this is

The layered stealth module of [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs)
(the core), which it depends on by git and uses only through its public API. Nothing is
published to crates.io.

Single library crate (`seleniumbase-rs-stealth`) organized into **4 independent layers** plus one
standalone HTTP/3 helper binary (`h3d/`):

| Layer / Package | Path | Feature | Role |
|---|---|---|---|
| **1. `identity`** | `src/identity/` | always on (`--no-default-features`) | Pure fingerprint & persona synthesis (`fingerprint/`), `Spoof`, `StealthPolicy`, `mimic` specimens, `profile_payloads`, `capture` probes, JSON Schema (`schemas/fingerprint.schema.json`). Zero BoringSSL. |
| **2. `client`** | `src/client/` | `client` (in default) | Client-side browser masking: `StealthLayer` (`Default`/`balanced`/`strict`/`minimal`), 42-surface `StealthScriptBundle` / `StealthScriptOptions`, 32 JS `EvasionProvider`s, `StealthPlan` CDP overrides, engine extension traits (`LaunchOptions`, `BrowserConfig`, `Page`, `BaseCase`), core Layer-0 re-exports. Zero BoringSSL. |
| **3. `network`** | `src/network/` | `network-spec` (zero BoringSSL) / `network` + `http3` | Pure wire `spec` (TLS/JA3/JA4/H2/H3/headers + `measured/*.json`) and `proxy` pools/geo/redaction; opt-in BoringSSL `wire` (`WireFormat`), `mitm` (`MitmProxy`), `tls_server` (`EphemeralCa`), `capture`, and unified `http3` (`Http3Client`, `Http3Pool`, `capture`). |
| **4. `session`** | `src/session/` | `session` (**default**) | Resilient multi-request workflows: `CdpSession` (zero BoringSSL), plus `HttpSession` and dual-mode `HybridSession` (with `network`), backed by pure `challenge`, `jar`, `ledger`, `passport`, `SessionError`, `SessionEvent`, and `io` stores. |
| **`sbrs-h3d`** | `h3d/` | standalone binary | Out-of-process HTTP/3 helper (`phantom-http = "=0.6.0"`). Kept standalone only because `phantom-btls-sys` and `btls-sys` collide on 6,047 BoringSSL C symbols. |

## Docs

- User-facing stealth docs live on the core Docs7 site under `docs/stealth` and `docs/scraping` in
  the `seleniumbase-rs` repository (`docs/docs.json`).
- This repository's `docs/` holds design and contributor notes (`docs/design/`,
  `docs/contributing/`).

## Build and check

```bash
export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0
cargo fmt --all -- --check
cargo clippy --no-default-features --all-targets -- -D warnings
cargo clippy --no-default-features --features client --all-targets -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo clippy --features "http3 test-util test-fixtures" --all-targets -- -D warnings
cargo test --features "http3 test-util test-fixtures"
```

- Default (`session`), `--no-default-features` (`identity`), and `--no-default-features --features client` build **no BoringSSL**.
- `--features network` or `--features http3` builds BoringSSL (`cmake`, C/C++ toolchain; see `docs/contributing/boringssl.md` and `scripts/boringssl-prebuilt.sh`).
- Chrome integration tests (`*_chrome.rs`) are `#[ignore]`d; run one binary at a time with `-- --ignored --test-threads=1`.
- For `h3d/`: `cd h3d && cargo fmt --all -- --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- To build against a local core checkout, patch the core's git URL in an uncommitted `.cargo/config.toml`.
