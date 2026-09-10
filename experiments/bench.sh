#!/bin/bash

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CRT=$ROOT/target/release/crt

BACKENDS=(single threads rayon tokio)
VARIANTS=("${BACKENDS[@]}" "${BACKENDS[@]/#/ts-}")
CONFIGS=("${VARIANTS[@]/#/crt-}" dcg)

MACHINE=${MACHINE:-}
DCG=${DCG:-}
OUT=${OUT:-$ROOT/results}
WORK=${WORK:-/tmp/crt-bench}

CAP=${CAP:-1024}
FIRE_BUDGET=${FIRE_BUDGET:-1024}
CORES=${CORES:-"1 2 4 6 8 12"}
CAPS=${CAPS:-"64 128 256 512 1024 2048 4096 8192"}
BUDGETS=${BUDGETS:-"1 4 16 64 256 1024 4096"}
NETWORKS=${NETWORKS:-"balanced wide pipeline zigbee"}
RUNS=${RUNS:-10}
WARMUP=${WARMUP:-1}
TIMEOUT=${TIMEOUT:-300}
ZIGBEE_REPEAT=${ZIGBEE_REPEAT:-100}

usage() {
	cat >&2 <<'EOF'
usage: MACHINE=<id> bench.sh <experiment>...

experiments:
  baseline   all configs at the default cap and firing budget, on all cores
  cores      sweep over $CORES with taskset
  cap        sweep over $CAPS (rebuilds crt and DCG per value)
  budget     sweep over $BUDGETS (crt only; DCG has no firing budget)
  all        every experiment above

environment:
  MACHINE          required, an id such as "ryzen" or "rpi"
  DCG              path to the Dataflow_Code_Generator binary; by default it is
                   built from the submodule into Dataflow_Code_Generator/build
  OUT              where index.json and raw/ are written (default <crt>/results)
  NETWORKS         default "balanced wide pipeline zigbee"
  CORES CAPS BUDGETS CAP FIRE_BUDGET RUNS WARMUP TIMEOUT ZIGBEE_REPEAT
EOF
	exit 1
}

[ $# -gt 0 ] || usage
[ -n "$MACHINE" ] || usage

EXPERIMENTS=()
for arg in "$@"; do
	case $arg in
	baseline | cores | cap | budget) EXPERIMENTS+=("$arg") ;;
	all) EXPERIMENTS=(baseline cores cap budget) ;;
	*)
		echo "unknown experiment: $arg" >&2
		usage
		;;
	esac
done

cd "$ROOT"

say() {
	printf '\n\033[1m==> %s\033[0m\n' "$*"
}

note() {
	printf '    %s\n' "$*"
}

MANIFEST=$WORK/manifest.tsv

record() {
	printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$@" >>"$MANIFEST"
}

cpuset() {
	if [ "$1" -le 1 ]; then echo 0; else echo "0-$(($1 - 1))"; fi
}

is_zigbee() {
	if [ "$1" = zigbee ]; then zigbee_input; fi
}

build_dcg_binary() {
	local src=$ROOT/Dataflow_Code_Generator
	local build=$src/build
	if [ -n "$DCG" ]; then
		note "using DCG at $DCG"
		return
	fi
	DCG=$build/Dataflow_Code_Generator
	if [ -x "$DCG" ]; then
		note "reusing DCG at $DCG"
		return
	fi
	if [ ! -f "$src/CMakeLists.txt" ]; then
		echo "the Dataflow_Code_Generator submodule is missing; run" >&2
		echo "  git submodule update --init --recursive" >&2
		exit 1
	fi
	say "building DCG from the submodule"
	cmake -S "$src" -B "$build" -DCMAKE_BUILD_TYPE=Release >/dev/null
	cmake --build "$build" --config Release -j "$(nproc)" >/dev/null
	if [ ! -x "$DCG" ]; then
		echo "DCG build finished but $DCG is missing" >&2
		exit 1
	fi
}

net_setup() {
	local net=$1
	CRT_FLAGS=()
	DCG_FLAGS=()
	RUN_ARGS=()
	case $net in
	balanced | wide | pipeline)
		XDF=custom-networks/$net/xdf/gen.xdf
		SRC=custom-networks/$net
		NATIVE=custom-networks/$net/native_rnd.c
		BIN=gen
		CRT_FLAGS=(--native-dir "$SRC")
		;;
	zigbee)
		XDF=custom-networks/ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf
		SRC=custom-networks/ZigBee/src
		NATIVE=custom-networks/ZigBee/lib/native/linux.c
		BIN=top_zigbee_tx
		CRT_FLAGS=(--orcc)
		DCG_FLAGS=(--orcc)
		RUN_ARGS=(-i "$WORK/zigbee.in" -w "$WORK/zigbee.out")
		;;
	*)
		echo "unknown network: $net" >&2
		exit 1
		;;
	esac
}

