#!/bin/bash

set -euxo pipefail

cd $(dirname "$0")/..

version=$(./ci/get-version.sh)

rg '^version = \"[\d\.]+"' --iglob 'pyproject.lock' --iglob "Cargo.toml" -m 1 -r "version = \"${version}\"" -q
rg '^version: [\d\.]+' --iglob 'pubspec.yaml' -m 1 -r "version: ${version}" -q

./ci/check-version.sh
