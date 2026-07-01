#!/bin/bash

set -e

rm -rf /tmp/histogram2

cargo r -r -- orc-apps/ImageProcessing/src/image/xdf/io/TestHistogramBinning2.xdf orc-apps/ImageProcessing/src --out /tmp/histogram2
cargo b -r --manifest-path /tmp/histogram2/Cargo.toml

/tmp/histogram2/target/release/testhistogrambinning2

rm -rf /tmp/histogram2
