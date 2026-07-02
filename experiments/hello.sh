#!/bin/bash

set -e

XDF=orc-apps/HelloWorld/src/Hello.xdf
SRC=orc-apps/HelloWorld/src
BIN=hello
NAIVE=/tmp/hello_naive
TOKIO=/tmp/hello_tokio
RAYON=/tmp/hello_rayon

rm -rf "$NAIVE" "$TOKIO" "$RAYON"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$RAYON" --backend rayon
cargo b -r --manifest-path "$RAYON/Cargo.toml"

echo "=== naive ==="
timeout 1 "$NAIVE/target/release/$BIN" || true
echo "=== tokio ==="
timeout 1 "$TOKIO/target/release/$BIN" || true
echo "=== rayon ==="
timeout 1 "$RAYON/target/release/$BIN" || true

rm -rf "$NAIVE" "$TOKIO" "$RAYON"
