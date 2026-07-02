#!/bin/bash

set -e

XDF=orc-apps/Filters/src/iir/IIR.xdf
SRC=orc-apps/Filters/src
BIN=iir
NAIVE=/tmp/iir_naive
TOKIO=/tmp/iir_tokio

rm -rf "$NAIVE" "$TOKIO"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

hyperfine --warmup 3 -N \
  -n naive "$NAIVE/target/release/$BIN" \
  -n tokio "$TOKIO/target/release/$BIN"

rm -rf "$NAIVE" "$TOKIO"
