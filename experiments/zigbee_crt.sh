#!/bin/bash

set -e

rm -rf /tmp/zigbee
mkdir /tmp/zigbee
N=100
for _ in $(seq $N); do cat ZigBee/lib/input_signals/tx_stream.in; done > /tmp/zigbee/big.in

cargo r -r -- ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf ZigBee/src --out /tmp/zigbee
cargo b -r --manifest-path /tmp/zigbee/Cargo.toml

hyperfine --warmup 3 -N '/tmp/zigbee/target/release/top_zigbee_tx -i /tmp/zigbee/big.in -w /tmp/zigbee/out.txt'

rm -rf /tmp/zigbee
mkdir /tmp/zigbee
N=100
for _ in $(seq $N); do cat ZigBee/lib/input_signals/tx_stream.in; done > /tmp/zigbee/big.in

Dataflow_Code_Generator -d ZigBee/src -n ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf -w /tmp/zigbee -s 16384 --orcc -c $(nproc) --opt_sched --silent
gcc -O3 -x c -I/tmp/zigbee -c ZigBee/lib/native/linux.cpp -o /tmp/zigbee/linux.o
g++ -O3 -std=c++11 -Wno-narrowing -I. -c /tmp/zigbee/main.cpp -o /tmp/zigbee/main.o
g++ -O3 -std=c++11 -Wno-narrowing -I. -c /tmp/zigbee/orcc_compatibility.cpp -o /tmp/zigbee/orcc_compatibility.o
g++ -O3 -std=c++11 /tmp/zigbee/main.o /tmp/zigbee/orcc_compatibility.o /tmp/zigbee/linux.o -o /tmp/zigbee/Top_ZigBee_tx_cpp -lpthread

hyperfine --warmup 3 -N '/tmp/zigbee/Top_ZigBee_tx_cpp -i /tmp/zigbee/big.in -w /tmp/zigbee/out.txt'

rm -rf /tmp/zigbee