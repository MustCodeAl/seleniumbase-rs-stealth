# Changelog

All notable changes to the unified `seleniumbase-rs-stealth` facade and its
`seleniumbase-rs-codec`, `seleniumbase-rs-scrape` and `seleniumbase-rs-wire` (formerly `seleniumbase-rs-mimic`) implementations. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The crates are experimental
and not published to crates.io, so their types and paths may change between commits.

## [Unreleased]

The workspace follows the fingerprint API v2 of seleniumbase-rs: one `Spoof<T>`
(`Natural`, `Random`, `Value`, `Off`) per setting, read through accessors
(`fp.timezone()`, `fp.screen().value()`), and a built `Fingerprint` that is never
mutated. A change goes through `fp.to_builder()....build()?`, and `build()` can fail.
The design is `docs/design/fingerprint-api-v2.md` in seleniumbase-rs, and its
`CHANGELOG.md` (`## Unreleased`) has the migration table. `Cargo.lock` pins the core
at `c0bd111`; a core from before that change does not compile against these crates.
Only `scrape` and `mimic` were affected; `codec` holds no fingerprint.


### Breaking

- **The launch options are built `LaunchOptions`, not builders.** The core crate keeps what
  `LaunchOptions` was built from (`LaunchOptions::fingerprint()`) and makes it again with
  `with_fingerprint`, `with_headless`, `with_proxy`, `with_args` and `with_url`, under one rule
  (a value set on the options wins over the fingerprint's, in any order). So
  `BrowserScraperBuilder::launch_options` and `StealthSessionBuilder::launch_options` take a
  `LaunchOptions` (add `.build()?` to the chain you passed), the scraper and the session give
  their fingerprint to a copy of it with `with_fingerprint` (a rotation replaces the previous
  fingerprint and nothing else), and `MitmProxy::configure_launch` takes built `LaunchOptions`
  and returns `Result<LaunchOptions, SeleniumBaseError>`: it reads a start page already set
  with `LaunchOptions::url()` instead of building a copy, and a start page set after it is
  `with_url`. Before and after:

  ```rust
  // Before
  let options = mitm.configure_launch(LaunchOptions::builder().headless(true)).build()?;
  // After
  let options = mitm.configure_launch(LaunchOptions::builder().headless(true).build()?)?;
  ```


- **The stealth API is named after seleniumbase-rs, not after the project it was ported
  from** (design note `docs/design/stealth_names.md`, PR #4). The features stay; the names
  and shapes that were copied with them change. No shims.
  - The `mimic` crate is `seleniumbase-rs-wire` (folder `crates/wire`, facade feature and
    path `wire`).
  - codec: the `emulation` module and `Emulation` are `measurement` and `WireMeasurement`
    (`WireMeasurementError`; the lookups are called through the module:
    `measurement::all`, `measurement::for_kind`, `measurement::get`, and the root aliases are
    gone); the spec errors are `H2SpecError` and `TlsSpecError` everywhere
    (the aliases `emulation::SpecError` and `Http2Spec` are gone); `Pseudo` ->
    `PseudoHeader`, `Extension` -> `TlsExtension`, `PeetField` -> `PeetprintField`, `Slot`
    -> `PeetprintSlot`, `HeaderValue` -> `HeaderSource`, `HeaderIdentity` -> `ClientHeaders`.
  - scrape: egress becomes proxy (`ProxyRoute`, `ProxyKey`, `ProxyRouteError`,
    `ProxyOutcome`, `ProxySwitch` in module `proxy_switch`, `ProxyLocale`, `proxy_route()`,
    the `proxy` fields, `rotate_proxy`, `ScrapeError::Proxy`, `NoProxyLeft`); `Mode` ->
    `FetchKind` (`fetch_kind`); `Transition` -> `Escalation`; `SessionPolicy` ->
    `EscalationPolicy`; `Action` -> `Mitigation` (`RotateFingerprint`, `RotateProxy`); the
    `StateStore` family -> `Ledger`, `InMemoryLedger`, `JsonLedger`, `LedgerError`,
    `DomainState` -> `SiteRecord`; `StealthIdentity` -> `Identity` (`wire_measurement`, `verify` -> `validate`); `CookieJar::store` -> `receive`;
    `Evidence` -> `ChallengeEvidence`; `Vendor` -> `ProtectionVendor`;
    `ChallengeKind::None` -> `Clear`; `ScraperEvent` -> `ScrapeEvent` with the event names
    `fetch_kind_changed`, `proxy_rotated`, `fingerprint_rotated`; builder setters without
    `with_` (`geo_proxies`, `max_challenge_attempts`, `ledger`, `event_sink`),
    `per_site_rate` -> `rate_limit`, `rotate_profile` -> `rotate_fingerprint`.
  - wire: `ImpersonationProfile` -> `WireFormat` (`WireFormat::builder()`, `h2_spec()`,
    `client_headers()`), `CustomProfileBuilder` -> `WireFormatBuilder`, `ImpersonateError` ->
    `WireFormatError`, the free function `impersonation_client` -> the method `WireFormat::client_builder`; `HttpReply` ->
    `HttpResponse`, `HttpOptions` -> `HttpSessionOptions` (`rate_limit`), the
    `http_session` types under their exported names (`HttpAttempt`, `HttpFetch`,
    `ResponseHeaders`, `HttpRules`, `HttpVerdict`, fields `escalation`); `MitmProxy`:
    `proxy_url`, `launch_args`, `swap_proxy`, `swap_wire_format`; `spki_launch_arg`;
    `LeafCert`; `StealthSession::http_session()`, `fetch_kind()`, and on its builder
    `escalation_policy`, `identity`, `http_session_options`.
  - Removed as copies: the facade's root re-export of `evasion::*` (use
    `evasion::fingerprint::...`), wire's `ReplyProtocol` (`HttpResponse::protocol` returns
    `WireProtocol`) and wire's own `Clock` (it re-exports scrape's).
  - Serialized names follow the renames (identity files, ledger files, event JSON).
  - Rust API naming review: `wire_client` is the method `WireFormat::client_builder()` (a
    function with an obvious receiver is a method, and it returns a builder); the codec root
    aliases are gone (`measurement::all`, `measurement::for_kind`, `measurement::get`;
    `measured` and `try_measured` are `all` and `try_all`); `Identity::verify` is `validate`;
    `CookieJar::store` is `receive`; `ProxyOutcome::ConnectError` is `ConnectFailed`.
    `WireProtocol`, `KnownGap`, `SetCookieOutcome` and `ProxyRouteError` are
    `#[non_exhaustive]`: each will grow (HTTP/3, newly found gaps, new outcomes).

