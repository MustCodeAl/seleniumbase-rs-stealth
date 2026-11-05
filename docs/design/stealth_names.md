# Stealth names: the copied API, and every public item

Status: proposal (phase 1, third round). No code changes yet; the work starts once the
owner approves this list.

## What the stealth crate is for

In the user's words:

> the stealth crate was literally supposed to be apis to control advanced stealthy things
> that made sense with seleniumbase and bridge the features that were missing from
> seleniumbase that were in stealthscraper-rs

> stealthscraper-rs features were already implemented in seleniumbase-rs in the stealth
> crate, the problem is the implementation literally copied the apis instead of renaming
> and adapting them into seleniumbase-rs to bridge the gap of features, and thats why i was
> complaining about duplicated concepts

> I knew stealthscraper-rs had low level spoofing features and those were the main
> features integrated

So the yardstick for every item: it either controls advanced stealth the SeleniumBase way,
through core's entry points (the `sb` and `sb_cdp` fixtures, `BrowserConfig` and
`LaunchOptions`, `StealthPolicy`, `Fingerprint`, settings and `SB_*` variables, `sbrs`
flags, `BaseCase` and `Page` methods), or it brings in one of stealthscraper-rs's features
(mainly the low-level spoofing: TLS/JA4 and HTTP/2 emulation, the MITM proxy, the
impersonating HTTP client) under seleniumbase-rs's names and concepts. The features are
all here already; what this note fixes is the API surface that was copied with them.

## Duplicated concepts

| Copied here | Core already has | Proposal |
|---|---|---|
| `StealthIdentity` (`export_identity`, `import_identity`, `restore`) | `sb_cdp::session_store::Session` and `SessionStore` (and `BrowserScraper` has both `export_identity` and `save_session`) | merge into `Session` |
| `BrowserScraper` and `BrowserScraperBuilder` (their `CloudScraper`), and again `StealthSession` and `StealthSessionBuilder` | `sb_cdp::Browser`, `Page`, `LaunchOptionsBuilder` | merge: the options join the launch builder, the actions become `Page` methods |
| `BrowserScraperBuilder::fingerprint`, `StealthSessionBuilder::fingerprint` | `LaunchOptionsBuilder::fingerprint` | remove the copies |
| `ImpersonationProfile` (their `BrowserProfile`, `random()`) and codec `HeaderIdentity` | `Fingerprint` (User-Agent, Accept-Language, client hints) | keep only the wire part as `WireFormat`; the rest is read from the `Fingerprint` |
| codec `BrowserKind` | `browser::config::Browser` | merge |
| `ProxyPool`, `RotationStrategy` | `config::proxy_list::ProxyList` (`--proxy-list`, `SB_PROXY_LIST`) | merge into `ProxyList` |
| `EgressRef`, `EgressKey`, `EgressSwitch`, `ContextProxySwitch` | `sb_cdp::ContextRoute`, `Proxy`, `ContextOptions::proxy`, `Browser::context_route` | merge into `ContextRoute`; "egress" becomes "proxy" in member names |
| `MitigationPolicy` (`max_challenge_attempts`, `backoff`) | `sb_cdp::captcha::SolveOptions` (`attempts`, `backoff`), `RetryPolicy` | merge into one `ChallengePolicy` that holds them |
| `Detection` and `Evidence`, `solve_challenge` | `sb_cdp::captcha::{Captcha, Evidence}`, `Page::solve_captcha` | merge into one `Challenge` beside `Captcha`; solving goes through `Page::solve_captcha` |
| `rotate_profile` | `Page::apply_fingerprint` | merge |
| `StateStore`, `InMemoryStateStore`, `JsonStateStore`, `DomainState` | `SessionStore` | merge; per-site records live in the `SessionStore` |
| `ScraperEvent`, `EventSink`, `NoopEventSink`, `CollectingEventSink`, `TracingEventSink` | `tracing` (core logs with it) | remove the sinks; events become `tracing` events |
| `CountryCode`, `LocaleExt` | `evasion::randomize::Locale`, `LocaleCode` | merge into `Locale` |
| `Secret`, `RedactedUrl` | `Proxy` and `ContextRoute` keep passwords out of `Debug` | one `Secret` in core `common`; `RedactedUrl` private |
| `per_site_rate`, `pacing` | `utilities::retry::RateLimiter` | one `rate_limit` option |
| `BrowserLauncher`, `ChromeLauncher` | `Browser::launch_mocked`, `BrowserPool::with_launcher` | private test seams |
| `CookieJar` | `Cookie`, `Cookies`, `Session` | private plumbing of the HTTP session |
| `Clock` (twice) | `core::allure::Clock` | private test seams |
| `BrowserFetch`, `StealthFetch`, `HttpFetched` | (one fetch) | one `FetchOutcome` |

What stays as this crate's own API: the codec formats (`Ja3`, `Ja4`, `Peetprint`,
`Akamai`, `TlsSpec`, `H2Spec`, `HeaderSpec`, ...), the measured wire data
(`WireMeasurement`), the wire format a client sends (`WireFormat` and its builder), the
impersonating HTTP client (`HttpSession`, `HttpRequest`, `HttpResponse`), the escalation
policy, the exit-country resolver and the errors. The MITM proxy and the raw wreq client
builder become plumbing behind a launch option (decision 6).

## Counts

| | Items | Of them copied |
|---|---|---|
| Reviewed | 1049 | 264 |
| Keep | 601 (19 also need an export) | 42 |
| Rename | 42 (5 also need an export) | 9 |
| Merge into a core concept | 196 | 132 |
| Make private (plumbing) | 170 | 57 |
| Remove (core already does it) | 40 | 24 |

| Crate | Keep | Rename | Merge | Private | Remove |
|---|---|---|---|---|---|
| packages, features, facade | 10 | 3 | 0 | 0 | 1 |
| codec | 367 | 12 | 16 | 0 | 1 |
| scrape | 50 | 10 | 122 | 112 | 29 |
| mimic | 174 | 17 | 58 | 58 | 9 |

The second round kept 924 items. Re-checked for copied names and shapes, 323 of them now
merge, go private or go away; most of the remaining keeps are codec's protocol terms and
the HTTP client, which is the feature core lacks. "Copied" counts an item whose name or
shape comes from stealthscraper-rs, with every member of such a type.

## How this list was made

- **The surface.** TokenSave's index of this branch (`friend/stealth-names`, on
  `origin/main` `7820793`), read with SQL over its `nodes` table, because
  `tokensave_module_api` stops after about 60 of codec's 644 public symbols. Every
  top-level module of codec, scrape and mimic is private, so an item is public API only if
  `lib.rs` re-exports it, it is a public field, method, variant or associated constant of
  such a type (every method of an exported trait), or it is a public type that appears in
  one of those signatures (the "export" rows), with its members. The other `pub` items in
  private modules are not reachable and not listed. Trait impls add no names and are not
  listed.
- **What was copied.** Judged from the stealthscraper-rs feature list in the project's
  notes (their `CloudScraper` builder and its options, `StealthSession` and
  `StealthIdentity`, `ProxyPool` with `RotationStrategy`, `ScraperEvent` and `EventSink`,
  `StateStore` and `DomainState`, `MitigationPolicy` and `Action`, `impersonation_client`,
  `BrowserProfile::random`, `BrowserKind`, `rotate_profile`, the `TlsSpoofingProxy`, and the
  names in their plan such as `EgressRef`, `JsonStateStore`, `SessionPolicy`). The
  stealthscraper-rs crate itself was not read, as the standing rule says.
- **Core.** The names of all 133 stealth types were compared with core's 986 public types
  (core at `41a2e3da`, main plus the mise removal, which changes no public type), and the
  core items above were read: `Session` and `SessionStore`, `ContextRoute`, `SolveOptions`
  and `Page::solve_captcha`, `Page::apply_fingerprint`, `ProxyList` with `SB_PROXY_LIST`,
  `StealthPolicy`, `LaunchOptionsBuilder`, `RateLimiter`, `RetryPolicy`, `Locale`.

## Core names the stealth items follow

| Core item | Where | What it means here |
|---|---|---|
| `LaunchOptions`, `LaunchOptionsBuilder` (`fingerprint`, `proxy`, plain setters) | `sb_cdp::launch` | the scraper's options become launch options: `proxy_list`, `challenge_policy`, `geo_resolver`, `rate_limit`, `wire_format`, `session_store` |
| `Page::solve_captcha`, `SolveOptions`, `Captcha`, `Evidence` | `sb_cdp::captcha` | `solve_challenge` goes through `solve_captcha`; `ChallengePolicy` holds a `SolveOptions`; `Challenge` sits beside `Captcha` |
| `Page::apply_fingerprint`, `Fingerprint`, `Fingerprint::seed` | `sb_cdp::page`, `evasion::fingerprint` | `rotate_profile` becomes applying a new fingerprint; `WireFormat` reads the fingerprint's headers |
| `Session`, `SessionStore` | `sb_cdp::session_store` | the saved identity and the per-site records |
| `ContextRoute`, `Proxy`, `ContextOptions::proxy`, `Browser::context_route` | `sb_cdp::context`, `sb_cdp::launch` | the proxy route; "egress" becomes "proxy" |
| `ProxyList`, `--proxy-list`, `SB_PROXY_LIST` | `config::proxy_list` | proxy rotation |
| `StealthPolicy`, `RetryPolicy`, `RateLimiter` | `evasion::policy`, `utilities::retry` | the `<Thing>Policy` pattern; `rate_limit` |
| `Locale`, `LocaleCode` | `evasion::randomize`, `browser::options::locale` | country lookups |
| `Browser` (enum) | `browser::config` | the measured browser |
| `tracing` | the whole crate | events |

## The new words in one place

| Now | Proposed |
|---|---|
| stealth identity, `export_identity`/`import_identity`, `restore` | core `Session`, `save_session`/`load_session`, `HttpSession::from_session` |
| browser scraper, stealth session (and their builders) | core `LaunchOptionsBuilder` options and `Page` methods; `HttpSession::from_page` for the handoff |
| impersonation profile | wire format (`WireFormat`) |
| emulation (codec's measured data) | wire measurement (`WireMeasurement`) |
| egress | proxy route (core `ContextRoute`) |
| mitigation policy, detection, evidence | challenge policy (`ChallengePolicy`), challenge (`Challenge`) |
| mode, transition, session policy | fetch kind (`FetchKind`), escalation (private), escalation policy (`EscalationPolicy`) |
| state store, domain state | core `SessionStore`, site record (`SiteRecord`) |
| profile rotation | `Page::apply_fingerprint` with a new fingerprint |
| events and sinks | `tracing` events |
| reply | response (`HttpResponse`) |
| `per_site_rate`, `pacing` | `rate_limit` |

## Decisions for the owner

1. **The saved identity.** Merge `StealthIdentity` into core's `Session` (core's `Session`
   gains the fingerprint, the wire measurement, the proxy route and the challenge
   clearances, and `SessionStore` saves it), or keep a stealth type named `Identity`?
2. **Where the browser-side features land.** `BrowserScraper` and `StealthSession` go away
   as types. Their options need a home on core's launch path: an extension trait in this
   crate on `LaunchOptionsBuilder` and `Page` (the code stays here, the calls read like
   core's), or fields and methods in core itself (or a core plugin through
   `ContextSettings`)? This also answers the `fingerprint(...)` question: the builders'
   `fingerprint` is a copy of core's `LaunchOptionsBuilder::fingerprint` and goes away.
3. **Moves into core** (`ProxyList` rotation, `Locale` countries, `Session` fields, one
   `Secret`): in the same round as the renames, or one core PR each, later? `Secret` touches
   security code that has its own owner.
4. **The `mimic` crate.** Rename the package `seleniumbase-rs-wire` (feature and facade
   path `wire`)?
5. **Two wire words.** `WireMeasurement` (codec: measured data with provenance) and
   `WireFormat` (mimic: what a client sends, built from a measurement and a fingerprint),
   or one word for both?
6. **The MITM proxy and the raw client builder.** Private plumbing behind a launch option
   (`wire_format`), or public tools for people who drive a browser this crate does not
   launch (then `MitmProxy` keeps its name and `impersonation_client` becomes
   `wire_client`)?
7. **Events.** Plain `tracing` events only, or also a typed `StealthEvent` that core's
   plugin observer can receive?

## Phase 2 (after approval)

