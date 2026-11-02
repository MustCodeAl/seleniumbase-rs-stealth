#!/usr/bin/env bash
# Writes certs/{key.pem,cert.pem,cert.der,spki.b64}: a throwaway P-256 certificate for
# localhost / 127.0.0.1. spki.b64 is the value of Chrome's --ignore-certificate-errors-spki-list
# (base64 of the SHA-256 of the SubjectPublicKeyInfo; see https://www.chromium.org/quic/playing-with-quic/).
# certs/ is gitignored. Needs openssl (LibreSSL from macOS is enough).
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/certs"
mkdir -p "$dir"
openssl ecparam -genkey -name prime256v1 -noout -out "$dir/key.pem"
openssl req -new -x509 -key "$dir/key.pem" -sha256 -days 30 \
  -config "$root/scripts/openssl-san.cnf" -out "$dir/cert.pem"
openssl x509 -in "$dir/cert.pem" -outform der -out "$dir/cert.der"
openssl x509 -in "$dir/cert.pem" -noout -pubkey \
  | openssl pkey -pubin -outform der \
  | openssl dgst -sha256 -binary \
  | openssl enc -base64 > "$dir/spki.b64"
echo "certs written to $dir; SPKI $(cat "$dir/spki.b64")"
