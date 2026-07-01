#!/bin/bash

set -e

rm -rf /tmp/iir

cargo r -r -- orc-apps/Filters/src/iir/IIR.xdf orc-apps/Filters/src --out /tmp/iir
cargo b -r --manifest-path /tmp/iir/Cargo.toml

hyperfine --warmup 3 -N /tmp/iir/target/release/iir

rm -rf /tmp/iir
