#!/bin/bash

cd $(dirname "$0")
cd ..

rm -rf target
mv /cache/windows-release-cache target
cargo build --release --target  x86_64-pc-windows-msvc
