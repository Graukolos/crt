#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check it4x4 \
	orc-apps/RVC/src/org/sc29/wg11/mpeg4/part10/cbp/Residual/IT4x4.xdf \
	orc-apps/RVC/src \
	it4x4