- **The core module `seleniumbase_rs::stealth` is now `seleniumbase_rs::evasion`.** The crates
  import it under the new name, and the facade re-exports it as
  `seleniumbase_rs_stealth::evasion` (it was `seleniumbase_rs_stealth::stealth`) and, as before,
  at its root (`seleniumbase_rs_stealth::fingerprint`). The crate names and the modules
  `codec`, `scrape` and `mimic` are unchanged. `Cargo.lock` still pins the core before the
  rename; bump it with the core release that has `evasion`.

- **`LocaleExt::apply_to` and `EgressLocale::apply_to` are now `localize` and return the
  next fingerprint.** They wrote into a `&mut Fingerprint`, and a built fingerprint has
  no setters now. The fingerprint they are given is left as it was. The result is the same one the
  old call left behind: the locale, the language list, `Accept-Language`, the time
  zone (the one the egress reported, for `EgressLocale`) and a position near the
  city, a function of the fingerprint's seed. Every other setting is kept.

  ```rust
  // before
  let mut fingerprint = Fingerprint::randomized(OsType::Macos, 7);
  locale.apply_to(&mut fingerprint);
  // after
  let fingerprint = locale.localize(&Fingerprint::randomized(OsType::Macos, 7))?;
  ```

  `LocaleExt::localize` fails with the core's `FingerprintError`, `EgressLocale::localize`
  with `ScrapeError::Fingerprint`. A fingerprint that was built before does not make
  either fail. The old name is gone on purpose: a call to `apply_to(&mut fingerprint)`
  would still have compiled (a mutable reference coerces to a shared one), dropped the
  `Result` with a mere warning and left the fingerprint in its real locale. Now it is a
  compile error.
- **`ScrapeError::Fingerprint(FingerprintError)`** is a new variant (the enum is
  `#[non_exhaustive]`). `rotate_egress`, `rotate_profile` and `build` of the browser
  scraper return it when a fingerprint cannot be built, and so do the rotations and
  `build` of `StealthSession`.
