#!/bin/bash

set -e
shopt -s nullglob

. ./config

cd "$BUILD_DIR"

git clone git@github.com:termux/proot.git
