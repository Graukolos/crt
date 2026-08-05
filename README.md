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
| `--native-dir <DIR>` | see below | Directory of `@native` C or C++ sources |
| `--cap <N>` | `1024` | Channel capacity in tokens; `0` means unbounded |
| `--fire-budget <N>` | `1024` | Max consecutive firings per actor visit; `0` means unlimited |
| `--orcc` | off | Emit the orcc compatibility layer |
| `--typestate` | off | Lift FSM state into type parameters |

`--cap` is denominated in tokens for every backend, so it is directly comparable with
DCG's `-s`. `--fire-budget` only affects `threads` and `rayon`, which are the backends
that loop on one actor before moving on; it bounds how long one actor can monopolise a
worker.

## Backends

| Backend | Model | Channels |
| --- | --- | --- |
| `naive` | Single-threaded round-robin over all actors | `Rc<RefCell<VecDeque>>` |
| `threads` | N OS threads contending for `Mutex`-guarded actors (DCG's architecture) | crossbeam |
| `rayon` | Bulk-synchronous parallel: one `rayon::scope` superstep per round | crossbeam |
| `tokio` | One async task per actor, chunked sends with credit-based backpressure | `tokio::mpsc` |


## Generated projects

The output directory is an ordinary Cargo project:

```
out/
  Cargo.toml          release profile: lto = true, codegen-units = 1
  build.rs            only when the network uses @native C
  native/             copies of the @native C sources
  src/
    main.rs           channels, actor instances, scheduler, shared constants
    m_<actor>.rs      one module per actor class
```

Actors become plain structs with a `fire()` method that attempts each action in
declaration order - respecting any CAL `priority` block via a stable topological sort -
and returns whether it fired.

### Native C functions

CAL `@native` functions are compiled and linked automatically. `crt` looks in
`--native-dir`, falling back to `<SOURCE_DIR>/../lib/native`, copies what it finds into
`out/native/`, emits a `build.rs` driving the `cc` crate, and declares the functions
`extern "C"` with safe wrappers.

### `--orcc`

Networks written for orcc expect a `-i input -w output` CLI and a global `opt` struct.
`--orcc` emits `options.h`, the `OrccOptions` glue and a `clap` front end providing
those flags. Networks such as ZigBee need it.

### `--typestate`

`--typestate` lifts an actor's FSM state into a type parameter - `Actor<St_Idle>` with a
per-state `impl` block and a wrapper enum - so illegal state/action pairs stop
compiling instead of being rejected at runtime.

## Experiments

`experiments/` holds one script per network, all built on `experiments/common.sh`.
Each generates, builds and exercises six variants: the four backends plus
`--typestate` on `naive` and `tokio`.

Only `zigbee.sh` and `generated.sh` are benchmarks - their networks have an `exit()`
native and therefore terminate. `zigbee.sh` verifies every variant against
`lib/reference_output/tx_stream.out` before benchmarking. The remaining eleven scripts
cover `orc-apps` networks that never terminate, so they generate, build, and sample each
variant under a timeout as a codegen regression check.

`CAP` and `FIRE_BUDGET` are environment overrides, and `CAP` is also passed to DCG's
`-s`, so both generators are compared at matched FIFO sizes.