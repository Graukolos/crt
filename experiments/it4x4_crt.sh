#!/bin/bash

set -e

rm -rf /tmp/it4x4

cargo r -r -- orc-apps/RVC/src/org/sc29/wg11/mpeg4/part10/cbp/Residual/IT4x4.xdf orc-apps/RVC/src --out /tmp/it4x4
cargo b -r --manifest-path /tmp/it4x4/Cargo.toml

hyperfine --warmup 3 -N /tmp/it4x4/target/release/it4x4

rm -rf /tmp/it4x4
