#!/usr/bin/env bash
# claims.sh: the numbers behind the "equal in every run" and "differs" statements of
# docs/design/http3.md, computed from the raw logs of 14 captures in out/<label>/ (gitignored):
#   headless Chrome:  chrome (the single capture), chrome-1 .. chrome-5 (samples.sh), chrome-qpack
#   phantom (h3):     phantom-h3 (the single capture), phantom-1 .. phantom-5 (samples.sh), phantom-qpack
# `chrome-qpack` and `phantom-qpack` ran against a server whose QPACK dynamic table is 4096 with
# 100 blocked streams (README.md); the other twelve against a table of 0.
#
# The committed copy of the output is results/claims-14-captures.txt:
#   scripts/claims.sh > results/claims-14-captures.txt
#
# What is compared is what the parsers in tools/src/initial.rs and h3srv.rs print: for the
# ClientHello, each extension's length and `decoded` form (key_share: group and key length; ECH:
# type, KDF, AEAD, enc length, payload length; 0xca34 trust_anchors and other unnamed extensions:
# length and the first 32 bytes; everything else: the full list), for transport parameters the
# full value. Needs jq.
# The label lists below are split into words on purpose.
# shellcheck disable=SC2046,SC2086

set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
chrome="chrome chrome-1 chrome-2 chrome-3 chrome-4 chrome-5 chrome-qpack"
phantom="phantom-h3 phantom-1 phantom-2 phantom-3 phantom-4 phantom-5 phantom-qpack"
default_runs="chrome chrome-1 chrome-2 chrome-3 chrome-4 chrome-5 phantom-h3 phantom-1 phantom-2 phantom-3 phantom-4 phantom-5"

# logs <relay|h3srv> <label>...: the log files of those captures.
logs() {
  kind="$1"
  shift
  for l in "$@"; do echo "$root/out/$l/$kind.jsonl"; done
}

DEFS='def lbl: input_filename | split("/") | .[-2]; def client: lbl | if startswith("chrome") then "chrome" else "phantom" end;'

echo "# claims.sh over: $chrome $phantom"

echo
echo "== 1. TLS ClientHello inside the QUIC Initial (14 captures)"
# shellcheck disable=SC2046
jq -c "$DEFS"'select(.ev == "client_hello") | .client_hello as $h | {
    c: client,
    suites: $h.cipher_suites,
    ids: ($h.extension_order | sort),
    grease_exts: ([$h.extensions[] | select(.grease)] | length),
    ext: [$h.extensions[] | select(.id != "0x0039" and .id != "0xfe0d") | {id, len, decoded}],
    ech: ($h.extensions[] | select(.id == "0xfe0d") | .decoded + {len})
  }' $(logs relay $chrome $phantom) | jq -s -r '
  group_by(.c) as $g
  | ($g[] | "\(.[0].c): \(length) captures; extensions flagged GREASE by the parser: \(map(.grease_exts) | add)"),
    ($g[] | "\(.[0].c) cipher suites, distinct lists: \(map(.suites) | unique | tojson)"),
    ($g[] | "\(.[0].c) extension ids (sorted), distinct lists: \(map(.ids) | unique | tojson)"),
    "extension ids (sorted) identical for both clients: \(map(.ids) | unique | length == 1)",
    ([.[] | .c as $c | .ext[] | . + {c: $c}] | group_by(.id)[]
      | "\(.[0].id) length \(.[0].len): distinct (length, decoded) values, chrome \([.[] | select(.c == "chrome") | {len, decoded}] | unique | length), phantom \([.[] | select(.c == "phantom") | {len, decoded}] | unique | length); identical across all 14: \([.[] | {len, decoded}] | unique | length == 1); decoded: \(.[0].decoded | tojson | .[0:72])"),
    ($g[] | "\(.[0].c) ECH 0xfe0d (type, kdf, aead, enc_len), distinct: \(map(.ech | {type, kdf, aead, enc_len}) | unique | tojson); payload_len in capture order (chrome, chrome-1..5, chrome-qpack / phantom-h3, phantom-1..5, phantom-qpack): \(map(.ech.payload_len) | tojson)")'

