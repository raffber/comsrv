#!/bin/bash

cd $(dirname "$0")
cd ..

rm -rf out
mkdir out

rm -rf target
mv /cache/windows-release-cache target
cargo build --release --target  x86_64-pc-windows-msvc
cp target/x86_64-pc-windows-msvc/release/comsrv.exe out
cp target/x86_64-pc-windows-msvc/release/comsrv.dll out
cp target/x86_64-pc-windows-msvc/release/comsrv.dll.lib out

rm -rf target
mv /cache/linux-release-cache target
cargo build --release
cp target/release/comsrv out
cp target/release/libcomsrv.so out
