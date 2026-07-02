#!/bin/bash

set -e

XDF=orc-apps/RVC/src/org/sc29/wg11/mpeg4/part10/cbp/Residual/IT4x4.xdf
SRC=orc-apps/RVC/src
BIN=it4x4
NAIVE=/tmp/it4x4_naive
TOKIO=/tmp/it4x4_tokio

rm -rf "$NAIVE" "$TOKIO"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

hyperfine --warmup 3 -N \
  -n naive "$NAIVE/target/release/$BIN" \
  -n tokio "$TOKIO/target/release/$BIN"

rm -rf "$NAIVE" "$TOKIO"
