# Faster BoringSSL builds

The `wire` crate's TLS goes through btls (`=0.5.6`), whose build script, `btls-sys`, compiles
BoringSSL with cmake: about 280 C and C++ files. A checkout or worktree with a fresh `target`
compiles all of them again. This note says what the compilation cache does with them, and how to
build them once and link the result.

## What was measured (2026-11-05)

On an 8 GB Apple M1, macOS, Rust 1.99.1, `CARGO_BUILD_JOBS=2`. Each run was a fresh crate that
depends on `btls-sys = "=0.5.6"` alone, built with plain `cargo build` (cargo goes through
mr-boxington, `mbx`, on this machine). cmake used Ninja through rninja, as this machine's shell
sets it.

| Build of btls-sys from a fresh target | Seconds | What `mbx explain --last` said |
|---|---|---|
| From source, as cargo builds it | 54 | 290 compilations bypassed the cache: 280 `cc-unknown-flag`, 8 `cc-compiler-query`, 4 `cc-missing-output`; 21 Rust crates missed (cold) |
| From source, writing the result to a folder (`BORING_BSSL_INSTALL_DIR`) | 42 | 283 bypassed (279 `cc-unknown-flag`); the btls-sys build script missed |
| Linking that folder (`BORING_BSSL_PATH`), first build | 6 | 28 hits, no misses |
| The same, built again | 1 | nothing to do |

So:

- **BoringSSL's C files are not cache hits.** mbx bypasses them: its C cache does not accept the
  flags cmake passes (`cc-unknown-flag`). The Rust crates around them are cached as usual.
- btls-sys `0.5.6` reads no compiler-launcher setting. `BORING_BSSL_COMPILER_LAUNCHER`, which
  passes `CMAKE_C_COMPILER_LAUNCHER` and `CMAKE_CXX_COMPILER_LAUNCHER`, is only on btls's main
  branch, after 0.5.6. mbx has no launcher mode anyway, and sccache is not used here.
- rninja's own cache may have served some of the C compiles in the two source builds, so a
  machine without it takes longer than 42 to 54 seconds.
- Linking a prebuilt BoringSSL takes the build of btls-sys from 42–54 s to 6 s, and the folder
  is 29 MB.

## Using a prebuilt BoringSSL

btls-sys 0.5.6 reads these (from its `build/config.rs` and `build/main.rs` at tag `v0.5.6`):

- `BORING_BSSL_INSTALL_DIR`: after building BoringSSL from source, copy `lib/libcrypto.a`,
  `lib/libssl.a` and `include/openssl/` into this folder, which must be empty.
- `BORING_BSSL_PATH`: link the BoringSSL in this folder instead of building it, with cmake and the
  patches skipped. Its headers come from `<path>/include` (or `BORING_BSSL_INCLUDE_PATH`).
  btls-sys warns that a provided BoringSSL "is expected to have the patches included". The folder
  `BORING_BSSL_INSTALL_DIR` writes has them, because btls-sys built it. btls-sys does not check
  which BoringSSL revision the folder holds.

`scripts/boringssl-prebuilt.sh` does both steps. It reads the btls-sys version from `Cargo.lock`
and builds once into `~/.cache/seleniumbase-rs-stealth/boringssl-btls-sys-<version>` (or under
`$XDG_CACHE_HOME`). It then prints the command to use:

```bash
sh scripts/boringssl-prebuilt.sh
BORING_BSSL_PATH="$HOME/.cache/seleniumbase-rs-stealth/boringssl-btls-sys-0.5.6" \
    cargo test --workspace --features seleniumbase-rs-stealth/wire
```

Rules for it:

- **Set `BORING_BSSL_PATH` for one command, not in a shell profile or `.cargo/config.toml`.** The
  name is shared with boring-sys and its forks, which may read it and must not link this build. A
  config file in this repository would also reach other packages built under it.
- **One folder per btls-sys version.** Nothing checks the revision, so when `Cargo.lock` moves
  btls-sys, run the script again. It builds a new folder named after the new version.
- **It is a speed-up for local builds only.** CI and releases build BoringSSL from source.