Renames with `tokensave rename` (a dry run first), the merges as one core PR and one
stealth PR (or per decision 3), every caller, the stealth API reference pages and the
Stealth sections of the core docs, and the contract files (`mise run contracts`). Breaking
changes, no shims.

## Every public item

One row per item. "Copied" marks an item whose name or shape comes from stealthscraper-rs
(with every member of such a type). Verdicts: keep, rename, merge (into the named core
concept), private (plumbing), remove (core already does it); "export" when a public
signature takes or returns the type but no path names it today.

## Packages, features and the facade

### (packages)

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `seleniumbase-rs-stealth` (package) | package |  | keep |  | says what it adds to seleniumbase-rs |
| `seleniumbase-rs-codec` (package) | package |  | keep |  | it encodes and decodes wire fingerprints |
| `seleniumbase-rs-scrape` (package) | package |  | keep |  | the scraping runtime; the docs say scrape |
| `seleniumbase-rs-mimic` (package) | package |  | rename | `seleniumbase-rs-wire` | "mimic" does not say what it holds: the HTTP client that sends a browser's wire format, the MITM proxy and the session that switches between them; the docs call this wire formats. Decision 4 |

### (features)

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `default = ["scrape"]` | feature |  | keep |  |  |
| `codec` | feature |  | keep |  | named after its crate |
| `scrape` | feature |  | keep |  | named after its crate |
| `mimic` | feature |  | rename | `wire` | follows the package. Decision 4 |
| `test-fixtures` (codec) | feature |  | keep |  | says what it turns on |

### src/lib.rs

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `pub use seleniumbase_rs::*` | re-export |  | keep |  | the facade is core plus the stealth crates |
| `pub use seleniumbase_rs::evasion::*` | re-export |  | remove |  | gives every `evasion` item a second path at the root; core keeps them under `evasion::`, one path per item |
| `codec` (`seleniumbase_rs_codec`) | re-export |  | keep |  |  |
| `scrape` (`seleniumbase_rs_scrape`) | re-export |  | keep |  |  |
| `mimic` (`seleniumbase_rs_mimic`) | re-export |  | rename | `wire` | follows the package. Decision 4 |

## codec (`seleniumbase_rs_stealth::codec`)

### `codec::akamai`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `Akamai` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `Akamai.settings` | field |  | keep |  | plain data name |
| `Akamai.window_update` | field |  | keep |  | plain data name |
| `Akamai.priority_frames` | field |  | keep |  | plain data name |
| `Akamai.pseudo_order` | field |  | keep |  | plain data name |
| `Akamai::parse` | method |  | keep |  | says what it does |
| `AkamaiError` | enum |  | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `AkamaiError::TooLong` | enum variant |  | keep |  | names the case |
| `AkamaiError::Sections` | enum variant |  | keep |  | names the case |
| `AkamaiError::Setting` | enum variant |  | keep |  | names the case |
| `AkamaiError::DuplicateSetting` | enum variant |  | keep |  | names the case |
| `AkamaiError::TooManySettings` | enum variant |  | keep |  | names the case |
| `AkamaiError::WindowUpdate` | enum variant |  | keep |  | names the case |
| `AkamaiError::PriorityFrame` | enum variant |  | keep |  | names the case |
| `AkamaiError::TooManyPriorityFrames` | enum variant |  | keep |  | names the case |
| `AkamaiError::PseudoOrder` | enum variant |  | keep |  | names the case |
| `PriorityFrame` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `PriorityFrame.stream_id` | field |  | keep |  | plain data name |
| `PriorityFrame.exclusive` | field |  | keep |  | plain data name |
| `PriorityFrame.depends_on` | field |  | keep |  | plain data name |
| `PriorityFrame.weight` | field |  | keep |  | plain data name |
| `Pseudo` | enum |  | rename, export | `PseudoHeader` | `Pseudo` alone does not say what it is |
| `Pseudo::Method` | enum variant |  | keep |  | names the case |
| `Pseudo::Authority` | enum variant |  | keep |  | names the case |
| `Pseudo::Scheme` | enum variant |  | keep |  | names the case |
| `Pseudo::Path` | enum variant |  | keep |  | names the case |
| `Pseudo::ALL` | const |  | keep |  | says what it does |
| `Pseudo::letter` | method |  | keep |  | says what it does |
| `Pseudo::header_name` | method |  | keep |  | says what it does |
| `Pseudo::from_letter` | method |  | keep |  | says what it does |
| `Pseudo::from_header_name` | method |  | keep |  | says what it does |

### `codec::client_hello`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ClientHello` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `ClientHello.record_version` | field |  | keep |  | plain data name |
| `ClientHello.legacy_version` | field |  | keep |  | plain data name |
| `ClientHello.random` | field |  | keep |  | plain data name |
| `ClientHello.session_id` | field |  | keep |  | plain data name |
| `ClientHello.cipher_suites` | field |  | keep |  | plain data name |
| `ClientHello.compression_methods` | field |  | keep |  | plain data name |
| `ClientHello.extensions` | field |  | keep |  | plain data name |
| `ClientHello.sni` | field |  | keep |  | plain data name |
| `ClientHello.alpn` | field |  | keep |  | plain data name |
| `ClientHello.supported_versions` | field |  | keep |  | plain data name |
| `ClientHello.supported_groups` | field |  | keep |  | plain data name |
| `ClientHello.key_share_groups` | field |  | keep |  | plain data name |
| `ClientHello.point_formats` | field |  | keep |  | plain data name |
| `ClientHello.signature_algorithms` | field |  | keep |  | plain data name |
| `ClientHello.psk_modes` | field |  | keep |  | plain data name |
| `ClientHello.cert_compression` | field |  | keep |  | plain data name |
| `ClientHello.alps` | field |  | keep |  | plain data name |
| `ClientHello.record_size_limit` | field |  | keep |  | plain data name |
| `ClientHello.delegated_credentials` | field |  | keep |  | plain data name |
| `ClientHello::needed_len` | method |  | keep |  | says what it does |
| `ClientHello::parse` | method |  | keep |  | says what it does |
| `ClientHello::extension` | method |  | keep |  | says what it does |
| `ClientHello::has_extension` | method |  | keep |  | says what it does |
| `ClientHello::extension_ids` | method |  | keep |  | says what it does |
| `ClientHello::has_grease` | method |  | keep |  | says what it does |
| `ClientHelloError` | enum |  | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `ClientHelloError::NotHandshake` | enum variant |  | keep |  | names the case |
| `ClientHelloError::NotClientHello` | enum variant |  | keep |  | names the case |
| `ClientHelloError::Truncated` | enum variant |  | keep |  | names the case |
| `ClientHelloError::Length` | enum variant |  | keep |  | names the case |
| `ClientHelloError::TooLarge` | enum variant |  | keep |  | names the case |
| `ClientHelloError::TooManyRecords` | enum variant |  | keep |  | names the case |
| `ClientHelloError::Extension` | enum variant |  | keep |  | names the case |
| `Extension` | struct |  | rename, export | `TlsExtension` | an extension in core is a browser extension (`LaunchOptionsBuilder::extension`, `utils::extensions`) |
| `Extension.id` | field |  | keep |  | plain data name |
| `Extension.data` | field |  | keep |  | plain data name |

