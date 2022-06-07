#!/bin/bash

set -euxfo pipefail

curdir=$(dirname "$0")
rootdir=$(realpath "$curdir"/..)

rm -rf "$rootdir"/comsrv/target

echo $(id -u)
echo $(id -g)
#docker run -v "$rootdir":/data -w /data/comsrv -u "$(id -u)":"$(id -g)" comsrv-agent cargo build --release
docker run -it -v "$rootdir":/data -w /data/ comsrv-agent ./jenkins/comsrv-build.sh "$(id -u)" "$(id -g)" 

