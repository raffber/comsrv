#!/bin/bash

set -euxfo pipefail

curdir=$(dirname "$0")
cd "$curdir/.."

docker build . -t comsrv-agent

docker run -it -v $PWD:/workspace comsrv-agent ci/build-ci.sh

