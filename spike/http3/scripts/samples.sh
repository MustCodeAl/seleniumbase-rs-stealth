#!/usr/bin/env bash
# samples.sh [n] [report]: question (c), the random parts. Runs headless Chrome and phantom (h3) n times each
# against fresh servers (default 5), then prints one line per run: the order of the QUIC
# transport parameters, the GREASE parameter's length, version_information, the TLS extension
# order, the ECH payload length, the Initial datagrams (length / packet number / DCID length /
# frame count) and the frames on the HTTP/3 control stream.
#
# A value that differs between runs of the SAME client is random by design (permuted order, GREASE),
# so a single capture cannot show that two clients differ in it; this does.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
n="${1:-5}"

# `samples.sh 5 report` only prints the table from the logs already in out/.
if [ "${2:-}" != "report" ]; then
  for i in $(seq 1 "$n"); do
    "$root/scripts/serve-and-run.sh" "chrome-$i" "$root/scripts/client-chrome.sh" >/dev/null 2>&1
    "$root/scripts/serve-and-run.sh" "phantom-$i" "$root/scripts/client-phantom.sh" h3 >/dev/null 2>&1
  done
fi

line() {
  label="$1"
  relay="$root/out/$label/relay.jsonl"
  srv="$root/out/$label/h3srv.jsonl"
  echo "-- $label"
  jq -r 'select(.ev == "client_hello") | .client_hello as $h | .initial_packets as $p
    | "  tp_order:    " + ([$h.transport_parameters.params[] | if .grease then "GREASE" else .id end] | join(" ")),
      "  grease_tp:   len=" + ([$h.transport_parameters.params[] | select(.grease) | .len | tostring] | join(",")),
      "  version_inf: " + ([$h.transport_parameters.params[] | select(.id == "0x11") | .value_hex] | join(",")),
      "  tls_ext:     " + ($h.extension_order | join(" ")),
      "  ech_payload: " + ([$h.extensions[] | select(.id == "0xfe0d") | .decoded.payload_len | tostring] | join(",")) + "   hello_len=\($h.message_len)",
      "  initials:    " + ([$p[] | "\(.datagram_len)B/pn\(.packet_number)/dcid\(.dcid | length / 2)/\(.frames | length)frames"] | join("  "))' "$relay" | head -7
  jq -r 'select(.ev == "uni_stream" and .stream_type == "control")
    | "  control:     " + ([.frames[] | "\(.type)(\(.type_raw))"] | join(" "))' "$srv"
}

for i in $(seq 1 "$n"); do
  line "chrome-$i"
done
for i in $(seq 1 "$n"); do
  line "phantom-$i"
done
