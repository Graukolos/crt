#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check bipartite \
	orc-apps/Basic/src/sdf/Bipartite.xdf \
	orc-apps/Basic/src \
	bipartite
