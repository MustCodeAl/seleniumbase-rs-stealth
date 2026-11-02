#!/usr/bin/env bash
# client-phantom.sh <h1|h2|h3> [extra h3fetch flags]: one phantom request to the local server.
# Run it through serve-and-run.sh, which sets H3_*.
set -u
proto="$1"
mkdir -p "$H3_OUT/qlog"   # phantom does not create the qlog directory (the error is a bare NotFound)
shift
"$H3_ROOT/bin/h3fetch" "$H3_PORT" "$H3_CERT_DER" "$proto" --qlog "$H3_OUT/qlog" "$@"
