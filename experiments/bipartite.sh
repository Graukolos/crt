#!/bin/bash

set -e

XDF=orc-apps/Basic/src/sdf/Bipartite.xdf
SRC=orc-apps/Basic/src
BIN=bipartite
NAIVE=/tmp/bipartite_naive
TOKIO=/tmp/bipartite_tokio
RAYON=/tmp/bipartite_rayon

rm -rf "$NAIVE" "$TOKIO" "$RAYON"

cargo r -r -- "$XDF" "$SRC" --out "$NAIVE" --backend naive
cargo b -r --manifest-path "$NAIVE/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$TOKIO" --backend tokio
cargo b -r --manifest-path "$TOKIO/Cargo.toml"

cargo r -r -- "$XDF" "$SRC" --out "$RAYON" --backend rayon
cargo b -r --manifest-path "$RAYON/Cargo.toml"

echo "=== naive ==="
timeout 5 "$NAIVE/target/release/$BIN" || true
echo "=== tokio ==="
timeout 5 "$TOKIO/target/release/$BIN" || true
echo "=== rayon ==="
timeout 5 "$RAYON/target/release/$BIN" || true

rm -rf "$NAIVE" "$TOKIO" "$RAYON"
