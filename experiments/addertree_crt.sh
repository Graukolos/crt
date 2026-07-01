#!/bin/bash

set -e

rm -rf /tmp/addertree

cargo r -r -- orc-apps/Predistortion/src/lowlevel_dpd/AdderTree.xdf orc-apps/Predistortion/src --out /tmp/addertree
cargo b -r --manifest-path /tmp/addertree/Cargo.toml

hyperfine --warmup 3 -N /tmp/addertree/target/release/addertree

rm -rf /tmp/addertree
