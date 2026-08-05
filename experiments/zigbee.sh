#!/bin/bash

source "$(dirname "$0")/common.sh"

XDF=custom-networks/ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf
SRC=custom-networks/ZigBee/src
NATIVE=custom-networks/ZigBee/lib/native/linux.c
INPUT=custom-networks/ZigBee/lib/input_signals/tx_stream.in
REFERENCE=custom-networks/ZigBee/lib/reference_output/tx_stream.out
BIN=top_zigbee_tx
WORK=/tmp/crt-zigbee
REPEAT=${ZIGBEE_REPEAT:-100}

normalize() {
	sed 's/^[[:space:]]*//' "$1"
}

crt_bootstrap

rm -rf "$WORK"
mkdir -p "$WORK/cpp"

say "ZigBee: crt codegen + build (--orcc)"
crt_variants "$WORK" "$XDF" "$SRC" --orcc

say "ZigBee: DCG codegen + build"
Dataflow_Code_Generator -d "$SRC" -n "$XDF" -w "$WORK/cpp" \
	-s "$CAP" --orcc -c "$(nproc)" --opt_sched --silent
gcc -O3 -x c -I"$WORK/cpp" -c "$NATIVE" -o "$WORK/cpp/linux.o"
g++ -O3 -std=c++11 -Wno-narrowing -I. -c "$WORK/cpp/main.cpp" -o "$WORK/cpp/main.o"
g++ -O3 -std=c++11 -Wno-narrowing -I. -c "$WORK/cpp/orcc_compatibility.cpp" -o "$WORK/cpp/orcc.o"
g++ -O3 -std=c++11 "$WORK/cpp/main.o" "$WORK/cpp/orcc.o" "$WORK/cpp/linux.o" \
	-o "$WORK/cpp/$BIN" -lpthread

declare -A RUNNER=([dcg-cpp]="$WORK/cpp/$BIN")
for variant in "${VARIANTS[@]}"; do
	RUNNER[crt-$variant]="$WORK/$variant/target/release/$BIN"
done
ORDER=(crt-naive crt-threads crt-rayon crt-tokio crt-ts-naive crt-ts-tokio dcg-cpp)

say "ZigBee: correctness against reference output (1x input)"
normalize "$REFERENCE" >"$WORK/reference.norm"
failed=()
for name in "${ORDER[@]}"; do
	"${RUNNER[$name]}" -i "$INPUT" -w "$WORK/$name.out" || true
	normalize "$WORK/$name.out" >"$WORK/$name.norm"
	if cmp -s "$WORK/$name.norm" "$WORK/reference.norm"; then
		note "$name: matches reference"
	else
		failed+=("$name")
		note "$name: MISMATCH ($(wc -l <"$WORK/$name.norm") of $(wc -l <"$WORK/reference.norm") lines)"
	fi
done
if [ ${#failed[@]} -eq 0 ]; then
	note "all variants match the reference"
else
	note "mismatching variants: ${failed[*]}"
	note "crt-tokio is a known open defect: its per-port mpsc channels let the sink's"
	note "'done' token overtake queued 'hsp' samples, and the native exit(0) in"
	note "print_cyclecount kills the process before those samples are written"
fi

say "ZigBee: benchmark (${REPEAT}x input)"
for _ in $(seq "$REPEAT"); do cat "$INPUT"; done >"$WORK/big.in"

args=()
for name in "${ORDER[@]}"; do
	args+=(-n "$name" "${RUNNER[$name]} -i $WORK/big.in -w $WORK/bench.out")
done
hyperfine --warmup 3 -N "${args[@]}"

rm -rf "$WORK"
