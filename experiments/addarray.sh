#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check addarray \
	orc-apps/AddArray/src/xdf/TopAddArray.xdf \
	orc-apps/AddArray/src \
	topaddarray
