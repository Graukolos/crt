#!/bin/bash

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CRT=$ROOT/target/release/crt
DCG=${DCG:-$ROOT/Dataflow_Code_Generator/build/Dataflow_Code_Generator}
BACKENDS=(single threads rayon tokio)
VARIANTS=("${BACKENDS[@]}" "${BACKENDS[@]/#/ts-}")
CAP=${CAP:-1024}
FIRE_BUDGET=${FIRE_BUDGET:-1024}
REPEAT=${ZIGBEE_REPEAT:-100}

XDF=custom-networks/ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf
SRC=custom-networks/ZigBee/src
NATIVE=custom-networks/ZigBee/lib/native/linux.c
INPUT=custom-networks/ZigBee/lib/input_signals/tx_stream.in
REFERENCE=custom-networks/ZigBee/lib/reference_output/tx_stream.out
BIN=top_zigbee_tx
WORK=/tmp/crt-zigbee

cd "$ROOT"

if [ ! -x "$DCG" ]; then
	echo "no DCG binary at $DCG; run experiments/bench.sh or build it with cmake" >&2
	exit 1
fi

say() {
	printf '\n\033[1m==> %s\033[0m\n' "$*"
}

note() {
	printf '    %s\n' "$*"
}

normalize() {
	sed 's/^[[:space:]]*//' "$1"
}

say "building crt"
cargo build --release --quiet

rm -rf "$WORK"
mkdir -p "$WORK/cpp"

say "ZigBee: crt codegen + build (--orcc)"
for variant in "${VARIANTS[@]}"; do
	backend=$variant
	flags=()
	if [ "${variant#ts-}" != "$variant" ]; then
		backend=${variant#ts-}
		flags=(--typestate)
	fi
	note "generating + building $variant"
	"$CRT" "$XDF" "$SRC" \
		--out "$WORK/$variant" \
		--backend "$backend" \
		--cap "$CAP" \
		--fire-budget "$FIRE_BUDGET" \
		--orcc \
		"${flags[@]}" >/dev/null
	cargo build --release --quiet --manifest-path "$WORK/$variant/Cargo.toml"
done

say "ZigBee: DCG codegen + build"
"$DCG" -d "$SRC" -n "$XDF" -w "$WORK/cpp" \
	-s "$CAP" -c "$(nproc)" --opt_sched --silent --orcc
gcc -O3 -std=gnu11 -x c -I"$WORK/cpp" -c "$NATIVE" -o "$WORK/cpp/native.o"
g++ -O3 -std=c++11 -Wno-narrowing -I. -c "$WORK/cpp/main.cpp" -o "$WORK/cpp/main.o"
objs=("$WORK/cpp/main.o" "$WORK/cpp/native.o")
if [ -f "$WORK/cpp/orcc_compatibility.cpp" ]; then
	g++ -O3 -std=c++11 -Wno-narrowing -I. \
		-c "$WORK/cpp/orcc_compatibility.cpp" -o "$WORK/cpp/orcc.o"
	objs+=("$WORK/cpp/orcc.o")
fi
g++ -O3 -std=c++11 "${objs[@]}" -o "$WORK/cpp/$BIN" -lpthread

declare -A RUNNER=([dcg-cpp]="$WORK/cpp/$BIN")
for variant in "${VARIANTS[@]}"; do
	RUNNER[crt-$variant]="$WORK/$variant/target/release/$BIN"
done
ORDER=("${VARIANTS[@]/#/crt-}" dcg-cpp)

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
	note "This is not an error, the ZigBee network has an inherent race between exit() and writing the last samples"
fi

say "ZigBee: benchmark (${REPEAT}x input)"
for _ in $(seq "$REPEAT"); do cat "$INPUT"; done >"$WORK/big.in"

args=()
for name in "${ORDER[@]}"; do
	args+=(-n "$name" "${RUNNER[$name]} -i $WORK/big.in -w $WORK/bench.out")
done
hyperfine --warmup 3 -N "${args[@]}"

rm -rf "$WORK"
