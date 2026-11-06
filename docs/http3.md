# HTTP/3 (experimental, off by default)

> [!IMPORTANT]
> This page is not on the docs site. Its user-facing copy is
> [HTTP/3](https://mustcodeal-seleniumbase-rs.docs7.io/stealth/advanced/http3) on the core site
> (`docs/stealth/advanced/http3.md` in the seleniumbase-rs repository). When the measurements
> below change (a phantom upgrade), update both.

The wire crate sends HTTP/1.1 and HTTP/2 with Chrome's TLS and HTTP/2 settings through wreq and
`BoringSSL`. HTTP/3 needs a QUIC stack. The one used here is
[phantom-http](https://crates.io/crates/phantom-http) `=0.6.0`, which brings its own `BoringSSL`.
The two `BoringSSL` builds cannot be linked into one binary: on macOS arm64 the linker crashes, and
they export 6,047 of the same symbol names (`docs/design/http3.md`). So HTTP/3 runs in a separate
program, `sbrs-h3d`, and the wire crate talks to it over stdin and stdout.

## Using it

```bash
cargo install --path crates/h3d            # builds sbrs-h3d (phantom and its BoringSSL)
```

```toml
seleniumbase-rs-stealth = { git = "https://github.com/MustCodeAl/seleniumbase-rs-stealth", features = ["http3"] }
```

```rust,ignore
use seleniumbase_rs_stealth::wire::http3::{Http3Client, Http3Options, Http3Request};

let mut client = Http3Client::start(&Http3Options::new())?; // sbrs-h3d on the PATH, or SBRS_H3D
let response = client.send(Http3Request::get("https://example.com/")).await?;
assert_eq!(response.protocol(), "h3");
```

- Every request goes over HTTP/3 only; nothing falls back to HTTP/2. To fall back, send the
  request again through `HttpSession`.
- A missing helper is the error `Http3Error::HelperMissing`, which says how to install it; a
  helper that ended is `Http3Error::HelperEnded`. Nothing panics.
- The helper uses phantom's `chrome-156-macos` profile unless `Http3Options::profile` names
  another. Its headers come from that profile's navigation template; a header you add keeps the
  template's place when the template has it.
- `Alt-Svc` is not learned: HTTP/3 is used only when you ask for it. With UDP blocked, a request
  fails at its time limit (30 seconds unless `Http3Request::timeout` says otherwise).
- An HTTP proxy cannot carry HTTP/3 (a CONNECT tunnel carries TCP), and the helper takes no
  proxy. Requests go straight from this machine.

## What matches Chrome, and what does not

These are the results of the HTTP/3 spike (`spike/http3`, `docs/design/http3.md`). It compared
phantom's `chrome-156-macos` profile with headless Chrome 156.0.8078.76 on macOS, both against a
local server that logged what it received. Nothing else was compared.

**Measured the same as Chrome 156:**

- the TLS ClientHello inside QUIC: cipher suites, the extension set, groups, key-share group ids and
  key lengths, signature algorithms, ALPN `h3`, ALPS, the ECH header fields, and the length and
  first 32 bytes of extension `0xca34`;
- the QUIC transport parameters: the 11 fixed values, and the shape of the other two;
- the HTTP/3 SETTINGS: ids, values and order;
- the request header names and the pseudo-header order;
- QUIC version 1.

**Measured different from Chrome** (a server can see each of these in the first flight):

- the Initial packets: 1200 bytes, not 1250; a 20-byte destination connection id, not 8 bytes;
  packet number 0, not 1; no "chaos protection" layout;
- the control stream: no GREASE frame and no `PRIORITY_UPDATE`;
- the QPACK streams: not seen from phantom when the server's table is 0, and different use of the
  dynamic table on the first request;
- Huffman coding of short header values.

**Not measured, so not known to match:** acknowledgements and their timing, flow-control updates,
loss recovery and probe timers, congestion control, pacing, path MTU discovery, short-header packet
sizes, key updates, connection close, 0-RTT and session resumption, Retry and tokens, connection
migration, QUIC v2, the spin bit, later requests on a warm connection, long-lived connections,
any real network or site, and Chrome on Windows or Linux.

In short, this is "Chrome 156 TLS, transport parameters, SETTINGS and headers over QUIC", not
"Chrome's QUIC". The wire crate's own HTTP/2 entry is Chrome 155. HTTP/3 uses phantom's 156 profile.
So a session that uses both presents two majors until the two entries are measured at one version
(step 4 of the integration plan in `docs/design/http3.md`).

## Version pin

`crates/h3d` pins `phantom-http = "=0.6.0"` and commits its own `Cargo.lock`. phantom is pre-1.0 and
changes fast. An upgrade is a pull request that runs the spike's capture again and updates the lists
above.

## Tests

`crates/h3d/tests/local_h3.rs` starts a local HTTP/3 server (quinn and h3 on 127.0.0.1, a fresh
self-signed certificate) and drives `sbrs-h3d` against it:

- a GET over `h3`, with the profile's Chrome `User-Agent`;
- a POST body and a caller header;
- a server that never answers, which gives an error within the time limit;
- a line that is not a request, which gets an answer while the next request still works.

The wire crate's tests check the line format against a stand-in script. Run them with:

```bash
cd crates/h3d && cargo test
cargo test -p seleniumbase-rs-wire --features http3 --lib http3
```

The root workspace never builds `crates/h3d`, so the usual gate does not build phantom.
