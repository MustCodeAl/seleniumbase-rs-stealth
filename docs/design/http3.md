# HTTP/3 impersonation

Status: spike finished 2026-11-02; this note was corrected the same day after a read-only check of its
statements against the captures. The measurements were made on one machine (macOS arm64,
Darwin 27.0.0, Rust 1.99.1, Apple clang 21.0.0) against a local server on 127.0.0.1. Code and
scripts: `spike/http3/` (see its `README.md` to run it again). The measured values quoted here are in
`spike/http3/results/`; `results/claims-14-captures.txt` is computed by `scripts/claims.sh` from the raw
logs of 14 captures (the gitignored `out/`) and backs every "equal in all runs" statement below. What was
only read (phantom's documentation and source, crates.io, Chromium pages) or was not measured says so where
it is mentioned.

## Question

The owner wants HTTP/3 impersonation. wreq has no HTTP/3 transport, so an earlier read-only
evaluation picked phantom (crate `phantom-http`, imported as `phantom`,
github.com/bywayhq/phantom) to sit beside wreq for HTTP/3 only, behind an optional feature that is
off by default. This spike checks the three things that pick rested on:

- (a) does phantom link next to the stealth crates (wreq over btls) in one binary, without
  BoringSSL symbol clashes, and do both TLS stacks work in one process;
- (b) does phantom really fetch a page over HTTP/3 from a local server;
- (c) how close is phantom's first flight to headless Chrome's, against the same server: QUIC
  transport parameters, HTTP/3 SETTINGS, QPACK, pseudo-header order.

Short answers:

| | Result |
|---|---|
| (a) one binary with both stacks | **No.** The link fails on macOS arm64: 1,332 duplicate symbols (or a crash of Apple's linker with the default flags). Both BoringSSL builds export the same unprefixed names. No binary exists, so "both stacks work after the other has run" is **not measured**. Each stack works alone, in its own process. |
| (b) HTTP/3 from a local server | **Yes.** phantom `=0.6.0` fetched `https://localhost:4433/h3fetch` over HTTP/3; the server saw a QUIC v1 handshake with ALPN `h3` and the request as an HTTP/3 HEADERS frame. |
| (c) wire identity against Chrome 156 | **Partly.** In all 14 captures (7 of each client), on the fields the tools compare (listed below), these are equal to Chrome's: the cipher suites and the 12 extensions of the TLS ClientHello inside QUIC, the 11 fixed QUIC transport parameter values (the other two parameters are `version_information` and one GREASE parameter, random in both), the HTTP/3 SETTINGS ids, values and order, and the request header field names and their order (pseudo-header order `:method :authority :scheme :path`). The QUIC Initial packets (size, connection id length, packet number, frame layout), the HTTP/3 control stream and the QPACK streams are **not**. |

Recommendation (details below): do not link phantom into the stealth binaries. If HTTP/3 is wanted,
run phantom in a separate helper process behind an off-by-default feature, measure our own
Chrome HTTP/3 entry with the tools of this spike, and say plainly that the QUIC packet layout is
not Chrome's.

## Setup

| What | Version or value |
|---|---|
| phantom | `phantom-http =0.6.0` (crates.io: the newest version, published 2026-11-01 20:43 UTC), feature `diagnostics` for the qlog. crates.io shows the crate first published on 2026-10-17 (0.1.0) and seven releases in all (0.2.0 on 10-19, 0.3.0 on 10-21, 0.4.0 on 10-25, 0.5.0 on 10-26, 0.5.1 on 10-27, 0.6.0 on 11-01). Pulls `phantom-btls 0.5.6-phantom.19`, `phantom-btls-sys 0.5.6-phantom.3`, `phantom-quinn 0.11.12-phantom.7`, `phantom-quinn-proto 0.11.18-phantom.7`, `phantom-h3 0.0.8-phantom.20`. |
| phantom profile | `chrome-156-macos` for the capture runs, `chrome-156-windows` in `clash` (what the macOS profile is: see the paragraph under this table). |
| stealth side | `wreq =6.0.0-rc.31` over `btls 0.5.6` / `btls-sys 0.5.6`, by path through `crates/mimic` and `crates/codec`; measured profile Chrome 155 macOS. |
| Chrome | Google Chrome 156.0.8078.76, macOS, `--headless`. |
| server | `h3srv`: **upstream** `quinn 0.11.12` + `rustls 0.23.46` (ring), not phantom's forks. It parses the HTTP/3 control stream, the QPACK streams and the request itself, so SETTINGS, their order and the field lines are what arrived. TCP side: TLS with ALPN `http/1.1` only. |
| relay | `relay`: a UDP relay in front of the server that decrypts the client's QUIC Initial packets (RFC 9001 section 5: keys from the Destination Connection ID) and prints the TLS ClientHello with the transport parameters in wire order. Its parser passes the RFC 9001 appendix A.2 test vector (15 unit tests, all pass). |
| certificate | throwaway P-256, SAN `localhost` and `127.0.0.1`, in `spike/http3/certs/` (gitignored). |

About `chrome-156-macos`. phantom's guide `crates/phantom/src/docs/browser-profiles.md` (the same sentences on
`main` and at tag `v0.6.0`, fetched 2026-11-02) says `chrome-156-linux` "provides Chrome 156.0.8078.47 TLS,
HTTP/2, HTTP/3, client hints and request templates from headed Ubuntu 24.04.4 captures on WSL2/WSLg x86_64",
and that `chrome-156-macos` provides Chrome 156.0.8078.48 "startup settings, client hints and request templates"
for macOS 26.6.2 arm64. So the guide ties the TLS, HTTP/2 and HTTP/3 layers to a Linux capture, and describes the
macOS profile as startup settings, client hints and request templates. I did not check where `chrome-156-macos`
takes its TLS, QUIC and HTTP/3 settings from. What the spike measured is what that profile puts on the wire,
against Chrome 156 on this macOS machine.

Chrome flags, from Chromium's page <https://www.chromium.org/quic/playing-with-quic/>:
`--enable-quic`, `--origin-to-force-quic-on=localhost:4433`, and
`--ignore-certificate-errors-spki-list=<base64 SHA-256 of the certificate's SPKI>` (the page says
this is the way to trust one custom certificate; a custom CA cannot be trusted with it). To keep
every packet on this machine the run also passes `--host-resolver-rules="MAP * 127.0.0.1"` (rule
grammar `MAP <pattern> <host>` / `EXCLUDE <pattern>` from Chromium's `net/base/host_mapping_rules.h`;
peter.sh's switch list gives only a one-line description for this switch), plus
`--headless --no-first-run --no-default-browser-check --disable-background-networking
--disable-component-update --dump-dom` and a fresh `--user-data-dir` (all in peter.sh's list of Chromium
switches), and `--disable-sync` (not in that list; I did not confirm it, Chrome did not object, and every host
name maps to loopback anyway). Chrome fetched the page with `--dump-dom` and also asked for `/favicon.ico`.

## Results

### (a) phantom next to wreq/btls in one binary

`client/src/bin/clash.rs` depends on `phantom-http =0.6.0` and on the stealth crates by path and
calls both TLS stacks in turn. Cargo accepts the two BoringSSL builds in one dependency graph
(`phantom-btls-sys` declares `links = "phantom_boringssl"`, so it does not collide with btls-sys's `links` key).
Both packages name their library `btls_sys`, so the final link gets two `libbtls_sys-*.rlib`, each
with a full BoringSSL.

| Link variant | Result |
|---|---|
| default (`cargo build --bin clash`) | 599 `ld: warning: duplicate symbol` entries, then `clang: error: unable to execute command: Trace/BPT trap: 5` / `linker command failed due to signal`: Apple's linker **crashes**, no binary. Seen in the first build and again in `scripts/link-experiments.sh`. |
| `-C link-arg=-ld_classic` | `ld: warning: -ld_classic is no longer supported and will be ignored`; same crash. |
| `-C link-dead-code` (no `-dead_strip`) | the linker finishes: **`ld: 1332 duplicate symbols`**, exit 1. |

A duplicate looks like `duplicate symbol '_BIO_meth_free' in: libbtls_sys-2f14b6403f42931e.rlib[134](bio.cc.o)
and libbtls_sys-48fa548f9eb4d2f3.rlib[134](bio.cc.o)`: the same object file from each build. `nm -g -U` on the two
static libraries (`results/symbols-nm.txt`): phantom's `libssl.a` + `libcrypto.a` define 9,644 global
symbols, btls-sys's define 7,339, and **6,047 names are in both**, with no prefix on either
(`_AES_cbc_encrypt`, `_EVP_AEAD_CTX_init`, ...).

Controls: `wreqonly` (stealth crates only) and `h3fetch` (phantom only) each link and run against the
same server. So the failure is having two BoringSSLs in one binary, not either stack.

What is **not** measured: whether each stack still works after the other has run in one process
(there is no such binary), and anything on Linux. A linker that takes the first definition and
reports nothing would give a binary in which both stacks use one of the two BoringSSL builds. The two builds
are not the same (9,644 and 7,339 global symbols, 6,047 in common). I did not identify the BoringSSL revision of
either, so I do not know how they differ. I did not test such a binary and would not trust it.

phantom's own way out is a `prefix-symbols` feature of `phantom-btls-sys` ("add a prefix to all
symbols ... to prevent conflicts with OpenSSL versions that might be linked in the same process").
From its `build/prefix.rs` (read, not run): it lists symbols with `nm`, keeps only those that do not
start with `_`, and renames them with `objcopy --redefine-syms`. On Mach-O every C symbol starts with
`_`, so as written it would select nothing here, and this machine has no `objcopy` on the PATH. I
did not try it. It is a candidate for Linux only.

### (b) an HTTP/3 fetch from a local server

`scripts/serve-and-run.sh phantom-h3 scripts/client-phantom.sh h3` runs `h3fetch <port> <cert.der> h3 --qlog
out/phantom-h3/qlog`, an exact-protocol HTTP/3 request (phantom's docs say an exact request does not fall back; I did
not test that against a blocked path). `h3fetch.rs:169-170` prints one line per request. The run's stdout was not
saved, so this is the line the code prints for this run, written from the code and from the body text `h3srv` sends
(`h3srv.rs:242`); `<ms>` stands for the elapsed time, which was not kept:

```text
#1 profile=chrome-156-macos asked=Exact(Http3) got=Http3 status=200 OK body: proto=h3 alpn=h3 sni=localhost stream=0 (<ms> ms)
```

The client's word is not the only proof. `h3srv` logged, independently:

- `quic_connection`: `alpn=h3 sni=localhost` (a QUIC handshake over UDP, not TCP);
- the relay decrypted QUIC version `0x00000001` Initial packets from the client;
- a client-opened HTTP/3 control stream with SETTINGS, and the request as a HEADERS frame on
  bidirectional stream 0, QPACK-encoded (`:method GET`, `:authority localhost:4433`, ...);
- the response body was written by the server on that stream and names the transport it saw.

One thing to know: pointing `--qlog` at a directory that does not exist makes phantom fail with a
bare `HTTP/3 request failed` (the cause, `NotFound`, is only in the error's source chain). That was
my first failure; `scripts/client-phantom.sh` now creates the directory.

### Alt-Svc and blocked UDP

`results/altsvc-and-blocked-udp.txt`. A negotiated (not exact) phantom client with
`ClientBuilder::alt_svc(16)` against a server that sends `Alt-Svc: h3=":4433"`:

- UDP open: request 1 went over HTTP/1.1 (TCP) and learned the alternative; requests 2 and 3 went
  over HTTP/3 (a new QUIC connection, then the same connection on stream 4).
- UDP silently dropped (the relay forwards to a port nobody serves): request 1 over TCP, then
  requests 2 and 3 each ran until the **harness's own 15 s total deadline** (`RequestTimeouts::new().total(15 s)`,
  `h3fetch.rs:154`) and failed with `request total deadline elapsed` (timeline: `ConnectionAcquisition` failed,
  `QuicHandshake` interrupted at 15.0 s). That is all that was observed. phantom's own QUIC handshake limit, whether it
  evicts the learned alternative, and whether anything falls back to HTTP/1.1 or HTTP/2 were **not observed**: the
  harness deadline ended both requests first. phantom's docs (read earlier, not re-read for this correction) say the
  default `AltSvcPolicy::sequential` "uses the alternative alone; setup failure is terminal and evicts it", and that
  an alternative's setup failure "never falls back implicitly to H1 or H2"; those are the docs' words, not seen here.
  `AltSvcPolicy::race` ("races alternative setup against delayed origin setup") exists and is **not tested**.

## Transport parameters and SETTINGS: phantom against headless Chrome

Method: the same server and relay, one client at a time. Single captures are in
`results/chrome-156-single.txt` and `results/phantom-h3-single.txt`; because order and GREASE are random
by design, `scripts/samples.sh` ran **5 connections of each client** (`results/samples-5.txt`). With one
capture each against a server whose QPACK table is allowed (`results/*-qpack4096.txt`) that makes **14 captures,
7 of each client**, which `scripts/claims.sh` summarises in `results/claims-14-captures.txt` (the "claims file"
below). Statements that name "runs 1 to 5" come from `results/samples-5.txt`.
Chrome's User-Agent in these runs says `HeadlessChrome/156.0.0.0`; phantom's says `Chrome/156.0.0.0`.

### QUIC transport parameters (RFC 9000 section 18, in the TLS `quic_transport_parameters` extension 0x39)

The same 13 parameters in all 14 captures; 11 of them have one fixed value, the same in all 14 captures and for
both clients (claims file, section 2):

| Id | Name | Chrome 156 | phantom `chrome-156-macos` | Same |
|---|---|---|---|---|
| 0x01 | max_idle_timeout | 30000 | 30000 | yes |
| 0x03 | max_udp_payload_size | 1472 | 1472 | yes |
| 0x04 | initial_max_data | 15728640 | 15728640 | yes |
| 0x05 | initial_max_stream_data_bidi_local | 6291456 | 6291456 | yes |
| 0x06 | initial_max_stream_data_bidi_remote | 6291456 | 6291456 | yes |
| 0x07 | initial_max_stream_data_uni | 6291456 | 6291456 | yes |
| 0x08 | initial_max_streams_bidi | 100 | 100 | yes |
| 0x09 | initial_max_streams_uni | 103 | 103 | yes |
| 0x0f | initial_source_connection_id | empty (length 0) | empty (length 0) | yes |
| 0x20 | max_datagram_frame_size | 65536 | 65536 | yes |
| 0x3128 | (no name in the tool) | `4f524947` (ASCII "ORIG") | `4f524947` | yes |
| 0x11 | version_information | chosen `00000001`, available {`00000001`, one GREASE version} | same shape | yes (the order of the two available versions varies in both) |
| GREASE | a reserved id, random each run | value of 12, 7, 7, 5, 7 bytes (runs 1 to 5) | value of 14, 10, 6, 12, 2 bytes (runs 1 to 5) | same kind; the lengths overlap, only 5 samples each |

Order: **random in both clients**, different in every one of the 14 captures (7 distinct orders per client, none
seen in both clients; for example Chrome run 1
`0xf 0x6 0x8 0x1 0x20 0x11 0x5 0x7 GREASE 0x3128 0x9 0x3 0x4`, phantom run 1
`0x1 0x3128 0x11 0x7 GREASE 0x9 0x4 0x20 0x8 0x5 0xf 0x6 0x3`). So a single capture cannot show an order
match or mismatch; the runs show both permute. Not present in either: `active_connection_id_limit`,
`disable_active_migration`, `ack_delay_exponent`, `max_ack_delay`.

### TLS ClientHello inside the Initial

Identical in all 14 captures (claims file, section 1): cipher suites `0x1301 0x1302 0x1303`, and the same 12
extensions for both clients, none of them flagged GREASE by the parser: `0x0000` SNI, `0x000a` supported_groups
(`0x11ec 0x001d 0x0017 0x0018`), `0x000d` signature_algorithms, `0x0010` ALPN (`h3`), `0x001b` compress_certificate
(`0x0002`, brotli), `0x002b` supported_versions (`0x0304`, TLS 1.3), `0x002d` psk_key_exchange_modes, `0x0033`
key_share (`0x11ec` X25519MLKEM768 with a 1216-byte key, and X25519 with a 32-byte key), `0x0039`
quic_transport_parameters, `0x44cd` ALPS (`h3`), `0xfe0d` ECH, and **`0xca34` trust_anchors, 186 bytes**.

What "identical" means is what the parser prints, no more. For each extension except `0xfe0d`: the length and the
decoded form. That is the full list for SNI, groups, signature algorithms, ALPN, ALPS, compress_certificate,
supported_versions and psk modes; for `key_share` only the group ids and the key lengths (the key bytes are random and
not compared); for `0xca34` only the length and the **first 32 bytes**, so the other 154 bytes are **not compared**
(`tools/src/initial.rs:628`). For `0xfe0d` (ECH): the type (0), KDF (1), AEAD (1) and enc length (32) are identical;
the payload length varies in both (runs 1 to 5: Chrome 176, 176, 208, 240, 176; phantom 144, 176, 240, 176, 208), and
the config id and the bytes are not compared. Whether this ECH is the GREASE form is not shown by the captures.
Extension order is random in both.

Side finding for the stealth crates: `KnownGap::TrustAnchorsExtension` says the TCP hello of wreq/btls 0.5.6
cannot send `0xca34`. phantom's BoringSSL fork does send it, and so does Chrome 156's QUIC hello (same length and same first 32 bytes in
all 14 captures).

### The QUIC Initial packets: different

| | Chrome 156 (7 captures) | phantom (7 captures) |
|---|---|---|
| datagrams | 2 x **1250** bytes | 2 x **1200** bytes |
| Destination Connection ID | **8** bytes | **20** bytes |
| Source Connection ID / token | 0 / 0 | 0 / 0 |
| first packet number | **1**, then 2 | **0**, then 1 |
| frames per datagram | **8 to 25**: the ClientHello is cut into CRYPTO frames at odd offsets and sent out of order (`off=18 len=3`, `off=1219 len=118`, `off=1856 len=84`, ...), with PING and PADDING frames between them | **1** (CRYPTO off=0 len=1149), then 2 (CRYPTO off=1149 len=732 + PADDING x416): in order |

Chrome's layout is what quiche's `QuicChaosProtector` does ("take a crypto frame and an amount of padding and
build a data packet that will parse to something equivalent", `quiche/quic/core/quic_chaos_protector.h`).
phantom does none of it. A server sees this in the first packet, before TLS finishes.

### HTTP/3 SETTINGS and the control stream

SETTINGS are equal in ids, values and order in all 14 captures (7 of each client; claims file, section 3), except
the GREASE setting, whose id and value are random in both:

| Id | Name | Chrome | phantom |
|---|---|---|---|
| 0x01 | QPACK_MAX_TABLE_CAPACITY | 65536 | 65536 |
| 0x06 | MAX_FIELD_SECTION_SIZE | 262144 | 262144 |
| 0x07 | QPACK_BLOCKED_STREAMS | 100 | 100 |
| 0x33 | H3_DATAGRAM | 1 | 1 |
| GREASE | random id and value | yes | yes |

Order is the same: `0x01, 0x06, 0x07, 0x33, GREASE`. The size of the control stream is not part of the comparison:
it varies with the GREASE id and value and, for Chrome, with its extra frames (as logged, type byte included:
phantom 30 or 34 bytes, Chrome 63 to 70). After SETTINGS the control stream differs:

| Control stream | Chrome 156 | phantom |
|---|---|---|
| frames | in all 7 captures: `SETTINGS`, one **GREASE frame**, then two **PRIORITY_UPDATE** frames (type `0xf0700`, which the parser prints as `unknown`), one per request stream: payload element id 0 and `u=0, i` for stream 0, element id 4 and `u=1, i` for stream 4 | `SETTINGS` only, in all 7 captures |

QPACK streams, with the server advertising a dynamic table of 0 (12 captures: `chrome`, `chrome-1` to `chrome-5`,
`phantom-h3`, `phantom-1` to `phantom-5`; claims file, section 4):

| Stream | Chrome (6 captures) | phantom (6 captures) |
|---|---|---|
| QPACK encoder (client uni stream 10) | seen in 6 of 6: the type byte and one instruction byte `0x20` (set capacity 0) | **not seen** in 6 of 6 |
| QPACK decoder (client uni stream 6) | seen with its type in 4 of 6 (`chrome-1` to `chrome-4`, 2 bytes); in `chrome` and `chrome-5` the server accepted a stream 6 but read no bytes from it | **not seen** in 6 of 6 |

The only client unidirectional stream phantom was seen to open in these 6 captures is stream 2 (control); Chrome's
were 2, 6 and 10. "Not seen" means the server logged no such stream. Where stream 6 arrived empty (Chrome, two
captures), the log does not show whether the client sent anything on it.

With the server advertising capacity 4096 and 100 blocked streams (one capture each, `results/chrome-qpack4096.txt`
and `results/phantom-qpack4096.txt`): Chrome opened the decoder stream (stream 6, type seen) and wrote 410 bytes on
its encoder stream; phantom wrote 439 bytes on its encoder stream (stream 10), and the server accepted a stream 6
from it but read no bytes from it, so **whether phantom opens a QPACK decoder stream is not shown** by this capture
(byte counts as logged, type byte included). The two also differ in how they use the table (claims file, section 5):
Chrome's first request is all literals (required insert count 0, a 457-byte HEADERS frame) and its second request
uses 14 dynamic-table references (required insert count 15, 18 bytes); phantom's first request already uses 14
(required insert count 15, 20 bytes). One capture each.

### Request header fields and pseudo-header order

In the 12 captures with a QPACK table of 0 (6 of each client; claims file, section 6), the first (navigation) request
has the same field names in the same order in both clients:
`:method GET`, `:authority`, `:scheme https`, `:path`, `sec-ch-ua`, `sec-ch-ua-mobile`, `sec-ch-ua-platform`,
`upgrade-insecure-requests`, `user-agent`, `accept`, `sec-fetch-site`, `sec-fetch-mode`, `sec-fetch-user`,
`sec-fetch-dest`, `accept-encoding: gzip, deflate, br, zstd`, `accept-language`, `priority: u=0, i`. So the pseudo-header
order is `m a s p` in both. The values match once `:path` is left out (`/chrome-nav` for Chrome, `/h3fetch` for
phantom: the two clients were pointed at different paths) and the `HeadlessChrome` token in Chrome's User-Agent is read
as `Chrome`.

Byte-level difference, one run each: Chrome writes short values (`?0`, `"macOS"`, `?1`) as plain literals; phantom
Huffman-codes every literal.

## What is good enough, and what is not

Good enough, measured equal to Chrome 156 here, on the fields described above: the QUIC-side TLS ClientHello
(cipher suites, extension set, groups, key-share group ids and key lengths, signature algorithms, ALPN, ALPS, the ECH
header fields, the length and first 32 bytes of `0xca34`), the 11 fixed transport-parameter values and the shape of
the other two, the SETTINGS ids, values and order, the header field names and pseudo-header order, ALPN `h3`, QUIC
version 1. It works as an HTTP/3 client against a local server, including Alt-Svc discovery with UDP open.

Not good enough, measured different: the Initial packets (1200 vs 1250 bytes, 20-byte vs 8-byte DCID, packet number 0 vs
1, no chaos layout), the control stream (no GREASE frame, no PRIORITY_UPDATE), the QPACK streams (not seen from phantom when the
server's table is 0; first-request dynamic-table use differs), Huffman coding of short values. All of these are visible
to a server in the first flight. I do not know which of them a given anti-bot system looks at.

Not measured, so not a match: acknowledgements and their timing, flow-control updates, loss recovery and probe timers,
congestion control, packet pacing, path MTU discovery, short-header packet sizes, key updates, connection close,
0-RTT and session resumption, Retry and tokens, connection migration, QUIC v2, the spin bit, later requests on a warm
connection, and a long-lived connection (both connections here are short: phantom's ends when `h3fetch` exits,
Chrome's when `--dump-dom` finishes). Also not measured: the two sides under a real network, any real site, and the HTTP/3 side of a
Windows or Linux Chrome. The Chrome here is headless and QUIC was forced by flag (a cold start with no Alt-Svc
history), which may differ from a normal navigation. Chrome 156.0.8078.76 (this machine) is a different build from the ones
phantom's guide names (156.0.8078.48 for the macOS profile, 156.0.8078.47 for the Linux profile whose TLS, HTTP/2 and
HTTP/3 layers it describes).

## Recommendation

1. **Do not put phantom in the same binary as wreq/btls.** On macOS arm64 it does not link, and the two BoringSSL
   builds export 6,047 of the same unprefixed names. The only mitigation phantom ships (`prefix-symbols`) is, from my
   reading of its `build/prefix.rs`, built for ELF, and I did not run it anywhere.
2. **If HTTP/3 is still wanted, use phantom out of process**: a small helper binary that depends on phantom and
   nothing else from this repository, called by `mimic` over stdin/stdout, behind a cargo feature that is off by default.
   The pick of phantom came from an earlier read-only evaluation (notes, not in this repository); this spike surveyed
   no other candidate. In this spike phantom's TLS and transport-parameter set matched Chrome 156's on the fields compared.
3. **Be honest about what it is**: "Chrome 156 TLS, transport parameters, SETTINGS and headers over QUIC", not "Chrome's
   QUIC". The Initial layout and control-stream differences above are the first things to take to phantom's authors
   (or to fix in a fork), and the capture tools of this spike are the test for it.
4. **Do not enable Alt-Svc learning with the default policy** until the UDP-blocked case is handled (below).
5. Risks that stand: pre-1.0 with fast churn (six releases from 0.2.0 to 0.6.0 in 13 days; 0.6.0 came five days
   after 0.5.1), many patched forks vendored under `phantom-*` names, and docs.rs showed its build-failure page for
   0.6.0 when I fetched it on 2026-11-02 (I used the 0.5.1 pages and the repository's guides). The lead's notes said
   "about six weeks old, one maintainer": crates.io shows the crate is 16 days old; I did not check the repository's
   age or how many people maintain it.

## Integration plan

Nothing here is built; it is the order I would build it in.

1. **Measure our own entry first.** Move the relay and the wire parsers (`tools/src/initial.rs`, `h3wire.rs`, no
   dependencies beyond `ring` and `serde_json`, both already in `mimic`) next to `capture_h2` in `crates/mimic` as
   `capture_h3`, and record an `H3Spec` for Chrome 155 macOS beside `chrome-155-macos.json` in `crates/codec`
   (transport parameters with a "random order" flag, SETTINGS, control-stream frames, QPACK stream behaviour, Initial
   layout). Nothing typed by hand, like the TLS and HTTP/2 entries.
2. **Where the code goes.** `crates/codec`: the `H3Spec` data only (no TLS). `crates/mimic`: a feature `http3`, **off by
   default**, adding `WireProtocol::Http3` and a client that talks to the helper; no phantom dependency in `mimic`.
   New package `crates/h3d` (binary `sbrs-h3d`, `publish = false`, not a default member): the only crate that depends
   on phantom. A failure to find the helper is a typed error (`H3Unavailable`), not a panic.
3. **Version pin.** `phantom-http = "=0.6.0"` (exact) in `crates/h3d` only, its `Cargo.lock` committed. An upgrade is a
   pull request that re-runs the capture and the diff of this spike; the wire test fails when a value in the table
   above moves.
4. **One identity for HTTP/2 and HTTP/3.** The stealth entry is Chrome 155 macOS; phantom's catalog, from its guides and
   the 0.5.1 docs (the code was not enumerated), has Chrome 154 and 156 for macOS and a 155 only for Linux. Two ways, in
   this order: (i) re-measure the stealth entry at 156 so both transports present one major; (ii) keep 155 and feed
   phantom our measured transport parameters through its `fingerprint-import` feature (`import_quic_transport_profile`
   takes the extension payload, which is what the relay prints), refusing to run (`VersionMismatch`, as for TLS and
   HTTP/2) when the majors differ. Headers always come from the one `HeaderSpec`, so an HTTP/2 and an HTTP/3 request
   carry the same list in the same order. Until `0xca34` can be sent over TCP, the TCP and QUIC hellos of one session
   differ in their extension set.
5. **Alt-Svc.** Off at first: HTTP/3 only when the caller asks for it. Later, learn `Alt-Svc: h3` from a TCP response only
   after a UDP probe to that origin succeeded, and use phantom's racing policy rather than the default sequential one.
   Routes: phantom's proxies guide (`crates/phantom/src/docs/proxies.md`, read on `main` on 2026-11-02, not compared with
   the 0.6.0 tag) says an HTTP CONNECT tunnel carries TCP, so an HTTP proxy cannot carry HTTP/3; SOCKS5 can when the
   proxy supports UDP ASSOCIATE; a MASQUE proxy (`ConnectUdpProxy`) carries exact HTTP/3 requests only. Which routes
   learn Alt-Svc, and what that means for the MITM path of `MitmProxy`, I did not check.
6. **When UDP is blocked.** Measured: with a learned alternative and UDP dropped, requests 2 and 3 each ran until the
   harness's own 15 s total deadline and failed (`h3fetch.rs:154`); phantom's own handshake limit, eviction and fallback
   were not observed. Since phantom may also wait out the caller's deadline (not shown, not ruled out), the helper must (i) probe once per network with a short budget (a QUIC Initial to the
   origin, 1 to 2 seconds), (ii) on a timeout remember "UDP blocked" for a while and send every request over HTTP/2 through
   wreq, and (iii) never let an origin that advertises `h3` fail a request only because QUIC is blocked. The caller sees a
   typed `H3Unavailable { reason: UdpBlocked }` when it asked for HTTP/3 explicitly.
7. **Not now.** Chaos protection, the missing control-stream frames and the QPACK stream behaviour need changes inside
   phantom; ask upstream first.