### `codec::emulation`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `emulation_for_kind` | function | yes | rename | `wire_measurement_for` | **copied** (`for_kind`); follows `WireMeasurement` |
| `get_emulation` | function |  | rename | `wire_measurement` | follows `WireMeasurement`; no `get_` prefix (Rust API guidelines, C-GETTER) |
| `measured_emulations` | function |  | rename | `wire_measurements` | follows `WireMeasurement` |
| `Emulation` | struct | yes | rename | `WireMeasurement` | **copied** word (their identity's "emulation kind"); **clash** with core `sb_cdp::state::Emulation` (device emulation). It is one browser's measured TLS, HTTP/2 and header shape |
| `Emulation.kind` | field | yes | keep |  | plain data name |
| `Emulation.major` | field | yes | keep |  | plain data name |
| `Emulation.full_version` | field | yes | keep |  | plain data name |
| `Emulation.os` | field | yes | keep |  | plain data name |
| `Emulation.user_agent` | field | yes | keep |  | plain data name |
| `Emulation.tls` | field | yes | keep |  | plain data name |
| `Emulation.tls_websocket` | field | yes | keep |  | plain data name |
| `Emulation.http2` | field | yes | keep |  | plain data name |
| `Emulation.headers` | field | yes | keep |  | plain data name |
| `Emulation.client_hints` | field | yes | keep |  | plain data name |
| `Emulation.provenance` | field | yes | keep |  | plain data name |
| `Emulation::from_json` | method | yes | keep |  | says what it does |
| `Emulation::to_json` | method | yes | keep |  | says what it does |
| `Emulation::file_stem` | method | yes | keep |  | says what it does |
| `EmulationError` | enum |  | rename | `WireMeasurementError` | follows `WireMeasurement` |
| `EmulationError::NotMeasured` | enum variant |  | keep |  | names the case |
| `EmulationError::VersionNotMeasured` | enum variant |  | keep |  | names the case |
| `EmulationError::Parse` | enum variant |  | keep |  | names the case |
| `EmulationError::TooLarge` | enum variant |  | keep |  | names the case |
| `EmulationError::Inconsistent` | enum variant |  | keep |  | names the case |
| `EmulationError::Headers` | enum variant |  | keep |  | names the case |
| `EmulationError::Duplicate` | enum variant |  | keep |  | names the case |
| `Provenance` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Provenance.captured_on` | field |  | keep |  | plain data name |
| `Provenance.tool` | field |  | keep |  | plain data name |
| `Provenance.browser` | field |  | keep |  | plain data name |
| `Provenance.known_gaps` | field |  | keep |  | plain data name |

### `codec::h2_spec`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `H2Spec` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `H2Spec.settings` | field |  | keep |  | plain data name |
| `H2Spec.window_update` | field |  | keep |  | plain data name |
| `H2Spec.priority_frames` | field |  | keep |  | plain data name |
| `H2Spec.headers_priority` | field |  | keep |  | plain data name |
| `H2Spec.pseudo_order` | field |  | keep |  | plain data name |
| `H2Spec.initial_stream_id` | field |  | keep |  | plain data name |
| `H2Spec::from_preface` | method |  | keep |  | says what it does |
| `H2Spec::from_capture` | method |  | keep |  | says what it does |
| `H2Spec::from_akamai` | method |  | keep |  | says what it does |
| `H2Spec::with_akamai` | method |  | keep |  | says what it does |
| `H2Spec::check_reproducible` | method |  | keep |  | says what it does |
| `H2Spec::akamai` | method |  | keep |  | says what it does |
| `H2Spec::setting` | method |  | keep |  | says what it does |
| `H2Spec::without_unsupported_settings` | method |  | keep |  | says what it does |
| `H2SpecError` | enum |  | rename | `H2SpecError` (source name) | make it the real name: the source calls it `SpecError` and `lib.rs` renames it; one spelling per item |
| `H2SpecError::UnsupportedSetting` | enum variant |  | keep |  | names the case |
| `H2SpecError::DuplicateSetting` | enum variant |  | keep |  | names the case |
| `H2SpecError::SettingValue` | enum variant |  | keep |  | names the case |
| `H2SpecError::SettingAlwaysSent` | enum variant |  | keep |  | names the case |
| `H2SpecError::WindowUpdate` | enum variant |  | keep |  | names the case |
| `H2SpecError::Priority` | enum variant |  | keep |  | names the case |
| `H2SpecError::HeadersPriority` | enum variant |  | keep |  | names the case |
| `H2SpecError::PseudoOrder` | enum variant |  | keep |  | names the case |
| `H2SpecError::InitialStreamId` | enum variant |  | keep |  | names the case |
| `H2SpecError::Incomplete` | enum variant |  | keep |  | names the case |
| `H2SpecError::Capture` | enum variant |  | keep |  | names the case |
| `Http2Spec` | type alias |  | remove |  | a second name for `H2Spec` |

### `codec::header_spec`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `HeaderIdentity` | struct |  | merge | core `Fingerprint` (User-Agent, Accept-Language, client hints) | its values are the fingerprint's; a second copy drifts. The wire format reads them from the `Fingerprint` |
| `HeaderIdentity.user_agent` | field |  | merge |  | goes with its type |
| `HeaderIdentity.accept_language` | field |  | merge |  | goes with its type |
| `HeaderIdentity.sec_ch_ua` | field |  | merge |  | goes with its type |
| `HeaderIdentity.sec_ch_ua_mobile` | field |  | merge |  | goes with its type |
| `HeaderIdentity.sec_ch_ua_platform` | field |  | merge |  | goes with its type |
| `HeaderIdentity.cookie` | field |  | merge |  | goes with its type |
| `HeaderIdentity.overrides` | field |  | merge |  | goes with its type |
| `HeaderIdentity::new` | method |  | merge |  | goes with its type |
| `HeaderIdentity::with_accept_language` | method |  | merge |  | goes with its type |
| `HeaderIdentity::with_client_hints` | method |  | merge |  | goes with its type |
| `HeaderIdentity::with_cookie` | method |  | merge |  | goes with its type |
| `HeaderIdentity::with_override` | method |  | merge |  | goes with its type |
| `HeaderIdentity::from_captured` | method |  | merge |  | goes with its type |
| `HeaderSpec` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `HeaderSpec.document` | field |  | keep |  | plain data name |
| `HeaderSpec.fetch` | field |  | keep |  | plain data name |
| `HeaderSpec.priority_by_dest` | field |  | keep |  | plain data name |
| `HeaderSpec::from_captured` | method |  | keep |  | says what it does |
| `HeaderSpec::validate` | method |  | keep |  | says what it does |
| `HeaderSpec::headers` | method |  | keep |  | says what it does |
| `HeaderSpec::render` | method |  | keep |  | says what it does |
| `HeaderSpec::priority_for` | method |  | keep |  | says what it does |
| `HeaderSpec::with_header` | method |  | keep |  | says what it does |
| `HeaderSpec::without_header` | method |  | keep |  | says what it does |
| `HeaderSpec::with_priority` | method |  | keep |  | says what it does |
| `HeaderSpecError` | enum |  | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `HeaderSpecError::InvalidName` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::InvalidValue` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::PseudoHeader` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::ReservedHeader` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::DuplicateName` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::DuplicateDestination` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::TooManyHeaders` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::TooManyDestinations` | enum variant |  | keep |  | names the case |
| `HeaderSpecError::PriorityWithoutDestination` | enum variant |  | keep |  | names the case |
| `RequestShape` | enum |  | keep |  | says which request it shapes (a document load or a fetch) |
| `RequestShape::Document` | enum variant |  | keep |  | names the case |
| `RequestShape::Fetch` | enum variant |  | keep |  | names the case |
| `HeaderValue` | enum |  | rename, export | `HeaderSource` | clashes with `http::HeaderValue`, which wreq hands to callers; the enum says where a value comes from (literal, user agent, client hints, language, cookie) |
| `HeaderValue::Literal` | enum variant |  | keep |  | names the case |
| `HeaderValue::UserAgent` | enum variant |  | keep |  | names the case |
| `HeaderValue::SecChUa` | enum variant |  | keep |  | names the case |
| `HeaderValue::SecChUaMobile` | enum variant |  | keep |  | names the case |
| `HeaderValue::SecChUaPlatform` | enum variant |  | keep |  | names the case |
| `HeaderValue::AcceptLanguage` | enum variant |  | keep |  | names the case |
| `HeaderValue::Cookie` | enum variant |  | keep |  | names the case |

### `codec::ja3`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `Ja3` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `Ja3.version` | field |  | keep |  | plain data name |
| `Ja3.ciphers` | field |  | keep |  | plain data name |
| `Ja3.extensions` | field |  | keep |  | plain data name |
| `Ja3.groups` | field |  | keep |  | plain data name |
| `Ja3.point_formats` | field |  | keep |  | plain data name |
| `Ja3::of` | method |  | keep |  | says what it does |
| `Ja3::parse` | method |  | keep |  | says what it does |
| `Ja3::hash` | method |  | keep |  | says what it does |
| `Ja3Error` | enum |  | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `Ja3Error::FieldCount` | enum variant |  | keep |  | names the case |
| `Ja3Error::Number` | enum variant |  | keep |  | names the case |
| `Ja3Error::Empty` | enum variant |  | keep |  | names the case |
| `Ja3Error::TooLong` | enum variant |  | keep |  | names the case |
| `Peetprint` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `Peetprint.versions` | field |  | keep |  | plain data name |
| `Peetprint.alpn` | field |  | keep |  | plain data name |
| `Peetprint.groups` | field |  | keep |  | plain data name |
| `Peetprint.signature_algorithms` | field |  | keep |  | plain data name |
| `Peetprint.psk_modes` | field |  | keep |  | plain data name |
| `Peetprint.cert_compression` | field |  | keep |  | plain data name |
| `Peetprint.ciphers` | field |  | keep |  | plain data name |
| `Peetprint.extensions` | field |  | keep |  | plain data name |
| `Peetprint::of` | method |  | keep |  | says what it does |
| `Peetprint::normalized` | method |  | keep |  | says what it does |
| `Peetprint::extension_ids` | method |  | keep |  | says what it does |
| `Peetprint::hash` | method |  | keep |  | says what it does |
| `Peetprint::parse` | method |  | keep |  | says what it does |
| `PeetprintError` | enum |  | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `PeetprintError::FieldCount` | enum variant |  | keep |  | names the case |
| `PeetprintError::Token` | enum variant |  | keep |  | names the case |
| `PeetprintError::TooLong` | enum variant |  | keep |  | names the case |
| `Ja3Field` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja3Field::Version` | enum variant |  | keep |  | names the case |
| `Ja3Field::Ciphers` | enum variant |  | keep |  | names the case |
| `Ja3Field::Extensions` | enum variant |  | keep |  | names the case |
| `Ja3Field::Groups` | enum variant |  | keep |  | names the case |
| `Ja3Field::PointFormats` | enum variant |  | keep |  | names the case |
| `Slot` | enum |  | rename, export | `PeetprintSlot` | `Slot` alone is vague; it is one entry of a Peetprint list (GREASE or an id) |
| `Slot::Grease` | enum variant |  | keep |  | names the case |
| `Slot::Id` | enum variant |  | keep |  | names the case |
| `Slot::from_id` | method |  | keep |  | says what it does |
| `PeetField` | enum |  | rename, export | `PeetprintField` | spell the format's name as `Peetprint` and `PeetprintError` do |
| `PeetField::Versions` | enum variant |  | keep |  | names the case |
| `PeetField::Alpn` | enum variant |  | keep |  | names the case |
| `PeetField::Groups` | enum variant |  | keep |  | names the case |
| `PeetField::SignatureAlgorithms` | enum variant |  | keep |  | names the case |
| `PeetField::PskModes` | enum variant |  | keep |  | names the case |
| `PeetField::CertCompression` | enum variant |  | keep |  | names the case |
| `PeetField::Ciphers` | enum variant |  | keep |  | names the case |
| `PeetField::Extensions` | enum variant |  | keep |  | names the case |

### `codec::ja4`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `Ja4` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `Ja4.a` | field |  | keep |  | plain data name |
| `Ja4.b` | field |  | keep |  | plain data name |
| `Ja4.c` | field |  | keep |  | plain data name |
| `Ja4::of` | method |  | keep |  | says what it does |
| `Ja4::of_original_order` | method |  | keep |  | says what it does |
| `Ja4::raw` | method |  | keep |  | says what it does |
| `Ja4::raw_original_order` | method |  | keep |  | says what it does |
| `Ja4::parse` | method |  | keep |  | says what it does |
| `Ja4::diff` | method |  | keep |  | says what it does |
| `Ja4Error` | enum |  | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `Ja4Error::Sections` | enum variant |  | keep |  | names the case |
| `Ja4Error::PartA` | enum variant |  | keep |  | names the case |
| `Ja4Error::Hash` | enum variant |  | keep |  | names the case |
| `Ja4Error::Hex` | enum variant |  | keep |  | names the case |
| `Ja4Error::TooLong` | enum variant |  | keep |  | names the case |
| `Ja4Raw` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `Ja4Raw.a` | field |  | keep |  | plain data name |
| `Ja4Raw.ciphers` | field |  | keep |  | plain data name |
| `Ja4Raw.extensions` | field |  | keep |  | plain data name |
| `Ja4Raw.signature_algorithms` | field |  | keep |  | plain data name |
| `Ja4Raw::parse` | method |  | keep |  | says what it does |
| `Ja4Raw::hashed` | method |  | keep |  | says what it does |
| `Ja4Diff` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Diff.a_fields` | field |  | keep |  | plain data name |
| `Ja4Diff.b` | field |  | keep |  | plain data name |
| `Ja4Diff.c` | field |  | keep |  | plain data name |
| `Ja4Diff::is_empty` | method |  | keep |  | says what it does |
| `Ja4Section` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Section::A` | enum variant |  | keep |  | names the case |
| `Ja4Section::Ciphers` | enum variant |  | keep |  | names the case |
| `Ja4Section::Extensions` | enum variant |  | keep |  | names the case |
| `Ja4Section::SignatureAlgorithms` | enum variant |  | keep |  | names the case |
| `Ja4Facts` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Facts.legacy_version` | field |  | keep |  | plain data name |
| `Ja4Facts.supported_versions` | field |  | keep |  | plain data name |
| `Ja4Facts.ciphers` | field |  | keep |  | plain data name |
| `Ja4Facts.extensions` | field |  | keep |  | plain data name |
| `Ja4Facts.first_alpn` | field |  | keep |  | plain data name |
| `Ja4Facts.signature_algorithms` | field |  | keep |  | plain data name |
| `Ja4Facts::of` | method |  | keep |  | says what it does |
| `Ja4Facts::a` | method |  | keep |  | says what it does |
| `Ja4Facts::raw` | method |  | keep |  | says what it does |
| `Ja4Facts::ja4` | method |  | keep |  | says what it does |
| `Ja4Order` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Order::Sorted` | enum variant |  | keep |  | names the case |
| `Ja4Order::Original` | enum variant |  | keep |  | names the case |
| `Ja4AField` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4AField::Protocol` | enum variant |  | keep |  | names the case |
| `Ja4AField::Version` | enum variant |  | keep |  | names the case |
| `Ja4AField::Sni` | enum variant |  | keep |  | names the case |
| `Ja4AField::CipherCount` | enum variant |  | keep |  | names the case |
| `Ja4AField::ExtensionCount` | enum variant |  | keep |  | names the case |
| `Ja4AField::Alpn` | enum variant |  | keep |  | names the case |

### `codec::tls_spec`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `KnownGap` | enum |  | keep |  | names a measured difference the client cannot reproduce yet |
| `KnownGap::TrustAnchorsExtension` | enum variant |  | keep |  | names the case |
| `KnownGap::MlDsaSignatureAlgorithms` | enum variant |  | keep |  | names the case |
| `TlsSpecError` | enum |  | rename | `TlsSpecError` (source name) | make it the real name: the source calls it `SpecError` and `lib.rs` renames it; one spelling per item |
| `TlsSpecError::LegacyVersion` | enum variant |  | keep |  | names the case |
| `TlsSpecError::UnsupportedVersion` | enum variant |  | keep |  | names the case |
| `TlsSpecError::NoCipherSuites` | enum variant |  | keep |  | names the case |
| `TlsSpecError::UnsupportedCipher` | enum variant |  | keep |  | names the case |
| `TlsSpecError::Tls13Suites` | enum variant |  | keep |  | names the case |
| `TlsSpecError::UnsupportedExtension` | enum variant |  | keep |  | names the case |
| `TlsSpecError::DuplicateExtension` | enum variant |  | keep |  | names the case |
| `TlsSpecError::UnsupportedGroup` | enum variant |  | keep |  | names the case |
| `TlsSpecError::KeyShareNotOffered` | enum variant |  | keep |  | names the case |
| `TlsSpecError::PointFormats` | enum variant |  | keep |  | names the case |
| `TlsSpecError::UnsupportedSigalg` | enum variant |  | keep |  | names the case |
| `TlsSpecError::Duplicate` | enum variant |  | keep |  | names the case |
| `TlsSpecError::ExtensionContent` | enum variant |  | keep |  | names the case |
| `TlsSpecError::Alpn` | enum variant |  | keep |  | names the case |
| `TlsSpecError::Ja4Mismatch` | enum variant |  | keep |  | names the case |
| `TlsSpecError::Ja4Alpn` | enum variant |  | keep |  | names the case |
| `TlsSpecError::PeetprintMismatch` | enum variant |  | keep |  | names the case |
| `TlsSpecError::HellosDiffer` | enum variant |  | keep |  | names the case |
| `TlsSpec` | struct |  | keep |  | a protocol or published-format term, not another library's API |
| `TlsSpec.legacy_version` | field |  | keep |  | plain data name |
| `TlsSpec.cipher_suites` | field |  | keep |  | plain data name |
| `TlsSpec.extensions` | field |  | keep |  | plain data name |
| `TlsSpec.permute_extensions` | field |  | keep |  | plain data name |
| `TlsSpec.grease` | field |  | keep |  | plain data name |
| `TlsSpec.supported_groups` | field |  | keep |  | plain data name |
| `TlsSpec.key_share_groups` | field |  | keep |  | plain data name |
| `TlsSpec.point_formats` | field |  | keep |  | plain data name |
| `TlsSpec.signature_algorithms` | field |  | keep |  | plain data name |
| `TlsSpec.alpn` | field |  | keep |  | plain data name |
| `TlsSpec.alps` | field |  | keep |  | plain data name |
| `TlsSpec.delegated_credentials` | field |  | keep |  | plain data name |
| `TlsSpec.supported_versions` | field |  | keep |  | plain data name |
| `TlsSpec.psk_modes` | field |  | keep |  | plain data name |
| `TlsSpec.cert_compression` | field |  | keep |  | plain data name |
| `TlsSpec.record_size_limit` | field |  | keep |  | plain data name |
| `TlsSpec::from_client_hello` | method |  | keep |  | says what it does |
| `TlsSpec::with_observed_order` | method |  | keep |  | says what it does |
| `TlsSpec::has_extension` | method |  | keep |  | says what it does |
| `TlsSpec::ocsp_stapling` | method |  | keep |  | says what it does |
| `TlsSpec::sct` | method |  | keep |  | says what it does |
| `TlsSpec::session_ticket` | method |  | keep |  | says what it does |
| `TlsSpec::renegotiation` | method |  | keep |  | says what it does |
| `TlsSpec::ech_grease` | method |  | keep |  | says what it does |
| `TlsSpec::extended_master_secret` | method |  | keep |  | says what it does |
| `TlsSpec::ja3` | method |  | keep |  | says what it does |
| `TlsSpec::ja4_facts` | method |  | keep |  | says what it does |
| `TlsSpec::ja4` | method |  | keep |  | says what it does |
| `TlsSpec::ja4_raw` | method |  | keep |  | says what it does |
| `TlsSpec::peetprint` | method |  | keep |  | says what it does |
| `TlsSpec::without_known_gaps` | method |  | keep |  | says what it does |
| `TlsSpec::check_reproducible` | method |  | keep |  | says what it does |
| `TlsSpec::with_ja3` | method |  | keep |  | says what it does |
| `TlsSpec::with_signature_algorithms` | method |  | keep |  | says what it does |
| `TlsSpec::with_ja4_raw` | method |  | keep |  | says what it does |
| `TlsSpec::with_peetprint` | method |  | keep |  | says what it does |
| `Alps` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Alps.protocols` | field |  | keep |  | plain data name |
| `Alps.new_codepoint` | field |  | keep |  | plain data name |

### `codec` (crate root)

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `BrowserKind` | enum | yes | merge | core `browser::config::Browser` | **copied** (`BrowserKind` picks the JA4/HTTP/2 source); core's `Browser` enum already names browsers |
| `BrowserKind::Chrome` | enum variant | yes | merge |  | goes with its type |

### `codec::h2_frames`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `HeadersPriority` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HeadersPriority.depends_on` | field |  | keep |  | plain data name |
| `HeadersPriority.weight` | field |  | keep |  | plain data name |
| `HeadersPriority.exclusive` | field |  | keep |  | plain data name |
| `H2Preface` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `H2Preface.settings` | field |  | keep |  | plain data name |
| `H2Preface.window_update` | field |  | keep |  | plain data name |
| `H2Preface.priority_frames` | field |  | keep |  | plain data name |
| `H2Preface.headers_priority` | field |  | keep |  | plain data name |
| `H2Preface.request_stream_id` | field |  | keep |  | plain data name |
| `H2Preface.headers` | field |  | keep |  | plain data name |
| `H2Preface.consumed` | field |  | keep |  | plain data name |
| `H2Preface::pseudo_order` | method |  | keep |  | says what it does |
| `H2Preface::akamai` | method |  | keep |  | says what it does |
| `H2Preface::regular_headers` | method |  | keep |  | says what it does |
| `H2Preface::header` | method |  | keep |  | says what it does |
| `H2CaptureError` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `H2CaptureError::BadPreface` | enum variant |  | keep |  | names the case |
| `H2CaptureError::FrameTooLarge` | enum variant |  | keep |  | names the case |
| `H2CaptureError::FrameSize` | enum variant |  | keep |  | names the case |
| `H2CaptureError::StreamId` | enum variant |  | keep |  | names the case |
| `H2CaptureError::BadPadding` | enum variant |  | keep |  | names the case |
| `H2CaptureError::ZeroWindowIncrement` | enum variant |  | keep |  | names the case |
| `H2CaptureError::UnexpectedFrame` | enum variant |  | keep |  | names the case |
| `H2CaptureError::ExpectedContinuation` | enum variant |  | keep |  | names the case |
| `H2CaptureError::HeaderBlockTooLarge` | enum variant |  | keep |  | names the case |
| `H2CaptureError::PrefaceTooLarge` | enum variant |  | keep |  | names the case |
| `H2CaptureError::TooManyFrames` | enum variant |  | keep |  | names the case |
| `H2CaptureError::ReaderFinished` | enum variant |  | keep |  | names the case |
| `H2CaptureError::MissingSettings` | enum variant |  | keep |  | names the case |
| `H2CaptureError::PseudoOrder` | enum variant |  | keep |  | names the case |
| `H2CaptureError::StreamIdRange` | enum variant |  | keep |  | names the case |
| `H2CaptureError::Hpack` | enum variant |  | keep |  | names the case |

### `codec::h2_frames::hpack`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `HeaderField` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HeaderField.name` | field |  | keep |  | plain data name |
| `HeaderField.value` | field |  | keep |  | plain data name |
| `HeaderField::new` | method |  | keep |  | says what it does |
| `HeaderField::is_pseudo` | method |  | keep |  | says what it does |
| `HpackError` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HpackError::Truncated` | enum variant |  | keep |  | names the case |
| `HpackError::IntegerOverflow` | enum variant |  | keep |  | names the case |
| `HpackError::IndexZero` | enum variant |  | keep |  | names the case |
| `HpackError::IndexOutOfRange` | enum variant |  | keep |  | names the case |
| `HpackError::Huffman` | enum variant |  | keep |  | names the case |
| `HpackError::NotUtf8` | enum variant |  | keep |  | names the case |
| `HpackError::SizeUpdateTooLarge` | enum variant |  | keep |  | names the case |
| `HpackError::SizeUpdateAfterField` | enum variant |  | keep |  | names the case |
| `HpackError::HeaderListTooLarge` | enum variant |  | keep |  | names the case |

### `codec::h2_frames::huffman`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `HuffmanError` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HuffmanError::EndOfString` | enum variant |  | keep |  | names the case |
| `HuffmanError::PaddingTooLong` | enum variant |  | keep |  | names the case |
| `HuffmanError::PaddingNotOnes` | enum variant |  | keep |  | names the case |
| `HuffmanError::InvalidCode` | enum variant |  | keep |  | names the case |

## scrape (`seleniumbase_rs_stealth::scrape`)

### `scrape::challenge::detect`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ChallengeKind` | enum | yes | keep |  | **copied** name, but no core type says what kind of block a page shows; becomes a field of `Challenge` |
| `ChallengeKind::None` | enum variant | yes | rename | `Clear` | `ChallengeKind::None` reads like `Option::None`; `HttpVerdict::is_clear` already says clear |
| `ChallengeKind::JsChallenge` | enum variant | yes | keep |  | names the case |
| `ChallengeKind::Interactive` | enum variant | yes | keep |  | names the case |
| `ChallengeKind::Blocked` | enum variant | yes | keep |  | names the case |
| `ChallengeKind::RateLimited` | enum variant | yes | keep |  | names the case |
| `Detection` | struct | yes | merge | `Challenge` (with `Evidence`) | **copied** (`detect`); with `Evidence` it repeats the result core's `sb_cdp::captcha` already gives (`Captcha`, `Evidence`). One `Challenge` type beside core's `Captcha` |
| `Detection.kind` | field | yes | merge |  | goes with its type |
| `Detection.evidence` | field | yes | merge |  | goes with its type |
| `Evidence` | struct | yes | merge | `Challenge` | **clash** with core `sb_cdp::captcha::Evidence`; its fields join `Challenge` |
| `Evidence.vendor` | field | yes | merge |  | goes with its type |
| `Evidence.markers` | field | yes | merge |  | goes with its type |
| `Evidence.widget` | field | yes | merge |  | goes with its type |
| `ResponseView` | struct |  | private |  | plumbing that detection reads; callers ask `HttpResponse::challenge` or the page |
| `ResponseView.url` | field |  | private |  | goes with its type (plumbing) |
| `ResponseView.status` | field |  | private |  | goes with its type (plumbing) |
| `ResponseView.headers` | field |  | private |  | goes with its type (plumbing) |
| `ResponseView.body` | field |  | private |  | goes with its type (plumbing) |
| `Vendor` | enum |  | rename | `ProtectionVendor` | too generic as `scrape::Vendor`; these are bot-protection vendors, while the widget is core's `Captcha` (`Evidence.widget`) |
| `Vendor::Cloudflare` | enum variant |  | keep |  | names the case |
| `Vendor::DataDome` | enum variant |  | keep |  | names the case |
| `Vendor::PerimeterX` | enum variant |  | keep |  | names the case |
| `Vendor::Akamai` | enum variant |  | keep |  | the company; codec's `Akamai` is the HTTP/2 format named after its research, documented as such |
| `Vendor::Imperva` | enum variant |  | keep |  | names the case |
| `Vendor::Kasada` | enum variant |  | keep |  | names the case |
| `Vendor::AwsWaf` | enum variant |  | keep |  | names the case |

### `scrape::challenge::mitigation`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `Action` | enum | yes | private | `Mitigation` | **copied**; the runtime acts on it, a SeleniumBase caller sets the policy and never matches on the step |
| `Action::Proceed` | enum variant | yes | private |  | goes with its type (plumbing) |
| `Action::Retry` | enum variant | yes | private |  | goes with its type (plumbing) |
| `Action::Escalate` | enum variant | yes | private |  | goes with its type (plumbing) |
| `Action::Solve` | enum variant | yes | private |  | goes with its type (plumbing) |
| `Action::RotateEgress` | enum variant | yes | private |  | goes with its type (plumbing) |
| `Action::RotateProfile` | enum variant | yes | private |  | goes with its type (plumbing) |
| `Action::Cooldown` | enum variant | yes | private |  | goes with its type (plumbing) |
| `Action::GiveUp` | enum variant | yes | private |  | goes with its type (plumbing) |
| `AttemptContext` | struct |  | private |  | the input of `decide`, built by the runtime |
| `AttemptContext.attempt` | field |  | private |  | goes with its type (plumbing) |
| `AttemptContext.mode` | field |  | private |  | goes with its type (plumbing) |
| `AttemptContext.consecutive_challenges` | field |  | private |  | goes with its type (plumbing) |
| `AttemptContext.can_rotate_egress` | field |  | private |  | goes with its type (plumbing) |
| `AttemptContext.can_escalate` | field |  | private |  | goes with its type (plumbing) |
| `AttemptContext.now` | field |  | private |  | goes with its type (plumbing) |
| `GiveUpReason` | enum |  | keep |  | says what it is; no core item of that name or concept |
| `GiveUpReason::AttemptsExhausted` | enum variant |  | keep |  | names the case |
| `GiveUpReason::Blocked` | enum variant |  | keep |  | names the case |
| `GiveUpReason::NoEgressLeft` | enum variant |  | keep |  | names the case |
| `MitigationPolicy` | struct | yes | merge | `ChallengePolicy` (holding core `SolveOptions` and `RetryPolicy`) | **copied**; repeats core's `SolveOptions` (`attempts`, `backoff`) and `RetryPolicy`. One `ChallengePolicy`, the `<Thing>Policy` pattern of `StealthPolicy`, with core's `SolveOptions` for the widget plus the rotate and cooldown rules |
| `MitigationPolicy.max_challenge_attempts` | field | yes | merge | core `SolveOptions::attempts` | the same setting |
| `MitigationPolicy.backoff` | field | yes | keep |  | already core's `RetryPolicy` |
| `MitigationPolicy.rotate_egress_after` | field | yes | rename | `rotate_proxy_after` | egress becomes core's proxy route |
| `MitigationPolicy.rotate_profile_after` | field | yes | rename | `rotate_fingerprint_after` | a profile in core is a browser profile |
| `MitigationPolicy.block_cooldown_secs` | field | yes | keep |  | says what it does |
| `MitigationPolicy::decide` | method | yes | private |  | the runtime calls it |
| `Mode` | enum |  | rename | `FetchKind` | core says mode for engine modes (UC mode, CDP Mode, `DriverMode`); this says whether a request goes through the browser or over HTTP. `Route` is taken by `egress::route` |
| `Mode::Browser` | enum variant |  | keep |  | names the case |
| `Mode::Http` | enum variant |  | keep |  | names the case |

### `scrape::egress::geo`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CountryCode` | struct | yes | merge | core `evasion::randomize` (the country of a `Locale`) | **copied** (`CountryCode` with a `Locale` table); core's `Locale` and `LocaleCode` already name locales |
| `CountryCode::new` | method | yes | merge |  | goes with its type |
| `CountryCode::as_str` | method | yes | merge |  | goes with its type |
| `GeoResolver` | trait | yes | keep |  | **copied** port, but nothing in core finds a proxy's exit country; it becomes a launch option (decision 2: an extension trait here on core's `LaunchOptionsBuilder`, or a core field) |
| `GeoResolver::resolve` | method | yes | keep |  | says what it does |
| `LocaleExt` | trait | yes | merge | inherent methods of core `Locale` | **copied** (the `Locale` table); an extension trait exists only because `Locale` lives in core |
| `LocaleExt::for_country` | method | yes | merge | `Locale::for_country` | with the trait |
| `LocaleExt::by_tag` | method | yes | merge | `Locale::by_tag` | with the trait |
| `LocaleExt::localize` | method | yes | merge | `Locale::localize` | with the trait |
| `StaticGeoResolver` | struct |  | private |  | the default resolver: a caller who wants it passes nothing |
| `GeoError` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `GeoError::InvalidCountry` | enum variant |  | keep |  | names the case |
| `GeoError::NoLocale` | enum variant |  | keep |  | names the case |
| `GeoError::Unknown` | enum variant |  | keep |  | names the case |
| `GeoInfo` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `GeoInfo.country` | field |  | keep |  | plain data name |
| `GeoInfo.timezone` | field |  | keep |  | plain data name |
| `GeoInfo.coordinates` | field |  | keep |  | make its type core's `Geolocation` instead of `(f64, f64)` |

