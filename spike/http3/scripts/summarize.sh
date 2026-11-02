#!/usr/bin/env bash
# summarize.sh <out-dir>: prints, from out/<label>/relay.jsonl and h3srv.jsonl, what one client
# sent: the QUIC Initial's TLS ClientHello (cipher suites, extension order), the QUIC transport
# parameters in wire order, the HTTP/3 stream types and SETTINGS in arrival order, and the
# request's header fields in order. Needs jq.
set -u
dir="$1"
relay="$dir/relay.jsonl"
srv="$dir/h3srv.jsonl"

echo "== $dir"
if [ -f "$relay" ]; then
  echo "-- QUIC Initial: ClientHello"
  jq -r 'select(.ev == "client_hello") | .client_hello
         | "cipher_suites (\(.cipher_suites | length)): \(.cipher_suites | join(" "))",
           "extension_order: \(.extension_order | join(" "))",
           "message_len: \(.message_len)"' "$relay"
  echo "-- QUIC Initial: packets"
  jq -r 'select(.ev == "client_hello") | .initial_packets[]
         | "datagram \(.datagram_n): len=\(.datagram_len) packet_len=\(.packet_len) version=\(.version) dcid_len=\(.dcid | length / 2) scid_len=\(.scid | length / 2) token_len=\(.token_len) pn=\(.packet_number) frames=\(.frames | join(","))"' "$relay"
  echo "-- QUIC transport parameters, wire order (id name grease len value)"
  jq -r 'select(.ev == "client_hello") | .client_hello.transport_parameters
         | "extension \(.extension)", (.params | to_entries[]
           | "\(.key + 1)\t\(.value.id)\t\(.value.name)\t\(if .value.grease then "GREASE" else "-" end)\tlen=\(.value.len)\t\(.value.value_int // .value.value_hex)")' "$relay"
fi
if [ -f "$srv" ]; then
  echo "-- server saw: QUIC connection"
  jq -r 'select(.ev == "quic_connection") | "alpn=\(.alpn) sni=\(.sni) max_datagram_size=\(.max_datagram_size)"' "$srv"
  echo "-- server saw: unidirectional streams, in arrival order of their log lines"
  jq -r 'select(.ev == "uni_stream")
         | "stream \(.stream_id) type=\(.stream_type) (\(.stream_type_raw)) bytes=\(.bytes)",
           ((.frames // [])[] | select(.type == "SETTINGS") | .settings[]?
             | "    setting \(.id) \(.name)\(if .grease then " (GREASE)" else "" end) = \(.value)")' "$srv"
  echo "-- server saw: request HEADERS, field order (name: value [representation])"
  jq -r 'select(.ev == "request") | "request stream \(.stream_id), frames: \([.frames[].type] | join(","))",
         ((.frames[] | select(.type == "HEADERS") | .section.fields[]?
           | "    \(.name): \(.value)   [\(.repr)\(if .huffman then ", huffman" else "" end)]"))' "$srv"
  echo "-- server saw: TCP requests"
  jq -r 'select(.ev == "tcp_request") | "alpn=\(.alpn) sni=\(.sni) tls=\(.tls_version) request_line=\(.request_line)"' "$srv"
fi
