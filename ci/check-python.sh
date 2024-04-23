#!/bin/bash

set -euxo pipefail
cd $(dirname "$0")/..

./pw check

./pw types
