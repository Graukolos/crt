#!/bin/bash

set -e

rm -rf /tmp/histogram

cargo r -r -- orc-apps/ImageProcessing/src/image/xdf/io/TestHistogramBinning.xdf orc-apps/ImageProcessing/src --out /tmp/histogram
cargo b -r --manifest-path /tmp/histogram/Cargo.toml

/tmp/histogram/target/release/testhistogrambinning

rm -rf /tmp/histogram