### `scrape::egress::pool`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ProxyPool` | struct | yes | merge | core `config::proxy_list::ProxyList` | **copied** (`ProxyPool` with `RotationStrategy`); core already has `ProxyList`, `--proxy-list` and `SB_PROXY_LIST`, and picks from it |
| `ProxyPool::new` | method | yes | merge |  | goes with its type |
| `ProxyPool::with_geo_proxies` | method | yes | merge | `ProxyList::from_geo_proxies` | a constructor; core says `from_*` (`ProxyList::from_file`) |
| `ProxyPool::quarantine` | method | yes | merge |  | goes with its type |
| `ProxyPool::pick` | method | yes | merge |  | goes with its type |
| `ProxyPool::pick_in` | method | yes | merge |  | goes with its type |
| `ProxyPool::pick_other` | method | yes | merge |  | goes with its type |
| `ProxyPool::report` | method | yes | merge |  | goes with its type |
| `RotationStrategy` | enum | yes | merge | core `ProxyList` (its pick order) | **copied**; goes with `ProxyPool` |
| `RotationStrategy::RoundRobin` | enum variant | yes | merge |  | goes with its type |
| `RotationStrategy::Random` | enum variant | yes | merge |  | goes with its type |
| `RotationStrategy::LeastFailures` | enum variant | yes | merge |  | goes with its type |
| `RotationStrategy::StickyPerSite` | enum variant | yes | merge |  | goes with its type |
| `EgressOutcome` | enum |  | merge | core `ProxyList` (what `report` records) | goes with `ProxyPool` |
| `EgressOutcome::Success` | enum variant |  | merge |  | goes with its type |
| `EgressOutcome::ConnectError` | enum variant |  | merge |  | goes with its type |
| `EgressOutcome::Challenged` | enum variant |  | merge |  | goes with its type |
| `EgressOutcome::Blocked` | enum variant |  | merge |  | goes with its type |

