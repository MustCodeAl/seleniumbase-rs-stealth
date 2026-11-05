# Stealth names: every public item

Status: proposal (phase 1, second round). No code changes yet; the renames start once
the owner approves this list.

The rule: take other projects' features, never their API names. Name things after this
project's own patterns, and let one name mean one thing across seleniumbase-rs and this
repository. A feature that a core module already handles moves into that module instead
of keeping a second copy here.

## Counts

| | Items |
|---|---|
| Reviewed | 1049 |
| Keep | 924 (26 of them also need an export) |
| Rename | 91 (7 of them also need an export) |
| Move into core | 29 |
| Remove | 5 |

| Crate | Keep | Rename | Move | Remove |
|---|---|---|---|---|
| packages, features, facade | 10 | 3 | 0 | 1 |
| codec | 382 | 13 | 0 | 1 |
| scrape | 259 | 36 | 28 | 0 |
| mimic | 273 | 39 | 1 | 3 |

"Export" means a public signature takes or returns the type, but no path names it (its
module is private and `lib.rs` does not re-export it). There are 33 such types.

## What changed since the first round

The first round looked only at the names `lib.rs` re-exports and judged them in groups
(about 100 names, 3 verdict groups per crate). This round lists every item a caller can
reach, one row each: types, traits, functions, methods, fields, enum variants, associated
constants, builder options, re-exports, packages and features. It found 91 renames
instead of about 30, 33 types that callers receive but cannot name, and the builder,
`mode`, `profile`, `state` and `reply` words that run through many members.

## How this list was made

- **The surface.** TokenSave's index of this branch (`friend/stealth-names`, on
  `origin/main` `7820793`), read with SQL over its `nodes` table, because
  `tokensave_module_api` stops after about 60 of codec's 644 public symbols. Every
  top-level module of codec, scrape and mimic is private, so an item is public API only if
  `lib.rs` re-exports it, it is a public field, method, variant or associated constant of
  such a type (every method of an exported trait), or it is a public type that appears in
  one of those signatures (the "export" rows), with its members. The remaining `pub` items
  in private modules are not reachable and are not listed; phase 2 can make them
  `pub(crate)`. Trait impls (`From`, `Display`, `Serialize`) add no names and are not
  listed.
- **Core.** The names of all 133 stealth types were compared with core's 986 public types
  (`src/`, core at `41a2e3da`; the mise removal on that branch changes no public type).
  Exact clashes: `Emulation`, `Evidence`, `Transition`, `HttpOptions`, `Clock`. Clashes in
  concept come from searching core's types for the same words (proxy, cookie, session,
  identity, policy, profile, locale, geo, captcha, launcher, rate, retry, state, request,
  response) and reading the matching items and their methods.
- **Where a name came from.** Claimed only where it is well known (curl_cffi's
  "impersonate"). stealthscraper-rs was not researched, as the standing rule says.
- **Defaults.** A row with no specific finding says why it stays: a protocol or
  published-format term (JA3, JA4, Peetprint, Akamai, ClientHello, HPACK), the
  `<Area>Error` pattern, a plain field or variant name, or a member that says what it
  does. The rows that change carry the reason and the core item they follow.

## Core names the stealth items follow

| Core item | Where | What it means here |
|---|---|---|
| `Fingerprint`, `Fingerprint::seed`, `Fingerprint::validate` | `evasion::fingerprint` | `seed` and `fingerprint(...)` stay; `StealthIdentity::verify` becomes `validate` |
| `LaunchOptionsBuilder` (`fingerprint(&Fingerprint)`, plain setters), `PoolOptionsBuilder` | `sb_cdp::launch`, `sb_cdp::pool` | builders drop `with_` (`geo_proxies`, `max_challenge_attempts`, `ledger`, `event_sink`) |
| `PoolOptions`, `PoolStats`, `LaunchOptions` | `sb_cdp::pool`, `sb_cdp::launch` | options and stats are named after their owner: `HttpSessionOptions`, `MitmOptions`, `MitmStats` |
| `BrowserPool::with_launcher` | `sb_cdp::pool` | `BrowserLauncher` keeps the word |
| `Session`, `SessionStore` | `sb_cdp::session_store` | `BrowserScraper::sessions` becomes `session_store`; "session policy" becomes `EscalationPolicy` |
| `ProxyList`, `ProxyEntry`, `Proxy`, `ProxyConfig` | `config::proxy_list`, `sb_cdp::launch`, `evasion::fingerprint` | `ProxyPool` and `RotationStrategy` move into `ProxyList`; `EgressRef` keeps wrapping `Proxy` |
| `StealthPolicy`, `RetryPolicy`, `RateLimiter` | `evasion::policy`, `utilities::retry` | the `<Thing>Policy` pattern stays; `MitigationPolicy.backoff` is already a `RetryPolicy`; `per_site_rate` and `pacing` become `rate_limit` |
| `Captcha`, `Evidence` | `sb_cdp::captcha` | `Evidence` becomes `ChallengeEvidence`; `Vendor` becomes `ProtectionVendor` (its widget is already a `Captcha`) |
| `Locale`, `LocaleCode`, `Geolocation` | `evasion::randomize`, `browser::options::locale`, `evasion::settings` | `LocaleExt` and `CountryCode` move to `Locale`; `GeoInfo.coordinates` becomes a `Geolocation` |
| `Cookie`, `Cookies`, `Emulation` | `sb_cdp::state` | no `State` or `Emulation` names here: `Ledger`, `SiteRecord`, `WireMeasurement` |
| `launch_args` | `evasion::evasions` | `MitmProxy::chrome_args` becomes `launch_args` |
| `Browser` (enum), `OsType` | `browser::config`, `evasion::fingerprint` | `BrowserKind` stays while only Chrome is measured |
| `sb_cdp::identity` (holds `StealthPlan`; there is no `Identity` type) | `sb_cdp::identity` | `StealthIdentity` becomes `Identity` (decision 1) |

## The new words in one place

