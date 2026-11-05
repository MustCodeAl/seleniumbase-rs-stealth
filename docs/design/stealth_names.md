# Stealth names: what each public item should be called

Status: proposal (phase 1). No code changes yet; the renames start once the owner
approves this table.

The rule: take other projects' features, never their API names. Name things after
this project's own patterns, and let one name mean one thing across seleniumbase-rs
and this repository. A feature that a core module already handles moves into that
module instead of keeping a second copy here.

## How this list was made

- The public surface comes from each crate's `lib.rs` re-exports (`codec`, `scrape`,
  `mimic`) and the facade's `src/lib.rs`, read with TokenSave on `origin/main`
  (`7820793`). The facade re-exports core (`pub use seleniumbase_rs::*` and
  `evasion::*`), so a name here that equals a core name is a real clash for anyone who
  depends on the facade.
- Each name was searched in core (`seleniumbase-rs` at `6790601c`) by name and by
  concept. Rows marked **clash** are verified against a named core item. Rows marked
  **review** are a judgment (a vague or misleading name) without a clash.
- Where a name came from was not checked. The standing rule is not to research
  stealthscraper-rs, so no row claims a name was copied from it.

## codec

| Current | Proposed | Why |
|---|---|---|
| `Emulation`, `EmulationError`, `get_emulation`, `measured_emulations`, `emulation_for_kind` | `WireFormat`, `WireFormatError`, `wire_format`, `measured_wire_formats`, `wire_format_for` | **clash**: core `sb_cdp::state::Emulation` is device emulation (viewport, touch, scale). Here it is the measured TLS, HTTP/2 and header shape of one browser. The docs already call this a "wire format" (`/stealth/advanced/wire-formats`). |
| `HeaderIdentity` | `HeaderOrder` (or merge into `HeaderSpec`) | **review**: "identity" means something else in `scrape` and in core (`sb_cdp::identity`). |
| `Ja3`, `Ja4`, `Ja4Raw`, `Peetprint`, `Akamai`, `ClientHello` | keep | These are the names of the published fingerprint formats, not another library's API. |
| `TlsSpec`, `H2Spec`, `HeaderSpec`, `RequestShape`, `KnownGap`, `BrowserKind` | keep | No clash. |

## scrape

