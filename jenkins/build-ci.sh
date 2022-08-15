#!/bin/bash

cd $(dirname "$0")
cd ..

rm -rf target
mv /cache/target target
cargo build --release
