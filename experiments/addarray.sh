#!/bin/bash

set -e

XDF=orc-apps/AddArray/src/xdf/TopAddArray.xdf
SRC=orc-apps/AddArray/src
BIN=topaddarray
NAIVE=/tmp/addarray_naive
TOKIO=/tmp/addarray_tokio

rm -rf "$NAIVE" "$TOKIO"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

hyperfine --warmup 3 -N \
  -n naive "$NAIVE/target/release/$BIN" \
  -n tokio "$TOKIO/target/release/$BIN"

rm -rf "$NAIVE" "$TOKIO"
