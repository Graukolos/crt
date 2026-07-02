#!/bin/bash

set -e

XDF=orc-apps/Basic/src/sdf/Simple.xdf
SRC=orc-apps/Basic/src
NAIVE=/tmp/simple_naive
TOKIO=/tmp/simple_tokio

rm -rf "$NAIVE" "$TOKIO"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

rm -rf "$NAIVE" "$TOKIO"
