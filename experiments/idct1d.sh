#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check idct1d \
	orc-apps/Research/src/com/xilinx/mpeg4/part2/sp/iDCT/Idct1d.xdf \
	orc-apps/Research/src \
	idct1d
