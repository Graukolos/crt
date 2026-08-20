#!/bin/bash

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CRT=$ROOT/target/release/crt
BACKENDS=(single threads rayon tokio)
VARIANTS=("${BACKENDS[@]}" "${BACKENDS[@]/#/ts-}")
CONFIGS=("${VARIANTS[@]/#/crt-}" dcg)
CAP=${CAP:-1024}
FIRE_BUDGET=${FIRE_BUDGET:-1024}
CORES=${CORES:-"1 2 4 6 8 12"}
REPEAT=${ZIGBEE_REPEAT:-100}
RUNS=${RUNS:-5}
TIMEOUT=${TIMEOUT:-300}
WORK=/tmp/crt-scaling
RESULTS=$WORK/results

BAL_XDF=custom-networks/balanced/xdf/gen.xdf
BAL_SRC=custom-networks/balanced
BAL_NATIVE=custom-networks/balanced/native_rnd.c
BAL_BIN=gen

WIDE_XDF=custom-networks/wide/xdf/gen.xdf
WIDE_SRC=custom-networks/wide
WIDE_NATIVE=custom-networks/wide/native_rnd.c
WIDE_BIN=gen

PIPE_XDF=custom-networks/pipeline/xdf/gen.xdf
PIPE_SRC=custom-networks/pipeline
PIPE_NATIVE=custom-networks/pipeline/native_rnd.c
PIPE_BIN=gen

ZB_XDF=custom-networks/ZigBee/src/multitoken_tx/Top_ZigBee_tx.xdf
ZB_SRC=custom-networks/ZigBee/src
ZB_NATIVE=custom-networks/ZigBee/lib/native/linux.c
ZB_INPUT=custom-networks/ZigBee/lib/input_signals/tx_stream.in
ZB_BIN=top_zigbee_tx

cd "$ROOT"

which=${1:-all}
case $which in
balanced | wide | pipeline | zb | all) ;;
*)
	echo "unknown network: $which" >&2
	exit 1
	;;
esac

say() {
	printf '\n\033[1m==> %s\033[0m\n' "$*"
}

note() {
	printf '    %s\n' "$*"
}

cpuset() {
	if [ "$1" -le 1 ]; then echo 0; else echo "0-$(($1 - 1))"; fi
}

