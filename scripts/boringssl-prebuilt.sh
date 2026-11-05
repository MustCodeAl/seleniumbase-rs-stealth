#!/bin/sh
# Builds the BoringSSL of btls-sys once, into a cache folder, and prints how to use it.
# A build of the workspace then links that library instead of compiling BoringSSL again
# (docs/contributing/boringssl.md).
#
#   sh scripts/boringssl-prebuilt.sh
#   BORING_BSSL_PATH=<the folder it prints> cargo test --workspace --features seleniumbase-rs-stealth/mimic
#
# The folder is named after the btls-sys version in Cargo.lock, so a version change builds a
# new one. Set BORING_BSSL_PATH for this workspace's commands only, never in a shell profile:
# other BoringSSL crates may read the same variable and must not link this build.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
pkgid=$(cd "$root" && cargo pkgid btls-sys)
version=${pkgid##*@}
case $version in
  [0-9]*) ;;
  *) echo "boringssl-prebuilt: no btls-sys version in Cargo.lock ($pkgid)" >&2; exit 1 ;;
esac
cache=${XDG_CACHE_HOME:-$HOME/.cache}
dir="$cache/seleniumbase-rs-stealth/boringssl-btls-sys-$version"

if [ -f "$dir/lib/libssl.a" ] && [ -f "$dir/lib/libcrypto.a" ] && [ -d "$dir/include/openssl" ]; then
  echo "boringssl-prebuilt: $dir is already built"
else
  rm -rf "$dir"
  mkdir -p "$dir"
  scratch=$(mktemp -d)
  trap 'rm -rf "$scratch"' EXIT
  mkdir -p "$scratch/src"
  printf '%s\n' '[package]' 'name = "boringssl-prebuilt"' 'version = "0.0.0"' 'edition = "2024"' \
    'publish = false' '' '[workspace]' '' '[dependencies]' "btls-sys = \"=$version\"" \
    > "$scratch/Cargo.toml"
  : > "$scratch/src/lib.rs"
  echo "boringssl-prebuilt: building BoringSSL of btls-sys $version into $dir"
  (cd "$scratch" && BORING_BSSL_INSTALL_DIR="$dir" cargo build --quiet)
fi

echo
echo "Use it for one command at a time:"
echo "  BORING_BSSL_PATH=\"$dir\" cargo test --workspace --features seleniumbase-rs-stealth/mimic"
