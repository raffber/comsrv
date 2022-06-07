#!/bin/bash

set -euxfo pipefail

ln -snf /usr/share/zoneinfo/$TZ /etc/localtime && echo $TZ > /etc/timezone

mkdir -p /data /home /rust /cargo
chmod a+rwx /data /home /rust /cargo

cd /root
apt-get update
apt-get install --yes build-essential libudev-dev udev pkg-config mingw-w64 libclang-dev libusb-1.0-0-dev curl

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

rustup install 1.61.0
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin