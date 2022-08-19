#!/bin/bash

set -euxo pipefail
cd $(dirname "$0")

cd ../python

python3 setup.py bdist_wheel

mkdir -p ../out
cp dist/*.whl ../out
