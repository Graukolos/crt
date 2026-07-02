#!/bin/bash

set -e

XDF=orc-apps/Basic/src/sdf/Simple.xdf
SRC=orc-apps/Basic/src
NAIVE=/tmp/simple_naive
TOKIO=/tmp/simple_tokio
RAYON=/tmp/simple_rayon

rm -rf "$NAIVE" "$TOKIO" "$RAYON"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$RAYON" --backend rayon
cargo b -r --manifest-path "$RAYON/Cargo.toml"

rm -rf "$NAIVE" "$TOKIO" "$RAYON"
