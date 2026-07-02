#!/bin/bash

set -e

XDF=orc-apps/AddArray/src/xdf/TopAddArray.xdf
SRC=orc-apps/AddArray/src
BIN=topaddarray
NAIVE=/tmp/addarray_naive
TOKIO=/tmp/addarray_tokio
RAYON=/tmp/addarray_rayon

rm -rf "$NAIVE" "$TOKIO" "$RAYON"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$RAYON" --backend rayon
cargo b -r --manifest-path "$RAYON/Cargo.toml"

hyperfine --warmup 3 -N \
  -n naive "$NAIVE/target/release/$BIN" \
  -n tokio "$TOKIO/target/release/$BIN" \
  -n rayon "$RAYON/target/release/$BIN"

rm -rf "$NAIVE" "$TOKIO" "$RAYON"