echo
echo "== 2. QUIC transport parameters (14 captures)"
# shellcheck disable=SC2046
jq -c "$DEFS"'select(.ev == "client_hello") | .client_hello.transport_parameters.params as $p | {
    c: client,
    order: [$p[] | if .grease then "GREASE" else .id end],
    grease_len: [$p[] | select(.grease) | .len],
    params: [$p[] | select(.grease | not) | {id, name, v: (.value_int // .value_hex)}]
  }' $(logs relay $chrome $phantom) | jq -s -r '
  def vals($c): [.[] | select(.c == $c) | .v] | unique;
  group_by(.c) as $g
  | ([$g[0][].order] | unique) as $a | ([$g[1][].order] | unique) as $b
  | ($g[] | "\(.[0].c): parameter orders, distinct of \(length) captures: \(map(.order) | unique | length); parameter count per capture: \(map(.order | length) | unique | tojson); GREASE parameter length per capture: \(map(.grease_len) | tojson)"),
    "parameter orders seen in both clients: \($a - ($a - $b) | length)",
    "parameter ids seen in any capture: \([.[].order[]] | unique | tojson)",
    ([.[] | .c as $c | .params[] | . + {c: $c}] | group_by(.id)
      | (.[] | "\(.[0].id) \(.[0].name): chrome \(vals("chrome") | length) distinct, phantom \(vals("phantom") | length) distinct" + (if (vals("chrome") | length) == 1 and vals("chrome") == vals("phantom") then "; one value in all 14 captures, same for both: \(vals("chrome")[0] | tojson)" else "" end)),
        "fixed parameters (one value in all 14 captures, same for both clients): \([.[] | select((vals("chrome") | length) == 1 and vals("chrome") == vals("phantom"))] | length) of \(length) non-GREASE parameters")'

echo
echo "== 3. HTTP/3 control stream (14 captures)"
# shellcheck disable=SC2046
jq -c "$DEFS"'select(.ev == "uni_stream" and .stream_type == "control") | {
    l: lbl, c: client, bytes,
    settings: [.frames[] | select(.type == "SETTINGS") | .settings[] | "\(if .grease then "GREASE" else .id end)=\(if .grease then "*" else (.value | tostring) end)"],
    frames: [.frames[] | if .type == "GREASE" then "GREASE" else "\(.type)(\(.type_raw))" end],
    priority_update: [.frames[] | select(.type_raw == "0xf0700") | .payload_hex]
  }' $(logs h3srv $chrome $phantom) | jq -s -r '
  (.[] | "\(.l): control stream \(.bytes) bytes as logged (type byte included); SETTINGS \(.settings | join(" ")); frames \(.frames | join(" "))" + (if (.priority_update | length) > 0 then "; 0xf0700 payloads (hex) \(.priority_update | join(" "))" else "" end)),
  (group_by(.c)[] | "\(.[0].c): distinct SETTINGS lists (GREASE value masked) \(map(.settings) | unique | length), distinct frame sequences \(map(.frames) | unique | length), control stream bytes seen \(map(.bytes) | unique | tojson)"),
  "SETTINGS lists identical for both clients: \(map(.settings) | unique | length == 1)"'

echo
echo "== 4. Client unidirectional streams as the server logged them, per capture"
# shellcheck disable=SC2046
jq -c "$DEFS"'select(.ev == "uni_stream") | {l: lbl, id: .stream_id, t: (.stream_type // "accepted, no bytes read (\(.error))"), b: .bytes, h: ((.bytes_hex // "") | .[0:8])}' $(logs h3srv $chrome $phantom) | jq -s -r '
  group_by(.l)[] | "\(.[0].l): " + (sort_by(.id) | map("stream \(.id) \(.t)" + (if .b != null then " \(.b) bytes" else "" end) + (if .t == "qpack-encoder" then " (first bytes after the type: \(.h))" else "" end)) | join("; "))'

echo
echo "== 5. QPACK request sections (server table 4096, 100 blocked streams)"
# shellcheck disable=SC2046
jq -r "$DEFS"'select(.ev == "request") | lbl as $l | .stream_id as $s | .frames[] | select(.type == "HEADERS")
  | "\($l) request stream \($s): HEADERS frame \(.len) bytes, required insert count \(.section.required_insert_count), dynamic-table references \([.section.fields[]? | select(.repr | startswith("indexed-dynamic"))] | length)"' $(logs h3srv chrome-qpack phantom-qpack)

echo
echo "== 6. Request header fields of the first request (the 12 captures with table 0)"
# shellcheck disable=SC2046
jq -c "$DEFS"'select(.ev == "request" and .stream_id == 0) | {c: client, f: [.frames[] | select(.type == "HEADERS") | .section.fields[]? | {n: .name, v: .value}]}' $(logs h3srv $default_runs) | jq -s -r '
  def norm: [.f[] | select(.n != ":path") | {n, v: (.v | sub("HeadlessChrome"; "Chrome"))}];
  (group_by(.c)[] | "\(.[0].c): \(length) captures, distinct field-name lists (order kept) \(map([.f[].n]) | unique | length), distinct :path values \([.[].f[] | select(.n == ":path") | .v] | unique | tojson)"),
  "field names and order identical for both clients: \(map([.f[].n]) | unique | length == 1)",
  "values identical for both clients once :path is left out and HeadlessChrome is read as Chrome: \(map(norm) | unique | length == 1)",
  "field names in order: \(.[0].f | map(.n) | join(" "))"'

echo
echo "== 7. QUIC Initial datagrams (14 captures)"
# shellcheck disable=SC2046
jq -c "$DEFS"'select(.ev == "client_hello") | {c: client, d: [.initial_packets[] | {len: .datagram_len, version, dcid: (.dcid | length / 2), scid: (.scid | length / 2), token: .token_len, pn: .packet_number, frames: (.frames | length)}]}' $(logs relay $chrome $phantom) | jq -s -r '
  group_by(.c)[] | "\(.[0].c): datagrams per capture \(map(.d | length) | unique | tojson); distinct (length, version, DCID length, SCID length, token length, packet number) per datagram: \(map(.d | map(del(.frames))) | unique | tojson); frames in the first datagram \(map(.d[0].frames) | min) to \(map(.d[0].frames) | max), in the second \(map(.d[1].frames) | min) to \(map(.d[1].frames) | max)"'

