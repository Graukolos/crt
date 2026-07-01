#!/bin/bash

set -e

rm -rf /tmp/idct1d

cargo r -r -- orc-apps/Research/src/com/xilinx/mpeg4/part2/sp/iDCT/Idct1d.xdf orc-apps/Research/src --out /tmp/idct1d
cargo b -r --manifest-path /tmp/idct1d/Cargo.toml

hyperfine --warmup 3 -N /tmp/idct1d/target/release/idct1d

rm -rf /tmp/idct1d