| Now | Proposed |
|---|---|
| emulation (codec's measured data) | wire measurement (`WireMeasurement`) |
| impersonation profile (mimic, what a client sends) | wire format (`WireFormat`) |
| header identity | client headers (`ClientHeaders`) |
| stealth identity | identity (`Identity`) |
| mode (browser or HTTP), transition, session policy | fetch kind (`FetchKind`), escalation (`Escalation`), escalation policy |
| state, state store, domain state | ledger (`Ledger`), site record (`SiteRecord`) |
| profile rotation | fingerprint rotation (`rotate_fingerprint`) |
| reply | response (`HttpResponse`, `ResponseHeaders`) |
| `with_*` builder setters | plain setters |
| `per_site_rate`, `pacing` | `rate_limit` |
| aliases in `lib.rs` (`Fetched as HttpFetched`, `SpecError as H2SpecError`, ...) | the exported name becomes the source name |

## Decisions for the owner

1. **"Identity".** Core has no `Identity` type; its `sb_cdp::identity` module holds
   `StealthPlan`. Should this repository's saved identity become `Identity`, and should
   core's module be renamed (for example `sb_cdp::plan`) so "identity" means one thing?
2. **`fingerprint(...)`.** `BrowserScraperBuilder::fingerprint` and
   `StealthSessionBuilder::fingerprint` set the fingerprint the browser presents;
   `BrowserScraper::fingerprint()` returns it. Core's `LaunchOptionsBuilder::fingerprint`
   uses the same name for the same thing (and takes `&Fingerprint`). This list keeps the
   name and proposes taking a reference, as core does. If the name itself should change
   (for example `present_fingerprint`), it changes in core too, in the same round.
3. **Moves into core** (`ProxyPool` into `ProxyList`, `LocaleExt` and `CountryCode` into
   `Locale`, `Secret` and `RedactedUrl` into `common`, one `Clock`): in the same round as
   the renames, or one core PR each, later? `Secret` touches security code that has its
   own owner.
4. **The `mimic` crate.** Rename the package `seleniumbase-rs-wire` (feature and facade
   path `wire`)?
5. **Two wire words.** `WireMeasurement` (codec: a browser's measured TLS, HTTP/2 and
   headers, with provenance) and `WireFormat` (mimic: the validated format a client sends,
   built from a measurement). Keep both, or one name for both?

## Phase 2 (after approval)

Renames with `tokensave rename` (a dry run first), every caller, the private module names
that follow their types (`emulation`, `ledger::state`, `client`), the stealth API reference
pages and the Stealth sections of the core docs, and the contract files
(`mise run contracts`). Breaking changes, no shims. One PR here, plus one in core for the
items that move there.

## Packages, features and the facade

### (packages)

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `seleniumbase-rs-stealth` (package) | package | keep |  | says what it adds to seleniumbase-rs |
| `seleniumbase-rs-codec` (package) | package | keep |  | it encodes and decodes wire fingerprints |
| `seleniumbase-rs-scrape` (package) | package | keep |  | the scraping runtime; the docs say scrape |
| `seleniumbase-rs-mimic` (package) | package | rename | `seleniumbase-rs-wire` | "mimic" does not say what it holds: the HTTP client that sends a browser's wire format, the MITM proxy and the session that switches between them; the docs call this wire formats. Decision 4 |

### (features)

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `default = ["scrape"]` | feature | keep |  |  |
| `codec` | feature | keep |  | named after its crate |
| `scrape` | feature | keep |  | named after its crate |
| `mimic` | feature | rename | `wire` | follows the package. Decision 4 |
| `test-fixtures` (codec) | feature | keep |  | says what it turns on |

### src/lib.rs

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `pub use seleniumbase_rs::*` | re-export | keep |  | the facade is core plus the stealth crates |
| `pub use seleniumbase_rs::evasion::*` | re-export | remove |  | gives every `evasion` item a second path at the root; core keeps them under `evasion::`, one path per item |
| `codec` (`seleniumbase_rs_codec`) | re-export | keep |  |  |
| `scrape` (`seleniumbase_rs_scrape`) | re-export | keep |  |  |
| `mimic` (`seleniumbase_rs_mimic`) | re-export | rename | `wire` | follows the package. Decision 4 |

## codec (`seleniumbase_rs_stealth::codec`)

### `codec::akamai`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `Akamai` | struct | keep |  | a protocol or published-format term, not another library's API |
| `Akamai.settings` | field | keep |  | plain data name |
| `Akamai.window_update` | field | keep |  | plain data name |
| `Akamai.priority_frames` | field | keep |  | plain data name |
| `Akamai.pseudo_order` | field | keep |  | plain data name |
| `Akamai::parse` | method | keep |  | says what it does |
| `AkamaiError` | enum | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `AkamaiError::TooLong` | enum variant | keep |  | names the case |
| `AkamaiError::Sections` | enum variant | keep |  | names the case |
| `AkamaiError::Setting` | enum variant | keep |  | names the case |
| `AkamaiError::DuplicateSetting` | enum variant | keep |  | names the case |
| `AkamaiError::TooManySettings` | enum variant | keep |  | names the case |
| `AkamaiError::WindowUpdate` | enum variant | keep |  | names the case |
| `AkamaiError::PriorityFrame` | enum variant | keep |  | names the case |
| `AkamaiError::TooManyPriorityFrames` | enum variant | keep |  | names the case |
| `AkamaiError::PseudoOrder` | enum variant | keep |  | names the case |
| `PriorityFrame` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `PriorityFrame.stream_id` | field | keep |  | plain data name |
| `PriorityFrame.exclusive` | field | keep |  | plain data name |
| `PriorityFrame.depends_on` | field | keep |  | plain data name |
| `PriorityFrame.weight` | field | keep |  | plain data name |
| `Pseudo` | enum | rename, export | `PseudoHeader` | `Pseudo` alone does not say what it is |
| `Pseudo::Method` | enum variant | keep |  | names the case |
| `Pseudo::Authority` | enum variant | keep |  | names the case |
| `Pseudo::Scheme` | enum variant | keep |  | names the case |
| `Pseudo::Path` | enum variant | keep |  | names the case |
| `Pseudo::ALL` | const | keep |  | says what it does |
| `Pseudo::letter` | method | keep |  | says what it does |
| `Pseudo::header_name` | method | keep |  | says what it does |
| `Pseudo::from_letter` | method | keep |  | says what it does |
| `Pseudo::from_header_name` | method | keep |  | says what it does |

### `codec::client_hello`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `ClientHello` | struct | keep |  | a protocol or published-format term, not another library's API |
| `ClientHello.record_version` | field | keep |  | plain data name |
| `ClientHello.legacy_version` | field | keep |  | plain data name |
| `ClientHello.random` | field | keep |  | plain data name |
| `ClientHello.session_id` | field | keep |  | plain data name |
| `ClientHello.cipher_suites` | field | keep |  | plain data name |
| `ClientHello.compression_methods` | field | keep |  | plain data name |
| `ClientHello.extensions` | field | keep |  | plain data name |
| `ClientHello.sni` | field | keep |  | plain data name |
| `ClientHello.alpn` | field | keep |  | plain data name |
| `ClientHello.supported_versions` | field | keep |  | plain data name |
| `ClientHello.supported_groups` | field | keep |  | plain data name |
| `ClientHello.key_share_groups` | field | keep |  | plain data name |
| `ClientHello.point_formats` | field | keep |  | plain data name |
| `ClientHello.signature_algorithms` | field | keep |  | plain data name |
| `ClientHello.psk_modes` | field | keep |  | plain data name |
| `ClientHello.cert_compression` | field | keep |  | plain data name |
| `ClientHello.alps` | field | keep |  | plain data name |
| `ClientHello.record_size_limit` | field | keep |  | plain data name |
| `ClientHello.delegated_credentials` | field | keep |  | plain data name |
| `ClientHello::needed_len` | method | keep |  | says what it does |
| `ClientHello::parse` | method | keep |  | says what it does |
| `ClientHello::extension` | method | keep |  | says what it does |
| `ClientHello::has_extension` | method | keep |  | says what it does |
| `ClientHello::extension_ids` | method | keep |  | says what it does |
| `ClientHello::has_grease` | method | keep |  | says what it does |
| `ClientHelloError` | enum | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `ClientHelloError::NotHandshake` | enum variant | keep |  | names the case |
| `ClientHelloError::NotClientHello` | enum variant | keep |  | names the case |
| `ClientHelloError::Truncated` | enum variant | keep |  | names the case |
| `ClientHelloError::Length` | enum variant | keep |  | names the case |
| `ClientHelloError::TooLarge` | enum variant | keep |  | names the case |
| `ClientHelloError::TooManyRecords` | enum variant | keep |  | names the case |
| `ClientHelloError::Extension` | enum variant | keep |  | names the case |
| `Extension` | struct | rename, export | `TlsExtension` | an extension in core is a browser extension (`LaunchOptionsBuilder::extension`, `utils::extensions`) |
| `Extension.id` | field | keep |  | plain data name |
| `Extension.data` | field | keep |  | plain data name |

### `codec::emulation`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `emulation_for_kind` | function | rename | `wire_measurement_for` | follows `Emulation` → `WireMeasurement` |
| `get_emulation` | function | rename | `wire_measurement` | follows `WireMeasurement`; no `get_` prefix (Rust API guidelines, C-GETTER) |
| `measured_emulations` | function | rename | `wire_measurements` | follows `WireMeasurement` |
| `Emulation` | struct | rename | `WireMeasurement` | **clash**: core `sb_cdp::state::Emulation` is device emulation (viewport, touch, scale); this is one browser's measured TLS, HTTP/2 and header shape; the docs already call it a wire format |
| `Emulation.kind` | field | keep |  | plain data name |
| `Emulation.major` | field | keep |  | plain data name |
| `Emulation.full_version` | field | keep |  | plain data name |
| `Emulation.os` | field | keep |  | plain data name |
| `Emulation.user_agent` | field | keep |  | plain data name |
| `Emulation.tls` | field | keep |  | plain data name |
| `Emulation.tls_websocket` | field | keep |  | plain data name |
| `Emulation.http2` | field | keep |  | plain data name |
| `Emulation.headers` | field | keep |  | plain data name |
| `Emulation.client_hints` | field | keep |  | plain data name |
| `Emulation.provenance` | field | keep |  | plain data name |
| `Emulation::from_json` | method | keep |  | says what it does |
| `Emulation::to_json` | method | keep |  | says what it does |
| `Emulation::file_stem` | method | keep |  | says what it does |
| `EmulationError` | enum | rename | `WireMeasurementError` | follows `WireMeasurement` |
| `EmulationError::NotMeasured` | enum variant | keep |  | names the case |
| `EmulationError::VersionNotMeasured` | enum variant | keep |  | names the case |
| `EmulationError::Parse` | enum variant | keep |  | names the case |
| `EmulationError::TooLarge` | enum variant | keep |  | names the case |
| `EmulationError::Inconsistent` | enum variant | keep |  | names the case |
| `EmulationError::Headers` | enum variant | keep |  | names the case |
| `EmulationError::Duplicate` | enum variant | keep |  | names the case |
| `Provenance` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Provenance.captured_on` | field | keep |  | plain data name |
| `Provenance.tool` | field | keep |  | plain data name |
| `Provenance.browser` | field | keep |  | plain data name |
| `Provenance.known_gaps` | field | keep |  | plain data name |

### `codec::h2_spec`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `H2Spec` | struct | keep |  | a protocol or published-format term, not another library's API |
| `H2Spec.settings` | field | keep |  | plain data name |
| `H2Spec.window_update` | field | keep |  | plain data name |
| `H2Spec.priority_frames` | field | keep |  | plain data name |
| `H2Spec.headers_priority` | field | keep |  | plain data name |
| `H2Spec.pseudo_order` | field | keep |  | plain data name |
| `H2Spec.initial_stream_id` | field | keep |  | plain data name |
| `H2Spec::from_preface` | method | keep |  | says what it does |
| `H2Spec::from_capture` | method | keep |  | says what it does |
| `H2Spec::from_akamai` | method | keep |  | says what it does |
| `H2Spec::with_akamai` | method | keep |  | says what it does |
| `H2Spec::check_reproducible` | method | keep |  | says what it does |
| `H2Spec::akamai` | method | keep |  | says what it does |
| `H2Spec::setting` | method | keep |  | says what it does |
| `H2Spec::without_unsupported_settings` | method | keep |  | says what it does |
| `H2SpecError` | enum | rename | `H2SpecError` (source name) | make it the real name: the source calls it `SpecError` and `lib.rs` renames it; one spelling per item |
| `H2SpecError::UnsupportedSetting` | enum variant | keep |  | names the case |
| `H2SpecError::DuplicateSetting` | enum variant | keep |  | names the case |
| `H2SpecError::SettingValue` | enum variant | keep |  | names the case |
| `H2SpecError::SettingAlwaysSent` | enum variant | keep |  | names the case |
| `H2SpecError::WindowUpdate` | enum variant | keep |  | names the case |
| `H2SpecError::Priority` | enum variant | keep |  | names the case |
| `H2SpecError::HeadersPriority` | enum variant | keep |  | names the case |
| `H2SpecError::PseudoOrder` | enum variant | keep |  | names the case |
| `H2SpecError::InitialStreamId` | enum variant | keep |  | names the case |
| `H2SpecError::Incomplete` | enum variant | keep |  | names the case |
| `H2SpecError::Capture` | enum variant | keep |  | names the case |
| `Http2Spec` | type alias | remove |  | a second name for `H2Spec` |

### `codec::header_spec`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `HeaderIdentity` | struct | rename | `ClientHeaders` | "identity" means the saved scrape identity and core's `sb_cdp::identity`; this holds what a client sends in its User-Agent, client-hint, language and cookie headers |
| `HeaderIdentity.user_agent` | field | keep |  | plain data name |
| `HeaderIdentity.accept_language` | field | keep |  | plain data name |
| `HeaderIdentity.sec_ch_ua` | field | keep |  | plain data name |
| `HeaderIdentity.sec_ch_ua_mobile` | field | keep |  | plain data name |
| `HeaderIdentity.sec_ch_ua_platform` | field | keep |  | plain data name |
| `HeaderIdentity.cookie` | field | keep |  | plain data name |
| `HeaderIdentity.overrides` | field | keep |  | plain data name |
| `HeaderIdentity::new` | method | keep |  | says what it does |
| `HeaderIdentity::with_accept_language` | method | keep |  | says what it does |
| `HeaderIdentity::with_client_hints` | method | keep |  | says what it does |
| `HeaderIdentity::with_cookie` | method | keep |  | says what it does |
| `HeaderIdentity::with_override` | method | keep |  | says what it does |
| `HeaderIdentity::from_captured` | method | keep |  | says what it does |
| `HeaderSpec` | struct | keep |  | a protocol or published-format term, not another library's API |
| `HeaderSpec.document` | field | keep |  | plain data name |
| `HeaderSpec.fetch` | field | keep |  | plain data name |
| `HeaderSpec.priority_by_dest` | field | keep |  | plain data name |
| `HeaderSpec::from_captured` | method | keep |  | says what it does |
| `HeaderSpec::validate` | method | keep |  | says what it does |
| `HeaderSpec::headers` | method | keep |  | says what it does |
| `HeaderSpec::render` | method | keep |  | says what it does |
| `HeaderSpec::priority_for` | method | keep |  | says what it does |
| `HeaderSpec::with_header` | method | keep |  | says what it does |
| `HeaderSpec::without_header` | method | keep |  | says what it does |
| `HeaderSpec::with_priority` | method | keep |  | says what it does |
| `HeaderSpecError` | enum | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `HeaderSpecError::InvalidName` | enum variant | keep |  | names the case |
| `HeaderSpecError::InvalidValue` | enum variant | keep |  | names the case |
| `HeaderSpecError::PseudoHeader` | enum variant | keep |  | names the case |
| `HeaderSpecError::ReservedHeader` | enum variant | keep |  | names the case |
| `HeaderSpecError::DuplicateName` | enum variant | keep |  | names the case |
| `HeaderSpecError::DuplicateDestination` | enum variant | keep |  | names the case |
| `HeaderSpecError::TooManyHeaders` | enum variant | keep |  | names the case |
| `HeaderSpecError::TooManyDestinations` | enum variant | keep |  | names the case |
| `HeaderSpecError::PriorityWithoutDestination` | enum variant | keep |  | names the case |
| `RequestShape` | enum | keep |  | says which request it shapes (a document load or a fetch) |
| `RequestShape::Document` | enum variant | keep |  | names the case |
| `RequestShape::Fetch` | enum variant | keep |  | names the case |
| `HeaderValue` | enum | rename, export | `HeaderSource` | clashes with `http::HeaderValue`, which wreq hands to callers; the enum says where a value comes from (literal, user agent, client hints, language, cookie) |
| `HeaderValue::Literal` | enum variant | keep |  | names the case |
| `HeaderValue::UserAgent` | enum variant | keep |  | names the case |
| `HeaderValue::SecChUa` | enum variant | keep |  | names the case |
| `HeaderValue::SecChUaMobile` | enum variant | keep |  | names the case |
| `HeaderValue::SecChUaPlatform` | enum variant | keep |  | names the case |
| `HeaderValue::AcceptLanguage` | enum variant | keep |  | names the case |
| `HeaderValue::Cookie` | enum variant | keep |  | names the case |

### `codec::ja3`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `Ja3` | struct | keep |  | a protocol or published-format term, not another library's API |
| `Ja3.version` | field | keep |  | plain data name |
| `Ja3.ciphers` | field | keep |  | plain data name |
| `Ja3.extensions` | field | keep |  | plain data name |
| `Ja3.groups` | field | keep |  | plain data name |
| `Ja3.point_formats` | field | keep |  | plain data name |
| `Ja3::of` | method | keep |  | says what it does |
| `Ja3::parse` | method | keep |  | says what it does |
| `Ja3::hash` | method | keep |  | says what it does |
| `Ja3Error` | enum | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `Ja3Error::FieldCount` | enum variant | keep |  | names the case |
| `Ja3Error::Number` | enum variant | keep |  | names the case |
| `Ja3Error::Empty` | enum variant | keep |  | names the case |
| `Ja3Error::TooLong` | enum variant | keep |  | names the case |
| `Peetprint` | struct | keep |  | a protocol or published-format term, not another library's API |
| `Peetprint.versions` | field | keep |  | plain data name |
| `Peetprint.alpn` | field | keep |  | plain data name |
| `Peetprint.groups` | field | keep |  | plain data name |
| `Peetprint.signature_algorithms` | field | keep |  | plain data name |
| `Peetprint.psk_modes` | field | keep |  | plain data name |
| `Peetprint.cert_compression` | field | keep |  | plain data name |
| `Peetprint.ciphers` | field | keep |  | plain data name |
| `Peetprint.extensions` | field | keep |  | plain data name |
| `Peetprint::of` | method | keep |  | says what it does |
| `Peetprint::normalized` | method | keep |  | says what it does |
| `Peetprint::extension_ids` | method | keep |  | says what it does |
| `Peetprint::hash` | method | keep |  | says what it does |
| `Peetprint::parse` | method | keep |  | says what it does |
| `PeetprintError` | enum | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `PeetprintError::FieldCount` | enum variant | keep |  | names the case |
| `PeetprintError::Token` | enum variant | keep |  | names the case |
| `PeetprintError::TooLong` | enum variant | keep |  | names the case |
| `Ja3Field` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja3Field::Version` | enum variant | keep |  | names the case |
| `Ja3Field::Ciphers` | enum variant | keep |  | names the case |
| `Ja3Field::Extensions` | enum variant | keep |  | names the case |
| `Ja3Field::Groups` | enum variant | keep |  | names the case |
| `Ja3Field::PointFormats` | enum variant | keep |  | names the case |
| `Slot` | enum | rename, export | `PeetprintSlot` | `Slot` alone is vague; it is one entry of a Peetprint list (GREASE or an id) |
| `Slot::Grease` | enum variant | keep |  | names the case |
| `Slot::Id` | enum variant | keep |  | names the case |
| `Slot::from_id` | method | keep |  | says what it does |
| `PeetField` | enum | rename, export | `PeetprintField` | spell the format's name as `Peetprint` and `PeetprintError` do |
| `PeetField::Versions` | enum variant | keep |  | names the case |
| `PeetField::Alpn` | enum variant | keep |  | names the case |
| `PeetField::Groups` | enum variant | keep |  | names the case |
| `PeetField::SignatureAlgorithms` | enum variant | keep |  | names the case |
| `PeetField::PskModes` | enum variant | keep |  | names the case |
| `PeetField::CertCompression` | enum variant | keep |  | names the case |
| `PeetField::Ciphers` | enum variant | keep |  | names the case |
| `PeetField::Extensions` | enum variant | keep |  | names the case |

### `codec::ja4`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `Ja4` | struct | keep |  | a protocol or published-format term, not another library's API |
| `Ja4.a` | field | keep |  | plain data name |
| `Ja4.b` | field | keep |  | plain data name |
| `Ja4.c` | field | keep |  | plain data name |
| `Ja4::of` | method | keep |  | says what it does |
| `Ja4::of_original_order` | method | keep |  | says what it does |
| `Ja4::raw` | method | keep |  | says what it does |
| `Ja4::raw_original_order` | method | keep |  | says what it does |
| `Ja4::parse` | method | keep |  | says what it does |
| `Ja4::diff` | method | keep |  | says what it does |
| `Ja4Error` | enum | keep |  | one `<Area>Error` per area, as core's `FingerprintError` |
| `Ja4Error::Sections` | enum variant | keep |  | names the case |
| `Ja4Error::PartA` | enum variant | keep |  | names the case |
| `Ja4Error::Hash` | enum variant | keep |  | names the case |
| `Ja4Error::Hex` | enum variant | keep |  | names the case |
| `Ja4Error::TooLong` | enum variant | keep |  | names the case |
| `Ja4Raw` | struct | keep |  | a protocol or published-format term, not another library's API |
| `Ja4Raw.a` | field | keep |  | plain data name |
| `Ja4Raw.ciphers` | field | keep |  | plain data name |
| `Ja4Raw.extensions` | field | keep |  | plain data name |
| `Ja4Raw.signature_algorithms` | field | keep |  | plain data name |
| `Ja4Raw::parse` | method | keep |  | says what it does |
| `Ja4Raw::hashed` | method | keep |  | says what it does |
| `Ja4Diff` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Diff.a_fields` | field | keep |  | plain data name |
| `Ja4Diff.b` | field | keep |  | plain data name |
| `Ja4Diff.c` | field | keep |  | plain data name |
| `Ja4Diff::is_empty` | method | keep |  | says what it does |
| `Ja4Section` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Section::A` | enum variant | keep |  | names the case |
| `Ja4Section::Ciphers` | enum variant | keep |  | names the case |
| `Ja4Section::Extensions` | enum variant | keep |  | names the case |
| `Ja4Section::SignatureAlgorithms` | enum variant | keep |  | names the case |
| `Ja4Facts` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Facts.legacy_version` | field | keep |  | plain data name |
| `Ja4Facts.supported_versions` | field | keep |  | plain data name |
| `Ja4Facts.ciphers` | field | keep |  | plain data name |
| `Ja4Facts.extensions` | field | keep |  | plain data name |
| `Ja4Facts.first_alpn` | field | keep |  | plain data name |
| `Ja4Facts.signature_algorithms` | field | keep |  | plain data name |
| `Ja4Facts::of` | method | keep |  | says what it does |
| `Ja4Facts::a` | method | keep |  | says what it does |
| `Ja4Facts::raw` | method | keep |  | says what it does |
| `Ja4Facts::ja4` | method | keep |  | says what it does |
| `Ja4Order` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4Order::Sorted` | enum variant | keep |  | names the case |
| `Ja4Order::Original` | enum variant | keep |  | names the case |
| `Ja4AField` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Ja4AField::Protocol` | enum variant | keep |  | names the case |
| `Ja4AField::Version` | enum variant | keep |  | names the case |
| `Ja4AField::Sni` | enum variant | keep |  | names the case |
| `Ja4AField::CipherCount` | enum variant | keep |  | names the case |
| `Ja4AField::ExtensionCount` | enum variant | keep |  | names the case |
| `Ja4AField::Alpn` | enum variant | keep |  | names the case |

### `codec::tls_spec`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `KnownGap` | enum | keep |  | names a measured difference the client cannot reproduce yet |
| `KnownGap::TrustAnchorsExtension` | enum variant | keep |  | names the case |
| `KnownGap::MlDsaSignatureAlgorithms` | enum variant | keep |  | names the case |
| `TlsSpecError` | enum | rename | `TlsSpecError` (source name) | make it the real name: the source calls it `SpecError` and `lib.rs` renames it; one spelling per item |
| `TlsSpecError::LegacyVersion` | enum variant | keep |  | names the case |
| `TlsSpecError::UnsupportedVersion` | enum variant | keep |  | names the case |
| `TlsSpecError::NoCipherSuites` | enum variant | keep |  | names the case |
| `TlsSpecError::UnsupportedCipher` | enum variant | keep |  | names the case |
| `TlsSpecError::Tls13Suites` | enum variant | keep |  | names the case |
| `TlsSpecError::UnsupportedExtension` | enum variant | keep |  | names the case |
| `TlsSpecError::DuplicateExtension` | enum variant | keep |  | names the case |
| `TlsSpecError::UnsupportedGroup` | enum variant | keep |  | names the case |
| `TlsSpecError::KeyShareNotOffered` | enum variant | keep |  | names the case |
| `TlsSpecError::PointFormats` | enum variant | keep |  | names the case |
| `TlsSpecError::UnsupportedSigalg` | enum variant | keep |  | names the case |
| `TlsSpecError::Duplicate` | enum variant | keep |  | names the case |
| `TlsSpecError::ExtensionContent` | enum variant | keep |  | names the case |
| `TlsSpecError::Alpn` | enum variant | keep |  | names the case |
| `TlsSpecError::Ja4Mismatch` | enum variant | keep |  | names the case |
| `TlsSpecError::Ja4Alpn` | enum variant | keep |  | names the case |
| `TlsSpecError::PeetprintMismatch` | enum variant | keep |  | names the case |
| `TlsSpecError::HellosDiffer` | enum variant | keep |  | names the case |
| `TlsSpec` | struct | keep |  | a protocol or published-format term, not another library's API |
| `TlsSpec.legacy_version` | field | keep |  | plain data name |
| `TlsSpec.cipher_suites` | field | keep |  | plain data name |
| `TlsSpec.extensions` | field | keep |  | plain data name |
| `TlsSpec.permute_extensions` | field | keep |  | plain data name |
| `TlsSpec.grease` | field | keep |  | plain data name |
| `TlsSpec.supported_groups` | field | keep |  | plain data name |
| `TlsSpec.key_share_groups` | field | keep |  | plain data name |
| `TlsSpec.point_formats` | field | keep |  | plain data name |
| `TlsSpec.signature_algorithms` | field | keep |  | plain data name |
| `TlsSpec.alpn` | field | keep |  | plain data name |
| `TlsSpec.alps` | field | keep |  | plain data name |
| `TlsSpec.delegated_credentials` | field | keep |  | plain data name |
| `TlsSpec.supported_versions` | field | keep |  | plain data name |
| `TlsSpec.psk_modes` | field | keep |  | plain data name |
| `TlsSpec.cert_compression` | field | keep |  | plain data name |
| `TlsSpec.record_size_limit` | field | keep |  | plain data name |
| `TlsSpec::from_client_hello` | method | keep |  | says what it does |
| `TlsSpec::with_observed_order` | method | keep |  | says what it does |
| `TlsSpec::has_extension` | method | keep |  | says what it does |
| `TlsSpec::ocsp_stapling` | method | keep |  | says what it does |
| `TlsSpec::sct` | method | keep |  | says what it does |
| `TlsSpec::session_ticket` | method | keep |  | says what it does |
| `TlsSpec::renegotiation` | method | keep |  | says what it does |
| `TlsSpec::ech_grease` | method | keep |  | says what it does |
| `TlsSpec::extended_master_secret` | method | keep |  | says what it does |
| `TlsSpec::ja3` | method | keep |  | says what it does |
| `TlsSpec::ja4_facts` | method | keep |  | says what it does |
| `TlsSpec::ja4` | method | keep |  | says what it does |
| `TlsSpec::ja4_raw` | method | keep |  | says what it does |
| `TlsSpec::peetprint` | method | keep |  | says what it does |
| `TlsSpec::without_known_gaps` | method | keep |  | says what it does |
| `TlsSpec::check_reproducible` | method | keep |  | says what it does |
| `TlsSpec::with_ja3` | method | keep |  | says what it does |
| `TlsSpec::with_signature_algorithms` | method | keep |  | says what it does |
| `TlsSpec::with_ja4_raw` | method | keep |  | says what it does |
| `TlsSpec::with_peetprint` | method | keep |  | says what it does |
| `Alps` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Alps.protocols` | field | keep |  | plain data name |
| `Alps.new_codepoint` | field | keep |  | plain data name |

### `codec` (crate root)

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `BrowserKind` | enum | keep |  | only `Chrome` is measured; once more browsers are, use core `browser::config::Browser` instead of a second browser list |
| `BrowserKind::Chrome` | enum variant | keep |  | names the case |

### `codec::h2_frames`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `HeadersPriority` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HeadersPriority.depends_on` | field | keep |  | plain data name |
| `HeadersPriority.weight` | field | keep |  | plain data name |
| `HeadersPriority.exclusive` | field | keep |  | plain data name |
| `H2Preface` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `H2Preface.settings` | field | keep |  | plain data name |
| `H2Preface.window_update` | field | keep |  | plain data name |
| `H2Preface.priority_frames` | field | keep |  | plain data name |
| `H2Preface.headers_priority` | field | keep |  | plain data name |
| `H2Preface.request_stream_id` | field | keep |  | plain data name |
| `H2Preface.headers` | field | keep |  | plain data name |
| `H2Preface.consumed` | field | keep |  | plain data name |
| `H2Preface::pseudo_order` | method | keep |  | says what it does |
| `H2Preface::akamai` | method | keep |  | says what it does |
| `H2Preface::regular_headers` | method | keep |  | says what it does |
| `H2Preface::header` | method | keep |  | says what it does |
| `H2CaptureError` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `H2CaptureError::BadPreface` | enum variant | keep |  | names the case |
| `H2CaptureError::FrameTooLarge` | enum variant | keep |  | names the case |
| `H2CaptureError::FrameSize` | enum variant | keep |  | names the case |
| `H2CaptureError::StreamId` | enum variant | keep |  | names the case |
| `H2CaptureError::BadPadding` | enum variant | keep |  | names the case |
| `H2CaptureError::ZeroWindowIncrement` | enum variant | keep |  | names the case |
| `H2CaptureError::UnexpectedFrame` | enum variant | keep |  | names the case |
| `H2CaptureError::ExpectedContinuation` | enum variant | keep |  | names the case |
| `H2CaptureError::HeaderBlockTooLarge` | enum variant | keep |  | names the case |
| `H2CaptureError::PrefaceTooLarge` | enum variant | keep |  | names the case |
| `H2CaptureError::TooManyFrames` | enum variant | keep |  | names the case |
| `H2CaptureError::ReaderFinished` | enum variant | keep |  | names the case |
| `H2CaptureError::MissingSettings` | enum variant | keep |  | names the case |
| `H2CaptureError::PseudoOrder` | enum variant | keep |  | names the case |
| `H2CaptureError::StreamIdRange` | enum variant | keep |  | names the case |
| `H2CaptureError::Hpack` | enum variant | keep |  | names the case |

### `codec::h2_frames::hpack`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `HeaderField` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HeaderField.name` | field | keep |  | plain data name |
| `HeaderField.value` | field | keep |  | plain data name |
| `HeaderField::new` | method | keep |  | says what it does |
| `HeaderField::is_pseudo` | method | keep |  | says what it does |
| `HpackError` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HpackError::Truncated` | enum variant | keep |  | names the case |
| `HpackError::IntegerOverflow` | enum variant | keep |  | names the case |
| `HpackError::IndexZero` | enum variant | keep |  | names the case |
| `HpackError::IndexOutOfRange` | enum variant | keep |  | names the case |
| `HpackError::Huffman` | enum variant | keep |  | names the case |
| `HpackError::NotUtf8` | enum variant | keep |  | names the case |
| `HpackError::SizeUpdateTooLarge` | enum variant | keep |  | names the case |
| `HpackError::SizeUpdateAfterField` | enum variant | keep |  | names the case |
| `HpackError::HeaderListTooLarge` | enum variant | keep |  | names the case |

### `codec::h2_frames::huffman`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `HuffmanError` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `HuffmanError::EndOfString` | enum variant | keep |  | names the case |
| `HuffmanError::PaddingTooLong` | enum variant | keep |  | names the case |
| `HuffmanError::PaddingNotOnes` | enum variant | keep |  | names the case |
| `HuffmanError::InvalidCode` | enum variant | keep |  | names the case |

## scrape (`seleniumbase_rs_stealth::scrape`)

### `scrape::challenge::detect`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `ChallengeKind` | enum | keep |  | challenge is used only here; core says captcha for the widget |
| `ChallengeKind::None` | enum variant | rename | `Clear` | `ChallengeKind::None` reads like `Option::None`; `HttpVerdict::is_clear` already says clear |
| `ChallengeKind::JsChallenge` | enum variant | keep |  | names the case |
| `ChallengeKind::Interactive` | enum variant | keep |  | names the case |
| `ChallengeKind::Blocked` | enum variant | keep |  | names the case |
| `ChallengeKind::RateLimited` | enum variant | keep |  | names the case |
| `Detection` | struct | keep |  | the result of challenge detection; no core item of that name |
| `Detection.kind` | field | keep |  | plain data name |
| `Detection.evidence` | field | keep |  | plain data name |
| `Evidence` | struct | rename | `ChallengeEvidence` | **clash**: core `sb_cdp::captcha::Evidence` is what showed a CAPTCHA was solved |
| `Evidence.vendor` | field | keep |  | plain data name |
| `Evidence.markers` | field | keep |  | plain data name |
| `Evidence.widget` | field | keep |  | already core's `Captcha` |
| `ResponseView` | struct | keep |  | a borrowed view of a response, from either fetch kind |
| `ResponseView.url` | field | keep |  | plain data name |
| `ResponseView.status` | field | keep |  | plain data name |
| `ResponseView.headers` | field | keep |  | plain data name |
| `ResponseView.body` | field | keep |  | plain data name |
| `Vendor` | enum | rename | `ProtectionVendor` | too generic as `scrape::Vendor`; these are bot-protection vendors, while the widget is core's `Captcha` (`Evidence.widget`) |
| `Vendor::Cloudflare` | enum variant | keep |  | names the case |
| `Vendor::DataDome` | enum variant | keep |  | names the case |
| `Vendor::PerimeterX` | enum variant | keep |  | names the case |
| `Vendor::Akamai` | enum variant | keep |  | the company; codec's `Akamai` is the HTTP/2 format named after its research, documented as such |
| `Vendor::Imperva` | enum variant | keep |  | names the case |
| `Vendor::Kasada` | enum variant | keep |  | names the case |
| `Vendor::AwsWaf` | enum variant | keep |  | names the case |

### `scrape::challenge::mitigation`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `Action` | enum | rename | `Mitigation` | too generic as a re-exported name; it is the step the `mitigation` module picks |
| `Action::Proceed` | enum variant | keep |  | names the case |
| `Action::Retry` | enum variant | keep |  | names the case |
| `Action::Escalate` | enum variant | keep |  | names the case |
| `Action::Solve` | enum variant | keep |  | names the case |
| `Action::RotateEgress` | enum variant | keep |  | names the case |
| `Action::RotateProfile` | enum variant | rename | `RotateFingerprint` | a profile in core is a browser profile (`profile_payloads`, the Browser profiles page); this draws a new fingerprint |
| `Action::Cooldown` | enum variant | keep |  | names the case |
| `Action::GiveUp` | enum variant | keep |  | names the case |
| `AttemptContext` | struct | keep |  | says what it is; no core item of that name or concept |
| `AttemptContext.attempt` | field | keep |  | plain data name |
| `AttemptContext.mode` | field | rename | `fetch_kind` | follows `Mode` → `FetchKind` |
| `AttemptContext.consecutive_challenges` | field | keep |  | plain data name |
| `AttemptContext.can_rotate_egress` | field | keep |  | plain data name |
| `AttemptContext.can_escalate` | field | keep |  | plain data name |
| `AttemptContext.now` | field | keep |  | plain data name |
| `GiveUpReason` | enum | keep |  | says what it is; no core item of that name or concept |
| `GiveUpReason::AttemptsExhausted` | enum variant | keep |  | names the case |
| `GiveUpReason::Blocked` | enum variant | keep |  | names the case |
| `GiveUpReason::NoEgressLeft` | enum variant | keep |  | names the case |
| `MitigationPolicy` | struct | keep |  | the `<Thing>Policy` pattern of core's `StealthPolicy` and `RetryPolicy` |
| `MitigationPolicy.max_challenge_attempts` | field | keep |  | plain data name |
| `MitigationPolicy.backoff` | field | keep |  | already core's `RetryPolicy` |
| `MitigationPolicy.rotate_egress_after` | field | keep |  | plain data name |
| `MitigationPolicy.rotate_profile_after` | field | rename | `rotate_fingerprint_after` | follows `RotateFingerprint` |
| `MitigationPolicy.block_cooldown_secs` | field | keep |  | plain data name |
| `MitigationPolicy::decide` | method | keep |  | says what it does |
| `Mode` | enum | rename | `FetchKind` | core says mode for engine modes (UC mode, CDP Mode, `DriverMode`); this says whether a request goes through the browser or over HTTP. `Route` is taken by `egress::route` |
| `Mode::Browser` | enum variant | keep |  | names the case |
| `Mode::Http` | enum variant | keep |  | names the case |

### `scrape::egress::geo`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `CountryCode` | struct | move | core `evasion::randomize::CountryCode` | goes with `Locale::for_country`; same pattern as core `LocaleCode` |
| `CountryCode::new` | method | move |  | moves with its type |
| `CountryCode::as_str` | method | move |  | moves with its type |
| `GeoResolver` | trait | keep |  | a trait named for what it does |
| `GeoResolver::resolve` | method | keep |  | says what it does |
| `LocaleExt` | trait | move | inherent methods of core `evasion::randomize::Locale` | an extension trait exists only because `Locale` lives in core |
| `LocaleExt::for_country` | method | move | `Locale::for_country` | with the trait |
| `LocaleExt::by_tag` | method | move | `Locale::by_tag` | with the trait |
| `LocaleExt::localize` | method | move | `Locale::localize` | with the trait |
| `StaticGeoResolver` | struct | keep |  | the built-in `GeoResolver` |
| `GeoError` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `GeoError::InvalidCountry` | enum variant | keep |  | names the case |
| `GeoError::NoLocale` | enum variant | keep |  | names the case |
| `GeoError::Unknown` | enum variant | keep |  | names the case |
| `GeoInfo` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `GeoInfo.country` | field | keep |  | plain data name |
| `GeoInfo.timezone` | field | keep |  | plain data name |
| `GeoInfo.coordinates` | field | keep |  | make its type core's `Geolocation` instead of `(f64, f64)` |

### `scrape::egress::pool`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `ProxyPool` | struct | move | core `config::proxy_list::ProxyList` | **clash in concept**: core `ProxyList` already holds proxies and picks one (`next_round_robin`, `random`); rotation belongs to that list |
| `ProxyPool::new` | method | move |  | moves with its type |
| `ProxyPool::with_geo_proxies` | method | move | `ProxyList::from_geo_proxies` | a constructor; core says `from_*` (`ProxyList::from_file`) |
| `ProxyPool::quarantine` | method | move |  | moves with its type |
| `ProxyPool::pick` | method | move |  | moves with its type |
| `ProxyPool::pick_in` | method | move |  | moves with its type |
| `ProxyPool::pick_other` | method | move |  | moves with its type |
| `ProxyPool::report` | method | move |  | moves with its type |
| `RotationStrategy` | enum | move | core `config::proxy_list::RotationStrategy` | with `ProxyPool` |
| `RotationStrategy::RoundRobin` | enum variant | move |  | moves with its type |
| `RotationStrategy::Random` | enum variant | move |  | moves with its type |
| `RotationStrategy::LeastFailures` | enum variant | move |  | moves with its type |
| `RotationStrategy::StickyPerSite` | enum variant | move |  | moves with its type |
| `EgressOutcome` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `EgressOutcome::Success` | enum variant | keep |  | names the case |
| `EgressOutcome::ConnectError` | enum variant | keep |  | names the case |
| `EgressOutcome::Challenged` | enum variant | keep |  | names the case |
| `EgressOutcome::Blocked` | enum variant | keep |  | names the case |

### `scrape::egress::redact`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `RedactedUrl` | struct | move | core `common` (a new `secret` module) | core's `sb_cdp::Proxy` hides its password with its own code; core security code has its own owner, so this needs the owner's OK |
| `RedactedUrl::new` | method | move |  | moves with its type |
| `RedactedUrl::placeholder` | method | move |  | moves with its type |
| `RedactedUrl::as_str` | method | move |  | moves with its type |
| `Secret` | struct | move | core `common` (a new `secret` module) | one `Secret` for every password the project holds; same caveat as `RedactedUrl` |
| `Secret::new` | method | move |  | moves with its type |
| `Secret::expose` | method | move |  | moves with its type |

### `scrape::egress::route`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `EgressKey` | struct | keep |  | egress is this repository's one word for the way out (a proxy or direct); no core clash |
| `EgressKey.scheme` | field | keep |  | plain data name |
| `EgressKey.host` | field | keep |  | plain data name |
| `EgressKey.port` | field | keep |  | plain data name |
| `EgressKey.country` | field | keep |  | plain data name |
| `EgressRef` | struct | keep |  | same; it wraps core's `Proxy` (`EgressRef::proxy`) |
| `EgressRef::parse` | method | keep |  | says what it does |
| `EgressRef::key` | method | keep |  | says what it does |
| `EgressRef::proxy` | method | keep |  | says what it does |
| `EgressError` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `EgressError::Scheme` | enum variant | keep |  | names the case |
| `EgressError::Address` | enum variant | keep |  | names the case |

### `scrape::error`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `ScrapeError` | enum | keep |  | the crate's error, as core's `SeleniumBaseError` |
| `ScrapeError::Egress` | enum variant | keep |  | names the case |
| `ScrapeError::Geo` | enum variant | keep |  | names the case |
| `ScrapeError::State` | enum variant | rename | `Ledger` | follows `StateStore` → `Ledger` |
| `ScrapeError::Identity` | enum variant | keep |  | names the case |
| `ScrapeError::Fingerprint` | enum variant | keep |  | names the case |
| `ScrapeError::Unsolved` | enum variant | keep |  | names the case |
| `ScrapeError::GaveUp` | enum variant | keep |  | names the case |
| `ScrapeError::CoolingDown` | enum variant | keep |  | names the case |
| `ScrapeError::Navigation` | enum variant | keep |  | names the case |
| `ScrapeError::NoEgressAvailable` | enum variant | keep |  | names the case |
| `ScrapeError::Browser` | enum variant | keep |  | names the case |

### `scrape::jar::store`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `CookieJar` | struct | keep |  | an RFC 6265 jar for HTTP requests; core's `Cookie` and `Cookies` are the browser's, and `from_cdp`/`to_cdp` convert |
| `CookieJar::new` | method | keep |  | says what it does |
| `CookieJar::len` | method | keep |  | says what it does |
| `CookieJar::is_empty` | method | keep |  | says what it does |
| `CookieJar::cookies` | method | keep |  | says what it does |
| `CookieJar::store` | method | rename | `apply_set_cookie` | store means a persistent store in core (`SessionStore`) and here (`StateStore`); this applies one Set-Cookie header |
| `CookieJar::header_for` | method | keep |  | says what it does |
| `CookieJar::from_cdp` | method | keep |  | says what it does |
| `CookieJar::to_cdp` | method | keep |  | says what it does |
| `CookieJar::expiry_of` | method | keep |  | says what it does |
| `CookieJar::remove_expired` | method | keep |  | says what it does |
| `StoredCookie` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `StoredCookie.name` | field | keep |  | plain data name |
| `StoredCookie.value` | field | keep |  | plain data name |
| `StoredCookie.domain` | field | keep |  | plain data name |
| `StoredCookie.host_only` | field | keep |  | plain data name |
| `StoredCookie.path` | field | keep |  | plain data name |
| `StoredCookie.expires` | field | keep |  | plain data name |
| `StoredCookie.secure` | field | keep |  | plain data name |
| `StoredCookie.http_only` | field | keep |  | plain data name |
| `StoredCookie.same_site` | field | keep |  | plain data name |
| `StoredCookie.created` | field | keep |  | plain data name |
| `StoredCookie.last_access` | field | keep |  | plain data name |
| `StoredCookie::is_expired` | method | keep |  | says what it does |
| `StoredCookie::size_bytes` | method | keep |  | says what it does |
| `StoredCookie::same_identity` | method | keep |  | says what it does |
| `StoreOutcome` | enum | rename, export | `SetCookieOutcome` | follows `apply_set_cookie` |
| `StoreOutcome::Stored` | enum variant | keep |  | names the case |
| `StoreOutcome::Replaced` | enum variant | keep |  | names the case |
| `StoreOutcome::Deleted` | enum variant | keep |  | names the case |
| `StoreOutcome::Ignored` | enum variant | keep |  | names the case |
| `SiteContext` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `SiteContext::SameSite` | enum variant | keep |  | names the case |
| `SiteContext::CrossSite` | enum variant | keep |  | names the case |
| `IgnoreReason` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `IgnoreReason::Malformed` | enum variant | keep |  | names the case |
| `IgnoreReason::DomainMismatch` | enum variant | keep |  | names the case |
| `IgnoreReason::PublicSuffix` | enum variant | keep |  | names the case |
| `IgnoreReason::IpLiteralDomain` | enum variant | keep |  | names the case |
| `IgnoreReason::SecureFromInsecure` | enum variant | keep |  | names the case |
| `IgnoreReason::HostPrefix` | enum variant | keep |  | names the case |
| `IgnoreReason::SecurePrefix` | enum variant | keep |  | names the case |
| `IgnoreReason::TooLarge` | enum variant | keep |  | names the case |
| `IgnoreReason::SameSiteNoneInsecure` | enum variant | keep |  | names the case |
| `IgnoreReason::UnsupportedUrl` | enum variant | keep |  | names the case |

### `scrape::ledger::events`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `CollectingEventSink` | struct | keep |  | follows `EventSink` |
| `CollectingEventSink::new` | method | keep |  | says what it does |
| `CollectingEventSink::events` | method | keep |  | says what it does |
| `CollectingEventSink::take` | method | keep |  | says what it does |
| `CollectingEventSink::len` | method | keep |  | says what it does |
| `CollectingEventSink::is_empty` | method | keep |  | says what it does |
| `EventSink` | trait | keep |  | core's sinks (`RunSink`, the plugin API's `Sink`) take other events; the `Event` prefix keeps them apart |
| `EventSink::emit` | method | keep |  | says what it does |
| `NoopEventSink` | struct | keep |  | follows `EventSink` |
| `ScraperEvent` | enum | rename | `ScrapeEvent` | the crate and its docs say scrape, not scraper |
| `ScraperEvent::Request` | enum variant | keep |  | names the case |
| `ScraperEvent::ChallengeDetected` | enum variant | keep |  | names the case |
| `ScraperEvent::ChallengeSolved` | enum variant | keep |  | names the case |
| `ScraperEvent::ActionChosen` | enum variant | keep |  | names the case |
| `ScraperEvent::ModeChanged` | enum variant | rename | `FetchKindChanged` | follows `FetchKind` |
| `ScraperEvent::EgressRotated` | enum variant | keep |  | names the case |
| `ScraperEvent::ProfileRotated` | enum variant | rename | `FingerprintRotated` | follows `RotateFingerprint` |
| `ScraperEvent::ClearanceExpiring` | enum variant | keep |  | names the case |
| `ScraperEvent::CooldownStarted` | enum variant | keep |  | names the case |
| `ScraperEvent::IdentityExported` | enum variant | keep |  | names the case |
| `ScraperEvent::IdentityRestored` | enum variant | keep |  | names the case |
| `ScraperEvent::name` | method | keep |  | says what it does |

### `scrape::ledger::state`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `DomainState` | struct | rename | `SiteRecord` | **clash**: core `sb_cdp::state` is a page's cookies and storage; this is one site's challenge history, which the module calls the ledger |
| `DomainState.site` | field | keep |  | plain data name |
| `DomainState.successes` | field | keep |  | plain data name |
| `DomainState.challenges` | field | keep |  | plain data name |
| `DomainState.blocks` | field | keep |  | plain data name |
| `DomainState.consecutive_challenges` | field | keep |  | plain data name |
| `DomainState.last_challenge` | field | keep |  | plain data name |
| `DomainState.last_seen` | field | keep |  | plain data name |
| `DomainState.cooldown_until` | field | keep |  | plain data name |
| `DomainState.preferred_mode` | field | rename | `preferred_kind` | follows `FetchKind` |
| `DomainState.last_egress` | field | keep |  | plain data name |
| `DomainState::new` | method | keep |  | says what it does |
| `DomainState::record` | method | keep |  | says what it does |
| `DomainState::start_cooldown` | method | keep |  | says what it does |
| `DomainState::in_cooldown` | method | keep |  | says what it does |
| `InMemoryStateStore` | struct | rename | `InMemoryLedger` | follows `Ledger` |
| `InMemoryStateStore::new` | method | keep |  | says what it does |
| `StateStore` | trait | rename | `Ledger` | **clash** with core `sb_cdp::state` (see `DomainState`); the module is already `ledger` |
| `StateStore::get` | method | keep |  | says what it does |
| `StateStore::put` | method | keep |  | says what it does |
| `StateStore::remove` | method | keep |  | says what it does |
| `StateStore::sites` | method | keep |  | says what it does |
| `StateError` | enum | rename, export | `LedgerError` | follows `Ledger` |
| `StateError::Io` | enum variant | keep |  | names the case |
| `StateError::Corrupt` | enum variant | keep |  | names the case |
| `StateError::Schema` | enum variant | keep |  | names the case |
| `StateError::TooLarge` | enum variant | keep |  | names the case |

### `scrape::passport::identity`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `EmulationRef` | struct | rename | `WireMeasurementRef` | follows codec `WireMeasurement` |
| `EmulationRef.kind` | field | keep |  | plain data name |
| `EmulationRef.major` | field | keep |  | plain data name |
| `EmulationRef.os` | field | keep |  | plain data name |
| `IdentityError` | enum | keep |  | follows `Identity` |
| `IdentityError::EgressMismatch` | enum variant | keep |  | names the case |
| `IdentityError::Schema` | enum variant | keep |  | names the case |
| `IdentityError::Incoherent` | enum variant | keep |  | names the case |
| `IdentityError::EmulationMismatch` | enum variant | rename | `WireMeasurementMismatch` | follows `WireMeasurement` |
| `StealthIdentity` | struct | rename | `Identity` | core has no `Identity` type (its `sb_cdp::identity` module holds `StealthPlan`); this is the one saved identity. Decision 1 |
| `StealthIdentity.schema` | field | keep |  | plain data name |
| `StealthIdentity.fingerprint` | field | keep |  | plain data name |
| `StealthIdentity.emulation` | field | rename | `wire_measurement` | follows `WireMeasurementRef` |
| `StealthIdentity.cookies` | field | keep |  | plain data name |
| `StealthIdentity.accept_ch` | field | keep |  | plain data name |
| `StealthIdentity.egress` | field | keep |  | plain data name |
| `StealthIdentity.clearances` | field | keep |  | plain data name |
| `StealthIdentity.created_at` | field | keep |  | plain data name |
| `StealthIdentity::new` | method | keep |  | says what it does |
| `StealthIdentity::verify` | method | rename | `validate` | core `Fingerprint::validate`; `HeaderSpec`, `ImpersonationProfile` and `HttpOptions` here say validate too |
| `StealthIdentity::refresh_clearances` | method | keep |  | says what it does |
| `StealthIdentity::clearance_for` | method | keep |  | says what it does |
| `StealthIdentity::check_egress` | method | keep |  | says what it does |
| `StealthIdentity::check_emulation` | method | rename | `check_wire_measurement` | follows `WireMeasurement` |
| `Clearance` | struct | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `Clearance.site` | field | keep |  | plain data name |
| `Clearance.vendor` | field | keep |  | plain data name |
| `Clearance.cookie` | field | keep |  | plain data name |
| `Clearance.expires_at` | field | keep |  | plain data name |

### `scrape::passport::policy`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `SessionPolicy` | struct | rename | `EscalationPolicy` | says what it decides (when to move between HTTP and the browser); a session in core is a saved `Session` |
| `SessionPolicy.require_clearance_for_http` | field | keep |  | plain data name |
| `SessionPolicy.clearance_margin` | field | keep |  | plain data name |
| `SessionPolicy.demote_after_solve` | field | keep |  | plain data name |
| `SessionPolicy.prefer` | field | keep |  | plain data name |
| `SessionPolicy::before_fetch` | method | keep |  | says what it does |
| `SessionPolicy::after_fetch` | method | keep |  | says what it does |
| `Transition` | enum | rename | `Escalation` | **clash**: core `api::presentation::Transition` is a slide transition |
| `Transition::Stay` | enum variant | keep |  | names the case |
| `Transition::Escalate` | enum variant | keep |  | names the case |
| `Transition::Demote` | enum variant | keep |  | names the case |
| `Transition::resulting_mode` | method | rename | `resulting_kind` | follows `FetchKind` |
| `EscalateReason` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `EscalateReason::Challenge` | enum variant | keep |  | names the case |
| `EscalateReason::ClearanceExpiring` | enum variant | keep |  | names the case |
| `EscalateReason::NoClearance` | enum variant | keep |  | names the case |
| `EscalateReason::SiteNeedsBrowser` | enum variant | keep |  | names the case |
| `EscalateReason::Manual` | enum variant | keep |  | names the case |
| `DemoteReason` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `DemoteReason::ChallengeSolved` | enum variant | keep |  | names the case |
| `DemoteReason::Manual` | enum variant | keep |  | names the case |

### `scrape::runtime::browser_scraper`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `BrowserFetch` | struct | keep |  | one fetch's result; matches `StealthFetch` and `HttpFetch` |
| `BrowserFetch.url` | field | keep |  | plain data name |
| `BrowserFetch.status` | field | keep |  | plain data name |
| `BrowserFetch.headers` | field | keep |  | plain data name |
| `BrowserFetch.body` | field | keep |  | plain data name |
| `BrowserFetch.detection` | field | keep |  | plain data name |
| `BrowserFetch.attempts` | field | keep |  | plain data name |
| `BrowserFetch.solved` | field | keep |  | plain data name |
| `BrowserFetch.egress` | field | keep |  | plain data name |
| `BrowserLauncher` | trait | keep |  | core's `BrowserPool::with_launcher` takes a closure, not a trait, for the same job; same word, same meaning |
| `BrowserLauncher::launch` | method | keep |  | says what it does |
| `BrowserScraper` | struct | keep |  | says what it is; no core item of that name |
| `BrowserScraper::builder` | method | keep |  | says what it does |
| `BrowserScraper::page` | method | keep |  | says what it does |
| `BrowserScraper::browser` | method | keep |  | says what it does |
| `BrowserScraper::fingerprint` | method | keep |  | returns the fingerprint in use, as core's getters do; see decision 2 |
| `BrowserScraper::seed` | method | keep |  | the value of core `Fingerprint::seed` |
| `BrowserScraper::egress` | method | keep |  | says what it does |
| `BrowserScraper::sessions` | method | rename | `session_store` | returns core's `SessionStore`; the builder method is already `session_store` |
| `BrowserScraper::close` | method | keep |  | says what it does |
| `BrowserScraper::fetch` | method | keep |  | says what it does |
| `BrowserScraper::solve_challenge` | method | keep |  | says what it does |
| `BrowserScraper::rotate_egress` | method | keep |  | says what it does |
| `BrowserScraper::rotate_profile` | method | rename | `rotate_fingerprint` | follows `RotateFingerprint` |
| `BrowserScraper::export_identity` | method | keep |  | says what it does |
| `BrowserScraper::import_identity` | method | keep |  | says what it does |
| `BrowserScraper::save_session` | method | keep |  | says what it does |
| `BrowserScraper::load_session` | method | keep |  | says what it does |
| `BrowserScraperBuilder` | struct | keep |  | core's `<Thing>Builder` pattern (`LaunchOptionsBuilder`) |
| `BrowserScraperBuilder::launch_options` | method | keep |  | says what it does |
| `BrowserScraperBuilder::with_geo_proxies` | method | rename | `geo_proxies` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `BrowserScraperBuilder::proxy_strategy` | method | keep |  | says what it does |
| `BrowserScraperBuilder::with_max_challenge_attempts` | method | rename | `max_challenge_attempts` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `BrowserScraperBuilder::mitigation` | method | keep |  | says what it does |
| `BrowserScraperBuilder::with_state_store` | method | rename | `ledger` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names; follows `Ledger` |
| `BrowserScraperBuilder::with_event_sink` | method | rename | `event_sink` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names; `HttpOptions::event_sink` already says it so |
| `BrowserScraperBuilder::geo_resolver` | method | keep |  | says what it does |
| `BrowserScraperBuilder::per_site_rate` | method | rename | `rate_limit` | core `RateLimiter::new(max_calls, window)`; `HttpOptions::pacing` is the same setting under a third name |
| `BrowserScraperBuilder::seed` | method | keep |  | the seed core `Fingerprint::seed` reports |
| `BrowserScraperBuilder::fingerprint` | method | keep |  | same name as core `LaunchOptionsBuilder::fingerprint`, which takes `&Fingerprint`; take a reference here too. Decision 2 |
| `BrowserScraperBuilder::egress_switch` | method | keep |  | says what it does |
| `BrowserScraperBuilder::launcher` | method | keep |  | says what it does |
| `BrowserScraperBuilder::session_store` | method | keep |  | says what it does |
| `BrowserScraperBuilder::solve_timeout` | method | keep |  | says what it does |
| `BrowserScraperBuilder::clock` | method | keep |  | says what it does |
| `BrowserScraperBuilder::build` | method | keep |  | says what it does |
| `ChromeLauncher` | struct | keep |  | the built-in `BrowserLauncher` |

### `scrape::runtime::egress_switch`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `ContextProxySwitch` | struct | keep |  | switches the egress through core's `ContextOptions::proxy` |
| `ContextProxySwitch::new` | method | keep |  | says what it does |
| `EgressSwitch` | trait | keep |  | follows egress |
| `EgressSwitch::switch` | method | keep |  | says what it does |

### `scrape::runtime::json_store`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `JsonStateStore` | struct | rename | `JsonLedger` | follows `Ledger` |
| `JsonStateStore::open` | method | keep |  | says what it does |
| `JsonStateStore::path` | method | keep |  | says what it does |

### `scrape::runtime::tracing_sink`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `TracingEventSink` | struct | keep |  | follows `EventSink`; the crate logs with `tracing` as core does |

### `scrape::runtime::browser_scraper::builder`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `Clock` | type alias | move | core `common::Clock` | core has `core::allure::Clock` (`Arc<dyn Fn() -> i64>`) and scrape and mimic each have their own; one `Clock` in core `common` |

## mimic (`seleniumbase_rs_stealth::mimic`)

### `mimic::client`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `impersonation_client` | function | rename | `wire_client` | "impersonate" is curl_cffi's word (`impersonate="chrome"`), and core dropped its `impersonate` feature in 0.3.0 |
| `CustomProfileBuilder` | struct | rename | `WireFormatBuilder` | follows `WireFormat`; profile means a browser profile in core |
| `CustomProfileBuilder::new` | method | keep |  | says what it does |
| `CustomProfileBuilder::base` | method | keep |  | says what it does |
| `CustomProfileBuilder::base_on` | method | keep |  | says what it does |
| `CustomProfileBuilder::ja3` | method | keep |  | says what it does |
| `CustomProfileBuilder::ja4_r` | method | keep |  | JA4_r is the format's name |
| `CustomProfileBuilder::ja4_ro` | method | keep |  | JA4_ro is the format's name |
| `CustomProfileBuilder::peetprint` | method | keep |  | says what it does |
| `CustomProfileBuilder::signature_algorithms` | method | keep |  | says what it does |
| `CustomProfileBuilder::grease` | method | keep |  | says what it does |
| `CustomProfileBuilder::permute_extensions` | method | keep |  | says what it does |
| `CustomProfileBuilder::akamai` | method | keep |  | says what it does |
| `CustomProfileBuilder::user_agent` | method | keep |  | says what it does |
| `CustomProfileBuilder::accept_language` | method | keep |  | says what it does |
| `CustomProfileBuilder::client_hints` | method | keep |  | says what it does |
| `CustomProfileBuilder::header` | method | keep |  | says what it does |
| `CustomProfileBuilder::without_header` | method | keep |  | says what it does |
| `CustomProfileBuilder::header_order` | method | keep |  | says what it does |
| `CustomProfileBuilder::priority` | method | keep |  | says what it does |
| `CustomProfileBuilder::headers` | method | keep |  | says what it does |
| `CustomProfileBuilder::build` | method | keep |  | says what it does |
| `ImpersonateError` | enum | rename | `WireFormatError` | follows `WireFormat` |
| `ImpersonateError::Ja3` | enum variant | keep |  | names the case |
| `ImpersonateError::Ja4` | enum variant | keep |  | names the case |
| `ImpersonateError::Peetprint` | enum variant | keep |  | names the case |
| `ImpersonateError::Akamai` | enum variant | keep |  | names the case |
| `ImpersonateError::Tls` | enum variant | keep |  | names the case |
| `ImpersonateError::Http2` | enum variant | keep |  | names the case |
| `ImpersonateError::Emulation` | enum variant | rename | `Measurement` | follows `WireMeasurement` |
| `ImpersonateError::Headers` | enum variant | keep |  | names the case |
| `ImpersonateError::Convert` | enum variant | keep |  | names the case |
| `ImpersonateError::VersionMismatch` | enum variant | keep |  | names the case |
| `ImpersonateError::Incomplete` | enum variant | keep |  | names the case |
| `ImpersonateError::Fingerprint` | enum variant | keep |  | names the case |
| `ImpersonateError::UnknownHeader` | enum variant | keep |  | names the case |
| `ImpersonateError::Build` | enum variant | keep |  | names the case |
| `ImpersonationProfile` | struct | rename | `WireFormat` | **clash**: a profile in core is a browser profile (`profile_payloads`); "impersonation" is curl_cffi's word. It is the wire format a client sends, built from a `WireMeasurement` |
| `ImpersonationProfile::measured` | method | keep |  | says what it does |
| `ImpersonationProfile::for_system` | method | keep |  | says what it does |
| `ImpersonationProfile::from_measured` | method | keep |  | says what it does |
| `ImpersonationProfile::random` | method | keep |  | says what it does |
| `ImpersonationProfile::from_fingerprint` | method | keep |  | says what it does |
| `ImpersonationProfile::with_locale` | method | keep |  | says what it does |
| `ImpersonationProfile::custom` | method | rename | `builder` | builders start from `builder()` here and in core (`BrowserScraper::builder`, `StealthSession::builder`) |
| `ImpersonationProfile::validate` | method | keep |  | says what it does |
| `ImpersonationProfile::source` | method | keep |  | says what it does |
| `ImpersonationProfile::kind` | method | keep |  | says what it does |
| `ImpersonationProfile::os` | method | keep |  | says what it does |
| `ImpersonationProfile::major` | method | keep |  | says what it does |
| `ImpersonationProfile::user_agent` | method | keep |  | says what it does |
| `ImpersonationProfile::accept_language` | method | keep |  | says what it does |
| `ImpersonationProfile::tls_spec` | method | keep |  | says what it does |
| `ImpersonationProfile::http2_spec` | method | rename | `h2_spec` | the type is `H2Spec` |
| `ImpersonationProfile::header_spec` | method | keep |  | says what it does |
| `ImpersonationProfile::identity` | method | rename | `client_headers` | follows codec `ClientHeaders` |
| `ImpersonationProfile::known_gaps` | method | keep |  | says what it does |
| `ImpersonationProfile::dropped_settings` | method | keep |  | says what it does |
| `ImpersonationProfile::headers_for` | method | keep |  | says what it does |
| `ImpersonationProfile::apply` | method | keep |  | says what it does |
| `WireProtocol` | enum | keep |  | and fold `ReplyProtocol` into it |
| `WireProtocol::Http1` | enum variant | keep |  | names the case |
| `WireProtocol::Http2` | enum variant | keep |  | names the case |

### `mimic::http_session`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `HttpAttempt` | struct | rename | `HttpAttempt` (source name) | make it the real name: the source calls it `Attempt` and `lib.rs` renames it; one spelling per item |
| `HttpAttempt.number` | field | keep |  | plain data name |
| `HttpAttempt.previous_challenges` | field | keep |  | plain data name |
| `HttpAttempt.can_rotate_egress` | field | keep |  | plain data name |
| `HttpAttempt.can_escalate` | field | keep |  | plain data name |
| `HttpFetched` | struct | rename | `HttpFetch` | matches `BrowserFetch` and `StealthFetch`; the source calls it `Fetched` |
| `HttpFetched.reply` | field | keep |  | plain data name |
| `HttpFetched.verdict` | field | keep |  | plain data name |
| `ReplyHeaders` | struct | rename | `ResponseHeaders` | follows `HttpResponse`; the source calls it `Headers` |
| `ReplyHeaders::get` | method | keep |  | says what it does |
| `ReplyHeaders::get_all` | method | keep |  | says what it does |
| `ReplyHeaders::iter` | method | keep |  | says what it does |
| `ReplyHeaders::as_pairs` | method | keep |  | says what it does |
| `ReplyHeaders::len` | method | keep |  | says what it does |
| `ReplyHeaders::is_empty` | method | keep |  | says what it does |
| `HttpError` | enum | keep |  | follows `HttpSession` |
| `HttpError::Impersonate` | enum variant | rename | `WireFormat` | follows `WireFormat` |
| `HttpError::VersionMismatch` | enum variant | keep |  | names the case |
| `HttpError::Identity` | enum variant | keep |  | names the case |
| `HttpError::BrowserProxy` | enum variant | keep |  | names the case |
| `HttpError::ReservedHeader` | enum variant | keep |  | names the case |
| `HttpError::InvalidHeader` | enum variant | keep |  | names the case |
| `HttpError::ForbiddenMethod` | enum variant | keep |  | names the case |
| `HttpError::Url` | enum variant | keep |  | names the case |
| `HttpError::TooManyRedirects` | enum variant | keep |  | names the case |
| `HttpError::RedirectRefused` | enum variant | keep |  | names the case |
| `HttpError::BodyTooLarge` | enum variant | keep |  | names the case |
| `HttpError::RequestTooLarge` | enum variant | keep |  | names the case |
| `HttpError::Request` | enum variant | keep |  | names the case |
| `HttpError::Client` | enum variant | keep |  | names the case |
| `HttpError::Options` | enum variant | keep |  | names the case |
| `HttpError::NotUtf8` | enum variant | keep |  | names the case |
| `HttpError::Json` | enum variant | keep |  | names the case |
| `HttpError::Browser` | enum variant | keep |  | names the case |
| `HttpOptions` | struct | rename | `HttpSessionOptions` | **clash**: core `mcp::http::HttpOptions` is the MCP server's HTTP transport; options are named after their owner (core `PoolOptions`, `LaunchOptions`; here `MitmOptions`) |
| `HttpOptions::new` | method | keep |  | says what it does |
| `HttpOptions::timeout` | method | keep |  | says what it does |
| `HttpOptions::connect_timeout` | method | keep |  | says what it does |
| `HttpOptions::max_body_bytes` | method | keep |  | says what it does |
| `HttpOptions::max_request_bytes` | method | keep |  | says what it does |
| `HttpOptions::max_redirects` | method | keep |  | says what it does |
| `HttpOptions::follow_redirects` | method | keep |  | says what it does |
| `HttpOptions::pacing` | method | rename | `rate_limit` | core `RateLimiter`; the builders' `per_site_rate` is the same setting |
| `HttpOptions::event_sink` | method | keep |  | says what it does |
| `HttpOptions::trust` | method | keep |  | says what it does |
| `HttpOptions::resolve` | method | keep |  | says what it does |
| `HttpOptions::bypass` | method | keep |  | says what it does |
| `HttpOptions::browser_proxy_exits_directly` | method | keep |  | says what it does |
| `HttpOptions::installed_chrome_major` | method | keep |  | says what it does |
| `HttpOptions::clock` | method | keep |  | says what it does |
| `HttpOptions::validate` | method | keep |  | says what it does |
| `HttpReply` | struct | rename | `HttpResponse` | HTTP's word, and core's (`sb_cdp::intercept::Response`, `HarResponse`) |
| `HttpReply::status` | method | keep |  | says what it does |
| `HttpReply::is_success` | method | keep |  | says what it does |
| `HttpReply::redacted_url` | method | keep |  | says what it does |
| `HttpReply::url` | method | keep |  | says what it does |
| `HttpReply::headers` | method | keep |  | says what it does |
| `HttpReply::bytes` | method | keep |  | says what it does |
| `HttpReply::text` | method | keep |  | says what it does |
| `HttpReply::json` | method | keep |  | says what it does |
| `HttpReply::protocol` | method | keep |  | says what it does |
| `HttpReply::redirects` | method | keep |  | says what it does |
| `HttpReply::elapsed` | method | keep |  | says what it does |
| `HttpReply::detect` | method | keep |  | says what it does |
| `HttpRequest` | struct | keep |  | the `Http` prefix keeps it apart from core's `sb_cdp::intercept::Request` |
| `HttpRequest::shape` | method | keep |  | says what it does |
| `HttpRequest::header` | method | keep |  | says what it does |
| `HttpRequest::body` | method | keep |  | says what it does |
| `HttpRequest::json` | method | keep |  | says what it does |
| `HttpRequest::form` | method | keep |  | says what it does |
| `HttpRequest::timeout` | method | keep |  | says what it does |
| `HttpRequest::send` | method | keep |  | says what it does |
| `HttpSession` | struct | keep |  | a live HTTP session, as core's `BrowserSession` is a live WebDriver one |
| `HttpSession::from_identity` | method | keep |  | says what it does |
| `HttpSession::request` | method | keep |  | says what it does |
| `HttpSession::get` | method | keep |  | says what it does |
| `HttpSession::profile` | method | rename | `wire_format` | follows `WireFormat` |
| `HttpSession::egress` | method | keep |  | says what it does |
| `HttpSession::cookie_count` | method | keep |  | says what it does |
| `HttpSession::assess` | method | keep |  | says what it does |
| `HttpSession::fetch` | method | keep |  | says what it does |
| `HttpSession::preflight` | method | keep |  | says what it does |
| `HttpSession::export_identity` | method | keep |  | says what it does |
| `HttpSession::from_page` | method | keep |  | says what it does |
| `HttpSession::sync_cookies_back` | method | keep |  | says what it does |
| `Method` | re-export | keep |  | re-export of `wreq::Method`, so callers import no wreq (the pattern of core's `drivers` and `json` re-exports) |
| `ReplyProtocol` | enum | remove | use `WireProtocol` | the same two values (HTTP/1.1, HTTP/2) as `WireProtocol` |
| `ReplyProtocol::Http2` | enum variant | remove |  | goes with its type |
| `ReplyProtocol::Http1` | enum variant | remove |  | goes with its type |
| `HttpRules` | struct | rename | `HttpRules` (source name) | make it the real name: the source calls it `Rules` and `lib.rs` renames it; one spelling per item |
| `HttpRules.mitigation` | field | keep |  | plain data name |
| `HttpRules.session` | field | rename | `escalation` | its type becomes `EscalationPolicy` |
| `HttpVerdict` | struct | rename | `HttpVerdict` (source name) | make it the real name: the source calls it `Verdict` and `lib.rs` renames it; one spelling per item |
| `HttpVerdict.detection` | field | keep |  | plain data name |
| `HttpVerdict.action` | field | keep |  | plain data name |
| `HttpVerdict.transition` | field | rename | `escalation` | follows `Escalation` |
| `HttpVerdict::is_clear` | method | keep |  | says what it does |

### `mimic::mitm`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `MitmError` | enum | keep |  | follows `MitmProxy` |
| `MitmError::Bind` | enum variant | keep |  | names the case |
| `MitmError::Tls` | enum variant | keep |  | names the case |
| `MitmError::InvalidOptions` | enum variant | keep |  | names the case |
| `MitmError::Random` | enum variant | keep |  | names the case |
| `MitmError::Upstream` | enum variant | keep |  | names the case |
| `MitmError::Egress` | enum variant | keep |  | names the case |
| `MitmError::Unauthorized` | enum variant | keep |  | names the case |
| `MitmError::BadTarget` | enum variant | keep |  | names the case |
| `MitmError::ForbiddenDestination` | enum variant | keep |  | names the case |
| `MitmError::NotIntercepted` | enum variant | keep |  | names the case |
| `MitmError::HostMismatch` | enum variant | keep |  | names the case |
| `MitmError::MalformedUpgrade` | enum variant | keep |  | names the case |
| `MitmError::BodyTooLarge` | enum variant | keep |  | names the case |
| `MitmError::VersionMismatch` | enum variant | keep |  | names the case |
| `MitmError::UpstreamRequest` | enum variant | keep |  | names the case |
| `MitmOptions` | struct | keep |  | options named after their owner, as core's `PoolOptions` |
| `MitmOptions::max_connections` | method | keep |  | says what it does |
| `MitmOptions::max_header_bytes` | method | keep |  | says what it does |
| `MitmOptions::max_headers` | method | keep |  | says what it does |
| `MitmOptions::max_body_bytes` | method | keep |  | says what it does |
| `MitmOptions::idle_timeout` | method | keep |  | says what it does |
| `MitmOptions::handshake_timeout` | method | keep |  | says what it does |
| `MitmOptions::connect_timeout` | method | keep |  | says what it does |
| `MitmOptions::read_timeout` | method | keep |  | says what it does |
| `MitmOptions::websocket_idle_timeout` | method | keep |  | says what it does |
| `MitmOptions::max_websocket_bytes` | method | keep |  | says what it does |
| `MitmOptions::trust_upstream_root` | method | keep |  | says what it does |
| `MitmOptions::resolve` | method | keep |  | says what it does |
| `MitmOptions::allow_private_destinations` | method | keep |  | says what it does |
| `MitmOptions::ca` | method | keep |  | says what it does |
| `MitmProxy` | struct | keep |  | MITM is the protocol term; no core proxy server |
| `MitmProxy::start` | method | keep |  | says what it does |
| `MitmProxy::prime_url` | method | keep |  | says what it does |
| `MitmProxy::port` | method | keep |  | says what it does |
| `MitmProxy::local_addr` | method | keep |  | says what it does |
| `MitmProxy::proxy_spec` | method | rename | `proxy_url` | it is the proxy URL with its password, as a `Secret` |
| `MitmProxy::browser_proxy` | method | keep |  | says what it does |
| `MitmProxy::chrome_args` | method | rename | `launch_args` | core's `evasion::evasions::launch_args` returns the same kind of list |
| `MitmProxy::configure_launch` | method | keep |  | says what it does |
| `MitmProxy::stats` | method | keep |  | says what it does |
| `MitmProxy::egress` | method | keep |  | says what it does |
| `MitmProxy::swap_upstream` | method | rename | `swap_egress` | it takes an `EgressRef`; egress is the word here |
| `MitmProxy::swap_profile` | method | rename | `swap_wire_format` | follows `WireFormat` |
| `MitmProxy::shutdown` | method | keep |  | says what it does |
| `MitmStats` | struct | keep |  | as core's `PoolStats` |
| `MitmStats.connections` | field | keep |  | plain data name |
| `MitmStats.tunnels` | field | keep |  | plain data name |
| `MitmStats.unauthorized` | field | keep |  | plain data name |
| `MitmStats.refused` | field | keep |  | plain data name |
| `MitmStats.forwarded` | field | keep |  | plain data name |
| `MitmStats.upstream_errors` | field | keep |  | plain data name |
| `MitmStats.websockets` | field | keep |  | plain data name |
| `MitmStats.primed` | field | keep |  | plain data name |

### `mimic::stealth_session`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `StealthError` | enum | keep |  | follows `StealthSession` |
| `StealthError::Scrape` | enum variant | keep |  | names the case |
| `StealthError::Http` | enum variant | keep |  | names the case |
| `StealthError::Mitm` | enum variant | keep |  | names the case |
| `StealthError::Impersonate` | enum variant | rename | `WireFormat` | follows `WireFormat` |
| `StealthFetch` | struct | keep |  | matches `BrowserFetch` and `HttpFetch` |
| `StealthFetch.url` | field | keep |  | plain data name |
| `StealthFetch.status` | field | keep |  | plain data name |
| `StealthFetch.headers` | field | keep |  | plain data name |
| `StealthFetch.body` | field | keep |  | plain data name |
| `StealthFetch.detection` | field | keep |  | plain data name |
| `StealthFetch.mode` | field | rename | `fetch_kind` | follows `FetchKind` |
| `StealthFetch.attempts` | field | keep |  | plain data name |
| `StealthFetch.solved` | field | keep |  | plain data name |
| `StealthFetch.egress` | field | keep |  | plain data name |
| `StealthSession` | struct | keep |  | the session that moves between `HttpSession` and the browser |
| `StealthSession::builder` | method | keep |  | says what it does |
| `StealthSession::mode` | method | rename | `fetch_kind` | follows `FetchKind` |
| `StealthSession::identity` | method | keep |  | says what it does |
| `StealthSession::egress` | method | keep |  | says what it does |
| `StealthSession::seed` | method | keep |  | says what it does |
| `StealthSession::http` | method | rename | `http_session` | it returns the `HttpSession`; `http` alone could be a client or a protocol |
| `StealthSession::page` | method | keep |  | says what it does |
| `StealthSession::close` | method | keep |  | says what it does |
| `StealthSession::fetch` | method | keep |  | says what it does |
| `StealthSession::rotate_profile` | method | rename | `rotate_fingerprint` | follows `RotateFingerprint` |
| `StealthSession::rotate_egress` | method | keep |  | says what it does |
| `StealthSession::export_identity` | method | keep |  | says what it does |
| `StealthSession::import_identity` | method | keep |  | says what it does |
| `StealthSession::escalate` | method | keep |  | says what it does |
| `StealthSession::demote` | method | keep |  | says what it does |
| `StealthSessionBuilder` | struct | keep |  | core's `<Thing>Builder` pattern |
| `StealthSessionBuilder::launch_options` | method | keep |  | says what it does |
| `StealthSessionBuilder::launcher` | method | keep |  | says what it does |
| `StealthSessionBuilder::with_geo_proxies` | method | rename | `geo_proxies` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `StealthSessionBuilder::proxy_strategy` | method | keep |  | says what it does |
| `StealthSessionBuilder::with_max_challenge_attempts` | method | rename | `max_challenge_attempts` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `StealthSessionBuilder::mitigation` | method | keep |  | says what it does |
| `StealthSessionBuilder::policy` | method | rename | `escalation_policy` | `policy` alone could be the `MitigationPolicy` or core's `StealthPolicy` |
| `StealthSessionBuilder::with_state_store` | method | rename | `ledger` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names; follows `Ledger` |
| `StealthSessionBuilder::with_event_sink` | method | rename | `event_sink` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `StealthSessionBuilder::geo_resolver` | method | keep |  | says what it does |
| `StealthSessionBuilder::per_site_rate` | method | rename | `rate_limit` | core `RateLimiter`; one name with `HttpOptions::pacing` |
| `StealthSessionBuilder::seed` | method | keep |  | the seed core `Fingerprint::seed` reports |
| `StealthSessionBuilder::fingerprint` | method | keep |  | same as `BrowserScraperBuilder::fingerprint`. Decision 2 |
| `StealthSessionBuilder::restore` | method | rename | `identity` | it takes a saved `Identity`; the session's own method is `import_identity` |
| `StealthSessionBuilder::http_options` | method | rename | `http_session_options` | follows `HttpSessionOptions` |
| `StealthSessionBuilder::mitm_options` | method | keep |  | says what it does |
| `StealthSessionBuilder::solve_timeout` | method | keep |  | says what it does |
| `StealthSessionBuilder::clock` | method | keep |  | says what it does |
| `StealthSessionBuilder::build` | method | keep |  | says what it does |

### `mimic::tls_server`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `CaOptions` | struct | keep |  | options named after their owner |
| `CaOptions.ca_validity` | field | keep |  | plain data name |
| `CaOptions.leaf_validity` | field | keep |  | plain data name |
| `CaOptions.leaf_renew_margin` | field | keep |  | plain data name |
| `CaOptions.leaf_capacity` | field | keep |  | plain data name |
| `EphemeralCa` | struct | keep |  | says what it is (a certificate authority that lives for one run) |
| `EphemeralCa::generate` | method | keep |  | says what it does |
| `EphemeralCa::generate_at` | method | keep |  | says what it does |
| `EphemeralCa::spki_pins` | method | keep |  | says what it does |
| `EphemeralCa::chrome_spki_arg` | method | rename | `spki_launch_arg` | follows `launch_args` |
| `EphemeralCa::leaf_spki_pin` | method | keep |  | says what it does |
| `EphemeralCa::certificate` | method | keep |  | says what it does |
| `EphemeralCa::options` | method | keep |  | says what it does |
| `EphemeralCa::not_after_unix` | method | keep |  | says what it does |
| `EphemeralCa::cached_leaves` | method | keep |  | says what it does |
| `EphemeralCa::leaf_for` | method | keep |  | says what it does |
| `Leaf` | struct | rename | `LeafCert` | `Leaf` alone does not say it is a certificate |
| `Leaf::certificate` | method | keep |  | says what it does |
| `Leaf::issuer` | method | keep |  | says what it does |
| `Leaf::host` | method | keep |  | says what it does |
| `Leaf::not_before_unix` | method | keep |  | says what it does |
| `Leaf::not_after_unix` | method | keep |  | says what it does |
| `TlsServerError` | enum | keep |  | follows the `tls_server` module |
| `TlsServerError::Ssl` | enum variant | keep |  | names the case |
| `TlsServerError::Handshake` | enum variant | keep |  | names the case |
| `TlsServerError::HandshakeTimeout` | enum variant | keep |  | names the case |
| `TlsServerError::InvalidHost` | enum variant | keep |  | names the case |
| `TlsServerError::InvalidAlpn` | enum variant | keep |  | names the case |
| `TlsServerError::InvalidOptions` | enum variant | keep |  | names the case |
| `TlsServerError::ClockOutOfRange` | enum variant | keep |  | names the case |
| `TlsServerError::CaExpired` | enum variant | keep |  | names the case |
| `TlsServerError::SniMismatch` | enum variant | keep |  | names the case |

### `mimic::convert`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `ConvertError` | enum | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `ConvertError::UnsupportedAlpn` | enum variant | keep |  | names the case |
| `ConvertError::UnsupportedAlps` | enum variant | keep |  | names the case |
| `ConvertError::CipherName` | enum variant | keep |  | names the case |
| `ConvertError::SigalgName` | enum variant | keep |  | names the case |
| `ConvertError::GroupName` | enum variant | keep |  | names the case |
| `ConvertError::KeyShare` | enum variant | keep |  | names the case |
| `ConvertError::Tls13SuitesNotFirst` | enum variant | keep |  | names the case |
| `ConvertError::VersionGap` | enum variant | keep |  | names the case |
| `ConvertError::ExtensionAlwaysSent` | enum variant | keep |  | names the case |
| `ConvertError::ExtensionVersion` | enum variant | keep |  | names the case |
| `ConvertError::PaddingUnavailable` | enum variant | keep |  | names the case |
| `ConvertError::PaddingAppended` | enum variant | keep |  | names the case |
| `ConvertError::GroupNeedsTls13` | enum variant | keep |  | names the case |
| `ConvertError::CertCompression` | enum variant | keep |  | names the case |
| `ConvertError::StreamId` | enum variant | keep |  | names the case |
| `ConvertError::Weight` | enum variant | keep |  | names the case |
| `ConvertError::Header` | enum variant | keep |  | names the case |

### `mimic::http_session::options`

| Item | Kind | Verdict | Proposed | Why |
|---|---|---|---|---|
| `Clock` | type alias | move | core `common::Clock` | core has `core::allure::Clock` (`Arc<dyn Fn() -> i64>`) and scrape and mimic each have their own; one `Clock` in core `common` |

