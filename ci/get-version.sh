#!/bin/bash

set -eufo pipefail

cd $(dirname "$0")
cd ..

version=$(grep -E -m 1 "^version" comsrv/Cargo.toml | cut -d "=" -f 2 | sed 's/[" ]//g')

echo $version