- **Identity files written by an earlier version no longer load.** Their fingerprint is
  in the flat layout of the old core (`screen_width`, `flags`, ...), which the core
  refuses with an error that names no value. `identity_file::load` reports it as a
  corrupt file. Make a new identity.

### Changed

- A fingerprint that denies geolocation (`Spoof::Off`) keeps denying it when it is moved
  to a locale. The old code left the popup mode alone, so a blocked popup kept denying
  the request; the position is not written now, so the denial stays.
- The seed of a restored or imported identity is `Fingerprint::seed()`. A built
  fingerprint always has one: the one given, else fresh entropy when a setting was drawn
  at random, else a value derived from its user agent (so a preset has the same seed
  every time). The old code fell back to the seed of the builder or of the session when
  the fingerprint had none. For an identity of a persona made by these crates the seed
  is the explicit one, as before. An imported identity whose fingerprint is a preset
  now continues from that derived seed, so two sessions that import it draw the same
  next profile on `rotate_profile`.
- `StealthSession` starts from the seed of its builder (`.seed(..)`, else a random one)
  whatever fingerprint it is given, as `BrowserScraper` does. Before this change a
  fingerprint of its own with a seed lent that seed to the session; with the v2 core a
  preset would lend a constant one, and every session on the same preset would then draw
  the same next profile on `rotate_profile`. A restored identity still brings its own
  seed.
- `StealthIdentity::new` clears the proxy by building the fingerprint again with the
  proxy `Natural`, and serialization does the same on a copy. `verify` asks
  `proxy().is_set()`.
- The test that a fingerprint with an infinite battery time cannot be saved is replaced:
  the core now writes that time as `null` and reads it back, so the default battery
  saves. A device memory of infinity, which does not come back equal, is still refused.
- `ImpersonationProfile::from_fingerprint` reads the accessors. A fingerprint whose user
  agent is `Natural` is refused as one with no user agent, as before.
- A challenge with a widget is attempted through the new `scrape::runtime::cdp::solve_widget`,
  which `BrowserScraper::solve_challenge` and `StealthSession` call. It uses the core's CAPTCHA
  solver (`Page::solve_captcha_with`), which presses the widget along a human pointer path, reads
  the page for a result and presses again, up to three times, and fails with
  `SeleniumBaseError::Captcha` when the page never shows it cleared. That error does not end a
  fetch: it is one failed attempt, and the wait for the clearance and the mitigation policy judge
  it, as they judge a script challenge that stands (`ScrapeError::Unsolved` once the attempts are
  used). Only a browser that cannot be read or refuses the input is `ScrapeError::Browser`. The
  scripted browsers of the tests now answer the solver's page scripts, which replaced the single
  locate script they used to match.


### Added

- HTTP/3, experimental and off by default (`docs/http3.md`). The new helper program `sbrs-h3d`
  (`crates/h3d`, a workspace of its own) sends requests with phantom-http `=0.6.0` and its
  Chrome 156 profile. It talks to the wire crate over stdin and stdout, because the two
  BoringSSL builds cannot share one binary. The wire crate's feature `http3` (facade feature
  `http3`) adds `wire::http3::{Http3Client, Http3Options, Http3Request, Http3Response,
  Http3Error}`. Requests go over HTTP/3 only, never falling back, and a missing helper is a typed
  error. The helper's tests drive it against a local quinn/h3 server; the docs page lists what
  matches Chrome and what does not.
- Unified root facade `seleniumbase-rs-stealth`: re-exports the core public API,
  including `sb_cdp`, `stealth` and `fingerprint`, plus default `codec` and
  `scrape` modules. `mimic` is opt-in for BoringSSL-backed TLS/HTTP2 impersonation.
- Single-dependency README examples with valid Git URLs and feature-gated
  combined imports; removed split-dependency setup instructions.
- Facade rustdoc uses feature-safe module references, so documentation with
  default features no longer fails on the disabled `mimic` module link.
- Tests for what the migration decided: a locale move leaves the given fingerprint as
  it was and keeps every other setting, keeps a denied geolocation denied and takes the
  time zone of the egress; an identity file in the old flat layout is refused without
  echoing it; a fingerprint with the natural position gets the position of the locale;
  sessions on the same fingerprint start from the seeds of their builders, and a restored
  identity brings its own.
- `README.md` and `docs/tutorial.md` describe the fingerprint API v2 where these crates
  use it (the locale move, the proxy of an identity, the new error).
