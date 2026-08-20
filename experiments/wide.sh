#!/bin/bash

set -euo pipefail

NET=wide
ROOT=$(cd "$(dirname "$0")/.." && pwd)
CRT=$ROOT/target/release/crt
BACKENDS=(single threads rayon tokio)
VARIANTS=("${BACKENDS[@]}" "${BACKENDS[@]/#/ts-}")
CAP=${CAP:-1024}
FIRE_BUDGET=${FIRE_BUDGET:-1024}

XDF=custom-networks/$NET/xdf/gen.xdf
SRC=custom-networks/$NET
NATIVE=custom-networks/$NET/native_rnd.c
BIN=gen
WORK=/tmp/crt-$NET

cd "$ROOT"

say() {
	printf '\n\033[1m==> %s\033[0m\n' "$*"
}

note() {
	printf '    %s\n' "$*"
}

say "building crt"
cargo build --release --quiet

rm -rf "$WORK"
mkdir -p "$WORK/cpp"

say "$NET: crt codegen + build"
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
		--native-dir "$SRC" \
		"${flags[@]}" >/dev/null
	cargo build --release --quiet --manifest-path "$WORK/$variant/Cargo.toml"
done

say "$NET: DCG codegen + build"
Dataflow_Code_Generator -d "$SRC" -n "$XDF" -w "$WORK/cpp" \
	-s "$CAP" -c "$(nproc)" --opt_sched --silent
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

say "$NET: termination check"
note "this network has no reference output; test_exit_rnd() calls exit(0), so the"
note "check is that every variant reaches it instead of hanging or crashing"
for name in "${ORDER[@]}"; do
	if timeout 900 "${RUNNER[$name]}" >/dev/null 2>&1; then
		note "$name: exited 0"
	else
		note "$name: FAILED (exit $?)"
	fi
done

say "$NET: benchmark"
args=()
for name in "${ORDER[@]}"; do
	args+=(-n "$name" "${RUNNER[$name]}")
done
hyperfine --warmup 1 --runs 5 -N "${args[@]}"

rm -rf "$WORK"
