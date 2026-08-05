#!/bin/bash

source "$(dirname "$0")/common.sh"

build_check feistel \
	orc-apps/Crypto/CTL/Block_Ciphers/Feistel_Networks/Feistel.xdf \
	orc-apps/Crypto/CTL \
	feistel
