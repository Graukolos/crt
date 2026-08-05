#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check iir \
	orc-apps/Filters/src/iir/IIR.xdf \
	orc-apps/Filters/src \
	iir
