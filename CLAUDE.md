# seleniumbase-rs-stealth: knowledge map

A short map of the repository for a coding assistant. Keep it correct; fix a line here when the
code stops matching it. `README.md` is the overview for people.

## What this is

The experimental stealth layer of [seleniumbase-rs](https://github.com/MustCodeAl/seleniumbase-rs)
(the core), which it depends on by git and uses only through its public API. Nothing is
published to crates.io.

| Package | Path | Role |
|---|---|---|
| `seleniumbase-rs-stealth` | `.` | The facade: re-exports the core and the crates below. Features `codec`, `scrape` (default), `wire`, `http3`. |
| `seleniumbase-rs-codec` | `crates/codec` | Wire formats with no I/O, and the measured browser table (`src/measured/*.json`). |
| `seleniumbase-rs-scrape` | `crates/scrape` | Challenge detection and mitigation, egress, the cookie jar, the ledger, `Identity`, and `BrowserScraper` over the core's pure CDP engine. No BoringSSL. |
| `seleniumbase-rs-wire` | `crates/wire` | Browser-identical TLS and HTTP/2 through wreq and BoringSSL: `WireFormat`, `HttpSession`, `MitmProxy`, `StealthSession`; HTTP/3 through `sbrs-h3d` (feature `http3`). Needs cmake. |
| `sbrs-h3d` | `crates/h3d` | The HTTP/3 helper program, a workspace of its own that the root workspace never builds. |

## Scope

The crates are APIs "to control advanced stealthy things that made sense with seleniumbase and
bridge the features that were missing from seleniumbase that were in stealthscraper-rs", in the
maintainers' words.

- They cover only what the core lacks. Do not add a second copy of something the core does.
- They are built on the core's types and names: `Fingerprint`, `StealthPolicy`, `ProxyList`,
  `SolveOptions`, `SessionStore` and so on. Take a core type where one fits, and the core's word
  where a stealth type names the same thing.
- Another algorithm or strategy for a job the core also does, one that can give different
  results, is kept as "another way of doing it", not removed.

## Docs

- The user docs of the stealth crates live on the core site, under `docs/stealth` (and the
  stealthy `docs/scraping` pages) in the core repository: the Docs7 site builds only the core's
  `docs/` and `docs/docs.json`. A change users should read goes to those pages, in a core pull
  request.
- This repository's `docs/` holds design and contributor notes (`docs/design/`,
  `docs/contributing/`). A page there is not on the site. `docs/tutorial.md`, `docs/examples.md`
  and `docs/http3.md` are older long-form copies; each says which site page is the user-facing
  one.

## Build and check

```bash
cargo fmt --all -- --check
cargo clippy -p seleniumbase-rs-stealth --all-targets -- -D warnings
cargo test -p seleniumbase-rs-stealth
```

- That default facade builds no BoringSSL. `--features wire` or `--workspace` builds it (cmake, a
  C/C++ toolchain, about 1 GB of target on the first build);
  `docs/contributing/boringssl.md` and `scripts/boringssl-prebuilt.sh` build it once and link it.
- Tests that start a real Chrome are the `*_chrome.rs` files, `#[ignore]`d: run them one binary
  at a time with `-- --ignored --test-threads=1`.
- To build against a local core checkout, patch the core's git URL in a `.cargo/config.toml` you
  do not commit (`README.md`, "Building this workspace").
