#!/bin/bash

set -e

XDF=orc-apps/HelloWorld/src/Hello.xdf
SRC=orc-apps/HelloWorld/src
BIN=hello
NAIVE=/tmp/hello_naive
TOKIO=/tmp/hello_tokio

rm -rf "$NAIVE" "$TOKIO"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

echo "=== naive ==="
timeout 1 "$NAIVE/target/release/$BIN" || true
echo "=== tokio ==="
timeout 1 "$TOKIO/target/release/$BIN" || true

rm -rf "$NAIVE" "$TOKIO"
