#!/usr/bin/env bash
# build.sh [extra cargo build flags]: builds every spike binary that links, and copies each one to
# bin/ so a cleaned or garbage-collected target/ (mr-boxington manages it) does not take the
# binaries the other scripts run with it.
#
# `clash` (both TLS stacks in one binary) is built apart, with `cargo build --bin clash`, because
# on macOS arm64 its link fails; see docs/design/http3.md, question (a).
set -eu
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
cargo build --bin h3srv --bin relay --bin h3fetch --bin wreqonly "$@"
mkdir -p bin
for name in h3srv relay h3fetch wreqonly; do
  cp -f "target/debug/$name" "bin/$name"
done
echo "copied to $root/bin"