### `scrape::egress::redact`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `RedactedUrl` | struct | yes | private |  | **copied** (credential redaction); plumbing for log lines. Core's `Proxy` and `ContextRoute` already keep passwords out of `Debug` |
| `RedactedUrl::new` | method | yes | private |  | goes with its type (plumbing) |
| `RedactedUrl::placeholder` | method | yes | private |  | goes with its type (plumbing) |
| `RedactedUrl::as_str` | method | yes | private |  | goes with its type (plumbing) |
| `Secret` | struct | yes | merge | core `common` (one `Secret`) | **copied** (credential redaction); one `Secret` for every password the project holds. Core security code has its own owner |
| `Secret::new` | method | yes | merge |  | goes with its type |
| `Secret::expose` | method | yes | merge |  | goes with its type |

### `scrape::egress::route`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `EgressKey` | struct |  | private |  | the pool's key for a proxy; once the pool is `ProxyList`, callers never build one |
| `EgressKey.scheme` | field |  | private |  | goes with its type (plumbing) |
| `EgressKey.host` | field |  | private |  | goes with its type (plumbing) |
| `EgressKey.port` | field |  | private |  | goes with its type (plumbing) |
| `EgressKey.country` | field |  | private |  | goes with its type (plumbing) |
| `EgressRef` | struct | yes | merge | core `ContextRoute` (a `Proxy` and its bypass list) plus the exit country | **copied** (`EgressRef`); core's `ContextRoute` is the proxy a context routes its tabs through, kept so plain HTTP requests take the same route: the same concept |
| `EgressRef::parse` | method | yes | merge |  | goes with its type |
| `EgressRef::key` | method | yes | merge |  | goes with its type |
| `EgressRef::proxy` | method | yes | merge | `ContextRoute.proxy` | goes with its type |
| `EgressError` | enum |  | merge | the error of core `Proxy::parse` | parses the same proxy URL |
| `EgressError::Scheme` | enum variant |  | merge |  | goes with its type |
| `EgressError::Address` | enum variant |  | merge |  | goes with its type |

### `scrape::error`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ScrapeError` | enum |  | keep |  | the crate's error, as core's `SeleniumBaseError` |
| `ScrapeError::Egress` | enum variant |  | rename | `Proxy` | egress becomes core's proxy route |
| `ScrapeError::Geo` | enum variant |  | keep |  | names the case |
| `ScrapeError::State` | enum variant |  | rename | `Session` | the store becomes core's `SessionStore` |
| `ScrapeError::Identity` | enum variant |  | merge | `ScrapeError::Session` | the identity becomes core's `Session` |
| `ScrapeError::Fingerprint` | enum variant |  | keep |  | names the case |
| `ScrapeError::Unsolved` | enum variant |  | keep |  | names the case |
| `ScrapeError::GaveUp` | enum variant |  | keep |  | names the case |
| `ScrapeError::CoolingDown` | enum variant |  | keep |  | names the case |
| `ScrapeError::Navigation` | enum variant |  | keep |  | names the case |
| `ScrapeError::NoEgressAvailable` | enum variant |  | rename | `NoProxyLeft` | same |
| `ScrapeError::Browser` | enum variant |  | keep |  | names the case |

### `scrape::jar::store`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CookieJar` | struct | yes | private |  | **copied** (RFC 6265 jar); the HTTP session's plumbing. Callers see cookies as core `Cookie`s in a core `Session` |
| `CookieJar::new` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::len` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::is_empty` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::cookies` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::store` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::header_for` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::from_cdp` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::to_cdp` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::expiry_of` | method | yes | private |  | goes with its type (plumbing) |
| `CookieJar::remove_expired` | method | yes | private |  | goes with its type (plumbing) |
| `StoredCookie` | struct |  | private |  | goes with `CookieJar` |
| `StoredCookie.name` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.value` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.domain` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.host_only` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.path` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.expires` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.secure` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.http_only` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.same_site` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.created` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie.last_access` | field |  | private |  | goes with its type (plumbing) |
| `StoredCookie::is_expired` | method |  | private |  | goes with its type (plumbing) |
| `StoredCookie::size_bytes` | method |  | private |  | goes with its type (plumbing) |
| `StoredCookie::same_identity` | method |  | private |  | goes with its type (plumbing) |
| `StoreOutcome` | enum |  | private |  | goes with `CookieJar` |
| `StoreOutcome::Stored` | enum variant |  | private |  | goes with its type (plumbing) |
| `StoreOutcome::Replaced` | enum variant |  | private |  | goes with its type (plumbing) |
| `StoreOutcome::Deleted` | enum variant |  | private |  | goes with its type (plumbing) |
| `StoreOutcome::Ignored` | enum variant |  | private |  | goes with its type (plumbing) |
| `SiteContext` | enum |  | private |  | goes with `CookieJar` |
| `SiteContext::SameSite` | enum variant |  | private |  | goes with its type (plumbing) |
| `SiteContext::CrossSite` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason` | enum |  | private |  | goes with `CookieJar` |
| `IgnoreReason::Malformed` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::DomainMismatch` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::PublicSuffix` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::IpLiteralDomain` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::SecureFromInsecure` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::HostPrefix` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::SecurePrefix` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::TooLarge` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::SameSiteNoneInsecure` | enum variant |  | private |  | goes with its type (plumbing) |
| `IgnoreReason::UnsupportedUrl` | enum variant |  | private |  | goes with its type (plumbing) |

### `scrape::ledger::events`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CollectingEventSink` | struct |  | remove |  | a test subscriber collects `tracing` events |
| `CollectingEventSink::new` | method |  | remove |  | goes with its type |
| `CollectingEventSink::events` | method |  | remove |  | goes with its type |
| `CollectingEventSink::take` | method |  | remove |  | goes with its type |
| `CollectingEventSink::len` | method |  | remove |  | goes with its type |
| `CollectingEventSink::is_empty` | method |  | remove |  | goes with its type |
| `EventSink` | trait | yes | remove |  | **copied** (`EventSink`); core reports through `tracing`; a subscriber already does what a sink does |
| `EventSink::emit` | method | yes | remove |  | goes with its type |
| `NoopEventSink` | struct | yes | remove |  | **copied** (`NoopEventSink`); no subscriber is the no-op |
| `ScraperEvent` | enum | yes | private | `StealthEvent`, sent as `tracing` events | **copied** (`ScraperEvent`); core reports through `tracing`; a subscriber already does what a sink does. Decision 7 |
| `ScraperEvent::Request` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::ChallengeDetected` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::ChallengeSolved` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::ActionChosen` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::ModeChanged` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::EgressRotated` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::ProfileRotated` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::ClearanceExpiring` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::CooldownStarted` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::IdentityExported` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::IdentityRestored` | enum variant | yes | private |  | goes with its type (plumbing) |
| `ScraperEvent::name` | method | yes | private |  | goes with its type (plumbing) |

