#!/bin/bash

set -e

Dataflow_Code_Generator -d ZigBee/src -n ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf -w /tmp/zigbee_dcg -s 16384 --orcc -c $(nproc) --opt_sched

gcc -O3 -x c -I/tmp/zigbee_dcg -c ZigBee/lib/native/linux.cpp -o /tmp/zigbee_dcg/linux.o
g++ -O3 -std=c++11 -Wno-narrowing -I. -c /tmp/zigbee_dcg/main.cpp -o /tmp/zigbee_dcg/main.o
g++ -O3 -std=c++11 -Wno-narrowing -I. -c /tmp/zigbee_dcg/orcc_compatibility.cpp -o /tmp/zigbee_dcg/orcc_compatibility.o
g++ -O3 -std=c++11 /tmp/zigbee_dcg/main.o /tmp/zigbee_dcg/orcc_compatibility.o /tmp/zigbee_dcg/linux.o -o /tmp/zigbee_dcg/Top_ZigBee_tx_cpp -lpthread

N=100
for _ in $(seq $N); do cat ZigBee/lib/input_signals/tx_stream.in; done > /tmp/zigbee_dcg/big.in

hyperfine --warmup 3 '/tmp/zigbee_dcg/Top_ZigBee_tx_cpp -i /tmp/zigbee_dcg/big.in -w /tmp/zigbee_dcg/out.txt'

rm -rf /tmp/zigbee_dcg