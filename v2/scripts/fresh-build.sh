#!/bin/sh
set -eu
V2_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
# owned.sh sets private HOME/TMP/XDG and retains only the existing toolchain.
if test "${JSON_RESUME_OWNED_BUILD:-}" != 1; then
  exec "$V2_ROOT/scripts/owned.sh" env JSON_RESUME_OWNED_BUILD=1 sh "$0"
fi
rustup run 1.85.0 rustc --version >/dev/null
CARGO_HOME=$(mktemp -d "$V2_ROOT/.qa/fresh-cargo-XXXXXXXX")
CARGO_TARGET_DIR=$(mktemp -d "$V2_ROOT/.qa/fresh-target-XXXXXXXX")
export CARGO_HOME CARGO_TARGET_DIR
[ "$CARGO_HOME" != "$CARGO_TARGET_DIR" ]
unset RUSTFLAGS
CARGO_ENCODED_RUSTFLAGS=$(printf '%s\037%s' "--remap-path-prefix=$V2_ROOT=/json-resume-v2" "--remap-path-prefix=$RUSTUP_HOME=/rustup")
export CARGO_ENCODED_RUSTFLAGS
cargo +1.85.0 build --locked --release --target aarch64-apple-darwin --manifest-path "$V2_ROOT/Cargo.toml"
printf '%s\n' "$CARGO_HOME" "$CARGO_TARGET_DIR" > "$V2_ROOT/.qa/fresh-build-paths.txt"
printf '%s\n' "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/json-resume-v2"
