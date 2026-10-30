# seleniumbase-rs-stealth

The experimental stealth layer of [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs),
the Rust port of SeleniumBase, packaged with a unified root facade crate (`seleniumbase-rs-stealth`) and three workspace crates of their own.

Documentation: <https://mustcodeal-seleniumbase-rs.docs7.io>. Start with the
[stealth pages](https://mustcodeal-seleniumbase-rs.docs7.io/stealth/overview) and the
[fingerprint tutorial](https://mustcodeal-seleniumbase-rs.docs7.io/tutorials/fingerprint_stealth).

| Crate | What it is |
|---|---|
| `seleniumbase-rs-stealth` (root `.`) | Unified facade crate: re-exports all of `seleniumbase-rs` (including `sb_cdp` and `evasion`) alongside `codec`, `scrape` (enabled by default), and `mimic` (with `features = ["mimic"]`) so downstream projects need only one dependency. |
| `seleniumbase-rs-codec` (`crates/codec`) | Pure wire formats, no I/O: a TLS `ClientHello` parser, JA3, JA4 and peetprint, the Akamai HTTP/2 fingerprint, HTTP/2 frame and HPACK reading, the TLS, HTTP/2 and header specs, and the measured browser table (`emulation`) embedded from `src/measured/*.json`. |
| `seleniumbase-rs-scrape` (`crates/scrape`) | Stealth scraping without TLS spoofing, over the pure CDP engine of seleniumbase-rs. Pure modules for challenge detection and the mitigation policy, egress (proxy pool, geo locale, URL redaction), an RFC 6265 cookie jar, the per-site ledger and its events, and the stealth identity with its session policy; the `runtime` module holds the I/O: `BrowserScraper`, `EgressSwitch`, `JsonStateStore`, the identity file, the tracing sink and the capture tools. No TLS spoofing and no BoringSSL. |
| `seleniumbase-rs-mimic` (`crates/mimic`) | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `ImpersonationProfile` and `CustomProfileBuilder`, `HttpSession` (plain HTTP that continues a browser's session), `MitmProxy` (a loopback proxy that gives a real Chrome the measured TLS), `StealthSession` (moves between plain HTTP and Chrome), `EphemeralCa`, and the HTTP/2 capture tool. Builds BoringSSL, so it needs `cmake` and a C/C++ toolchain. |

`mimic` depends on `scrape`, which depends on `codec`; all three depend on `seleniumbase-rs`, and the root `seleniumbase-rs-stealth` crate re-exports them all.

## Status

**Experimental.** The types and paths may change between commits; nothing here is published to
crates.io.

**Measured only for Chrome 155 on macOS.** The wire tables hold one entry. On any other system,
or against another Chrome major version, the impersonation refuses to run (`NotMeasured`,
`VersionMismatch`) instead of borrowing the values of another system. `codec` and `scrape` work
everywhere; `scrape` does no TLS spoofing.

Everything was tested against local servers and a local Chrome, never against live services.

## Features

What the stealth layer does today, in plain words. Each claim was checked
against the code. All of it is experimental, measured only for Chrome 155 on
macOS, and tested against local servers and a local Chrome, not against live
services.

### Measured fingerprints, checked by a round trip

- The capture tools (`capture_tls` and `capture_hints` in `scrape`,
  `capture_h2` in `mimic`) record the TLS `ClientHello`, the client hints,
  `User-Agent`, `Accept-Language`, the HTTP/2 connection start and the header
  lists of a local Chrome. They write
  `crates/codec/src/measured/chrome-155-macos.json` and keep the raw bytes as
  test fixtures.
- Nothing is typed by hand. `seleniumbase_rs_stealth::codec::emulation` embeds that file.
  A system that was not measured gets `NotMeasured`, and a Chrome of another
  major version gets `VersionMismatch`; no other system's values are borrowed.
- The round trip is capture, emulate, compare. The wire test builds a client from
  the profile, connects it to a loopback TLS server of the crate, reads what
  arrived with the crate's own parsers (`ClientHello`, `h2_frames`) and compares it
  with the measurement: the TLS spec equals the measured one minus two known
  gaps (`KnownGap::TrustAnchorsExtension` and `KnownGap::MlDsaSignatureAlgorithms`,
  which BoringSSL and wreq cannot send), the Akamai HTTP/2 string equals the
  measured one, and the headers arrive in the measured order and spelling. The
  committed HTTP/2 spec is also derived again from the stored evidence and the
  raw frame fixtures.

### TLS, JA4 and HTTP/2 emulation through a MITM proxy

`MitmProxy::start` binds a proxy on `127.0.0.1` for a real Chrome. It ends
Chrome's TLS with a leaf certificate of an in-memory CA (`EphemeralCa`),
which Chrome trusts through `--ignore-certificate-errors-spki-list` with the
leaf key pin (never `--ignore-certificate-errors`; the CA is not installed
anywhere). It then replays each request through the impersonating wreq client,
so the site sees the measured `ClientHello` (and so the measured JA4),
the measured HTTP/2 SETTINGS and their order, the connection window, the PRIORITY
frames, the HEADERS priority, the pseudo-header order, and the header order and
spelling, plus Chrome's `priority` header from the measured table. Bodies are
streamed both ways within size limits. A `wss://` or `ws://` WebSocket handshake
is relayed with the measured WebSocket `ClientHello`. By default the proxy
refuses loopback, private and cloud-metadata destinations. Chrome's own TLS
handshake ends at the proxy, so the proxy is the one the site sees.

### A plain HTTP client without a browser

`impersonation_client(&profile)` takes an `ImpersonationProfile` and returns a
`wreq::ClientBuilder` (or an `ImpersonateError`) with the measured TLS and HTTP/2
options and header order set; you add proxies, a trust store and timeouts, then
build it. `ImpersonationProfile::apply` writes the identity headers of one
request. `CustomProfileBuilder` layers your own JA3, `JA4_r` or `JA4_ro`,
peetprint, Akamai string and header set over a measured base, and refuses what
the stack cannot send before a client exists. On top of it, `HttpSession` sends
requests with that client, keeps an RFC 6265 cookie jar (with the public suffix
list), follows redirects by hand, enforces limits (time, body size, redirects,
requests per window and site) and detects challenges on every reply. It does not
run JavaScript or solve challenges. A request body is held in memory; the reply
is read as a stream and capped.

### Dual-mode sessions

`StealthSession` holds one `StealthIdentity` (the fingerprint with its locale,
the measured browser, the cookies, the client hints, and the `egress` they were
earned through) and works in one of two modes. In HTTP mode an `HttpSession`
sends the requests. In browser mode a real Chrome sends them behind the
`MitmProxy`, so both modes show the same handshake and a clearance earned in one
is valid in the other. When a challenge appears in HTTP mode the session
escalates to the browser, the browser solves it, and the cookies move into the
identity; the session then goes back to HTTP (`SessionPolicy` decides, and a
clearance about to expire sends it back to the browser). The proxy and the HTTP
client use the same proxy egress. A Chrome of another major version than the
measurement is refused before it loads a page.

### The CDP client

The pure CDP engine of seleniumbase-rs (`sb_cdp`) starts Chrome itself with
`--remote-debugging-port` (a port the system picks unless you set one) and
talks to it over one WebSocket for all tabs; it does not use a pipe. It is
async (tokio). The launch arguments contain no `--enable-automation`. For
each tab it sends `Page.enable` and `Emulation.setFocusEmulationEnabled`;
`Network` and `Fetch` are switched on only for features that need them (ad
blocking, proxy authentication, request interception, HAR, a fingerprint). It
never switches on `Runtime`, `DOM`, `Log`, `Debugger` or `Profiler`, because a
page can feel that traffic; a test in seleniumbase-rs
(`tests/sb_cdp_domain_hygiene.rs`) fails if a new call does. Elements are found by
running `document.querySelector` through `Runtime.evaluate`. The `scrape` crate
adds only `Network`.

### Stealth overrides in the page

A set of built-in evasion providers (`seleniumbase_rs::evasion::providers`)
writes one script that runs before any page script
(`Page.addScriptToEvaluateOnNewDocument`), applied by `Page::apply_fingerprint`
or `LaunchOptionsBuilder::fingerprint`. Replaced methods go on their prototype
and replaced getters on the prototype that owns them, so
`Object.getOwnPropertyNames` is unchanged. `Function.prototype.toString` is
masked (`NativeToStringProvider`) so patched functions report `[native code]`.
Canvas noise (`toDataURL`, `toBlob`, `getImageData`) and audio noise
(`AudioBuffer.getChannelData`, `copyFromChannel`,
`AnalyserNode.getFloatFrequencyData`) are a function of the fingerprint seed and
the position, so reading the same canvas twice gives the same bytes. Other
providers cover WebGL, navigator, plugins, permissions, WebRTC, time zone and
locale, geolocation and more.

### Challenge detection and mitigation

`challenge::detect` reads a response and returns a `Detection` with a
`ChallengeKind` (`None`, `JsChallenge`, `Interactive`, `Blocked`,
`RateLimited`) and evidence naming the `Vendor` (Cloudflare, DataDome,
PerimeterX, Akamai, Imperva, Kasada, AWS WAF). A header, a widget or a bare
403 is evidence only; the verdict needs an interstitial marker.
`MitigationPolicy::decide` is a pure function that turns a detection and the
attempt context into one `Action`: `Proceed`, `Retry`, `Escalate`, `Solve`,
`RotateEgress`, `RotateProfile`, `Cooldown` or `GiveUp`. The defaults are
3 attempts per URL, a new egress after 2 challenges in a row, a new profile
after 3, a 900 second cooldown after a block, 60 seconds after a rate limit
without `Retry-After`, and a `Retry-After` capped at 3600 seconds, with
`RetryPolicy::network()` for backoff. `BrowserScraper::solve_challenge` is the
solving step: for a challenge with a widget it moves the pointer onto the page
and calls `Page::solve_captcha` (a click or a drag on Turnstile, reCAPTCHA,
hCaptcha, Friendly Captcha or the DataDome slider; the site decides whether it
is accepted), then waits for the vendor's clearance cookie or for the page to
stop being a challenge, bounded by a timeout. A script challenge is only waited
for. There is no connection to a solving service.

### Proxy pool, rotation and locale from the proxy's country

`ProxyPool` holds `(proxy, country)` pairs and picks by `RotationStrategy`
(`RoundRobin`, `Random`, `LeastFailures`, `StickyPerSite`), quarantining a proxy
after repeated failures. Changing egress opens a new browser context behind the
new proxy (`ContextProxySwitch`) and drops the old one with its cookies, so the
browser is not relaunched. That holds when the new proxy is in a country whose
language and time zone the profile already presents; for another country the
browser is started again, because the fingerprint is written into every
document before its scripts run. The country comes from the proxy spec, and
`StaticGeoResolver` maps it through a built-in table of locales (no network
lookup): language list, `Accept-Language`, time zone and a position near the city,
derived from the fingerprint seed. A built `Fingerprint` is never edited (the core's
fingerprint API v2): `Locale::localize` and `EgressLocale::localize` return the next
one, and a fingerprint that denies geolocation keeps denying it.

### Profile and identity rotation

`BrowserScraper::rotate_profile` draws a new fingerprint from a derived seed for
the same operating system, in the locale of the current egress, and restarts the
browser with it (cookies start empty; what is known of each site stays).
`export_identity` and `import_identity` move a `StealthIdentity` out of and into
a scraper; an identity is only accepted through the egress it names, and for
the same Chrome version. The identity file is written atomically with mode
`0600` and is refused on read if others can access it.

### Session state behind a store trait

Per-site state (successes, challenges, cooldowns) is saved through the
`StateStore` trait. Two implementations ship: `InMemoryStateStore` and
`JsonStateStore`, one file written to a temporary sibling, synced and renamed
over the target, with owner-only permissions. There is no other backend. The
JSON store keeps the whole table in memory and rewrites the file on each
change, so it suits a few thousand sites at most, and one process may write a
file at a time.

### Observability

The scraper reports typed events (`ScraperEvent`: request, challenge detected
and solved, action chosen, mode changed, egress and profile rotated,
clearance expiring, cooldown started, identity exported and restored) to an
`EventSink`. URLs and proxies are redacted by construction. Three sinks ship:
`NoopEventSink`, `CollectingEventSink` and `TracingEventSink`, which writes to
the `tracing` crate (not `log`). The proxy also keeps counters (`MitmStats`).

### Human-like input

`Page::human` (in seleniumbase-rs, not in these crates) plays a `Humanizer`
plan on a page: the pointer follows a curved, cubic Bézier stroke with a
bell-shaped speed profile (minimum jerk), a movement time from Fitts's law, a
faint hand tremor, and sometimes an overshoot and correction. The click lands
at a point normally distributed inside the element. Typing has Gaussian
intervals around a words-per-minute rate, a slowly drifting rhythm, longer
pauses after spaces and punctuation, rare hesitations and, if you ask for it,
typos that are noticed and corrected. Scrolling uses wheel notches eased at
both ends. With a seed the plans are reproducible. `BrowserScraper` uses it only
for a short pointer placement and scroll before it attempts a widget.
Pointer drift while idle is not built.

### Code quality

All crates declare `#![forbid(unsafe_code)]`, and their error types are
`thiserror` enums.

### TLS stack and licences

`mimic` uses wreq `=6.0.0-rc.31` (Apache-2.0) over `btls` and `tokio-btls`
`=0.5.6`, which build BoringSSL (`btls-sys`) and need `cmake` and a C/C++
toolchain. The core crate uses `reqwest` with `rustls` and `ring` for its own
HTTP (the WebDriver engine, downloads), and the CDP engine talks to Chrome over
a plain WebSocket; `codec` and `mimic` also use `ring` for hashing. The stealth
crates are MIT (workspace `Cargo.toml` and `LICENSE`); BoringSSL carries its own
licences.

## Using the unified facade

Use one Git dependency for core automation, in-page stealth and scraping:

```toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth" }
```

Default features include `codec` and `scrape`, with no BoringSSL or cmake.
For TLS/HTTP2 impersonation, use this dependency instead:

```toml
[dependencies]
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["mimic"] }
```

`mimic` builds BoringSSL and needs cmake and a C/C++ toolchain. With that
feature enabled, the combined import is:

```rust
use seleniumbase_rs_stealth::{
    fingerprint::{CanvasNoise, Fingerprint, StealthPolicy},
    scrape::BrowserScraper,
    mimic::{StealthSession, MitmProxy},
};
```

The facade also re-exports `sb_cdp`, `evasion`, `BrowserConfig`, `BaseCase` and
the rest of the core public API. Import these through `seleniumbase_rs_stealth`
to share the same core types; no separate core or implementation-crate Git
dependency is needed. Without `mimic`, omit the `mimic` imports. Core APIs behind
optional Cargo features retain their gates; this facade currently exposes the
`codec`, `scrape` and `mimic` feature switches.

Fingerprints use API v2: one `Spoof` per setting, read through accessors and
changed with `to_builder()....build()?`. The core dependency selected by Cargo
supplies these types to every enabled module.

## Building this workspace

The workspace builds against the core by its plain git URL, at the commit `Cargo.lock` records.
To build against a local checkout of `seleniumbase-rs` next to it instead, patch that source in
a `.cargo/config.toml` that you do not commit:

```toml
[patch."https://github.com/MustCodeAl/seleniumbase-rs"]
seleniumbase-rs = { path = "../seleniumbase-rs" }
```

A local core checkout is optional. Without that override, Cargo retrieves the
core from Git. The default facade can be checked without BoringSSL:

```sh
git clone https://github.com/MustCodeAl/seleniumbase-rs-stealth
cd seleniumbase-rs-stealth

cargo fmt --all -- --check
cargo clippy -p seleniumbase-rs-stealth --all-targets -- -D warnings
cargo test -p seleniumbase-rs-stealth
```

Use `--features mimic` for impersonation, or `--workspace` to check every
implementation crate; both include BoringSSL.
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
