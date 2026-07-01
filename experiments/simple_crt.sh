#!/bin/bash

set -e

rm -rf /tmp/simple

cargo r -r -- orc-apps/Basic/src/sdf/Simple.xdf orc-apps/Basic/src --out /tmp/simple
cargo b -r --manifest-path /tmp/simple/Cargo.toml

rm -rf /tmp/simple
