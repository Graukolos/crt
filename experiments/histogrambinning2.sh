#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check histogrambinning2 \
	orc-apps/ImageProcessing/src/image/xdf/io/TestHistogramBinning2.xdf \
	orc-apps/ImageProcessing/src \
	testhistogrambinning2
