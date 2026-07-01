#!/bin/bash

set -e

rm -rf /tmp/addarray

cargo r -r -- orc-apps/AddArray/src/xdf/TopAddArray.xdf orc-apps/AddArray/src --out /tmp/addarray
cargo b -r --manifest-path /tmp/addarray/Cargo.toml

hyperfine --warmup 3 -N /tmp/addarray/target/release/topaddarray

rm -rf /tmp/addarray
