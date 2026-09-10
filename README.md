# crt

`crt` compiles RVC-CAL dataflow networks to standalone Rust programs.
It takes an XDF network description plus the `.cal` actor sources it references, and writes a self-contained Cargo project that
you can build and run.

It is a Rust counterpart to the C++ [`Dataflow_Code_Generator`](https://github.com/Graukolos/Dataflow_Code_Generator)
(DCG), and reuses DCG's lexer, parser and network reader through a FFI bridge.

## Requirements

- A Rust toolchain supporting edition 2024
- A C++20 compiler
- The `Dataflow_Code_Generator` submodule, which is **required to build**

For the comparison experiments in `experiments/` you additionally need
`cmake` (to build the DCG binary), `gcc`/`g++`, [`hyperfine`](https://github.com/sharkdp/hyperfine),
and the `orc-apps` submodule.

## Build

```sh
git clone --recursive <repo>
cd crt
cargo build --release
```

If you already cloned without submodules:

```sh
git submodule update --init --recursive
```

## Usage

```sh
crt <XDF> <SOURCE_DIR> [OPTIONS]
```

`<XDF>` is the top-level network; `<SOURCE_DIR>` is the classpath root against which
the network's dotted class names (`multitoken_tx.ZigBee_tx`) are resolved to `.cal`
files.

### Options

| Option | Default | Meaning |
| --- | --- | --- |
| `--out <DIR>` | `generated` | Where to write the Cargo project |
| `--backend <B>` | `single` | `single`, `threads`, `rayon` or `tokio` |
| `--native-dir <DIR>` | `../lib/native` | Directory of `@native` C or C++ sources |
| `--cap <N>` | `1024` | Channel capacity in tokens; must be at least 1 |
| `--fire-budget <N>` | `1024` | Max consecutive firings per `schedule()` call; `0` means unlimited |
| `--orcc` | `false` | Emit the orcc compatibility layer |
| `--typestate` | `false` | Lift FSM state into type parameters |

## Backends

| Backend | Model | Channels |
| --- | --- | --- |
| `single` | Single-threaded round-robin over all actors | `Rc` ring of `Cell` slots |
| `threads` | N OS threads contending for `Mutex`-guarded actors (DCG's architecture) | lock-free SPSC ring |
| `rayon` | Bulk-synchronous parallel: one `rayon::scope` superstep per round | lock-free SPSC ring |
| `tokio` | One async task per actor, chunked sends with credit-based backpressure | `tokio::mpsc` |

## Generated projects

The output directory is an ordinary Cargo project:

```
out/
  Cargo.toml          release profile: lto = "thin", panic = "abort"
  build.rs            only when the network uses @native C
  native/             copies of the @native C sources
  src/
    main.rs           channels, actor instances, scheduler, shared constants
    chan.rs           channel and port types, plus the CAP constant
    m_<actor>.rs      one module per actor class
```

## Experiments

`bench.sh` is the benchmark driver. It generates, builds and measures all nine
configurations - the four backends with and without `--typestate`, plus DCG - over the
`balanced`, `wide`, `pipeline` and `zigbee` networks, and writes hyperfine's raw JSON
plus an `index.json` describing it.

```sh
MACHINE=ryzen OUT=/path/to/thesis/results experiments/bench.sh all
```

`MACHINE` is required and names the machine in the index; `OUT` defaults to
`results/` inside this repo. DCG is built from the submodule into
`Dataflow_Code_Generator/build` on first use and reused afterwards; set `DCG` to a
binary path to use one built elsewhere. The experiments are:

| Experiment | Sweeps | Notes |
| --- | --- | --- |
| `baseline` | nothing | all configurations at the default `CAP` and `FIRE_BUDGET`, on every core |
| `cores` | `$CORES` | pinned with `taskset`; DCG is rebuilt per core count |
| `cap` | `$CAPS` | `crt` and DCG are both rebuilt per value, matched via `--cap` and `-s` |
| `budget` | `$BUDGETS` | `crt` only; DCG has no firing budget |

`all` runs the four in order. `CAP`, `FIRE_BUDGET`, `CORES`, `CAPS`, `BUDGETS`,
`NETWORKS`, `RUNS`, `WARMUP`, `TIMEOUT` and `ZIGBEE_REPEAT` are environment overrides;
`bench.sh` with no arguments prints them.

Output layout:

```
$OUT/
  index.json                                    coordinates, machine and toolchain metadata
  raw/<machine>/<experiment>/<stem>.json        hyperfine --export-json, unmodified
```

Runs accumulate: sweeping a second machine merges into the same `index.json` rather
than replacing it, and re-running one experiment replaces only that machine's entry for
it. Builds are cached in `$WORK` (default `/tmp/crt-bench`), so an interrupted sweep
resumes without recompiling.

`report.py` prints the wall-time and speedup tables from a finished `index.json`:

```sh
python3 experiments/report.py results ryzen
```

The remaining scripts are for interactive use and print to stdout only.
`zigbee.sh`, `balanced.sh`, `wide.sh` and `pipeline.sh` each build one network and
benchmark its variants; `zigbee.sh` also verifies every variant against
`lib/reference_output/tx_stream.out`. `build-checks.sh` covers eleven `orc-apps`
networks that never terminate, so it generates, builds, and samples each variant under
a timeout as a codegen regression check.
