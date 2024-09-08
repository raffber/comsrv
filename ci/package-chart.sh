#!/bin/bash

set -euxo pipefail

cd $(dirname "$0")/..

mkdir -p out
helm package ./deploy/helm -d out
