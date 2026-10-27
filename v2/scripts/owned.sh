#!/bin/sh
set -eu
V2_ROOT=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
export RUSTUP_HOME="${RUSTUP_HOME:-$(rustup show home)}"
export HOME="$V2_ROOT/.qa/home"
export XDG_CONFIG_HOME="$HOME/config" XDG_CACHE_HOME="$HOME/cache" XDG_DATA_HOME="$HOME/data"
export CARGO_HOME="$V2_ROOT/.qa/cargo-home"
export CARGO_TARGET_DIR="$V2_ROOT/.qa/target"
export npm_config_cache="$V2_ROOT/.qa/npm-cache" TMPDIR="$V2_ROOT/.qa/tmp/"
mkdir -p "$HOME" "$XDG_CONFIG_HOME" "$XDG_CACHE_HOME" "$XDG_DATA_HOME" "$CARGO_HOME" "$CARGO_TARGET_DIR" "$TMPDIR"
exec "$@"
