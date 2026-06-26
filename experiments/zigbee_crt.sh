#!/bin/bash

set -e

cargo run -r -- ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf ZigBee/src --out /tmp/zigbee_crt --backend naive

cargo build --release --manifest-path /tmp/zigbee_crt/Cargo.toml

N=100
for _ in $(seq $N); do cat ZigBee/lib/input_signals/tx_stream.in; done > /tmp/zigbee_crt/big.in

hyperfine --warmup 3 '/tmp/zigbee_crt/target/release/top_zigbee_tx -i /tmp/zigbee_crt/big.in -w /tmp/zigbee_crt/out.txt'

rm -rf /tmp/zigbee_crt
