#!/bin/bash

set -eufo pipefail

cd $(dirname "$0")

version=$(./get-version.sh)
changelog_version=$(grep -m 1 -oE '## \[.*?\]' ../CHANGELOG.md | sed -e 's/[# \[]//g' -e 's/\]//g')

if [[ $version != $changelog_version ]]; then
    echo "Invalid version in changelog"
    exit 1
fi
