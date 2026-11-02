#!/usr/bin/env bash
# symbols.sh: question (a), the symbol side. Finds the BoringSSL static libraries the two TLS
# stacks build (phantom-btls-sys for phantom, btls-sys for wreq) and compares the global symbols
# each one defines, then looks at what the final `clash` binary defines.
#
# A non-empty intersection of two un-prefixed archives is a clash in waiting: the linker either
# stops with "duplicate symbol", or silently uses one definition for both stacks. This prints
# the numbers; read them next to the link result.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
build="$root/target/debug/build"
work="${TMPDIR:-/tmp}/h3-symbols.$$"
mkdir -p "$work"

nm_defined() { nm -g -U "$1" 2>/dev/null | awk 'NF >= 3 && $2 ~ /^[TDSBCRA]$/ { print $3 }' | sort -u; }

for stack in phantom-btls-sys btls-sys; do
  : > "$work/$stack.syms"
  for lib in $(find "$build" -path "*/$stack-*/out/*" \( -name 'libssl.a' -o -name 'libcrypto.a' -o -name 'libbssl_wrapper.a' \) 2>/dev/null); do
    echo "$stack: $lib"
    nm_defined "$lib" >> "$work/$stack.syms"
  done
  sort -u -o "$work/$stack.syms" "$work/$stack.syms"
  echo "$stack: $(wc -l < "$work/$stack.syms" | tr -d ' ') global symbols defined"
done

echo "--- defined by both archives (intersection):"
comm -12 "$work/phantom-btls-sys.syms" "$work/btls-sys.syms" > "$work/both.syms"
echo "count: $(wc -l < "$work/both.syms" | tr -d ' ')"
head -15 "$work/both.syms"

echo "--- sample of each stack's symbol naming:"
echo "phantom-btls-sys:"; grep -m5 -i 'SSL_new\|EVP_' "$work/phantom-btls-sys.syms"
echo "btls-sys:"; grep -m5 -i 'SSL_new\|EVP_' "$work/btls-sys.syms"

bin="$root/target/debug/clash"
if [ -f "$bin" ]; then
  echo "--- final binary $bin"
  nm -g -U "$bin" 2>/dev/null | awk 'NF >= 3 { print $3 }' | sort > "$work/bin.syms"
  echo "global symbols: $(wc -l < "$work/bin.syms" | tr -d ' ')"
  echo "SSL_new definitions: $(grep -c '^_*SSL_new$' "$work/bin.syms")"
  echo "unprefixed BoringSSL symbols present: $(grep -c '^_*\(SSL_\|EVP_\|BIO_\|X509_\|OPENSSL_\)' "$work/bin.syms")"
fi
rm -rf "$work"
