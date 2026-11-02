#!/usr/bin/env bash
# link-experiments.sh: question (a), the link side. Builds the `clash` binary (phantom and the
# stealth crates in one process) several ways and counts what the linker said. Run it after
# `scripts/build.sh` so only the final link repeats. Logs go to out/link-<variant>.log.
#
#   default       what `cargo build --bin clash` does
#   ld_classic    ask Apple's old linker (-ld_classic) instead of the new one
#   link_dead     keep dead code (no -dead_strip)
#
# On success the binary is copied to bin/clash-<variant>.
set -u
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
mkdir -p out bin

run() {
  variant="$1"
  shift
  log="$root/out/link-$variant.log"
  cargo rustc -p http3-spike-client --bin clash -- "$@" >"$log" 2>&1
  status=$?
  dup=$(grep -c 'duplicate symbol' "$log")
  warn=$(grep -c 'ld: warning:' "$log")
  crash=$(grep -c 'Trace/BPT trap\|linker command failed due to signal' "$log")
  err=$(grep -c 'ld: error\|^error' "$log")
  echo "== $variant: cargo exit $status; ld warnings $warn (duplicate symbol $dup); linker crashed $crash; error lines $err"
  if [ "$status" -eq 0 ]; then
    cp -f target/debug/clash "bin/clash-$variant"
    echo "   binary: bin/clash-$variant; running it against the local server"
    CLASH_BIN="clash-$variant" "$root/scripts/serve-and-run.sh" "clash-$variant" "$root/scripts/client-clash.sh"
    echo "   exit $?"
  fi
}

# mr-boxington may delete target/ between cargo commands when the disk is low, which turns every
# step below into a full rebuild; the symbol listing therefore runs right after the first link.
run default
"$root/scripts/symbols.sh" >"$root/out/symbols.txt" 2>&1
echo "== symbols.sh output saved to out/symbols.txt"
run ld_classic -C link-arg=-ld_classic
run link_dead -C link-dead-code