measure() {
	local net=$1 config=$2 cores=$3
	shift 3
	local set json
	set=$(cpuset "$cores")
	json=$RESULTS/$net-$config-$cores.json

	printf '    %-8s %-15s %2sc  ' "$net" "$config" "$cores"
	if ! timeout "$TIMEOUT" taskset -c "$set" "$@" >/dev/null 2>&1; then
		echo "FAILED or timed out after ${TIMEOUT}s"
		echo fail >"$RESULTS/$net-$config-$cores.fail"
		return
	fi
	hyperfine --warmup 1 --runs "$RUNS" -N --style none --export-json "$json" \
		"timeout $TIMEOUT taskset -c $set $*" >/dev/null 2>&1
	python3 -c "
import json
r = json.load(open('$json'))['results'][0]
print(f\"{r['mean'] * 1000:8.1f} ms  +- {(r['stddev'] or 0) * 1000:5.1f}\")"
}

sweep() {
	local net=$1 xdf=$2 src=$3 native=$4 bin=$5
	local cores variant backend out objs
	local flags=()

	say "$net: crt codegen + build"
	for variant in "${VARIANTS[@]}"; do
		backend=$variant
		flags=()
		if [ "${variant#ts-}" != "$variant" ]; then
			backend=${variant#ts-}
			flags=(--typestate)
		fi
		note "building $variant"
		"$CRT" "$xdf" "$src" \
			--out "$WORK/$net/$variant" \
			--backend "$backend" \
			--cap "$CAP" \
			--fire-budget "$FIRE_BUDGET" \
			"${CRT_FLAGS[@]}" \
			"${flags[@]}" >/dev/null
		cargo build --release --quiet \
			--manifest-path "$WORK/$net/$variant/Cargo.toml"
	done

	say "$net: DCG codegen + build (one per core count)"
	for cores in $CORES; do
		note "building -c $cores"
		out=$WORK/$net/dcg-$cores
		mkdir -p "$out"
		Dataflow_Code_Generator -d "$src" -n "$xdf" -w "$out" \
			-s "$CAP" -c "$cores" --opt_sched --silent "${DCG_FLAGS[@]}"
		gcc -O3 -std=gnu11 -x c -I"$out" -c "$native" -o "$out/native.o"
		g++ -O3 -std=c++11 -Wno-narrowing -I. -c "$out/main.cpp" -o "$out/main.o"
		objs=("$out/main.o" "$out/native.o")
		if [ -f "$out/orcc_compatibility.cpp" ]; then
			g++ -O3 -std=c++11 -Wno-narrowing -I. \
				-c "$out/orcc_compatibility.cpp" -o "$out/orcc.o"
			objs+=("$out/orcc.o")
		fi
		g++ -O3 -std=c++11 "${objs[@]}" -o "$out/$bin" -lpthread
	done

	say "$net: sweep over $CORES cores"
	for cores in $CORES; do
		for variant in "${VARIANTS[@]}"; do
			measure "$net" "crt-$variant" "$cores" \
				"$WORK/$net/$variant/target/release/$bin" "${RUN_ARGS[@]}"
		done
		measure "$net" dcg "$cores" "$WORK/$net/dcg-$cores/$bin" "${RUN_ARGS[@]}"
	done
}

report() {
	say "results"
	CORES=$CORES CONFIGS="${CONFIGS[*]}" RESULTS=$RESULTS python3 - <<'EOF'
import json
import os
import re

cores = [int(c) for c in os.environ["CORES"].split()]
configs = os.environ["CONFIGS"].split()

data = {}
for entry in os.scandir(os.environ["RESULTS"]):
    m = re.match(r"(\w+)-(.+)-(\d+)\.(json|fail)$", entry.name)
    if not m:
        continue
    key = (m.group(1), m.group(2), int(m.group(3)))
    if m.group(4) == "fail":
        data[key] = None
    else:
        r = json.load(open(entry.path))["results"][0]
        data[key] = (r["mean"] * 1000, (r["stddev"] or 0) * 1000)

def table(net, title, cell):
    print(f"\n    {net}: {title}\n")
    print("    " + "config".ljust(16) + "".join(f"{n}c".rjust(13) for n in cores))
    for config in configs:
        row = "".join(cell(config, n).rjust(13) for n in cores)
        print("    " + config.ljust(16) + row)

for net in sorted({key[0] for key in data}):
    def wall(config, n, net=net):
        if (net, config, n) not in data:
            return "-"
        v = data[(net, config, n)]
        return "FAIL" if v is None else f"{v[0]:.0f} ({v[1]:.0f})"

    def speedup(config, n, net=net):
        v, base = data.get((net, config, n)), data.get((net, config, cores[0]))
        return f"{base[0] / v[0]:.2f}x" if v and base else "-"

    table(net, "mean wall time, ms (stddev)", wall)
    table(net, f"speedup vs own {cores[0]}-core time", speedup)
EOF
}

say "building crt"
cargo build --release --quiet

rm -rf "$WORK"
mkdir -p "$RESULTS"

if [ "$which" = balanced ] || [ "$which" = all ]; then
	CRT_FLAGS=(--native-dir "$BAL_SRC")
	DCG_FLAGS=()
	RUN_ARGS=()
	sweep balanced "$BAL_XDF" "$BAL_SRC" "$BAL_NATIVE" "$BAL_BIN"
fi

if [ "$which" = wide ] || [ "$which" = all ]; then
	CRT_FLAGS=(--native-dir "$WIDE_SRC")
	DCG_FLAGS=()
	RUN_ARGS=()
	sweep wide "$WIDE_XDF" "$WIDE_SRC" "$WIDE_NATIVE" "$WIDE_BIN"
fi

if [ "$which" = pipeline ] || [ "$which" = all ]; then
	CRT_FLAGS=(--native-dir "$PIPE_SRC")
	DCG_FLAGS=()
	RUN_ARGS=()
	sweep pipeline "$PIPE_XDF" "$PIPE_SRC" "$PIPE_NATIVE" "$PIPE_BIN"
fi

if [ "$which" = zb ] || [ "$which" = all ]; then
	say "zb: building ${REPEAT}x input"
	for _ in $(seq "$REPEAT"); do cat "$ZB_INPUT"; done >"$WORK/big.in"
	CRT_FLAGS=(--orcc)
	DCG_FLAGS=(--orcc)
	RUN_ARGS=(-i "$WORK/big.in" -w "$WORK/zb-bench.out")
	sweep zb "$ZB_XDF" "$ZB_SRC" "$ZB_NATIVE" "$ZB_BIN"
fi

report
note "raw hyperfine JSON kept in $RESULTS"