### `scrape::ledger::state`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `DomainState` | struct | yes | merge | `SiteRecord`, kept by core `SessionStore` | **copied** (`DomainState`); **clash** with core `sb_cdp::state`. One site's challenge history |
| `DomainState.site` | field | yes | merge |  | goes with its type |
| `DomainState.successes` | field | yes | merge |  | goes with its type |
| `DomainState.challenges` | field | yes | merge |  | goes with its type |
| `DomainState.blocks` | field | yes | merge |  | goes with its type |
| `DomainState.consecutive_challenges` | field | yes | merge |  | goes with its type |
| `DomainState.last_challenge` | field | yes | merge |  | goes with its type |
| `DomainState.last_seen` | field | yes | merge |  | goes with its type |
| `DomainState.cooldown_until` | field | yes | merge |  | goes with its type |
| `DomainState.preferred_mode` | field | yes | merge |  | goes with its type |
| `DomainState.last_egress` | field | yes | merge |  | goes with its type |
| `DomainState::new` | method | yes | merge |  | goes with its type |
| `DomainState::record` | method | yes | merge |  | goes with its type |
| `DomainState::start_cooldown` | method | yes | merge |  | goes with its type |
| `DomainState::in_cooldown` | method | yes | merge |  | goes with its type |
| `InMemoryStateStore` | struct | yes | merge | core `SessionStore` | **copied**; `SessionStore` is the in-memory store |
| `InMemoryStateStore::new` | method | yes | merge |  | goes with its type |
| `StateStore` | trait | yes | merge | core `SessionStore` | **copied** (`StateStore` port); core's `SessionStore` already keeps a run's saved state by name (`save`, `load`, `prune_expired`) |
| `StateStore::get` | method | yes | merge |  | goes with its type |
| `StateStore::put` | method | yes | merge |  | goes with its type |
| `StateStore::remove` | method | yes | merge |  | goes with its type |
| `StateStore::sites` | method | yes | merge |  | goes with its type |
| `StateError` | enum |  | merge | `ScrapeError::Session` | goes with the store |
| `StateError::Io` | enum variant |  | merge |  | goes with its type |
| `StateError::Corrupt` | enum variant |  | merge |  | goes with its type |
| `StateError::Schema` | enum variant |  | merge |  | goes with its type |
| `StateError::TooLarge` | enum variant |  | merge |  | goes with its type |

### `scrape::passport::identity`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `EmulationRef` | struct |  | rename | `WireMeasurementRef` | follows codec `WireMeasurement`; becomes a field of the `Session` |
| `EmulationRef.kind` | field |  | keep |  | plain data name |
| `EmulationRef.major` | field |  | keep |  | plain data name |
| `EmulationRef.os` | field |  | keep |  | plain data name |
| `IdentityError` | enum |  | merge | `ScrapeError::Session` | the identity becomes core's `Session` |
| `IdentityError::EgressMismatch` | enum variant |  | merge |  | goes with its type |
| `IdentityError::Schema` | enum variant |  | merge |  | goes with its type |
| `IdentityError::Incoherent` | enum variant |  | merge |  | goes with its type |
| `IdentityError::EmulationMismatch` | enum variant |  | merge |  | goes with its type |
| `StealthIdentity` | struct | yes | merge | core `Session` (plus fingerprint, wire measurement, route and clearances) | **copied** (`StealthIdentity`); core's `Session` is a browser's login state (cookies, storage) and `SessionStore` saves it, and `BrowserScraper` already has both `export_identity` and `save_session`. One concept: the saved `Session`. Decision 1 |
| `StealthIdentity.schema` | field | yes | merge |  | goes with its type |
| `StealthIdentity.fingerprint` | field | yes | merge |  | goes with its type |
| `StealthIdentity.emulation` | field | yes | merge |  | goes with its type |
| `StealthIdentity.cookies` | field | yes | merge |  | goes with its type |
| `StealthIdentity.accept_ch` | field | yes | merge |  | goes with its type |
| `StealthIdentity.egress` | field | yes | merge |  | goes with its type |
| `StealthIdentity.clearances` | field | yes | merge |  | goes with its type |
| `StealthIdentity.created_at` | field | yes | merge |  | goes with its type |
| `StealthIdentity::new` | method | yes | merge |  | goes with its type |
| `StealthIdentity::verify` | method | yes | merge |  | goes with its type |
| `StealthIdentity::refresh_clearances` | method | yes | merge |  | goes with its type |
| `StealthIdentity::clearance_for` | method | yes | merge |  | goes with its type |
| `StealthIdentity::check_egress` | method | yes | merge |  | goes with its type |
| `StealthIdentity::check_emulation` | method | yes | merge |  | goes with its type |
| `Clearance` | struct |  | keep, export |  | becomes part of the `Session`; nothing in core holds a challenge clearance |
| `Clearance.site` | field |  | keep |  | plain data name |
| `Clearance.vendor` | field |  | keep |  | plain data name |
| `Clearance.cookie` | field |  | keep |  | plain data name |
| `Clearance.expires_at` | field |  | keep |  | plain data name |

### `scrape::passport::policy`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `SessionPolicy` | struct | yes | rename | `EscalationPolicy` | **copied** (`SessionPolicy`, same fields); nothing in core decides HTTP or browser, so it keeps a SeleniumBase-shaped name and becomes a field of `HttpSessionOptions` |
| `SessionPolicy.require_clearance_for_http` | field | yes | keep |  | plain data name |
| `SessionPolicy.clearance_margin` | field | yes | keep |  | plain data name |
| `SessionPolicy.demote_after_solve` | field | yes | keep |  | plain data name |
| `SessionPolicy.prefer` | field | yes | keep |  | says what it does |
| `SessionPolicy::before_fetch` | method | yes | private |  | the runtime calls it |
| `SessionPolicy::after_fetch` | method | yes | private |  | the runtime calls it |
| `Transition` | enum |  | private | `Escalation` | **clash** with core `api::presentation::Transition`; only the runtime reads it once `preflight` and `HttpVerdict` are private |
| `Transition::Stay` | enum variant |  | private |  | goes with its type (plumbing) |
| `Transition::Escalate` | enum variant |  | private |  | goes with its type (plumbing) |
| `Transition::Demote` | enum variant |  | private |  | goes with its type (plumbing) |
| `Transition::resulting_mode` | method |  | private |  | goes with its type (plumbing) |
| `EscalateReason` | enum |  | private |  | goes with `Transition` |
| `EscalateReason::Challenge` | enum variant |  | private |  | goes with its type (plumbing) |
| `EscalateReason::ClearanceExpiring` | enum variant |  | private |  | goes with its type (plumbing) |
| `EscalateReason::NoClearance` | enum variant |  | private |  | goes with its type (plumbing) |
| `EscalateReason::SiteNeedsBrowser` | enum variant |  | private |  | goes with its type (plumbing) |
| `EscalateReason::Manual` | enum variant |  | private |  | goes with its type (plumbing) |
| `DemoteReason` | enum |  | private |  | goes with `Transition` |
| `DemoteReason::ChallengeSolved` | enum variant |  | private |  | goes with its type (plumbing) |
| `DemoteReason::Manual` | enum variant |  | private |  | goes with its type (plumbing) |

