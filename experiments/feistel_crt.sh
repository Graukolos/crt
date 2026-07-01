#!/bin/bash

set -e

rm -rf /tmp/feistel

cargo r -r -- orc-apps/Crypto/CTL/Block_Ciphers/Feistel_Networks/Feistel.xdf orc-apps/Crypto/CTL --out /tmp/feistel
cargo b -r --manifest-path /tmp/feistel/Cargo.toml

hyperfine --warmup 3 -N /tmp/feistel/target/release/feistel

rm -rf /tmp/feistel
