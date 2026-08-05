#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check addertree \
	orc-apps/Predistortion/src/lowlevel_dpd/AdderTree.xdf \
	orc-apps/Predistortion/src \
	addertree
