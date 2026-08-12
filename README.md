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
| `--backend <B>` | `naive` | `naive`, `threads`, `rayon` or `tokio` |
| `--native-dir <DIR>` | `../lib/native` | Directory of `@native` C or C++ sources |
| `--cap <N>` | `1024` | Channel capacity in tokens; must be at least 1 |
| `--fire-budget <N>` | `1024` | Max consecutive firings per `schedule()` call; `0` means unlimited |
| `--orcc` | `false` | Emit the orcc compatibility layer |
| `--typestate` | `false` | Lift FSM state into type parameters |

## Backends

| Backend | Model | Channels |
| --- | --- | --- |
| `naive` | Single-threaded round-robin over all actors | `Rc` ring of `Cell` slots |
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

`experiments/` holds one self-contained script per network - no shared library, each
runs on its own. Each generates, builds and exercises eight variants: the four backends,
each with and without `--typestate`.

`zigbee.sh`, `balanced.sh`, `wide.sh` and `pipeline.sh` are the benchmarks - their
networks have an `exit()` native and therefore terminate. `zigbee.sh` verifies every
variant against `lib/reference_output/tx_stream.out` before benchmarking. `scaling.sh`
sweeps those networks over core counts with `taskset`. `build-checks.sh` covers eleven
`orc-apps` networks that never terminate, so it generates, builds, and samples each
variant under a timeout as a codegen regression check.

`CAP` and `FIRE_BUDGET` are environment overrides, and `CAP` is also passed to DCG's
`-s`, so both generators are compared at matched FIFO sizes.