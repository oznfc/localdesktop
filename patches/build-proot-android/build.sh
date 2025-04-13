#!/bin/bash

set -e

./make-talloc-static.sh
./make-proot.sh
./pack.sh
