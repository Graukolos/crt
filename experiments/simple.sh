#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check simple \
	orc-apps/Basic/src/sdf/Simple.xdf \
	orc-apps/Basic/src \
	simple
