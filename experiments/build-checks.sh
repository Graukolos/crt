#!/bin/bash

set -euo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
CRT=$ROOT/target/release/crt
BACKENDS=(single threads rayon tokio)
VARIANTS=("${BACKENDS[@]}" "${BACKENDS[@]/#/ts-}")
CAP=${CAP:-1024}
FIRE_BUDGET=${FIRE_BUDGET:-1024}
SAMPLE=${SAMPLE:-2}

NETWORKS=(
	"addarray|orc-apps/AddArray/src/xdf/TopAddArray.xdf|orc-apps/AddArray/src|topaddarray"
	"addertree|orc-apps/Predistortion/src/lowlevel_dpd/AdderTree.xdf|orc-apps/Predistortion/src|addertree"
	"bipartite|orc-apps/Basic/src/sdf/Bipartite.xdf|orc-apps/Basic/src|bipartite"
	"feistel|orc-apps/Crypto/CTL/Block_Ciphers/Feistel_Networks/Feistel.xdf|orc-apps/Crypto/CTL|feistel"
	"hello|orc-apps/HelloWorld/src/Hello.xdf|orc-apps/HelloWorld/src|hello"
	"histogrambinning|orc-apps/ImageProcessing/src/image/xdf/io/TestHistogramBinning.xdf|orc-apps/ImageProcessing/src|testhistogrambinning"
	"histogrambinning2|orc-apps/ImageProcessing/src/image/xdf/io/TestHistogramBinning2.xdf|orc-apps/ImageProcessing/src|testhistogrambinning2"
	"idct1d|orc-apps/Research/src/com/xilinx/mpeg4/part2/sp/iDCT/Idct1d.xdf|orc-apps/Research/src|idct1d"
	"iir|orc-apps/Filters/src/iir/IIR.xdf|orc-apps/Filters/src|iir"
	"it4x4|orc-apps/RVC/src/org/sc29/wg11/mpeg4/part10/cbp/Residual/IT4x4.xdf|orc-apps/RVC/src|it4x4"
	"simple|orc-apps/Basic/src/sdf/Simple.xdf|orc-apps/Basic/src|simple"
)

cd "$ROOT"

say() {
	printf '\n\033[1m==> %s\033[0m\n' "$*"
}

note() {
	printf '    %s\n' "$*"
}

build_check() {
	local name=$1 xdf=$2 src=$3 bin=$4
	local work=/tmp/crt-$name
	local variant backend
	local flags=()

	rm -rf "$work"
	say "$name: codegen + build"
	for variant in "${VARIANTS[@]}"; do
		backend=$variant
		flags=()
		if [ "${variant#ts-}" != "$variant" ]; then
			backend=${variant#ts-}
			flags=(--typestate)
		fi
		note "generating + building $variant"
		"$CRT" "$xdf" "$src" \
			--out "$work/$variant" \
			--backend "$backend" \
			--cap "$CAP" \
			--fire-budget "$FIRE_BUDGET" \
			"${flags[@]}" >/dev/null
		cargo build --release --quiet --manifest-path "$work/$variant/Cargo.toml"
	done

	say "$name: ${SAMPLE}s sample of each variant"
	note "this network has no @native exit(); crt terminates only via exit(),"
	note "so every variant runs forever and is cut off by timeout"
	for variant in "${VARIANTS[@]}"; do
		printf '\n--- %s/%s ---\n' "$name" "$variant"
		timeout "$SAMPLE" "$work/$variant/target/release/$bin" 2>&1 | head -n 20 || true
	done

	say "$name: OK (${#VARIANTS[@]} variants generated, built and started)"
	rm -rf "$work"
}

selected=()
if [ $# -eq 0 ]; then
	selected=("${NETWORKS[@]}")
else
	for want in "$@"; do
		hit=
		for entry in "${NETWORKS[@]}"; do
			if [ "${entry%%|*}" = "$want" ]; then
				selected+=("$entry")
				hit=1
			fi
		done
		[ -n "$hit" ] || { echo "unknown network: $want" >&2 && exit 1; }
	done
fi

say "building crt"
cargo build --release --quiet

passed=()
failed=()
for entry in "${selected[@]}"; do
	IFS='|' read -r name xdf src bin <<<"$entry"
	if (build_check "$name" "$xdf" "$src" "$bin"); then
		passed+=("$name")
	else
		failed+=("$name")
	fi
done

say "summary: ${#passed[@]} passed, ${#failed[@]} failed"
[ ${#passed[@]} -eq 0 ] || note "passed: ${passed[*]}"
if [ ${#failed[@]} -gt 0 ]; then
	note "failed: ${failed[*]}"
	exit 1
fi
