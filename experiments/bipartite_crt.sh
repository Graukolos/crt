#!/bin/bash

set -e

rm -rf /tmp/bipartite

cargo r -r -- orc-apps/Basic/src/sdf/Bipartite.xdf orc-apps/Basic/src --out /tmp/bipartite
cargo b -r --manifest-path /tmp/bipartite/Cargo.toml

/tmp/bipartite/target/release/bipartite

rm -rf /tmp/bipartite
