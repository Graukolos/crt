#!/bin/bash

set -e

rm -rf /tmp/hello

cargo r -r -- orc-apps/HelloWorld/src/Hello.xdf orc-apps/HelloWorld/src --out /tmp/hello
cargo r -r --manifest-path /tmp/hello/Cargo.toml