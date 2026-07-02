#!/bin/bash

set -e

XDF=orc-apps/Crypto/CTL/Block_Ciphers/Feistel_Networks/Feistel.xdf
SRC=orc-apps/Crypto/CTL
BIN=feistel
NAIVE=/tmp/feistel_naive
TOKIO=/tmp/feistel_tokio

rm -rf "$NAIVE" "$TOKIO"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

hyperfine --warmup 3 -N \
  -n naive "$NAIVE/target/release/$BIN" \
  -n tokio "$TOKIO/target/release/$BIN"

rm -rf "$NAIVE" "$TOKIO"
