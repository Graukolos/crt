#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check histogrambinning \
	orc-apps/ImageProcessing/src/image/xdf/io/TestHistogramBinning.xdf \
	orc-apps/ImageProcessing/src \
	testhistogrambinning
