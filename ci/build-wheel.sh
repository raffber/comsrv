#!/bin/bash

set -euxo pipefail
cd $(dirname "$0")/..

./pw install

./pw run python3 -m build .

mkdir -p out
cp dist/*.whl out