| Current | Proposed | Why |
|---|---|---|
| `StealthIdentity`, `IdentityError`, `EmulationRef` | `Identity`, `IdentityError`, `WireFormatRef` | **clash**: core's `sb_cdp::identity` module holds `StealthPlan`, so "identity" means two things. This type is the richer one (fingerprint, wire format, cookies and egress, saved between runs). Proposed: this is the identity, and core's module becomes `sb_cdp::plan`. That is a core rename (see the decisions). |
| `DomainState`, `StateStore`, `InMemoryStateStore`, `JsonStateStore` | `SiteRecord`, `Ledger`, `InMemoryLedger`, `JsonLedger` | **clash**: core `sb_cdp::state` is a page's cookies and storage. Here it is the per-site history of challenges, which the module already calls the `ledger`. |
| `Evidence` (challenge detection) | `ChallengeEvidence`, or merged (see below) | **clash**: core `sb_cdp::captcha::Evidence` is what showed a CAPTCHA was solved. |
| `Vendor`, `ChallengeKind`, `Detection`, `ResponseView` | merge `Vendor` into core `sb_cdp::captcha::Captcha`; keep the rest | **clash in concept**: core `Captcha` already names the vendors (Turnstile, reCAPTCHA, hCaptcha, Friendly Captcha, DataDome). Two vendor lists drift. |
| `Mode` (`Browser`, `Http`) | `FetchKind` | **review**: core uses "mode" for engine modes (UC mode, CDP Mode, `DriverMode`). This one says how one request is made. `Transport` is taken by core `CdpTransport`. |
| `Action` | `Mitigation` | **review**: too generic for a re-exported name. It is the next step of the module named `mitigation`. Its variant `RotateProfile` becomes `RotateIdentity`, because "profile" means a browser profile in core. |
| `MitigationPolicy`, `AttemptContext`, `GiveUpReason` | keep | No clash. |
| `ScraperEvent`, `EventSink`, `NoopEventSink`, `CollectingEventSink`, `TracingEventSink` | `ScrapeEvent`, keep the sinks | **review**: the crate and its docs say "scrape", not "scraper". The `*EventSink` names stay apart from the plugin API's `Sink`. |
| `ProxyPool`, `RotationStrategy` | move into core `config::proxy_list` (`ProxyList` with a `RotationStrategy`) | **clash in concept**: core `ProxyList` already holds a list of proxies and picks one. Rotation is a feature of that list, not a second type. |
| `Secret`, `RedactedUrl` | move into core `common` (one `Secret` for every password the project holds) | **review**: core's `sb_cdp::Proxy` hides its password with its own code, so the redaction exists twice. |
| `CookieJar` | keep the name; compare with core `SessionStore` before phase 2 | **review**: core `sb_cdp::session_store::{Session, SessionStore}` keeps cookies and storage in memory. If the jar is the same thing on disk, it belongs in that module. |
| `CountryCode`, `GeoResolver`, `StaticGeoResolver`, `LocaleExt` | keep; candidate for core `evasion` (making a fingerprint's locale fit its egress) | **review**: no clash. |
| `EgressKey`, `EgressRef`, `EgressSwitch`, `ContextProxySwitch` | keep "egress"; `ContextProxySwitch` is a candidate for core `sb_cdp::context` (`ContextOptions::proxy`) | **review**: "egress" is used consistently in this repository. |
| `BrowserScraper`, `BrowserScraperBuilder`, `BrowserFetch`, `ScrapeError` | keep | No clash. |
| `BrowserLauncher`, `ChromeLauncher` | use the launcher seam core `BrowserPool::with_launcher` has, if it is the same shape | **review**: core already has a "launcher" concept for supplying browsers. |

## mimic

| Current | Proposed | Why |
|---|---|---|
| `impersonation_client`, `ImpersonationProfile`, `ImpersonateError`, `CustomProfileBuilder` | `wire_client`, `WireFormat` (shared with codec), `WireError`, `CustomWireFormatBuilder` | **clash**: core removed its `impersonate` feature in 0.3.0, and "profile" in core is a browser profile (`profile_payloads`, the Browser profiles page). |
| `Fetched as HttpFetched`, `Attempt as HttpAttempt`, `Headers as ReplyHeaders`, `Rules as HttpRules`, `Verdict as HttpVerdict` | `HttpFetch`, `HttpAttempt`, `ReplyHeaders`, `HttpRules`, `HttpVerdict` as the real names | **review**: the aliases exist because the module names are too generic. One spelling per item. `HttpFetch` matches `BrowserFetch` and `StealthFetch`. |
| `HttpSession`, `HttpOptions`, `HttpReply`, `HttpRequest`, `HttpError`, `Method`, `ReplyProtocol`, `WireProtocol` | keep | No clash. |
| `StealthSession`, `StealthSessionBuilder`, `StealthFetch`, `StealthError` | keep | No clash. |
| `MitmProxy`, `MitmOptions`, `MitmStats`, `MitmError`, `EphemeralCa`, `CaOptions`, `Leaf`, `TlsServerError` | keep | No clash. |

## Decisions for the owner

1. **"Identity"**: should this repository's saved identity take the name `Identity`,
   with core's `sb_cdp::identity` module renamed `sb_cdp::plan`? Or should this one get
   a new name and core keep its module?
2. **The `fingerprint(...)` example**: `BrowserScraperBuilder::fingerprint` and
   `StealthSessionBuilder::fingerprint` both take core's `Fingerprint`, and
   `BrowserScraper::fingerprint()` returns `&Fingerprint`. Which call did the example
   mean? I found none that takes something else under that name.
3. **Moves into core** (`ProxyPool` → `ProxyList`, `Secret`, the challenge vendors →
   `Captcha`): in the same round as the renames, or one PR each, later?

## Phase 2 (after approval)

Renames with `tokensave rename` (a dry run first), every caller, the stealth API
reference pages and the Stealth and Scraping sections of the core docs, and the
contract files (`mise run contracts`). Breaking changes, no shims. One PR here, plus one
in core for the items that move there.
