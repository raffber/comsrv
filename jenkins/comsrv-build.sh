#!/bin/bash

curdir=$(dirname "$0")
rootdir=$(realpath "$curdir"/..)

rm -rf "$rootdir"/comsrv/target

docker run -v "$rootdir":/data -w /data/comsrv -u "$(id -u)":"$(id -g)" comsrv-agent cargo build --release
docker run -v "$rootdir":/data -w /data/comsrv -u "$(id -u)":"$(id -g)" comsrv-agent cargo build --target x86_64-pc-windows-gnu --release


