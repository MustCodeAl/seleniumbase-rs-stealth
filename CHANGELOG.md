# Changelog

All notable changes to the unified `seleniumbase-rs-stealth` facade and its
`seleniumbase-rs-codec`, `seleniumbase-rs-scrape` and `seleniumbase-rs-mimic` implementations. The format follows
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
