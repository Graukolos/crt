#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check hello \
	orc-apps/HelloWorld/src/Hello.xdf \
	orc-apps/HelloWorld/src \
	hello
