# Stealth names: what the stealth API should cover, build on, add to core and be called

Status: proposal (phase 1, fifth round). No code changes yet; the work starts once the
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

> Its not about merging the stealth apis into main, its about making sure the stealth apis
> covering ground the core project doesnt and adding onto them and making sure these apis
> fit with the rest of the project names

> if the some of the stealthscraping features have different strategy of doing same
> certain things then its fine to integrate them into the core as additional ways of doing
> things, not just getting rid of it cause core does

So every stealth item answers five questions:

1. Does it cover ground core does not? Then it stays here.
2. Does it do a core job with a **different strategy**? Then that strategy is added to core
   as another way of doing the job: an enum variant or a builder option on the core type,
   not a parallel type.
3. Does it hold the same data as a core concept? Then it builds on the core type (takes or
   extends core's `Fingerprint`, `Session`, `Proxy`, `Geolocation`, ...) and adds only the
   stealth part.
4. Is it a copy of a core item with the same strategy? Then it goes, in favour of the core
   item.
5. Does its name fit the project's names and patterns? If not, it is renamed.

The verdicts are **keep**, **rename**, **add to core** (naming the core type and the
option or variant), **build on core** (naming the core type) and **remove**. The low-level
spoofing that stealthscraper-rs brought (TLS/JA4 and HTTP/2 emulation, the MITM proxy, the
impersonating HTTP client) is new ground and stays here.

## Duplicated concepts

| Stealth item | Core concept | Same strategy? | Verdict |
|---|---|---|---|
| `ProxyPool`, `RotationStrategy` | `ProxyList` (`--proxy-list`, `SB_PROXY_LIST`; round-robin or random) | no: least failures, sticky per site, quarantine after failures | **add to core**: `RotationStrategy` (`RoundRobin`, `Random`, `LeastFailures`, `StickyPerSite`) set with `ProxyList::rotation`, plus `quarantine`, `report(name, ProxyOutcome)`, `pick(site)`, `pick_other`. Country picks stay here as an extension trait on `ProxyList` |
| `MitigationPolicy` | `SolveOptions` (`attempts`, `backoff`: press the widget again) | no: rounds of a page, rotating the proxy or fingerprint, cooling down | **add to core**: `SolveOptions::rounds`, `rotate_proxy_after`, `rotate_fingerprint_after`, `cooldown`. The step from HTTP to the browser stays here |
| `BrowserScraper::solve_challenge` | `Page::solve_captcha` (press a widget, check its response or a marker) | no: also waits out a JS interstitial and checks for the clearance | **add to core**: `solve_captcha` waits out an interstitial with no widget; `SolveOptions::clearance_cookie(name)` as another success check. Detection and mitigation around it stay here |
| `solve_timeout` (both builders) | `SolveOptions::verify_timeout` | yes: how long to wait for a solve's result | remove |
| `EgressSwitch`, `ContextProxySwitch` | `ContextOptions::proxy`, `Browser::new_context_with` | `ContextProxySwitch` yes (core's path); the MITM upstream swap no | `ContextProxySwitch` removed; `EgressSwitch` renamed `ProxySwitch`, the seam for core's path and the MITM swap, which needs the MITM proxy and stays here |
| `BrowserLauncher`, `ChromeLauncher` | `Browser::launch`, `BrowserPool::with_launcher` | `ChromeLauncher` yes (it calls `Browser::launch`) | `ChromeLauncher` removed; `BrowserLauncher` builds on core: a launch closure, as `BrowserPool::with_launcher` takes |
| `rotate_profile` | `Page::apply_fingerprint` (in place) | no: relaunches with a new fingerprint in the proxy's locale | rename `rotate_fingerprint`; stays here |
| builders' `seed` | `Fingerprint::seed` | no: seeds a chain, each rotation deriving the next seed | keep |
| builders' `fingerprint` | `LaunchOptionsBuilder::fingerprint` | same data (the first fingerprint) | build on core: read it from the `LaunchOptionsBuilder` given to `launch_options` |
| `StealthIdentity` | `Session`, `Fingerprint` | same data plus stealth parts | build on core: `Identity` holding a core `Session` and `Fingerprint`, plus the wire measurement, proxy route and clearances |
| `ImpersonationProfile` (User-Agent, Accept-Language, client hints) and the matching `CustomProfileBuilder` setters | `Fingerprint` | same data | build on core: `WireFormat` reads them from the `Fingerprint`; `user_agent`, `accept_language`, `with_locale` removed (they return or set the fingerprint's values). `random` stays: it builds on `Fingerprint::randomized_with_major` and also picks the measured wire entry |
| codec `HeaderIdentity` | `Fingerprint` | same data | build on core: `ClientHeaders::from_fingerprint` |
| `EgressRef`, "egress" | `Proxy`, `ContextRoute` | same data plus the exit country | build on core: `ProxyRoute`; "egress" becomes "proxy" in member names |
| `GeoInfo.coordinates` | `Geolocation` | same data | build on core |
| codec `BrowserKind` | `browser::config::Browser` | yes: a list of browsers | remove |
| mimic `Clock` | scrape's `Clock` | yes: the same alias | remove |

Names that clash with core items while covering new ground are renamed: `Emulation` →
`WireMeasurement`, `Evidence` → `ChallengeEvidence`, `StateStore`/`DomainState` →
`Ledger`/`SiteRecord`, `Transition` → `Escalation`, `HttpOptions` → `HttpSessionOptions`.
Already built on core: `LocaleExt` (core `Locale`), `CookieJar` (core `Cookie`),
`ChallengeEvidence.widget` (core `Captcha`), `BrowserScraper` and `StealthSession` (core
`Browser`, `Page`, `LaunchOptionsBuilder`), `save_session`/`load_session` (core
`SessionStore`), `HttpSession::from_page`, `MitmProxy::configure_launch`. Nothing here
duplicates `StealthPolicy` or `BrowserPool`.

## What core gains

The strategies above, in core's patterns, as one core PR in phase 2:

- `config::proxy_list`: `RotationStrategy` (`RoundRobin`, `Random`, `LeastFailures`,
  `StickyPerSite`), `ProxyOutcome`, and on `ProxyList` the options `rotation`,
  `quarantine(after_failures, for_secs)` and the calls `pick(site)`, `pick_other(name,
  site)`, `report(name, outcome)`. `next_round_robin` and `random` become two strategies of
  the same pick.
- `sb_cdp::captcha`: on `SolveOptions` the options `rounds(n)`, `rotate_proxy_after(n)`,
  `rotate_fingerprint_after(n)`, `cooldown(duration)` and `clearance_cookie(name)`; and
  `Page::solve_captcha` waits out a JS interstitial that has no widget.

## Counts

| | Items | Of them copied |
|---|---|---|
| Reviewed | 1049 | 264 |
| Keep | 880 (24 also need an export) | 175 |
| Rename | 106 (8 also need an export) | 55 |
| Add to core (another strategy) | 23 | 18 |
| Build on core | 24 | 9 |
| Remove (same-strategy copy) | 16 | 7 |

| Crate | Keep | Rename | Add to core | Build on core | Remove |
|---|---|---|---|---|---|
| packages, features, facade | 10 | 3 | 0 | 0 | 1 |
| codec | 372 | 12 | 0 | 9 | 3 |
| scrape | 238 | 49 | 23 | 9 | 4 |
| mimic | 260 | 42 | 0 | 6 | 8 |

"Copied" counts an item whose name or shape comes from stealthscraper-rs, with every member
of such a type. "Export" means a public signature takes or returns the type but no path
names it today (32 types). Each "remove" row says in one line why it is the same strategy
as the item that replaces it.

## What changed since the fourth round

The fourth round removed 28 items and built 24 on core. Re-checked for a different
strategy: the proxy rotation strategies, the challenge mitigation (rounds, rotation,
cooldown) and the clearance wait of `solve_challenge` are other ways of doing core jobs,
so they become 23 "add to core" rows. The builders' `seed` (a rotation chain) and
`ImpersonationProfile::random` (it also picks the wire entry) are not copies and stay.
`BrowserLauncher` builds on core's launcher closure instead of going. Sixteen rows remain
removals, each a same-strategy copy.

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
  notes; the stealthscraper-rs crate itself was not read, as the standing rule says.
- **Strategies.** Compared from the doc comments of both sides in their indexes: for
  example core's `SolveOptions::attempts` ("how many times to press the widget") against
  `MitigationPolicy.max_challenge_attempts` (rounds of a page), core's
  `ProxyList::next_round_robin` and `random` against `RotationStrategy`,
  `ChromeLauncher` ("starts Chrome with `Browser::launch`") against core's launch.
- **Core.** The names of all 133 stealth types were compared with core's 986 public types
  (core at `41a2e3da`), and the core items named above were read in core's index.

## The new words in one place

| Now | Proposed |
|---|---|
| stealth identity | `Identity` (on core `Session` and `Fingerprint`) |
| impersonation profile, custom profile builder | `WireFormat`, `WireFormatBuilder` (on core `Fingerprint`) |
| emulation (codec's measured data) | wire measurement (`WireMeasurement`) |
| header identity | client headers (`ClientHeaders`, from core `Fingerprint`) |
| egress, `EgressRef`, `EgressKey`, `EgressSwitch` | proxy route (`ProxyRoute`, `ProxyKey`), `ProxySwitch`, `rotate_proxy`, `swap_proxy` |
| proxy pool strategies | core `RotationStrategy` on `ProxyList` |
| mitigation policy fields | core `SolveOptions` options (`rounds`, `rotate_proxy_after`, `rotate_fingerprint_after`, `cooldown`) |
| mode, transition, session policy | fetch kind (`FetchKind`), escalation (`Escalation`), escalation policy (`EscalationPolicy`) |
| state store, domain state | ledger (`Ledger`), site record (`SiteRecord`) |
| profile rotation | fingerprint rotation (`rotate_fingerprint`) |
| scraper event | scrape event (`ScrapeEvent`) |
| reply | response (`HttpResponse`, `ResponseHeaders`) |
| `with_*` builder setters | plain setters, as core's builders |
| `per_site_rate`, `pacing` | `rate_limit` (core's `RateLimiter`) |
| aliases in `lib.rs` (`Fetched as HttpFetched`, `SpecError as H2SpecError`, ...) | the exported name becomes the source name |

## Decisions for the owner

1. **`Identity`.** The saved identity becomes `Identity`, built on core's `Session` and
   `Fingerprint`. Core's `sb_cdp::identity` module holds `StealthPlan` (there is no core
   `Identity` type). Is the name right?
2. **`fingerprint(...)`.** The stealth builders read the first fingerprint from the
   `LaunchOptionsBuilder` given to `launch_options` (core's `fingerprint` option) instead
   of their own setter. Agreed?
3. **Where the challenge strategies go.** The proposal puts rounds, proxy and fingerprint
   rotation, cooldown and the clearance check on core's `SolveOptions`, and the interstitial
   wait in `Page::solve_captcha`. Rotating needs the page to reach a proxy list and a
   fingerprint source; is `SolveOptions` the right home, or a separate core option on the
   launch (`LaunchOptionsBuilder::challenges(...)`)?
4. **Proxy countries.** Core's `ProxyList` gains the rotation strategies. Should
   `ProxyEntry` also carry an exit country (a country code type in core), or do country
   picks stay a stealth extension trait on `ProxyList`, as proposed?
5. **The `mimic` crate.** Rename the package `seleniumbase-rs-wire` (feature and facade
   path `wire`)?
6. **Two wire words.** `WireMeasurement` (codec: measured data with provenance) and
   `WireFormat` (mimic: what a client sends, built from a measurement and a core
   `Fingerprint`), or one word for both?
7. **Events.** Keep the typed `EventSink` with its `tracing` sink, or send stealth events
   through `tracing` only?

## Phase 2 (after approval)

One core PR for the strategies in "What core gains", then one stealth PR: renames with
`tokensave rename` (a dry run first), the build-on-core changes, the removals, the switch
to core's new strategies, every caller and test, the stealth API reference pages and the
contract files. Breaking changes, no shims.

## Every public item

One row per item. "Copied" marks an item whose name or shape comes from stealthscraper-rs
(with every member of such a type). Verdicts: keep, rename, add to core (another strategy,
with the core type and the option or variant), build on core (the core type it should take
or extend), remove (a same-strategy copy, with the reason); "export" when a public signature
takes or returns the type but no path names it today.

## Packages, features and the facade

### (packages)

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `seleniumbase-rs-stealth` (package) | package |  | keep |  | says what it adds to seleniumbase-rs |
| `seleniumbase-rs-codec` (package) | package |  | keep |  | it encodes and decodes wire fingerprints |
| `seleniumbase-rs-scrape` (package) | package |  | keep |  | the scraping runtime; the docs say scrape |
| `seleniumbase-rs-mimic` (package) | package |  | rename | `seleniumbase-rs-wire` | "mimic" does not say what it holds: the HTTP client that sends a browser's wire format, the MITM proxy and the session that switches between them; the docs call this wire formats. Decision 5 |

### (features)

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `default = ["scrape"]` | feature |  | keep |  |  |
| `codec` | feature |  | keep |  | named after its crate |
| `scrape` | feature |  | keep |  | named after its crate |
| `mimic` | feature |  | rename | `wire` | follows the package. Decision 5 |
| `test-fixtures` (codec) | feature |  | keep |  | says what it turns on |

### src/lib.rs

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `pub use seleniumbase_rs::*` | re-export |  | keep |  | the facade is core plus the stealth crates |
| `pub use seleniumbase_rs::evasion::*` | re-export |  | remove |  | same strategy: gives every `evasion` item a second path at the root; core keeps them under `evasion::`, one path per item |
| `codec` (`seleniumbase_rs_codec`) | re-export |  | keep |  |  |
| `scrape` (`seleniumbase_rs_scrape`) | re-export |  | keep |  |  |
| `mimic` (`seleniumbase_rs_mimic`) | re-export |  | rename | `wire` | follows the package. Decision 5 |

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
| `emulation_for_kind` | function | yes | rename | `wire_measurement_for` | **copied** (`for_kind`); follows `WireMeasurement`, and takes core's `Browser` once `BrowserKind` goes |
| `get_emulation` | function |  | rename | `wire_measurement` | follows `WireMeasurement`; no `get_` prefix (Rust API guidelines, C-GETTER) |
| `measured_emulations` | function |  | rename | `wire_measurements` | follows `WireMeasurement` |
| `Emulation` | struct | yes | rename | `WireMeasurement` | **copied** word (their identity's "emulation kind"); **clash** with core `sb_cdp::state::Emulation` (device emulation). It is one browser's measured TLS, HTTP/2 and header shape, which core does not have |
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
| `Http2Spec` | type alias |  | remove | `H2Spec` | same strategy: a second name for `H2Spec` |

### `codec::header_spec`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `HeaderIdentity` | struct |  | build on core | `ClientHeaders`, made from core `Fingerprint` (`from_fingerprint`); keeps the cookie and the overrides | User-Agent, Accept-Language and client hints are the fingerprint's claims; a parallel copy drifts from them |
| `HeaderIdentity.user_agent` | field |  | build on core | read from core `Fingerprint` | the fingerprint's User-Agent |
| `HeaderIdentity.accept_language` | field |  | build on core | read from core `Fingerprint` | the fingerprint's languages |
| `HeaderIdentity.sec_ch_ua` | field |  | build on core | read from core `Fingerprint` | the fingerprint's client hints |
| `HeaderIdentity.sec_ch_ua_mobile` | field |  | build on core | read from core `Fingerprint` | the fingerprint's client hints |
| `HeaderIdentity.sec_ch_ua_platform` | field |  | build on core | read from core `Fingerprint` | the fingerprint's client hints |
| `HeaderIdentity.cookie` | field |  | keep |  | plain data name |
| `HeaderIdentity.overrides` | field |  | keep |  | plain data name |
| `HeaderIdentity::new` | method |  | build on core | `from_fingerprint(&Fingerprint)` | the values come from core's `Fingerprint` |
| `HeaderIdentity::with_accept_language` | method |  | build on core | set on core `Fingerprint` | the fingerprint's languages |
| `HeaderIdentity::with_client_hints` | method |  | build on core | set on core `Fingerprint` | the fingerprint's client hints |
| `HeaderIdentity::with_cookie` | method |  | keep |  | says what it does |
| `HeaderIdentity::with_override` | method |  | keep |  | says what it does |
| `HeaderIdentity::from_captured` | method |  | keep |  | says what it does |
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
| `BrowserKind` | enum | yes | remove | core `browser::config::Browser` | **copied** (`BrowserKind`); same strategy: a list of browsers, as core's `Browser` enum (one variant here, `Chrome`) |
| `BrowserKind::Chrome` | enum variant | yes | remove |  | goes with its type |

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
| `ChallengeKind` | enum | yes | keep |  | covers ground core does not: what kind of block a page or response shows |
| `ChallengeKind::None` | enum variant | yes | rename | `Clear` | `ChallengeKind::None` reads like `Option::None`; `HttpVerdict::is_clear` already says clear |
| `ChallengeKind::JsChallenge` | enum variant | yes | keep |  | names the case |
| `ChallengeKind::Interactive` | enum variant | yes | keep |  | names the case |
| `ChallengeKind::Blocked` | enum variant | yes | keep |  | names the case |
| `ChallengeKind::RateLimited` | enum variant | yes | keep |  | names the case |
| `Detection` | struct | yes | keep |  | covers ground core does not: detection on HTTP responses as well as pages |
| `Detection.kind` | field | yes | keep |  | plain data name |
| `Detection.evidence` | field | yes | keep |  | plain data name |
| `Evidence` | struct | yes | rename | `ChallengeEvidence` | **clash** with core `sb_cdp::captcha::Evidence`; its `widget` already builds on core's `Captcha` |
| `Evidence.vendor` | field | yes | keep |  | plain data name |
| `Evidence.markers` | field | yes | keep |  | plain data name |
| `Evidence.widget` | field | yes | keep |  | already core's `Captcha` |
| `ResponseView` | struct |  | keep |  | covers ground core does not |
| `ResponseView.url` | field |  | keep |  | plain data name |
| `ResponseView.status` | field |  | keep |  | plain data name |
| `ResponseView.headers` | field |  | keep |  | plain data name |
| `ResponseView.body` | field |  | keep |  | plain data name |
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
| `Action` | enum | yes | rename | `Mitigation` | too generic as a re-exported name; it is the step the `mitigation` module picks |
| `Action::Proceed` | enum variant | yes | keep |  | names the case |
| `Action::Retry` | enum variant | yes | keep |  | names the case |
| `Action::Escalate` | enum variant | yes | keep |  | names the case |
| `Action::Solve` | enum variant | yes | keep |  | names the case |
| `Action::RotateEgress` | enum variant | yes | keep |  | names the case |
| `Action::RotateProfile` | enum variant | yes | rename | `RotateFingerprint` | a profile in core is a browser profile (`profile_payloads`, the Browser profiles page); this draws a new fingerprint |
| `Action::Cooldown` | enum variant | yes | keep |  | names the case |
| `Action::GiveUp` | enum variant | yes | keep |  | names the case |
| `AttemptContext` | struct |  | keep |  | covers ground core does not |
| `AttemptContext.attempt` | field |  | keep |  | plain data name |
| `AttemptContext.mode` | field |  | rename | `fetch_kind` | follows `Mode` → `FetchKind` |
| `AttemptContext.consecutive_challenges` | field |  | keep |  | plain data name |
| `AttemptContext.can_rotate_egress` | field |  | keep |  | plain data name |
| `AttemptContext.can_escalate` | field |  | keep |  | plain data name |
| `AttemptContext.now` | field |  | keep |  | plain data name |
| `GiveUpReason` | enum |  | keep |  | says what it is; no core item of that name or concept |
| `GiveUpReason::AttemptsExhausted` | enum variant |  | keep |  | names the case |
| `GiveUpReason::Blocked` | enum variant |  | keep |  | names the case |
| `GiveUpReason::NoEgressLeft` | enum variant |  | keep |  | names the case |
| `MitigationPolicy` | struct | yes | add to core | core `SolveOptions`: `rounds(n)`, `rotate_proxy_after(n)`, `rotate_fingerprint_after(n)`, `cooldown(duration)`; the step to the browser stays here | **copied**; core retries by pressing the widget again (`attempts`, `backoff`), this retries across rounds by rotating the proxy or the fingerprint and cooling down: another retry strategy. Decision 3 |
| `MitigationPolicy.max_challenge_attempts` | field | yes | add to core | `SolveOptions::rounds(n)` | not core's `attempts` (presses of one widget): it counts rounds of a page through challenges |
| `MitigationPolicy.backoff` | field | yes | build on core | core `SolveOptions::backoff` | already core's `RetryPolicy`; once the strategy is in `SolveOptions`, it is that option |
| `MitigationPolicy.rotate_egress_after` | field | yes | add to core | `SolveOptions::rotate_proxy_after(n)` | another retry step |
| `MitigationPolicy.rotate_profile_after` | field | yes | add to core | `SolveOptions::rotate_fingerprint_after(n)` | another retry step |
| `MitigationPolicy.block_cooldown_secs` | field | yes | add to core | `SolveOptions::cooldown(duration)` | another retry step; a `Duration`, as core's options take |
| `MitigationPolicy::decide` | method | yes | add to core | inside core's `Page::solve_captcha` | the strategy's logic |
| `Mode` | enum |  | rename | `FetchKind` | core says mode for engine modes (UC mode, CDP Mode, `DriverMode`); this says whether a request goes through the browser or over HTTP. `Route` is taken by `egress::route` |
| `Mode::Browser` | enum variant |  | keep |  | names the case |
| `Mode::Http` | enum variant |  | keep |  | names the case |

### `scrape::egress::geo`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CountryCode` | struct | yes | keep |  | core's `LocaleCode` is a language tag; the ISO country of a proxy's exit is new ground |
| `CountryCode::new` | method | yes | keep |  | says what it does |
| `CountryCode::as_str` | method | yes | keep |  | says what it does |
| `GeoResolver` | trait | yes | keep |  | **copied** port, but covers ground core does not: finding a proxy's exit country |
| `GeoResolver::resolve` | method | yes | keep |  | says what it does |
| `LocaleExt` | trait | yes | keep |  | already builds on core's `Locale`, the shape this note asks for |
| `LocaleExt::for_country` | method | yes | keep |  | extends core's `Locale` |
| `LocaleExt::by_tag` | method | yes | keep |  | extends core's `Locale` |
| `LocaleExt::localize` | method | yes | keep |  | extends core's `Locale` and `Fingerprint` |
| `StaticGeoResolver` | struct |  | keep |  | the built-in `GeoResolver` |
| `GeoError` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `GeoError::InvalidCountry` | enum variant |  | keep |  | names the case |
| `GeoError::NoLocale` | enum variant |  | keep |  | names the case |
| `GeoError::Unknown` | enum variant |  | keep |  | names the case |
| `GeoInfo` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `GeoInfo.country` | field |  | keep |  | plain data name |
| `GeoInfo.timezone` | field |  | keep |  | plain data name |
| `GeoInfo.coordinates` | field |  | build on core | core `Geolocation` | core's type for a latitude and longitude, instead of `(f64, f64)` |

### `scrape::egress::pool`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ProxyPool` | struct | yes | add to core | core `ProxyList`: `rotation(RotationStrategy)`, `quarantine(after_failures, for_secs)`, `report(name, ProxyOutcome)`, `pick(site)`, `pick_other(name, site)`; country picks stay here as an extension trait on `ProxyList` | **copied** (`ProxyPool`); core's `ProxyList` picks round-robin or at random, the pool adds other ways (least failures, sticky per site, quarantine after failures). Decision 4 |
| `ProxyPool::new` | method | yes | add to core | core `ProxyList` with `rotation(RotationStrategy)` | the list is core's; the strategy is an option on it |
| `ProxyPool::with_geo_proxies` | method | yes | rename | `from_geo_proxies` | a constructor with countries; core says `from_*` (`ProxyList::from_file`). Part of the country extension |
| `ProxyPool::quarantine` | method | yes | add to core | `ProxyList::quarantine(after_failures, for_secs)` | another way to rotate: rest a proxy after failures |
| `ProxyPool::pick` | method | yes | add to core | `ProxyList::pick(site)` | picks by the list's `RotationStrategy` |
| `ProxyPool::pick_in` | method | yes | keep |  | picks by exit country, which core does not know; an extension trait on core's `ProxyList` |
| `ProxyPool::pick_other` | method | yes | add to core | `ProxyList::pick_other(name, site)` | the next proxy when one fails |
| `ProxyPool::report` | method | yes | add to core | `ProxyList::report(name, ProxyOutcome)` | feeds `LeastFailures` and the quarantine |
| `RotationStrategy` | enum | yes | add to core | core `config::proxy_list::RotationStrategy`, set with `ProxyList::rotation` | **copied** name; core's `RoundRobin` and `Random` become two variants beside `LeastFailures` and `StickyPerSite`. Named like core's `PageLoadStrategy` |
| `RotationStrategy::RoundRobin` | enum variant | yes | add to core | variant of core `RotationStrategy` | core's `ProxyList::next_round_robin` today |
| `RotationStrategy::Random` | enum variant | yes | add to core | variant of core `RotationStrategy` | core's `ProxyList::random` today |
| `RotationStrategy::LeastFailures` | enum variant | yes | add to core | variant of core `RotationStrategy` | another strategy core lacks |
| `RotationStrategy::StickyPerSite` | enum variant | yes | add to core | variant of core `RotationStrategy` | another strategy core lacks |
| `EgressOutcome` | enum |  | add to core | core `ProxyOutcome`, the argument of `ProxyList::report` | goes with the rotation strategies |
| `EgressOutcome::Success` | enum variant |  | add to core | variant of core `ProxyOutcome` | goes with its type |
| `EgressOutcome::ConnectError` | enum variant |  | add to core | variant of core `ProxyOutcome` | goes with its type |
| `EgressOutcome::Challenged` | enum variant |  | add to core | variant of core `ProxyOutcome` | goes with its type |
| `EgressOutcome::Blocked` | enum variant |  | add to core | variant of core `ProxyOutcome` | goes with its type |

### `scrape::egress::redact`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `RedactedUrl` | struct | yes | keep |  | covers ground core does not: core has no redacted URL type |
| `RedactedUrl::new` | method | yes | keep |  | says what it does |
| `RedactedUrl::placeholder` | method | yes | keep |  | says what it does |
| `RedactedUrl::as_str` | method | yes | keep |  | says what it does |
| `Secret` | struct | yes | keep |  | covers ground core does not: core's `Proxy` hides its password in its own `Debug`, with no secret type |
| `Secret::new` | method | yes | keep |  | says what it does |
| `Secret::expose` | method | yes | keep |  | says what it does |

### `scrape::egress::route`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `EgressKey` | struct |  | rename | `ProxyKey` | core says proxy |
| `EgressKey.scheme` | field |  | keep |  | plain data name |
| `EgressKey.host` | field |  | keep |  | plain data name |
| `EgressKey.port` | field |  | keep |  | plain data name |
| `EgressKey.country` | field |  | keep |  | plain data name |
| `EgressRef` | struct | yes | build on core | `ProxyRoute`, on core `Proxy` and `ContextRoute`, plus the exit country | **copied** (`EgressRef`); it already wraps core's `Proxy`, and core's `ContextRoute` is the same route kept for plain HTTP requests. The name should be core's word |
| `EgressRef::parse` | method | yes | keep |  | says what it does |
| `EgressRef::key` | method | yes | keep |  | says what it does |
| `EgressRef::proxy` | method | yes | keep |  | says what it does |
| `EgressError` | enum |  | rename, export | `ProxyRouteError` | follows `ProxyRoute` |
| `EgressError::Scheme` | enum variant |  | keep |  | names the case |
| `EgressError::Address` | enum variant |  | keep |  | names the case |

### `scrape::error`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ScrapeError` | enum |  | keep |  | the crate's error, as core's `SeleniumBaseError` |
| `ScrapeError::Egress` | enum variant |  | rename | `Proxy` | core says proxy |
| `ScrapeError::Geo` | enum variant |  | keep |  | names the case |
| `ScrapeError::State` | enum variant |  | rename | `Ledger` | follows `StateStore` → `Ledger` |
| `ScrapeError::Identity` | enum variant |  | keep |  | names the case |
| `ScrapeError::Fingerprint` | enum variant |  | keep |  | names the case |
| `ScrapeError::Unsolved` | enum variant |  | keep |  | names the case |
| `ScrapeError::GaveUp` | enum variant |  | keep |  | names the case |
| `ScrapeError::CoolingDown` | enum variant |  | keep |  | names the case |
| `ScrapeError::Navigation` | enum variant |  | keep |  | names the case |
| `ScrapeError::NoEgressAvailable` | enum variant |  | rename | `NoProxyLeft` | core says proxy |
| `ScrapeError::Browser` | enum variant |  | keep |  | names the case |

### `scrape::jar::store`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CookieJar` | struct | yes | keep |  | **copied** (RFC 6265 jar), but covers ground core does not (an HTTP cookie jar) and already builds on core's `Cookie` (`from_cdp`, `to_cdp`) |
| `CookieJar::new` | method | yes | keep |  | says what it does |
| `CookieJar::len` | method | yes | keep |  | says what it does |
| `CookieJar::is_empty` | method | yes | keep |  | says what it does |
| `CookieJar::cookies` | method | yes | keep |  | says what it does |
| `CookieJar::store` | method | yes | rename | `apply_set_cookie` | store means a persistent store in core (`SessionStore`) and here (`StateStore`); this applies one Set-Cookie header |
| `CookieJar::header_for` | method | yes | keep |  | says what it does |
| `CookieJar::from_cdp` | method | yes | keep |  | says what it does |
| `CookieJar::to_cdp` | method | yes | keep |  | says what it does |
| `CookieJar::expiry_of` | method | yes | keep |  | says what it does |
| `CookieJar::remove_expired` | method | yes | keep |  | says what it does |
| `StoredCookie` | struct |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `StoredCookie.name` | field |  | keep |  | plain data name |
| `StoredCookie.value` | field |  | keep |  | plain data name |
| `StoredCookie.domain` | field |  | keep |  | plain data name |
| `StoredCookie.host_only` | field |  | keep |  | plain data name |
| `StoredCookie.path` | field |  | keep |  | plain data name |
| `StoredCookie.expires` | field |  | keep |  | plain data name |
| `StoredCookie.secure` | field |  | keep |  | plain data name |
| `StoredCookie.http_only` | field |  | keep |  | plain data name |
| `StoredCookie.same_site` | field |  | keep |  | plain data name |
| `StoredCookie.created` | field |  | keep |  | plain data name |
| `StoredCookie.last_access` | field |  | keep |  | plain data name |
| `StoredCookie::is_expired` | method |  | keep |  | says what it does |
| `StoredCookie::size_bytes` | method |  | keep |  | says what it does |
| `StoredCookie::same_identity` | method |  | keep |  | says what it does |
| `StoreOutcome` | enum |  | rename, export | `SetCookieOutcome` | follows `apply_set_cookie` |
| `StoreOutcome::Stored` | enum variant |  | keep |  | names the case |
| `StoreOutcome::Replaced` | enum variant |  | keep |  | names the case |
| `StoreOutcome::Deleted` | enum variant |  | keep |  | names the case |
| `StoreOutcome::Ignored` | enum variant |  | keep |  | names the case |
| `SiteContext` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `SiteContext::SameSite` | enum variant |  | keep |  | names the case |
| `SiteContext::CrossSite` | enum variant |  | keep |  | names the case |
| `IgnoreReason` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `IgnoreReason::Malformed` | enum variant |  | keep |  | names the case |
| `IgnoreReason::DomainMismatch` | enum variant |  | keep |  | names the case |
| `IgnoreReason::PublicSuffix` | enum variant |  | keep |  | names the case |
| `IgnoreReason::IpLiteralDomain` | enum variant |  | keep |  | names the case |
| `IgnoreReason::SecureFromInsecure` | enum variant |  | keep |  | names the case |
| `IgnoreReason::HostPrefix` | enum variant |  | keep |  | names the case |
| `IgnoreReason::SecurePrefix` | enum variant |  | keep |  | names the case |
| `IgnoreReason::TooLarge` | enum variant |  | keep |  | names the case |
| `IgnoreReason::SameSiteNoneInsecure` | enum variant |  | keep |  | names the case |
| `IgnoreReason::UnsupportedUrl` | enum variant |  | keep |  | names the case |

### `scrape::ledger::events`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CollectingEventSink` | struct |  | keep |  | follows `EventSink` |
| `CollectingEventSink::new` | method |  | keep |  | says what it does |
| `CollectingEventSink::events` | method |  | keep |  | says what it does |
| `CollectingEventSink::take` | method |  | keep |  | says what it does |
| `CollectingEventSink::len` | method |  | keep |  | says what it does |
| `CollectingEventSink::is_empty` | method |  | keep |  | says what it does |
| `EventSink` | trait | yes | keep |  | **copied** name, but covers ground core does not: a typed stream of stealth events. Decision 7 |
| `EventSink::emit` | method | yes | keep |  | says what it does |
| `NoopEventSink` | struct | yes | keep |  | follows `EventSink` |
| `ScraperEvent` | enum | yes | rename | `ScrapeEvent` | the crate and its docs say scrape, not scraper |
| `ScraperEvent::Request` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::ChallengeDetected` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::ChallengeSolved` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::ActionChosen` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::ModeChanged` | enum variant | yes | rename | `FetchKindChanged` | follows `FetchKind` |
| `ScraperEvent::EgressRotated` | enum variant | yes | rename | `ProxyRotated` | core says proxy |
| `ScraperEvent::ProfileRotated` | enum variant | yes | rename | `FingerprintRotated` | follows `RotateFingerprint` |
| `ScraperEvent::ClearanceExpiring` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::CooldownStarted` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::IdentityExported` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::IdentityRestored` | enum variant | yes | keep |  | names the case |
| `ScraperEvent::name` | method | yes | keep |  | says what it does |

### `scrape::ledger::state`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `DomainState` | struct | yes | rename | `SiteRecord` | **clash**: core `sb_cdp::state` is a page's cookies and storage; this is one site's challenge history, which the module calls the ledger |
| `DomainState.site` | field | yes | keep |  | plain data name |
| `DomainState.successes` | field | yes | keep |  | plain data name |
| `DomainState.challenges` | field | yes | keep |  | plain data name |
| `DomainState.blocks` | field | yes | keep |  | plain data name |
| `DomainState.consecutive_challenges` | field | yes | keep |  | plain data name |
| `DomainState.last_challenge` | field | yes | keep |  | plain data name |
| `DomainState.last_seen` | field | yes | keep |  | plain data name |
| `DomainState.cooldown_until` | field | yes | keep |  | plain data name |
| `DomainState.preferred_mode` | field | yes | rename | `preferred_kind` | follows `FetchKind` |
| `DomainState.last_egress` | field | yes | rename | `last_proxy` | core says proxy |
| `DomainState::new` | method | yes | keep |  | says what it does |
| `DomainState::record` | method | yes | keep |  | says what it does |
| `DomainState::start_cooldown` | method | yes | keep |  | says what it does |
| `DomainState::in_cooldown` | method | yes | keep |  | says what it does |
| `InMemoryStateStore` | struct | yes | rename | `InMemoryLedger` | follows `Ledger` |
| `InMemoryStateStore::new` | method | yes | keep |  | says what it does |
| `StateStore` | trait | yes | rename | `Ledger` | **clash** with core `sb_cdp::state` (see `DomainState`); the module is already `ledger` |
| `StateStore::get` | method | yes | keep |  | says what it does |
| `StateStore::put` | method | yes | keep |  | says what it does |
| `StateStore::remove` | method | yes | keep |  | says what it does |
| `StateStore::sites` | method | yes | keep |  | says what it does |
| `StateError` | enum |  | rename, export | `LedgerError` | follows `Ledger` |
| `StateError::Io` | enum variant |  | keep |  | names the case |
| `StateError::Corrupt` | enum variant |  | keep |  | names the case |
| `StateError::Schema` | enum variant |  | keep |  | names the case |
| `StateError::TooLarge` | enum variant |  | keep |  | names the case |

### `scrape::passport::identity`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `EmulationRef` | struct |  | rename | `WireMeasurementRef` | follows codec `WireMeasurement` |
| `EmulationRef.kind` | field |  | keep |  | plain data name |
| `EmulationRef.major` | field |  | keep |  | plain data name |
| `EmulationRef.os` | field |  | keep |  | plain data name |
| `IdentityError` | enum |  | keep |  | follows `Identity` |
| `IdentityError::EgressMismatch` | enum variant |  | rename | `ProxyMismatch` | core says proxy |
| `IdentityError::Schema` | enum variant |  | keep |  | names the case |
| `IdentityError::Incoherent` | enum variant |  | keep |  | names the case |
| `IdentityError::EmulationMismatch` | enum variant |  | rename | `WireMeasurementMismatch` | follows `WireMeasurement` |
| `StealthIdentity` | struct | yes | build on core | `Identity`, holding core `Session` (cookies, storage) and `Fingerprint`, plus the wire measurement, proxy route and clearances | **copied** (`StealthIdentity`); core's `Session` is a browser's login state and core's `Fingerprint` its claims. The identity should be made of them and add only what is stealth's. Decision 1 |
| `StealthIdentity.schema` | field | yes | keep |  | plain data name |
| `StealthIdentity.fingerprint` | field | yes | keep |  | already core's `Fingerprint` |
| `StealthIdentity.emulation` | field | yes | rename | `wire_measurement` | follows `WireMeasurementRef` |
| `StealthIdentity.cookies` | field | yes | build on core | core `Session` | cookies (and storage) as core keeps them |
| `StealthIdentity.accept_ch` | field | yes | keep |  | plain data name |
| `StealthIdentity.egress` | field | yes | rename | `proxy_route` | follows `ProxyRoute` |
| `StealthIdentity.clearances` | field | yes | keep |  | plain data name |
| `StealthIdentity.created_at` | field | yes | keep |  | plain data name |
| `StealthIdentity::new` | method | yes | keep |  | says what it does |
| `StealthIdentity::verify` | method | yes | rename | `validate` | core `Fingerprint::validate`; `HeaderSpec`, `ImpersonationProfile` and `HttpOptions` here say validate too |
| `StealthIdentity::refresh_clearances` | method | yes | keep |  | says what it does |
| `StealthIdentity::clearance_for` | method | yes | keep |  | says what it does |
| `StealthIdentity::check_egress` | method | yes | rename | `check_proxy` | core says proxy |
| `StealthIdentity::check_emulation` | method | yes | rename | `check_wire_measurement` | follows `WireMeasurement` |
| `Clearance` | struct |  | keep, export |  | covers ground core does not: a challenge clearance |
| `Clearance.site` | field |  | keep |  | plain data name |
| `Clearance.vendor` | field |  | keep |  | plain data name |
| `Clearance.cookie` | field |  | keep |  | plain data name |
| `Clearance.expires_at` | field |  | keep |  | plain data name |

### `scrape::passport::policy`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `SessionPolicy` | struct | yes | rename | `EscalationPolicy` | **copied** (`SessionPolicy`, same fields); covers ground core does not (when to move between HTTP and the browser), and a session in core is a saved `Session` |
| `SessionPolicy.require_clearance_for_http` | field | yes | keep |  | plain data name |
| `SessionPolicy.clearance_margin` | field | yes | keep |  | plain data name |
| `SessionPolicy.demote_after_solve` | field | yes | keep |  | plain data name |
| `SessionPolicy.prefer` | field | yes | keep |  | plain data name |
| `SessionPolicy::before_fetch` | method | yes | keep |  | says what it does |
| `SessionPolicy::after_fetch` | method | yes | keep |  | says what it does |
| `Transition` | enum |  | rename | `Escalation` | **clash**: core `api::presentation::Transition` is a slide transition |
| `Transition::Stay` | enum variant |  | keep |  | names the case |
| `Transition::Escalate` | enum variant |  | keep |  | names the case |
| `Transition::Demote` | enum variant |  | keep |  | names the case |
| `Transition::resulting_mode` | method |  | rename | `resulting_kind` | follows `FetchKind` |
| `EscalateReason` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `EscalateReason::Challenge` | enum variant |  | keep |  | names the case |
| `EscalateReason::ClearanceExpiring` | enum variant |  | keep |  | names the case |
| `EscalateReason::NoClearance` | enum variant |  | keep |  | names the case |
| `EscalateReason::SiteNeedsBrowser` | enum variant |  | keep |  | names the case |
| `EscalateReason::Manual` | enum variant |  | keep |  | names the case |
| `DemoteReason` | enum |  | keep, export |  | keep the name and export it: a public signature takes or returns it, but callers cannot name it |
| `DemoteReason::ChallengeSolved` | enum variant |  | keep |  | names the case |
| `DemoteReason::Manual` | enum variant |  | keep |  | names the case |

### `scrape::runtime::browser_scraper`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `BrowserFetch` | struct |  | keep |  | one fetch's result; matches `StealthFetch` and `HttpFetch` |
| `BrowserFetch.url` | field |  | keep |  | plain data name |
| `BrowserFetch.status` | field |  | keep |  | plain data name |
| `BrowserFetch.headers` | field |  | keep |  | plain data name |
| `BrowserFetch.body` | field |  | keep |  | plain data name |
| `BrowserFetch.detection` | field |  | keep |  | plain data name |
| `BrowserFetch.attempts` | field |  | keep |  | plain data name |
| `BrowserFetch.solved` | field |  | keep |  | plain data name |
| `BrowserFetch.egress` | field |  | rename | `proxy_route` | follows `ProxyRoute` |
| `BrowserLauncher` | trait |  | build on core | a launch closure, as core's `BrowserPool::with_launcher` takes | the scraper relaunches on rotation, which is its own; the seam itself is core's pattern (a function that starts a browser), so take a closure the same way |
| `BrowserLauncher::launch` | method |  | build on core | the closure's call | goes with its type |
| `BrowserScraper` | struct | yes | keep |  | **copied** shape (their `CloudScraper`), but covers ground core does not (the challenge loop, proxy rotation, the ledger) and it already builds on core's `Browser` and `Page`; its members drop what repeats core |
| `BrowserScraper::builder` | method | yes | keep |  | says what it does |
| `BrowserScraper::page` | method | yes | keep |  | says what it does |
| `BrowserScraper::browser` | method | yes | keep |  | says what it does |
| `BrowserScraper::fingerprint` | method | yes | keep |  | returns the core `Fingerprint` in use |
| `BrowserScraper::seed` | method | yes | keep |  | the chain's current seed, which differs from the fingerprint's own `Fingerprint::seed` |
| `BrowserScraper::egress` | method | yes | rename | `proxy_route` | follows `ProxyRoute` |
| `BrowserScraper::sessions` | method | yes | rename | `session_store` | returns core's `SessionStore`; the builder method is already `session_store` |
| `BrowserScraper::close` | method | yes | keep |  | says what it does |
| `BrowserScraper::fetch` | method | yes | keep |  | says what it does |
| `BrowserScraper::solve_challenge` | method | yes | add to core | core `Page::solve_captcha`: wait out a JS interstitial that has no widget, and `SolveOptions::clearance_cookie(name)` as another success check | **copied** name; core presses a widget and checks its response field or a success marker, this also waits out an interstitial and checks for the clearance. Detection and mitigation around it stay here. Decision 3 |
| `BrowserScraper::rotate_egress` | method | yes | rename | `rotate_proxy` | core says proxy |
| `BrowserScraper::rotate_profile` | method | yes | rename | `rotate_fingerprint` | **copied** name; a different way from core's in-place `Page::apply_fingerprint`: it relaunches the browser with a new fingerprint of the same system, in the proxy's locale. A profile in core is a browser profile |
| `BrowserScraper::export_identity` | method | yes | keep |  | says what it does |
| `BrowserScraper::import_identity` | method | yes | keep |  | says what it does |
| `BrowserScraper::save_session` | method | yes | keep |  | builds on core's `SessionStore` |
| `BrowserScraper::load_session` | method | yes | keep |  | builds on core's `SessionStore` |
| `BrowserScraperBuilder` | struct | yes | keep |  | **copied** shape (their `CloudScraper` builder), but it already builds on core's `LaunchOptionsBuilder` (`launch_options`); its members drop what repeats core |
| `BrowserScraperBuilder::launch_options` | method | yes | keep |  | says what it does |
| `BrowserScraperBuilder::with_geo_proxies` | method | yes | rename | `geo_proxies` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `BrowserScraperBuilder::proxy_strategy` | method | yes | keep |  | says what it does |
| `BrowserScraperBuilder::with_max_challenge_attempts` | method | yes | rename | `max_challenge_attempts` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `BrowserScraperBuilder::mitigation` | method | yes | keep |  | says what it does |
| `BrowserScraperBuilder::with_state_store` | method | yes | rename | `ledger` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names; follows `Ledger` |
| `BrowserScraperBuilder::with_event_sink` | method | yes | rename | `event_sink` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names; `HttpOptions::event_sink` already says it so |
| `BrowserScraperBuilder::geo_resolver` | method | yes | keep |  | says what it does |
| `BrowserScraperBuilder::per_site_rate` | method | yes | rename | `rate_limit` | core `RateLimiter::new(max_calls, window)`; `HttpOptions::pacing` is the same setting under a third name |
| `BrowserScraperBuilder::seed` | method | yes | keep |  | not core's `Fingerprint::seed`: it seeds a chain, each rotation deriving the next seed, so a whole run repeats |
| `BrowserScraperBuilder::fingerprint` | method | yes | build on core | read the first fingerprint from the `LaunchOptionsBuilder` given to `launch_options` (core's `fingerprint` option) | the first link of the rotation chain is the launch fingerprint core already takes; one place to set it. Decision 2 |
| `BrowserScraperBuilder::egress_switch` | method | yes | rename | `proxy_switch` | follows `ProxySwitch` |
| `BrowserScraperBuilder::launcher` | method | yes | build on core | a launch closure, as `BrowserPool::with_launcher` | follows `BrowserLauncher` |
| `BrowserScraperBuilder::session_store` | method | yes | keep |  | says what it does |
| `BrowserScraperBuilder::solve_timeout` | method | yes | remove | core `SolveOptions::verify_timeout` | same strategy: how long to wait for a solve to show its result |
| `BrowserScraperBuilder::clock` | method | yes | keep |  | says what it does |
| `BrowserScraperBuilder::build` | method | yes | keep |  | says what it does |
| `ChromeLauncher` | struct |  | remove | core `Browser::launch` | same strategy: it calls `Browser::launch` with the launch options, the default every launch already uses |

### `scrape::runtime::egress_switch`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `ContextProxySwitch` | struct |  | remove | core `Browser::new_context_with(ContextOptions::proxy)` | same strategy: it opens a context with `ContextOptions::proxy`, exactly core's path |
| `ContextProxySwitch::new` | method |  | remove |  | goes with its type |
| `EgressSwitch` | trait |  | rename | `ProxySwitch` | a seam for two ways to change proxy: a new context (core's `ContextOptions::proxy`) and the MITM proxy's upstream swap (`MitmProxy::swap_proxy`), which core cannot do without the MITM proxy |
| `EgressSwitch::switch` | method |  | keep |  | says what it does |

### `scrape::runtime::json_store`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `JsonStateStore` | struct | yes | rename | `JsonLedger` | follows `Ledger` |
| `JsonStateStore::open` | method | yes | keep |  | says what it does |
| `JsonStateStore::path` | method | yes | keep |  | says what it does |

### `scrape::runtime::tracing_sink`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `TracingEventSink` | struct | yes | keep |  | builds on `tracing`, as core logs |

### `scrape::runtime::browser_scraper::builder`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `Clock` | type alias |  | keep |  | a test seam for time; core's `core::allure::Clock` belongs to the report |

## mimic (`seleniumbase_rs_stealth::mimic`)

### `mimic::client`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `impersonation_client` | function | yes | rename | `wire_client` | "impersonate" is curl_cffi's word (`impersonate="chrome"`), and core dropped its `impersonate` feature in 0.3.0 |
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
| `CustomProfileBuilder::user_agent` | method |  | build on core | core `Fingerprint` (the builder takes one) | the custom input stays, set where core keeps it |
| `CustomProfileBuilder::accept_language` | method |  | build on core | core `Fingerprint` | same |
| `CustomProfileBuilder::client_hints` | method |  | build on core | core `Fingerprint` | same |
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
| `ImpersonationProfile` | struct | yes | build on core | `WireFormat`, built on core `Fingerprint` (User-Agent, Accept-Language and client hints come from it) | **copied** (their `BrowserProfile`, `random()`); the TLS, HTTP/2 and header-order part is new ground, the header values repeat core's `Fingerprint` |
| `ImpersonationProfile::measured` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::for_system` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::from_measured` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::random` | method | yes | keep |  | already builds on core's `Fingerprint::randomized_with_major`; the seed also picks the measured wire entry, which core does not have |
| `ImpersonationProfile::from_fingerprint` | method | yes | keep |  | builds on core's `Fingerprint` |
| `ImpersonationProfile::with_locale` | method | yes | remove | core `Fingerprint`'s locale | same strategy: it sets the Accept-Language a core `Fingerprint`'s locale already sets |
| `ImpersonationProfile::custom` | method | yes | rename | `builder` | builders start from `builder()` (core `LaunchOptions::builder`) |
| `ImpersonationProfile::validate` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::source` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::kind` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::os` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::major` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::user_agent` | method | yes | remove | core `Fingerprint` | same strategy: it returns the fingerprint's User-Agent |
| `ImpersonationProfile::accept_language` | method | yes | remove | core `Fingerprint` | same strategy: it returns the fingerprint's languages |
| `ImpersonationProfile::tls_spec` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::http2_spec` | method | yes | rename | `h2_spec` | the type is `H2Spec` |
| `ImpersonationProfile::header_spec` | method | yes | keep |  | says what it does |
| `ImpersonationProfile::identity` | method | yes | rename | `client_headers` | follows codec `ClientHeaders` |
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
| `HttpAttempt` | struct |  | rename | `HttpAttempt` (source name) | make it the real name: the source calls it `Attempt` and `lib.rs` renames it; one spelling per item |
| `HttpAttempt.number` | field |  | keep |  | plain data name |
| `HttpAttempt.previous_challenges` | field |  | keep |  | plain data name |
| `HttpAttempt.can_rotate_egress` | field |  | keep |  | plain data name |
| `HttpAttempt.can_escalate` | field |  | keep |  | plain data name |
| `HttpFetched` | struct |  | rename | `HttpFetch` | matches `BrowserFetch` and `StealthFetch`; the source calls it `Fetched` |
| `HttpFetched.reply` | field |  | keep |  | plain data name |
| `HttpFetched.verdict` | field |  | keep |  | plain data name |
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
| `HttpError::Identity` | enum variant |  | keep |  | names the case |
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
| `HttpOptions::event_sink` | method |  | keep |  | says what it does |
| `HttpOptions::trust` | method |  | keep |  | says what it does |
| `HttpOptions::resolve` | method |  | keep |  | says what it does |
| `HttpOptions::bypass` | method |  | keep |  | the same list as core's `ContextRoute.bypass`; keep the name |
| `HttpOptions::browser_proxy_exits_directly` | method |  | keep |  | says what it does |
| `HttpOptions::installed_chrome_major` | method |  | keep |  | says what it does |
| `HttpOptions::clock` | method |  | keep |  | says what it does |
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
| `HttpReply::detect` | method |  | keep |  | says what it does |
| `HttpRequest` | struct |  | keep |  | the `Http` prefix keeps it apart from core's `sb_cdp::intercept::Request` |
| `HttpRequest::shape` | method |  | keep |  | says what it does |
| `HttpRequest::header` | method |  | keep |  | says what it does |
| `HttpRequest::body` | method |  | keep |  | says what it does |
| `HttpRequest::json` | method |  | keep |  | says what it does |
| `HttpRequest::form` | method |  | keep |  | says what it does |
| `HttpRequest::timeout` | method |  | keep |  | says what it does |
| `HttpRequest::send` | method |  | keep |  | says what it does |
| `HttpSession` | struct |  | keep |  | a live HTTP session, as core's `BrowserSession` is a live WebDriver one |
| `HttpSession::from_identity` | method |  | keep |  | says what it does |
| `HttpSession::request` | method |  | keep |  | says what it does |
| `HttpSession::get` | method |  | keep |  | says what it does |
| `HttpSession::profile` | method |  | rename | `wire_format` | follows `WireFormat` |
| `HttpSession::egress` | method |  | rename | `proxy_route` | follows `ProxyRoute` |
| `HttpSession::cookie_count` | method |  | keep |  | says what it does |
| `HttpSession::assess` | method |  | keep |  | says what it does |
| `HttpSession::fetch` | method |  | keep |  | says what it does |
| `HttpSession::preflight` | method |  | keep |  | says what it does |
| `HttpSession::export_identity` | method |  | keep |  | says what it does |
| `HttpSession::from_page` | method |  | keep |  | says what it does |
| `HttpSession::sync_cookies_back` | method |  | keep |  | says what it does |
| `Method` | re-export |  | keep |  | re-export of `wreq::Method`, so callers import no wreq (the pattern of core's `drivers` and `json` re-exports) |
| `ReplyProtocol` | enum |  | remove | `WireProtocol` | same strategy: the same two values (HTTP/1.1, HTTP/2) as `WireProtocol` in this crate |
| `ReplyProtocol::Http2` | enum variant |  | remove |  | goes with its type |
| `ReplyProtocol::Http1` | enum variant |  | remove |  | goes with its type |
| `HttpRules` | struct |  | rename | `HttpRules` (source name) | make it the real name: the source calls it `Rules` and `lib.rs` renames it; one spelling per item |
| `HttpRules.mitigation` | field |  | keep |  | plain data name |
| `HttpRules.session` | field |  | rename | `escalation` | its type becomes `EscalationPolicy` |
| `HttpVerdict` | struct |  | rename | `HttpVerdict` (source name) | make it the real name: the source calls it `Verdict` and `lib.rs` renames it; one spelling per item |
| `HttpVerdict.detection` | field |  | keep |  | plain data name |
| `HttpVerdict.action` | field |  | keep |  | plain data name |
| `HttpVerdict.transition` | field |  | rename | `escalation` | follows `Escalation` |
| `HttpVerdict::is_clear` | method |  | keep |  | says what it does |

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
| `MitmOptions` | struct |  | keep |  | options named after their owner, as core's `PoolOptions` |
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
| `MitmProxy` | struct | yes | keep |  | **copied** shape (their `TlsSpoofingProxy`), but covers ground core does not; `configure_launch` already builds on core's `LaunchOptionsBuilder` |
| `MitmProxy::start` | method | yes | keep |  | says what it does |
| `MitmProxy::prime_url` | method | yes | keep |  | says what it does |
| `MitmProxy::port` | method | yes | keep |  | says what it does |
| `MitmProxy::local_addr` | method | yes | keep |  | says what it does |
| `MitmProxy::proxy_spec` | method | yes | rename | `proxy_url` | it is the proxy URL with its password, as a `Secret` |
| `MitmProxy::browser_proxy` | method | yes | keep |  | says what it does |
| `MitmProxy::chrome_args` | method | yes | rename | `launch_args` | core's `evasion::evasions::launch_args` returns the same kind of list |
| `MitmProxy::configure_launch` | method | yes | keep |  | says what it does |
| `MitmProxy::stats` | method | yes | keep |  | says what it does |
| `MitmProxy::egress` | method | yes | rename | `proxy_route` | follows `ProxyRoute` |
| `MitmProxy::swap_upstream` | method | yes | rename | `swap_proxy` | it takes the proxy route; core says proxy |
| `MitmProxy::swap_profile` | method | yes | rename | `swap_wire_format` | follows `WireFormat` |
| `MitmProxy::shutdown` | method | yes | keep |  | says what it does |
| `MitmStats` | struct |  | keep |  | as core's `PoolStats` |
| `MitmStats.connections` | field |  | keep |  | plain data name |
| `MitmStats.tunnels` | field |  | keep |  | plain data name |
| `MitmStats.unauthorized` | field |  | keep |  | plain data name |
| `MitmStats.refused` | field |  | keep |  | plain data name |
| `MitmStats.forwarded` | field |  | keep |  | plain data name |
| `MitmStats.upstream_errors` | field |  | keep |  | plain data name |
| `MitmStats.websockets` | field |  | keep |  | plain data name |
| `MitmStats.primed` | field |  | keep |  | plain data name |

### `mimic::stealth_session`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `StealthError` | enum |  | keep |  | follows `StealthSession` |
| `StealthError::Scrape` | enum variant |  | keep |  | names the case |
| `StealthError::Http` | enum variant |  | keep |  | names the case |
| `StealthError::Mitm` | enum variant |  | keep |  | names the case |
| `StealthError::Impersonate` | enum variant |  | rename | `WireFormat` | follows `WireFormat` |
| `StealthFetch` | struct |  | keep |  | matches `BrowserFetch` and `HttpFetch` |
| `StealthFetch.url` | field |  | keep |  | plain data name |
| `StealthFetch.status` | field |  | keep |  | plain data name |
| `StealthFetch.headers` | field |  | keep |  | plain data name |
| `StealthFetch.body` | field |  | keep |  | plain data name |
| `StealthFetch.detection` | field |  | keep |  | plain data name |
| `StealthFetch.mode` | field |  | rename | `fetch_kind` | follows `FetchKind` |
| `StealthFetch.attempts` | field |  | keep |  | plain data name |
| `StealthFetch.solved` | field |  | keep |  | plain data name |
| `StealthFetch.egress` | field |  | rename | `proxy_route` | follows `ProxyRoute` |
| `StealthSession` | struct | yes | keep |  | **copied** name, but covers ground core does not (moving between HTTP and the browser); it builds on core's `Page` |
| `StealthSession::builder` | method | yes | keep |  | says what it does |
| `StealthSession::mode` | method | yes | rename | `fetch_kind` | follows `FetchKind` |
| `StealthSession::identity` | method | yes | keep |  | says what it does |
| `StealthSession::egress` | method | yes | rename | `proxy_route` | follows `ProxyRoute` |
| `StealthSession::seed` | method | yes | keep |  | says what it does |
| `StealthSession::http` | method | yes | rename | `http_session` | it returns the `HttpSession`; `http` alone could be a client or a protocol |
| `StealthSession::page` | method | yes | keep |  | says what it does |
| `StealthSession::close` | method | yes | keep |  | says what it does |
| `StealthSession::fetch` | method | yes | keep |  | says what it does |
| `StealthSession::rotate_profile` | method | yes | rename | `rotate_fingerprint` | follows `RotateFingerprint` |
| `StealthSession::rotate_egress` | method | yes | keep |  | says what it does |
| `StealthSession::export_identity` | method | yes | keep |  | says what it does |
| `StealthSession::import_identity` | method | yes | keep |  | says what it does |
| `StealthSession::escalate` | method | yes | keep |  | says what it does |
| `StealthSession::demote` | method | yes | keep |  | says what it does |
| `StealthSessionBuilder` | struct | yes | keep |  | builds on core's `LaunchOptionsBuilder` (`launch_options`); its members drop what repeats core |
| `StealthSessionBuilder::launch_options` | method | yes | keep |  | says what it does |
| `StealthSessionBuilder::launcher` | method | yes | build on core | a launch closure, as `BrowserPool::with_launcher` | follows `BrowserLauncher` |
| `StealthSessionBuilder::with_geo_proxies` | method | yes | rename | `geo_proxies` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `StealthSessionBuilder::proxy_strategy` | method | yes | keep |  | says what it does |
| `StealthSessionBuilder::with_max_challenge_attempts` | method | yes | rename | `max_challenge_attempts` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `StealthSessionBuilder::mitigation` | method | yes | keep |  | says what it does |
| `StealthSessionBuilder::policy` | method | yes | rename | `escalation_policy` | `policy` alone could be the `MitigationPolicy` or core's `StealthPolicy` |
| `StealthSessionBuilder::with_state_store` | method | yes | rename | `ledger` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names; follows `Ledger` |
| `StealthSessionBuilder::with_event_sink` | method | yes | rename | `event_sink` | core builders use plain setter names (`LaunchOptionsBuilder`, `PoolOptionsBuilder`); this builder mixes `with_*` and plain names |
| `StealthSessionBuilder::geo_resolver` | method | yes | keep |  | says what it does |
| `StealthSessionBuilder::per_site_rate` | method | yes | rename | `rate_limit` | core `RateLimiter`; one name with `HttpOptions::pacing` |
| `StealthSessionBuilder::seed` | method | yes | keep |  | the same rotation chain as `BrowserScraperBuilder::seed` |
| `StealthSessionBuilder::fingerprint` | method | yes | build on core | read it from the `LaunchOptionsBuilder` given to `launch_options` | same as `BrowserScraperBuilder::fingerprint`. Decision 2 |
| `StealthSessionBuilder::restore` | method | yes | rename | `identity` | it takes a saved `Identity`; the session's own method is `import_identity` |
| `StealthSessionBuilder::http_options` | method | yes | rename | `http_session_options` | follows `HttpSessionOptions` |
| `StealthSessionBuilder::mitm_options` | method | yes | keep |  | says what it does |
| `StealthSessionBuilder::solve_timeout` | method | yes | remove | core `SolveOptions::verify_timeout` | same strategy: how long to wait for a solve to show its result |
| `StealthSessionBuilder::clock` | method | yes | keep |  | says what it does |
| `StealthSessionBuilder::build` | method | yes | keep |  | says what it does |

### `mimic::tls_server`

| Item | Kind | Copied | Verdict | Proposed | Why |
|---|---|---|---|---|---|
| `CaOptions` | struct |  | keep |  | options named after their owner |
| `CaOptions.ca_validity` | field |  | keep |  | plain data name |
| `CaOptions.leaf_validity` | field |  | keep |  | plain data name |
| `CaOptions.leaf_renew_margin` | field |  | keep |  | plain data name |
| `CaOptions.leaf_capacity` | field |  | keep |  | plain data name |
| `EphemeralCa` | struct |  | keep |  | says what it is (a certificate authority that lives for one run) |
| `EphemeralCa::generate` | method |  | keep |  | says what it does |
| `EphemeralCa::generate_at` | method |  | keep |  | says what it does |
| `EphemeralCa::spki_pins` | method |  | keep |  | says what it does |
| `EphemeralCa::chrome_spki_arg` | method |  | rename | `spki_launch_arg` | follows `launch_args` |
| `EphemeralCa::leaf_spki_pin` | method |  | keep |  | says what it does |
| `EphemeralCa::certificate` | method |  | keep |  | says what it does |
| `EphemeralCa::options` | method |  | keep |  | says what it does |
| `EphemeralCa::not_after_unix` | method |  | keep |  | says what it does |
| `EphemeralCa::cached_leaves` | method |  | keep |  | says what it does |
| `EphemeralCa::leaf_for` | method |  | keep |  | says what it does |
| `Leaf` | struct |  | rename | `LeafCert` | `Leaf` alone does not say it is a certificate |
| `Leaf::certificate` | method |  | keep |  | says what it does |
| `Leaf::issuer` | method |  | keep |  | says what it does |
| `Leaf::host` | method |  | keep |  | says what it does |
| `Leaf::not_before_unix` | method |  | keep |  | says what it does |
| `Leaf::not_after_unix` | method |  | keep |  | says what it does |
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
| `Clock` | type alias |  | remove | scrape's `Clock` | same strategy: the same alias defined twice in this repository |

