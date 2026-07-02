#!/bin/bash

set -e

XDF=orc-apps/Research/src/com/xilinx/mpeg4/part2/sp/iDCT/Idct1d.xdf
SRC=orc-apps/Research/src
BIN=idct1d
NAIVE=/tmp/idct1d_naive
TOKIO=/tmp/idct1d_tokio

rm -rf "$NAIVE" "$TOKIO"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

hyperfine --warmup 3 -N \
  -n naive "$NAIVE/target/release/$BIN" \
  -n tokio "$TOKIO/target/release/$BIN"

rm -rf "$NAIVE" "$TOKIO"
