#!/usr/bin/env bash
# serve-and-run.sh <label> <command...>
#
# Starts the local test server and the UDP relay on 127.0.0.1, runs <command...> with the
# environment below, then stops both. Everything stays on loopback. Logs go to out/<label>/.
#
#   TCP  127.0.0.1:$H3_PORT        h3srv, TLS with ALPN http/1.1
#   UDP  127.0.0.1:$H3_PORT        relay (decrypts the client's QUIC Initial, forwards unchanged)
#   UDP  127.0.0.1:$H3_UPSTREAM    h3srv, QUIC with ALPN h3
#
# Environment given to <command...>: H3_PORT, H3_CERT_DER, H3_SPKI, H3_OUT, H3_ROOT.
# Optional input: H3_PORT (4433), H3_UPSTREAM (4434), H3SRV_ARGS (extra h3srv flags, e.g. --alt-svc).
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
label="$1"
shift
out="$root/out/$label"
port="${H3_PORT:-4433}"
upstream="${H3_UPSTREAM:-4434}"
mkdir -p "$out"
rm -f "$out"/h3srv.jsonl "$out"/relay.jsonl
[ -f "$root/certs/cert.der" ] || "$root/scripts/mkcert.sh" >/dev/null

# shellcheck disable=SC2086  # H3SRV_ARGS is a list of flags on purpose
"$root/bin/h3srv" --cert "$root/certs/cert.pem" --key "$root/certs/key.pem" \
  --tcp "127.0.0.1:$port" --udp "127.0.0.1:$upstream" --out "$out/h3srv.jsonl" \
  ${H3SRV_ARGS:-} 2>/dev/null &
srv=$!
"$root/bin/relay" --listen "127.0.0.1:$port" --to "127.0.0.1:$upstream" \
  --out "$out/relay.jsonl" 2>/dev/null &
rel=$!
sleep 1

export H3_PORT="$port" H3_CERT_DER="$root/certs/cert.der" H3_SPKI="$(cat "$root/certs/spki.b64")"
export H3_OUT="$out" H3_ROOT="$root"
"$@"
status=$?

sleep 1
kill "$srv" "$rel" 2>/dev/null
wait "$srv" "$rel" 2>/dev/null
exit "$status"
