#!/usr/bin/env bash
# client-chrome.sh: one headless Chrome navigation to https://localhost:$H3_PORT/chrome-nav, with
# QUIC forced to that origin. Run it through serve-and-run.sh, which sets H3_*.
#
# Flags (https://www.chromium.org/quic/playing-with-quic/ and
# https://source.chromium.org/chromium/chromium/src/+/main:net/base/host_mapping_rules.h):
#   --enable-quic                     QUIC on
#   --origin-to-force-quic-on=H:P     use QUIC for that origin without waiting for Alt-Svc
#   --ignore-certificate-errors-spki-list=SPKI   trust the one throwaway certificate by its SPKI hash
#   --host-resolver-rules="MAP * 127.0.0.1"      any host name at all resolves to loopback, so no
#                                                packet leaves this machine
# --headless only: no window. A fresh --user-data-dir under out/ (gitignored).
set -eu
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
profile="$H3_OUT/chrome-profile"
rm -rf "$profile"
"$chrome" --headless --no-first-run --no-default-browser-check \
  --disable-background-networking --disable-component-update --disable-sync \
  --user-data-dir="$profile" \
  --host-resolver-rules="MAP * 127.0.0.1" \
  --enable-quic \
  --origin-to-force-quic-on="localhost:$H3_PORT" \
  --ignore-certificate-errors-spki-list="$H3_SPKI" \
  --dump-dom "https://localhost:$H3_PORT/chrome-nav" >"$H3_OUT/chrome.dom.txt" 2>"$H3_OUT/chrome.stderr.txt"
echo "chrome exit $?; dom: $(cat "$H3_OUT/chrome.dom.txt")"
