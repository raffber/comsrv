#!/bin/bash

cd $(dirname "$0")
cd ..


# this is inteded to be run after ./build-ci.sh
# thus we still have the target directory from the linux build
cargo test --release
