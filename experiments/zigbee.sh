#!/bin/bash

set -e

XDF=ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf
SRC=ZigBee/src
BIN=top_zigbee_tx

rm -rf /tmp/zigbee
mkdir -p /tmp/zigbee/cpp
N=100
for _ in $(seq $N); do cat ZigBee/lib/input_signals/tx_stream.in; done > /tmp/zigbee/big.in

cargo r -r -- "$XDF" "$SRC" --out /tmp/zigbee/naive --backend naive
cargo b -r --manifest-path /tmp/zigbee/naive/Cargo.toml

cargo r -r -- "$XDF" "$SRC" --out /tmp/zigbee/tokio --backend tokio
cargo b -r --manifest-path /tmp/zigbee/tokio/Cargo.toml

cargo r -r -- "$XDF" "$SRC" --out /tmp/zigbee/rayon --backend rayon
cargo b -r --manifest-path /tmp/zigbee/rayon/Cargo.toml

Dataflow_Code_Generator -d "$SRC" -n "$XDF" -w /tmp/zigbee/cpp -s 16384 --orcc -c $(nproc) --opt_sched --silent
gcc -O3 -x c -I/tmp/zigbee/cpp -c ZigBee/lib/native/linux.cpp -o /tmp/zigbee/cpp/linux.o
g++ -O3 -std=c++11 -Wno-narrowing -I. -c /tmp/zigbee/cpp/main.cpp -o /tmp/zigbee/cpp/main.o
g++ -O3 -std=c++11 -Wno-narrowing -I. -c /tmp/zigbee/cpp/orcc_compatibility.cpp -o /tmp/zigbee/cpp/orcc_compatibility.o
g++ -O3 -std=c++11 /tmp/zigbee/cpp/main.o /tmp/zigbee/cpp/orcc_compatibility.o /tmp/zigbee/cpp/linux.o -o /tmp/zigbee/cpp/Top_ZigBee_tx_cpp -lpthread

hyperfine --warmup 3 -N \
  -n crt-naive "/tmp/zigbee/naive/target/release/$BIN -i /tmp/zigbee/big.in -w /tmp/zigbee/out.txt" \
  -n crt-tokio "/tmp/zigbee/tokio/target/release/$BIN -i /tmp/zigbee/big.in -w /tmp/zigbee/out.txt" \
  -n crt-rayon "/tmp/zigbee/rayon/target/release/$BIN -i /tmp/zigbee/big.in -w /tmp/zigbee/out.txt" \
  -n dcg-cpp "/tmp/zigbee/cpp/Top_ZigBee_tx_cpp -i /tmp/zigbee/big.in -w /tmp/zigbee/out.txt"

rm -rf /tmp/zigbee