### `scrape::runtime::browser_scraper`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `BrowserFetch` | struct |  | merge | `FetchOutcome` | three result types for one fetch (`BrowserFetch`, `StealthFetch`, `HttpFetched`) |
| `BrowserFetch.url` | field |  | merge |  | goes with its type |
| `BrowserFetch.status` | field |  | merge |  | goes with its type |
| `BrowserFetch.headers` | field |  | merge |  | goes with its type |
| `BrowserFetch.body` | field |  | merge |  | goes with its type |
| `BrowserFetch.detection` | field |  | merge |  | goes with its type |
| `BrowserFetch.attempts` | field |  | merge |  | goes with its type |
| `BrowserFetch.solved` | field |  | merge |  | goes with its type |
| `BrowserFetch.egress` | field |  | merge |  | goes with its type |
| `BrowserLauncher` | trait |  | private |  | a test seam; core's `Browser::launch_mocked` and `BrowserPool::with_launcher` already cover launching |
| `BrowserLauncher::launch` | method |  | private |  | goes with its type (plumbing) |
| `BrowserScraper` | struct | yes | merge | core `sb_cdp::Browser` and `Page`, through launch options and page methods | **copied** (their `CloudScraper`: the same builder options, `solve_challenge`, `rotate_profile`); it wraps core's `Browser` and `Page` and repeats their jobs. A SeleniumBase caller launches with `LaunchOptions` or the `sb_cdp` fixture and calls methods on `Page`. Decision 2 |
| `BrowserScraper::builder` | method | yes | merge | `LaunchOptions::builder()` and `Browser::launch` | the launch builder is the entry point |
| `BrowserScraper::page` | method | yes | remove |  | the caller holds the core `Page` |
| `BrowserScraper::browser` | method | yes | remove |  | the caller holds the core `Browser` |
| `BrowserScraper::fingerprint` | method | yes | remove |  | the caller passed it to core `LaunchOptionsBuilder::fingerprint`; `Page::capture_fingerprint` reads it back |
| `BrowserScraper::seed` | method | yes | remove |  | core `Fingerprint::seed` |
| `BrowserScraper::egress` | method | yes | remove |  | core `Browser::context_route` returns it |
| `BrowserScraper::sessions` | method | yes | remove |  | the caller's core `SessionStore` |
| `BrowserScraper::close` | method | yes | remove |  | core `Browser::close` |
| `BrowserScraper::fetch` | method | yes | merge | a `Page` method (open a URL through challenges) | the page is the entry point |
| `BrowserScraper::solve_challenge` | method | yes | merge | core `Page::solve_captcha` | **copied**; core already solves the widget on a page, and the policy adds the rotate and cooldown steps |
| `BrowserScraper::rotate_egress` | method | yes | merge | a `Page` method `rotate_proxy` | a new context with the next `ProxyList` proxy (core `Browser::new_context_with`) |
| `BrowserScraper::rotate_profile` | method | yes | merge | core `Page::apply_fingerprint` with a new `Fingerprint` | **copied** (`rotate_profile`); core already applies a fingerprint to a page |
| `BrowserScraper::export_identity` | method | yes | remove |  | repeats `save_session` on the same type |
| `BrowserScraper::import_identity` | method | yes | remove |  | repeats `load_session` on the same type |
| `BrowserScraper::save_session` | method | yes | merge | a `Page` method over core `SessionStore` | keeps the name |
| `BrowserScraper::load_session` | method | yes | merge | a `Page` method over core `SessionStore` | keeps the name |
| `BrowserScraperBuilder` | struct | yes | merge | core `LaunchOptionsBuilder` | **copied** (their `CloudScraper` builder); it wraps `LaunchOptionsBuilder` (`launch_options`) and adds stealth options, which join the launch builder. Decision 2 |
| `BrowserScraperBuilder::launch_options` | method | yes | remove |  | it is the launch builder |
| `BrowserScraperBuilder::with_geo_proxies` | method | yes | merge | `LaunchOptionsBuilder::proxy_list(ProxyList)` (core `--proxy-list`) | **copied** |
| `BrowserScraperBuilder::proxy_strategy` | method | yes | merge | `ProxyList`'s pick order | **copied** |
| `BrowserScraperBuilder::with_max_challenge_attempts` | method | yes | merge | core `SolveOptions::attempts` in `ChallengePolicy` | **copied** |
| `BrowserScraperBuilder::mitigation` | method | yes | merge | `LaunchOptionsBuilder::challenge_policy` | a launch option (decision 2: an extension trait here on core's `LaunchOptionsBuilder`, or a core field) |
| `BrowserScraperBuilder::with_state_store` | method | yes | merge | core `SessionStore` | **copied** |
| `BrowserScraperBuilder::with_event_sink` | method | yes | remove |  | **copied**; events go to `tracing` |
| `BrowserScraperBuilder::geo_resolver` | method | yes | merge | `LaunchOptionsBuilder::geo_resolver` | a launch option (decision 2: an extension trait here on core's `LaunchOptionsBuilder`, or a core field) |
| `BrowserScraperBuilder::per_site_rate` | method | yes | merge | `LaunchOptionsBuilder::rate_limit` (core `RateLimiter`) | core `RateLimiter::new(max_calls, window)` is the same setting |
| `BrowserScraperBuilder::seed` | method | yes | remove |  | pass a seeded core `Fingerprint` |
| `BrowserScraperBuilder::fingerprint` | method | yes | remove |  | core `LaunchOptionsBuilder::fingerprint` already exists; this is a second copy of it |
| `BrowserScraperBuilder::egress_switch` | method | yes | remove |  | core contexts carry their proxy (`ContextOptions::proxy`) |
| `BrowserScraperBuilder::launcher` | method | yes | private |  | a test seam; core's `Browser::launch_mocked` and `BrowserPool::with_launcher` already cover launching |
| `BrowserScraperBuilder::session_store` | method | yes | merge | `LaunchOptionsBuilder::session_store` | a launch option (decision 2: an extension trait here on core's `LaunchOptionsBuilder`, or a core field) |
| `BrowserScraperBuilder::solve_timeout` | method | yes | merge | core `SolveOptions` timeouts | the same kind of setting |
| `BrowserScraperBuilder::clock` | method | yes | private |  | a test seam |
| `BrowserScraperBuilder::build` | method | yes | remove |  | core `LaunchOptionsBuilder::build` and `Browser::launch` |
| `ChromeLauncher` | struct |  | private |  | a test seam; core's `Browser::launch_mocked` and `BrowserPool::with_launcher` already cover launching |

### `scrape::runtime::egress_switch`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ContextProxySwitch` | struct |  | remove |  | core's `Browser::new_context_with(ContextOptions::proxy)` already switches a context's proxy |
| `ContextProxySwitch::new` | method |  | remove |  | goes with its type |
| `EgressSwitch` | trait |  | remove |  | same: core contexts carry their proxy |
| `EgressSwitch::switch` | method |  | remove |  | goes with its type |

### `scrape::runtime::json_store`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `JsonStateStore` | struct | yes | merge | a file form of core `SessionStore` | **copied**; one store, written to a file, instead of a second store type |
| `JsonStateStore::open` | method | yes | merge |  | goes with its type |
| `JsonStateStore::path` | method | yes | merge |  | goes with its type |

### `scrape::runtime::tracing_sink`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `TracingEventSink` | struct | yes | remove |  | **copied** (their `LogEventSink`); the events go to `tracing` directly |

### `scrape::runtime::browser_scraper::builder`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `Clock` | type alias |  | private |  | a test seam; core has another `Clock` (`core::allure`) |

## mimic (`seleniumbase_rs_stealth::mimic`)

### `mimic::client`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `impersonation_client` | function | yes | private |  | **copied** (`impersonation_client`, same name and shape); `HttpSession` is the entry point. If it stays public, `wire_client`. Decision 6 |
| `CustomProfileBuilder` | struct |  | rename | `WireFormatBuilder` | follows `WireFormat`; profile means a browser profile in core |
| `CustomProfileBuilder::new` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::base` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::base_on` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::ja3` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::ja4_r` | method |  | keep |  | JA4_r is the format's name |
| `CustomProfileBuilder::ja4_ro` | method |  | keep |  | JA4_ro is the format's name |
| `CustomProfileBuilder::peetprint` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::signature_algorithms` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::grease` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::permute_extensions` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::akamai` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::user_agent` | method |  | merge | core `Fingerprint` | set it on the fingerprint; the custom input stays, in one place |
| `CustomProfileBuilder::accept_language` | method |  | merge | core `Fingerprint` | same |
| `CustomProfileBuilder::client_hints` | method |  | merge | core `Fingerprint` | same |
| `CustomProfileBuilder::header` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::without_header` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::header_order` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::priority` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::headers` | method |  | keep |  | says what it does |
| `CustomProfileBuilder::build` | method |  | keep |  | says what it does |
| `ImpersonateError` | enum |  | rename | `WireFormatError` | follows `WireFormat` |
| `ImpersonateError::Ja3` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Ja4` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Peetprint` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Akamai` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Tls` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Http2` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Emulation` | enum variant |  | rename | `Measurement` | follows `WireMeasurement` |
| `ImpersonateError::Headers` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Convert` | enum variant |  | keep |  | names the case |
| `ImpersonateError::VersionMismatch` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Incomplete` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Fingerprint` | enum variant |  | keep |  | names the case |
| `ImpersonateError::UnknownHeader` | enum variant |  | keep |  | names the case |
| `ImpersonateError::Build` | enum variant |  | keep |  | names the case |
| `ImpersonationProfile` | struct | yes | rename | `WireFormat` | **copied** (their `BrowserProfile`, `random()`); its User-Agent, Accept-Language and client hints repeat core's `Fingerprint`. Keep the TLS, HTTP/2 and header-order part and read the rest from the `Fingerprint` |
| `ImpersonationProfile::measured` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::for_system` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::from_measured` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::random` | method | yes | remove |  | **copied** (`BrowserProfile::random`); draw a seeded core `Fingerprint` and call `from_fingerprint` |
| `ImpersonationProfile::from_fingerprint` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::with_locale` | method | yes | remove |  | core `Fingerprint`'s locale |
| `ImpersonationProfile::custom` | method | yes | rename | `builder` | builders start from `builder()` (core `LaunchOptions::builder`) |
| `ImpersonationProfile::validate` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::source` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::kind` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::os` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::major` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::user_agent` | method | yes | remove |  | core `Fingerprint` holds it |
| `ImpersonationProfile::accept_language` | method | yes | remove |  | core `Fingerprint` holds it |
| `ImpersonationProfile::tls_spec` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::http2_spec` | method | yes | rename | `h2_spec` | the type is `H2Spec` |
| `ImpersonationProfile::header_spec` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::identity` | method | yes | remove |  | the client headers merge into `Fingerprint` |
| `ImpersonationProfile::known_gaps` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::dropped_settings` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::headers_for` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::apply` | method | yes | keep |  | says what it does |
| `WireProtocol` | enum |  | keep |  | and fold `ReplyProtocol` into it |
| `WireProtocol::Http1` | enum variant |  | keep |  | names the case |
| `WireProtocol::Http2` | enum variant |  | keep |  | names the case |

### `mimic::http_session`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `HttpAttempt` | struct |  | private |  | the input of `assess`, which becomes private |
| `HttpAttempt.number` | field |  | private |  | goes with its type (plumbing) |
| `HttpAttempt.previous_challenges` | field |  | private |  | goes with its type (plumbing) |
| `HttpAttempt.can_rotate_egress` | field |  | private |  | goes with its type (plumbing) |
| `HttpAttempt.can_escalate` | field |  | private |  | goes with its type (plumbing) |
| `HttpFetched` | struct |  | merge | `FetchOutcome` | three result types for one fetch (`BrowserFetch`, `StealthFetch`, `HttpFetched`) |
| `HttpFetched.reply` | field |  | merge |  | goes with its type |
| `HttpFetched.verdict` | field |  | merge |  | goes with its type |
| `ReplyHeaders` | struct |  | rename | `ResponseHeaders` | follows `HttpResponse`; the source calls it `Headers` |
| `ReplyHeaders::get` | method |  | keep |  | says what it does |
| `ReplyHeaders::get_all` | method |  | keep |  | says what it does |
| `ReplyHeaders::iter` | method |  | keep |  | says what it does |
| `ReplyHeaders::as_pairs` | method |  | keep |  | says what it does |
| `ReplyHeaders::len` | method |  | keep |  | says what it does |
| `ReplyHeaders::is_empty` | method |  | keep |  | says what it does |
| `HttpError` | enum |  | keep |  | follows `HttpSession` |
| `HttpError::Impersonate` | enum variant |  | rename | `WireFormat` | follows `WireFormat` |
| `HttpError::VersionMismatch` | enum variant |  | keep |  | names the case |
| `HttpError::Identity` | enum variant |  | rename | `Session` | the identity becomes core's `Session` |
| `HttpError::BrowserProxy` | enum variant |  | keep |  | names the case |
| `HttpError::ReservedHeader` | enum variant |  | keep |  | names the case |
| `HttpError::InvalidHeader` | enum variant |  | keep |  | names the case |
| `HttpError::ForbiddenMethod` | enum variant |  | keep |  | names the case |
| `HttpError::Url` | enum variant |  | keep |  | names the case |
| `HttpError::TooManyRedirects` | enum variant |  | keep |  | names the case |
| `HttpError::RedirectRefused` | enum variant |  | keep |  | names the case |
| `HttpError::BodyTooLarge` | enum variant |  | keep |  | names the case |
| `HttpError::RequestTooLarge` | enum variant |  | keep |  | names the case |
| `HttpError::Request` | enum variant |  | keep |  | names the case |
| `HttpError::Client` | enum variant |  | keep |  | names the case |
| `HttpError::Options` | enum variant |  | keep |  | names the case |
| `HttpError::NotUtf8` | enum variant |  | keep |  | names the case |
| `HttpError::Json` | enum variant |  | keep |  | names the case |
| `HttpError::Browser` | enum variant |  | keep |  | names the case |
| `HttpOptions` | struct |  | rename | `HttpSessionOptions` | **clash**: core `mcp::http::HttpOptions` is the MCP server's HTTP transport; options are named after their owner (core `PoolOptions`, `LaunchOptions`; here `MitmOptions`) |
| `HttpOptions::new` | method |  | keep |  | says what it does |
| `HttpOptions::timeout` | method |  | keep |  | says what it does |
| `HttpOptions::connect_timeout` | method |  | keep |  | says what it does |
| `HttpOptions::max_body_bytes` | method |  | keep |  | says what it does |
| `HttpOptions::max_request_bytes` | method |  | keep |  | says what it does |
| `HttpOptions::max_redirects` | method |  | keep |  | says what it does |
| `HttpOptions::follow_redirects` | method |  | keep |  | says what it does |
| `HttpOptions::pacing` | method |  | rename | `rate_limit` | core `RateLimiter`; the builders' `per_site_rate` is the same setting |
| `HttpOptions::event_sink` | method |  | remove |  | events go to `tracing` |
| `HttpOptions::trust` | method |  | keep |  | says what it does |
| `HttpOptions::resolve` | method |  | keep |  | says what it does |
| `HttpOptions::bypass` | method |  | merge | core `ContextRoute.bypass` | the same list |
| `HttpOptions::browser_proxy_exits_directly` | method |  | keep |  | says what it does |
| `HttpOptions::installed_chrome_major` | method |  | keep |  | says what it does |
| `HttpOptions::clock` | method |  | private |  | a test seam |
| `HttpOptions::validate` | method |  | keep |  | says what it does |
| `HttpReply` | struct |  | rename | `HttpResponse` | HTTP's word, and core's (`sb_cdp::intercept::Response`, `HarResponse`) |
| `HttpReply::status` | method |  | keep |  | says what it does |
| `HttpReply::is_success` | method |  | keep |  | says what it does |
| `HttpReply::redacted_url` | method |  | keep |  | says what it does |
| `HttpReply::url` | method |  | keep |  | says what it does |
| `HttpReply::headers` | method |  | keep |  | says what it does |
| `HttpReply::bytes` | method |  | keep |  | says what it does |
| `HttpReply::text` | method |  | keep |  | says what it does |
| `HttpReply::json` | method |  | keep |  | says what it does |
| `HttpReply::protocol` | method |  | keep |  | says what it does |
| `HttpReply::redirects` | method |  | keep |  | says what it does |
| `HttpReply::elapsed` | method |  | keep |  | says what it does |
| `HttpReply::detect` | method |  | rename | `challenge` | returns the merged `Challenge` |
| `HttpRequest` | struct |  | keep |  | the `Http` prefix keeps it apart from core's `sb_cdp::intercept::Request` |
| `HttpRequest::shape` | method |  | keep |  | says what it does |
| `HttpRequest::header` | method |  | keep |  | says what it does |
| `HttpRequest::body` | method |  | keep |  | says what it does |
| `HttpRequest::json` | method |  | keep |  | says what it does |
| `HttpRequest::form` | method |  | keep |  | says what it does |
| `HttpRequest::timeout` | method |  | keep |  | says what it does |
| `HttpRequest::send` | method |  | keep |  | says what it does |
| `HttpSession` | struct |  | keep |  | a live HTTP session, as core's `BrowserSession` is a live WebDriver one |
| `HttpSession::from_identity` | method |  | rename | `from_session` | takes a core `Session` |
| `HttpSession::request` | method |  | keep |  | says what it does |
| `HttpSession::get` | method |  | keep |  | says what it does |
| `HttpSession::profile` | method |  | rename | `wire_format` | follows `WireFormat` |
| `HttpSession::egress` | method |  | rename | `route` | returns a core `ContextRoute` |
| `HttpSession::cookie_count` | method |  | keep |  | says what it does |
| `HttpSession::assess` | method |  | private |  | the copied domain layer's port leaks here; the session calls it itself |
| `HttpSession::fetch` | method |  | keep |  | says what it does |
| `HttpSession::preflight` | method |  | private |  | same |
| `HttpSession::export_identity` | method |  | rename | `session` | returns a core `Session` |
| `HttpSession::from_page` | method |  | keep |  | says what it does |
| `HttpSession::sync_cookies_back` | method |  | keep |  | says what it does |
| `Method` | re-export |  | keep |  | re-export of `wreq::Method`, so callers import no wreq (the pattern of core's `drivers` and `json` re-exports) |
| `ReplyProtocol` | enum |  | remove | use `WireProtocol` | the same two values (HTTP/1.1, HTTP/2) as `WireProtocol` |
| `ReplyProtocol::Http2` | enum variant |  | remove |  | goes with its type |
| `ReplyProtocol::Http1` | enum variant |  | remove |  | goes with its type |
| `HttpRules` | struct |  | private |  | the input of `assess` and `preflight`; `HttpSessionOptions` holds the policies |
| `HttpRules.mitigation` | field |  | private |  | goes with its type (plumbing) |
| `HttpRules.session` | field |  | private |  | goes with its type (plumbing) |
| `HttpVerdict` | struct |  | private |  | the output of `assess` |
| `HttpVerdict.detection` | field |  | private |  | goes with its type (plumbing) |
| `HttpVerdict.action` | field |  | private |  | goes with its type (plumbing) |
| `HttpVerdict.transition` | field |  | private |  | goes with its type (plumbing) |
| `HttpVerdict::is_clear` | method |  | private |  | goes with its type (plumbing) |

### `mimic::mitm`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `MitmError` | enum |  | keep |  | follows `MitmProxy` |
| `MitmError::Bind` | enum variant |  | keep |  | names the case |
| `MitmError::Tls` | enum variant |  | keep |  | names the case |
| `MitmError::InvalidOptions` | enum variant |  | keep |  | names the case |
| `MitmError::Random` | enum variant |  | keep |  | names the case |
| `MitmError::Upstream` | enum variant |  | keep |  | names the case |
| `MitmError::Egress` | enum variant |  | keep |  | names the case |
| `MitmError::Unauthorized` | enum variant |  | keep |  | names the case |
| `MitmError::BadTarget` | enum variant |  | keep |  | names the case |
| `MitmError::ForbiddenDestination` | enum variant |  | keep |  | names the case |
| `MitmError::NotIntercepted` | enum variant |  | keep |  | names the case |
| `MitmError::HostMismatch` | enum variant |  | keep |  | names the case |
| `MitmError::MalformedUpgrade` | enum variant |  | keep |  | names the case |
| `MitmError::BodyTooLarge` | enum variant |  | keep |  | names the case |
| `MitmError::VersionMismatch` | enum variant |  | keep |  | names the case |
| `MitmError::UpstreamRequest` | enum variant |  | keep |  | names the case |
| `MitmOptions` | struct |  | keep |  | its limits stay settable through the launch option |
| `MitmOptions::max_connections` | method |  | keep |  | says what it does |
| `MitmOptions::max_header_bytes` | method |  | keep |  | says what it does |
| `MitmOptions::max_headers` | method |  | keep |  | says what it does |
| `MitmOptions::max_body_bytes` | method |  | keep |  | says what it does |
| `MitmOptions::idle_timeout` | method |  | keep |  | says what it does |
| `MitmOptions::handshake_timeout` | method |  | keep |  | says what it does |
| `MitmOptions::connect_timeout` | method |  | keep |  | says what it does |
| `MitmOptions::read_timeout` | method |  | keep |  | says what it does |
| `MitmOptions::websocket_idle_timeout` | method |  | keep |  | says what it does |
| `MitmOptions::max_websocket_bytes` | method |  | keep |  | says what it does |
| `MitmOptions::trust_upstream_root` | method |  | keep |  | says what it does |
| `MitmOptions::resolve` | method |  | keep |  | says what it does |
| `MitmOptions::allow_private_destinations` | method |  | keep |  | says what it does |
| `MitmOptions::ca` | method |  | keep |  | says what it does |
| `MitmProxy` | struct | yes | private |  | **copied** (their `TlsSpoofingProxy` with a hot-swappable upstream); plumbing behind a launch option (decision 2: an extension trait here on core's `LaunchOptionsBuilder`, or a core field), as core keeps proxy passwords inside the launch (`sb_cdp::proxy_auth`). Decision 6 |
| `MitmProxy::start` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::prime_url` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::port` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::local_addr` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::proxy_spec` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::browser_proxy` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::chrome_args` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::configure_launch` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::stats` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::egress` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::swap_upstream` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::swap_profile` | method | yes | private |  | goes with its type (plumbing) |
| `MitmProxy::shutdown` | method | yes | private |  | goes with its type (plumbing) |
| `MitmStats` | struct |  | private |  | goes with `MitmProxy` |
| `MitmStats.connections` | field |  | private |  | goes with its type (plumbing) |
| `MitmStats.tunnels` | field |  | private |  | goes with its type (plumbing) |
| `MitmStats.unauthorized` | field |  | private |  | goes with its type (plumbing) |
| `MitmStats.refused` | field |  | private |  | goes with its type (plumbing) |
| `MitmStats.forwarded` | field |  | private |  | goes with its type (plumbing) |
| `MitmStats.upstream_errors` | field |  | private |  | goes with its type (plumbing) |
| `MitmStats.websockets` | field |  | private |  | goes with its type (plumbing) |
| `MitmStats.primed` | field |  | private |  | goes with its type (plumbing) |

### `mimic::stealth_session`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `StealthError` | enum |  | merge | `HttpError` | goes with `StealthSession` |
| `StealthError::Scrape` | enum variant |  | merge |  | goes with its type |
| `StealthError::Http` | enum variant |  | merge |  | goes with its type |
| `StealthError::Mitm` | enum variant |  | merge |  | goes with its type |
| `StealthError::Impersonate` | enum variant |  | merge |  | goes with its type |
| `StealthFetch` | struct |  | merge | `FetchOutcome` | three result types for one fetch (`BrowserFetch`, `StealthFetch`, `HttpFetched`) |
| `StealthFetch.url` | field |  | merge |  | goes with its type |
| `StealthFetch.status` | field |  | merge |  | goes with its type |
| `StealthFetch.headers` | field |  | merge |  | goes with its type |
| `StealthFetch.body` | field |  | merge |  | goes with its type |
| `StealthFetch.detection` | field |  | merge |  | goes with its type |
| `StealthFetch.mode` | field |  | merge |  | goes with its type |
| `StealthFetch.attempts` | field |  | merge |  | goes with its type |
| `StealthFetch.solved` | field |  | merge |  | goes with its type |
| `StealthFetch.egress` | field |  | merge |  | goes with its type |
| `StealthSession` | struct | yes | merge | `HttpSession` made from a `Page` (`HttpSession::from_page`), which escalates back to that page | **copied** (`StealthSession`, dual mode); it holds a core `Page` and an `HttpSession` and repeats `BrowserScraper`'s options. The handoff already exists as `HttpSession::from_page` and `sync_cookies_back`. Decision 2 |
| `StealthSession::builder` | method | yes | merge |  | goes with its type |
| `StealthSession::mode` | method | yes | merge |  | goes with its type |
| `StealthSession::identity` | method | yes | merge |  | goes with its type |
| `StealthSession::egress` | method | yes | merge |  | goes with its type |
| `StealthSession::seed` | method | yes | merge |  | goes with its type |
| `StealthSession::http` | method | yes | merge |  | goes with its type |
| `StealthSession::page` | method | yes | merge |  | goes with its type |
| `StealthSession::close` | method | yes | merge |  | goes with its type |
| `StealthSession::fetch` | method | yes | merge |  | goes with its type |
| `StealthSession::rotate_profile` | method | yes | merge |  | goes with its type |
| `StealthSession::rotate_egress` | method | yes | merge |  | goes with its type |
| `StealthSession::export_identity` | method | yes | merge |  | goes with its type |
| `StealthSession::import_identity` | method | yes | merge |  | goes with its type |
| `StealthSession::escalate` | method | yes | merge |  | goes with its type |
| `StealthSession::demote` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder` | struct | yes | merge | core `LaunchOptionsBuilder` plus `HttpSessionOptions` | **copied** (`restore()` and the `CloudScraper` options a second time) |
| `StealthSessionBuilder::launch_options` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::launcher` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::with_geo_proxies` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::proxy_strategy` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::with_max_challenge_attempts` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::mitigation` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::policy` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::with_state_store` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::with_event_sink` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::geo_resolver` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::per_site_rate` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::seed` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::fingerprint` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::restore` | method | yes | merge | `HttpSession::from_session` | **copied** (`restore()`) |
| `StealthSessionBuilder::http_options` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::mitm_options` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::solve_timeout` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::clock` | method | yes | merge |  | goes with its type |
| `StealthSessionBuilder::build` | method | yes | merge |  | goes with its type |

### `mimic::tls_server`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CaOptions` | struct |  | keep |  | options named after their owner |
| `CaOptions.ca_validity` | field |  | keep |  | plain data name |
| `CaOptions.leaf_validity` | field |  | keep |  | plain data name |
| `CaOptions.leaf_renew_margin` | field |  | keep |  | plain data name |
| `CaOptions.leaf_capacity` | field |  | keep |  | plain data name |
| `EphemeralCa` | struct |  | private |  | the MITM proxy's plumbing |
| `EphemeralCa::generate` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::generate_at` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::spki_pins` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::chrome_spki_arg` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::leaf_spki_pin` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::certificate` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::options` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::not_after_unix` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::cached_leaves` | method |  | private |  | goes with its type (plumbing) |
| `EphemeralCa::leaf_for` | method |  | private |  | goes with its type (plumbing) |
| `Leaf` | struct |  | private |  | the MITM proxy's plumbing |
| `Leaf::certificate` | method |  | private |  | goes with its type (plumbing) |
| `Leaf::issuer` | method |  | private |  | goes with its type (plumbing) |
| `Leaf::host` | method |  | private |  | goes with its type (plumbing) |
| `Leaf::not_before_unix` | method |  | private |  | goes with its type (plumbing) |
| `Leaf::not_after_unix` | method |  | private |  | goes with its type (plumbing) |
| `TlsServerError` | enum |  | keep |  | follows the `tls_server` module |
| `TlsServerError::Ssl` | enum variant |  | keep |  | names the case |
| `TlsServerError::Handshake` | enum variant |  | keep |  | names the case |
| `TlsServerError::HandshakeTimeout` | enum variant |  | keep |  | names the case |
| `TlsServerError::InvalidHost` | enum variant |  | keep |  | names the case |
| `TlsServerError::InvalidAlpn` | enum variant |  | keep |  | names the case |
| `TlsServerError::InvalidOptions` | enum variant |  | keep |  | names the case |
| `TlsServerError::ClockOutOfRange` | enum variant |  | keep |  | names the case |
| `TlsServerError::CaExpired` | enum variant |  | keep |  | names the case |
| `TlsServerError::SniMismatch` | enum variant |  | keep |  | names the case |

### `mimic::convert`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ConvertError` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `ConvertError::UnsupportedAlpn` | enum variant |  | keep |  | names the case |
| `ConvertError::UnsupportedAlps` | enum variant |  | keep |  | names the case |
| `ConvertError::CipherName` | enum variant |  | keep |  | names the case |
| `ConvertError::SigalgName` | enum variant |  | keep |  | names the case |
| `ConvertError::GroupName` | enum variant |  | keep |  | names the case |
| `ConvertError::KeyShare` | enum variant |  | keep |  | names the case |
| `ConvertError::Tls13SuitesNotFirst` | enum variant |  | keep |  | names the case |
| `ConvertError::VersionGap` | enum variant |  | keep |  | names the case |
| `ConvertError::ExtensionAlwaysSent` | enum variant |  | keep |  | names the case |
| `ConvertError::ExtensionVersion` | enum variant |  | keep |  | names the case |
| `ConvertError::PaddingUnavailable` | enum variant |  | keep |  | names the case |
| `ConvertError::PaddingAppended` | enum variant |  | keep |  | names the case |
| `ConvertError::GroupNeedsTls13` | enum variant |  | keep |  | names the case |
| `ConvertError::CertCompression` | enum variant |  | keep |  | names the case |
| `ConvertError::StreamId` | enum variant |  | keep |  | names the case |
| `ConvertError::Weight` | enum variant |  | keep |  | names the case |
| `ConvertError::Header` | enum variant |  | keep |  | names the case |

### `mimic::http_session::options`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `Clock` | type alias |  | private |  | a test seam; core has another `Clock` |

