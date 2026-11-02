#!/usr/bin/env bash
# client-clash.sh [--tcp-only]: question (a). One process that calls wreq (btls 0.5.6) and
# phantom (its own BoringSSL) alternately against the local server. Run it through
# serve-and-run.sh, which sets H3_*.
set -u
"$H3_ROOT/bin/${CLASH_BIN:-clash}" "$H3_PORT" "$H3_CERT_DER" "$@"
