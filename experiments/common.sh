set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
CRT=$ROOT/target/release/crt
BACKENDS=(naive threads rayon tokio)
CAP=${CAP:-1024}
FIRE_BUDGET=${FIRE_BUDGET:-1024}

cd "$ROOT"

say() {
	printf '\n\033[1m==> %s\033[0m\n' "$*"
}

note() {
	printf '    %s\n' "$*"
}

crt_bootstrap() {
	say "building crt"
	cargo build --release --quiet
}

crt_project() {
	local out=$1 backend=$2 xdf=$3 src=$4
	shift 4
	"$CRT" "$xdf" "$src" \
		--out "$out" \
		--backend "$backend" \
		--cap "$CAP" \
		--fire-budget "$FIRE_BUDGET" \
		"$@" >/dev/null
	cargo build --release --quiet --manifest-path "$out/Cargo.toml"
}

crt_variants() {
	local work=$1 xdf=$2 src=$3
	shift 3
	local backend
	for backend in "${BACKENDS[@]}"; do
		note "generating + building $backend"
		crt_project "$work/$backend" "$backend" "$xdf" "$src" "$@"
	done
	note "generating + building naive --typestate"
	crt_project "$work/ts-naive" naive "$xdf" "$src" "$@" --typestate
	note "generating + building tokio --typestate"
	crt_project "$work/ts-tokio" tokio "$xdf" "$src" "$@" --typestate
}

VARIANTS=("${BACKENDS[@]}" ts-naive ts-tokio)

build_check() {
	local name=$1 xdf=$2 src=$3 bin=$4 seconds=${5:-2}
	local work=/tmp/crt-$name
	local variant

	crt_bootstrap
	rm -rf "$work"
	say "$name: codegen + build"
	crt_variants "$work" "$xdf" "$src"

	say "$name: ${seconds}s sample of each variant"
	note "this network has no @native exit(); crt terminates only via exit(),"
	note "so every variant runs forever and is cut off by timeout"
	for variant in "${VARIANTS[@]}"; do
		printf '\n--- %s/%s ---\n' "$name" "$variant"
		timeout "$seconds" "$work/$variant/target/release/$bin" 2>&1 | head -n 20 || true
	done

	say "$name: OK (${#VARIANTS[@]} variants generated, built and started)"
	rm -rf "$work"
}