zigbee_input() {
	if [ -f "$WORK/zigbee.in" ]; then return; fi
	note "building ${ZIGBEE_REPEAT}x ZigBee input"
	local src=custom-networks/ZigBee/lib/input_signals/tx_stream.in
	for _ in $(seq "$ZIGBEE_REPEAT"); do cat "$src"; done >"$WORK/zigbee.in"
}

build_crt() {
	local net=$1 variant=$2 cap=$3 budget=$4 out=$5
	local backend=$variant
	local flags=()
	if [ "${variant#ts-}" != "$variant" ]; then
		backend=${variant#ts-}
		flags=(--typestate)
	fi
	if [ -x "$out/target/release/$BIN" ]; then return; fi
	"$CRT" "$XDF" "$SRC" \
		--out "$out" \
		--backend "$backend" \
		--cap "$cap" \
		--fire-budget "$budget" \
		"${CRT_FLAGS[@]}" \
		"${flags[@]}" >/dev/null
	cargo build --release --quiet --manifest-path "$out/Cargo.toml"
}

build_dcg() {
	local net=$1 cores=$2 cap=$3 out=$4
	local objs
	if [ -x "$out/$BIN" ]; then return; fi
	mkdir -p "$out"
	"$DCG" -d "$SRC" -n "$XDF" -w "$out" \
		-s "$cap" -c "$cores" --opt_sched --silent "${DCG_FLAGS[@]}"
	gcc -O3 -std=gnu11 -x c -I"$out" -c "$NATIVE" -o "$out/native.o"
	g++ -O3 -std=c++11 -Wno-narrowing -I. -c "$out/main.cpp" -o "$out/main.o"
	objs=("$out/main.o" "$out/native.o")
	if [ -f "$out/orcc_compatibility.cpp" ]; then
		g++ -O3 -std=c++11 -Wno-narrowing -I. \
			-c "$out/orcc_compatibility.cpp" -o "$out/orcc.o"
		objs+=("$out/orcc.o")
	fi
	g++ -O3 -std=c++11 "${objs[@]}" -o "$out/$BIN" -lpthread
}

# measure <experiment> <stem> <network> <config> <cores> <cap> <budget> <binary>
measure() {
	local experiment=$1 stem=$2 net=$3 config=$4 cores=$5 cap=$6 budget=$7 binary=$8
	local cpus rel json
	cpus=$(cpuset "$cores")
	rel=raw/$MACHINE/$experiment/$stem.json
	json=$OUT/$rel
	mkdir -p "$(dirname "$json")"

	printf '    %-9s %-16s %-14s ' "$net" "$config" "$stem"
	if ! timeout "$TIMEOUT" taskset -c "$cpus" "$binary" "${RUN_ARGS[@]}" >/dev/null 2>&1; then
		echo "FAILED or timed out after ${TIMEOUT}s"
		record "$experiment" "$net" "$config" "$cores" "$cap" "$budget" timeout "" ""
		return
	fi
	hyperfine --warmup "$WARMUP" --runs "$RUNS" -N --style none --export-json "$json" \
		"timeout $TIMEOUT taskset -c $cpus $binary ${RUN_ARGS[*]}" >/dev/null 2>&1
	record "$experiment" "$net" "$config" "$cores" "$cap" "$budget" ok "$rel" 0
	python3 -c "
import json
r = json.load(open('$json'))['results'][0]
print(f\"{r['mean'] * 1000:8.1f} ms  +- {(r['stddev'] or 0) * 1000:5.1f}\")"
}

run_baseline() {
	local net cores variant args name rel json i
	cores=$(nproc)
	for net in $NETWORKS; do
		net_setup "$net"
		is_zigbee "$net"
		say "baseline: $net"
		for variant in "${VARIANTS[@]}"; do
			note "building crt-$variant"
			build_crt "$net" "$variant" "$CAP" "$FIRE_BUDGET" \
				"$WORK/$net/crt-$variant-c$CAP-b$FIRE_BUDGET"
		done
		note "building dcg"
		build_dcg "$net" "$cores" "$CAP" "$WORK/$net/dcg-$cores-c$CAP"

		args=()
		for variant in "${VARIANTS[@]}"; do
			args+=(-n "crt-$variant"
				"$WORK/$net/crt-$variant-c$CAP-b$FIRE_BUDGET/target/release/$BIN ${RUN_ARGS[*]}")
		done
		args+=(-n dcg "$WORK/$net/dcg-$cores-c$CAP/$BIN ${RUN_ARGS[*]}")

		rel=raw/$MACHINE/baseline/$net.json
		json=$OUT/$rel
		mkdir -p "$(dirname "$json")"
		note "benchmarking ${#CONFIGS[@]} configurations"
		hyperfine --warmup "$WARMUP" --runs "$RUNS" -N --style none \
			--export-json "$json" "${args[@]}" >/dev/null 2>&1

		i=0
		for name in "${CONFIGS[@]}"; do
			record baseline "$net" "$name" "$cores" "$CAP" "$FIRE_BUDGET" ok "$rel" "$i"
			i=$((i + 1))
		done
		python3 -c "
import json
for r in json.load(open('$json'))['results']:
    print(f\"    {r['command'].split('/')[-1][:40]:44s}{r['mean'] * 1000:8.1f} ms\")"
	done
}

run_cores() {
	local net cores variant
	for net in $NETWORKS; do
		net_setup "$net"
		is_zigbee "$net"
		say "core scaling: $net"
		for variant in "${VARIANTS[@]}"; do
			note "building crt-$variant"
			build_crt "$net" "$variant" "$CAP" "$FIRE_BUDGET" \
				"$WORK/$net/crt-$variant-c$CAP-b$FIRE_BUDGET"
		done
		for cores in $CORES; do
			note "building dcg -c $cores"
			build_dcg "$net" "$cores" "$CAP" "$WORK/$net/dcg-$cores-c$CAP"
		done
		for cores in $CORES; do
			for variant in "${VARIANTS[@]}"; do
				measure core-scaling \
					"$net-crt-$variant-${cores}c" \
					"$net" "crt-$variant" "$cores" "$CAP" "$FIRE_BUDGET" \
					"$WORK/$net/crt-$variant-c$CAP-b$FIRE_BUDGET/target/release/$BIN"
			done
			measure core-scaling \
				"$net-dcg-${cores}c" \
				"$net" dcg "$cores" "$CAP" "$FIRE_BUDGET" \
				"$WORK/$net/dcg-$cores-c$CAP/$BIN"
		done
	done
}

run_cap() {
	local net cores cap variant
	cores=$(nproc)
	for net in $NETWORKS; do
		net_setup "$net"
		is_zigbee "$net"
		say "cap scaling: $net"
		for cap in $CAPS; do
			for variant in "${VARIANTS[@]}"; do
				note "building crt-$variant cap=$cap"
				build_crt "$net" "$variant" "$cap" "$FIRE_BUDGET" \
					"$WORK/$net/crt-$variant-c$cap-b$FIRE_BUDGET"
			done
			note "building dcg -s $cap"
			build_dcg "$net" "$cores" "$cap" "$WORK/$net/dcg-$cores-c$cap"
		done
		for cap in $CAPS; do
			for variant in "${VARIANTS[@]}"; do
				measure cap-scaling \
					"$net-crt-$variant-cap$cap" \
					"$net" "crt-$variant" "$cores" "$cap" "$FIRE_BUDGET" \
					"$WORK/$net/crt-$variant-c$cap-b$FIRE_BUDGET/target/release/$BIN"
			done
			measure cap-scaling \
				"$net-dcg-cap$cap" \
				"$net" dcg "$cores" "$cap" "$FIRE_BUDGET" \
				"$WORK/$net/dcg-$cores-c$cap/$BIN"
		done
	done
}

run_budget() {
	local net cores budget variant
	cores=$(nproc)
	for net in $NETWORKS; do
		net_setup "$net"
		is_zigbee "$net"
		say "budget scaling: $net"
		for budget in $BUDGETS; do
			for variant in "${VARIANTS[@]}"; do
				note "building crt-$variant budget=$budget"
				build_crt "$net" "$variant" "$CAP" "$budget" \
					"$WORK/$net/crt-$variant-c$CAP-b$budget"
			done
		done
		for budget in $BUDGETS; do
			for variant in "${VARIANTS[@]}"; do
				measure budget-scaling \
					"$net-crt-$variant-fire_budget$budget" \
					"$net" "crt-$variant" "$cores" "$CAP" "$budget" \
					"$WORK/$net/crt-$variant-c$CAP-b$budget/target/release/$BIN"
			done
		done
	done
}

emit_index() {
	say "writing $OUT/index.json"
	MACHINE=$MACHINE OUT=$OUT MANIFEST=$MANIFEST RUNS=$RUNS WARMUP=$WARMUP \
		CAP=$CAP FIRE_BUDGET=$FIRE_BUDGET ROOT=$ROOT CONFIGS="${CONFIGS[*]}" \
		python3 "$ROOT/experiments/index.py"
}

mkdir -p "$WORK" "$OUT"
: >"$MANIFEST"

say "building crt"
cargo build --release --quiet

for experiment in "${EXPERIMENTS[@]}"; do
	if [ "$experiment" != budget ]; then
		build_dcg_binary
		break
	fi
done

for experiment in "${EXPERIMENTS[@]}"; do
	case $experiment in
	baseline) run_baseline ;;
	cores) run_cores ;;
	cap) run_cap ;;
	budget) run_budget ;;
	esac
done

emit_index
note "generated projects and binaries kept in $WORK"
