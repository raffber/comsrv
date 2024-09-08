#!/bin/bash

set -euxo pipefail

cd $(dirname "$0")/..

mkdir -p out
tar -C deploy/helm -czf out/chart.tgz Chart.yaml values.yaml templates/
