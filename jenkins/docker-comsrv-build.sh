#!/bin/bash

set -euxfo pipefail

curdir=$(dirname "$0")
rootdir=$(realpath "$curdir"/..)

rm -rf "$rootdir"/comsrv/target

docker run -v "$rootdir":/data -w /data/ comsrv-agent ./jenkins/comsrv-build.sh "$(id -u)" "$(id -g)" 

