#!/bin/bash

set -e

XDF=orc-apps/Predistortion/src/lowlevel_dpd/AdderTree.xdf
SRC=orc-apps/Predistortion/src
BIN=addertree
NAIVE=/tmp/addertree_naive
TOKIO=/tmp/addertree_tokio
RAYON=/tmp/addertree_rayon

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
