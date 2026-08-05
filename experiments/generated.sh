#!/bin/bash

source "$(dirname "$0")/common.sh"

XDF=custom-networks/generated/xdf/gen.xdf
SRC=custom-networks/generated
NATIVE=custom-networks/generated/native_rnd.c
BIN=gen
WORK=/tmp/crt-generated

crt_bootstrap

rm -rf "$WORK"
mkdir -p "$WORK/cpp"

say "generated: crt codegen + build"
crt_variants "$WORK" "$XDF" "$SRC" --native-dir "$SRC"

say "generated: DCG codegen + build"
Dataflow_Code_Generator -d "$SRC" -n "$XDF" -w "$WORK/cpp" \
	-s "$CAP" -c "$(nproc)" --opt_sched --silent
gcc -O3 -std=c11 -I"$WORK/cpp" -c "$NATIVE" -o "$WORK/cpp/native_rnd.o"
g++ -O3 -std=c++11 -Wno-narrowing -I. -c "$WORK/cpp/main.cpp" -o "$WORK/cpp/main.o"
g++ -O3 -std=c++11 "$WORK/cpp/main.o" "$WORK/cpp/native_rnd.o" \
	-o "$WORK/cpp/$BIN" -lpthread

declare -A RUNNER=([dcg-cpp]="$WORK/cpp/$BIN")
for variant in "${VARIANTS[@]}"; do
	RUNNER[crt-$variant]="$WORK/$variant/target/release/$BIN"
done
ORDER=(crt-naive crt-threads crt-rayon crt-tokio crt-ts-naive crt-ts-tokio dcg-cpp)

say "generated: termination check"
note "this network has no reference output; test_exit_rnd() calls exit(0), so the"
note "check is that every variant reaches it instead of hanging or crashing"
for name in "${ORDER[@]}"; do
	if timeout 900 "${RUNNER[$name]}" >/dev/null 2>&1; then
		note "$name: exited 0"
	else
		note "$name: FAILED (exit $?)"
	fi
done

say "generated: benchmark"
args=()
for name in "${ORDER[@]}"; do
	args+=(-n "$name" "${RUNNER[$name]}")
done
hyperfine --warmup 1 --runs 5 -N "${args[@]}"

rm -rf "$WORK"
