#!/bin/bash

set -eufo pipefail

cd $(dirname "$0")
cd ..

version=$(grep -m 1 -oE '## \[.*?\]' CHANGELOG.md | sed -e 's/[# \[]//g' -e 's/\]//g')

echo $version
